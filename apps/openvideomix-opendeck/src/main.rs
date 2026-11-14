//! Minimal OpenAction plugin for the OPENVIDEOMIX localhost control API.
//! Works only when the host plugin process receives the same bearer token as
//! OPENVIDEOMIX. Do not store tokens in exported OpenDeck profile settings.

use openaction::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

const API: &str = "http://127.0.0.1:21873/v1/command";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct EmptySettings {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct InputSettings {
    input: u32,
}

impl Default for InputSettings {
    fn default() -> Self {
        Self { input: 1 }
    }
}

async fn dispatch(payload: Value) -> bool {
    let Ok(token) = std::env::var("OPENVIDEOMIX_CONTROL_TOKEN") else {
        return false;
    };
    if token.len() < 32 || token.len() > 256 || !token.is_ascii() {
        return false;
    }
    let client = reqwest::Client::new();
    match client
        .post(API)
        .bearer_auth(token)
        .timeout(Duration::from_secs(4))
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

struct CutAction;

#[async_trait]
impl Action for CutAction {
    const UUID: ActionUuid = "com.openvideomix.control.cut";
    type Settings = EmptySettings;

    async fn key_up(
        &self,
        instance: &Instance,
        _settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let ok = dispatch(json!({"action": "cut"})).await;
        instance
            .set_title(Some(if ok { "CUT" } else { "OFFLINE" }.to_string()), None)
            .await
    }
}

struct PreviewAction;

#[async_trait]
impl Action for PreviewAction {
    const UUID: ActionUuid = "com.openvideomix.control.preview";
    type Settings = InputSettings;

    async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
        let ok = settings.input > 0
            && dispatch(json!({
                "action": "select_preview",
                "args": { "input": settings.input }
            }))
            .await;
        let label = if ok {
            format!("PVW {}", settings.input)
        } else {
            "OFFLINE".to_string()
        };
        instance.set_title(Some(label), None).await
    }
}

struct OverlayAction;

#[async_trait]
impl Action for OverlayAction {
    const UUID: ActionUuid = "com.openvideomix.control.overlay1";
    type Settings = InputSettings;

    async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
        let ok = settings.input > 0
            && dispatch(json!({
                "action": "toggle_overlay1",
                "args": { "input": settings.input }
            }))
            .await;
        let label = if ok {
            format!("OV1 {}", settings.input)
        } else {
            "OFFLINE".to_string()
        };
        instance.set_title(Some(label), None).await
    }
}

#[tokio::main]
async fn main() -> OpenActionResult<()> {
    register_action(CutAction).await;
    register_action(PreviewAction).await;
    register_action(OverlayAction).await;
    run(std::env::args().collect()).await
}
