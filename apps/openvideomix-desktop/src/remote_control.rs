//! Opt-in, localhost-only control transport for OpenDeck/OpenAction.
//!
//! The server never mutates production state itself. Every request is
//! handed to the GUI's existing command dispatcher and receives its result.
//! Transport is disabled unless an explicit strong token is configured.

use std::sync::{mpsc, Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use openvideomix_core::commands::{CommandDispatcher, ProductionCommand, ProductionSnapshot};
use openvideomix_core::MixerState;
use serde::Serialize;
use subtle::ConstantTimeEq;
use tokio::sync::oneshot;

const MAX_PENDING_REQUESTS: usize = 64;
const CONTROL_PORT: u16 = 21873;

struct ControlRequest {
    command: ProductionCommand,
    response: oneshot::Sender<Result<ProductionSnapshot, String>>,
}

struct ServerState {
    token: String,
    requests: mpsc::SyncSender<ControlRequest>,
    snapshot: Arc<RwLock<ProductionSnapshot>>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

type HttpError = (StatusCode, Json<ErrorResponse>);

fn deny(code: StatusCode, message: &str) -> HttpError {
    (
        code,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}

fn authenticate(headers: &HeaderMap, token: &str) -> Result<(), HttpError> {
    // No cross-origin browser access: OpenAction/desktop clients do not need
    // Origin; browsers cannot silently use a user's configured API token.
    if headers.contains_key(header::ORIGIN) {
        return Err(deny(
            StatusCode::FORBIDDEN,
            "Browser origins are not permitted",
        ));
    }
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| deny(StatusCode::UNAUTHORIZED, "Bearer token is required"))?;
    let expected = token.as_bytes();
    let bytes = supplied.as_bytes();
    if bytes.len() != expected.len() || !bool::from(bytes.ct_eq(expected)) {
        return Err(deny(StatusCode::UNAUTHORIZED, "Invalid bearer token"));
    }
    Ok(())
}

async fn get_state(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Result<Json<ProductionSnapshot>, HttpError> {
    authenticate(&headers, &state.token)?;
    let snapshot = state
        .snapshot
        .read()
        .map_err(|_| deny(StatusCode::SERVICE_UNAVAILABLE, "State unavailable"))?
        .clone();
    Ok(Json(snapshot))
}

async fn post_command(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Json(command): Json<ProductionCommand>,
) -> Result<Json<ProductionSnapshot>, HttpError> {
    authenticate(&headers, &state.token)?;
    let (response, receiver) = oneshot::channel();
    state
        .requests
        .try_send(ControlRequest { command, response })
        .map_err(|_| {
            deny(
                StatusCode::SERVICE_UNAVAILABLE,
                "Production dispatcher busy",
            )
        })?;
    let result = tokio::time::timeout(Duration::from_secs(3), receiver)
        .await
        .map_err(|_| deny(StatusCode::GATEWAY_TIMEOUT, "Production command timed out"))?
        .map_err(|_| deny(StatusCode::SERVICE_UNAVAILABLE, "Dispatcher unavailable"))?;
    result
        .map(Json)
        .map_err(|message| deny(StatusCode::BAD_REQUEST, &message))
}

/// Optional server. A missing token means no network listener is created.
/// The token must be provided by the operator through the environment.
/// It is intentionally never written to logs or exposed in the GUI.
pub struct RemoteControl {
    receiver: mpsc::Receiver<ControlRequest>,
    snapshot: Arc<RwLock<ProductionSnapshot>>,
    shutdown: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl RemoteControl {
    pub fn start_if_configured(mixer: &MixerState) -> Result<Option<Self>, String> {
        let Ok(token) = std::env::var("OPENVIDEOMIX_CONTROL_TOKEN") else {
            return Ok(None);
        };
        if token.len() < 32 || token.len() > 256 || !token.is_ascii() {
            return Err(
                "OPENVIDEOMIX_CONTROL_TOKEN must contain 32-256 ASCII characters".to_string(),
            );
        }
        let (requests, receiver) = mpsc::sync_channel(MAX_PENDING_REQUESTS);
        let snapshot = Arc::new(RwLock::new(ProductionSnapshot::from_state(0, mixer)));
        let shared_snapshot = Arc::clone(&snapshot);
        let (shutdown, shutdown_rx) = oneshot::channel::<()>();
        let (startup_tx, startup_rx) = mpsc::sync_channel::<Result<(), String>>(1);
        let worker = thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = startup_tx.send(Err(format!("Runtime: {error}")));
                    return;
                }
            };
            runtime.block_on(async move {
                let listener = match tokio::net::TcpListener::bind((
                    std::net::Ipv4Addr::LOCALHOST,
                    CONTROL_PORT,
                ))
                .await
                {
                    Ok(listener) => listener,
                    Err(error) => {
                        let _ = startup_tx.send(Err(format!("Bind localhost: {error}")));
                        return;
                    }
                };
                let router = Router::new()
                    .route("/v1/state", get(get_state))
                    .route("/v1/command", post(post_command))
                    .layer(DefaultBodyLimit::max(8192))
                    .with_state(Arc::new(ServerState {
                        token,
                        requests,
                        snapshot: shared_snapshot,
                    }));
                let _ = startup_tx.send(Ok(()));
                let _ = axum::serve(listener, router)
                    .with_graceful_shutdown(async {
                        let _ = shutdown_rx.await;
                    })
                    .await;
            });
        });
        match startup_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Some(Self {
                receiver,
                snapshot,
                shutdown: Some(shutdown),
                worker: Some(worker),
            })),
            Ok(Err(message)) => {
                let _ = worker.join();
                Err(message)
            }
            Err(error) => Err(format!("Control server failed to start: {error}")),
        }
    }

    /// Run from the eframe thread, never from the real-time audio callback.
    /// GUI remains authoritative for all command validation and state changes.
    pub fn poll(&mut self, dispatcher: &mut CommandDispatcher, mixer: &mut MixerState) {
        for _ in 0..MAX_PENDING_REQUESTS {
            let Ok(request) = self.receiver.try_recv() else {
                break;
            };
            let result = dispatcher
                .dispatch(mixer, request.command)
                .map_err(|error| error.to_string());
            let _ = request.response.send(result);
        }
    }

    pub fn refresh_snapshot(&mut self, dispatcher: &CommandDispatcher, mixer: &MixerState) {
        if let Ok(mut guard) = self.snapshot.write() {
            *guard = ProductionSnapshot::from_state(dispatcher.revision(), mixer);
        }
    }
}

impl Drop for RemoteControl {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn http_router_requires_auth_and_dispatches_valid_command() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let (sender, receiver) = mpsc::sync_channel(2);
        let mixer = MixerState::default();
        let server = Arc::new(ServerState {
            token: "a".repeat(32),
            requests: sender,
            snapshot: Arc::new(RwLock::new(ProductionSnapshot::from_state(0, &mixer))),
        });
        let app = Router::new()
            .route("/v1/state", get(get_state))
            .route("/v1/command", post(post_command))
            .layer(DefaultBodyLimit::max(8192))
            .with_state(server);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/state")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/state")
                    .header(header::AUTHORIZATION, format!("Bearer {}", "a".repeat(32)))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let worker = std::thread::spawn(move || {
            let request = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
            let mut state = mixer;
            let mut dispatcher = CommandDispatcher::default();
            let response = dispatcher
                .dispatch(&mut state, request.command)
                .map_err(|error| error.to_string());
            request.response.send(response).unwrap();
            assert_eq!(state.program_input, Some(5));
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/command")
                    .header(header::AUTHORIZATION, format!("Bearer {}", "a".repeat(32)))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"action":"cut"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        worker.join().unwrap();
    }

    #[test]
    fn rejects_missing_wrong_and_browser_credentials() {
        let mut headers = HeaderMap::new();
        assert!(authenticate(&headers, &"a".repeat(32)).is_err());
        headers.insert(
            header::AUTHORIZATION,
            "Bearer aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".parse().unwrap(),
        );
        assert!(authenticate(&headers, &"a".repeat(32)).is_ok());
        assert!(authenticate(&headers, &"b".repeat(32)).is_err());
        headers.insert(header::ORIGIN, "http://localhost:9000".parse().unwrap());
        assert!(authenticate(&headers, &"a".repeat(32)).is_err());
    }
}
