# Local production control API (OpenDeck integration foundation)

## Status

**Opt-in proof of concept; not a shipped OpenDeck plugin.** The API is part of the OPENVIDEOMIX desktop process and dispatches via the same `CommandDispatcher` as the GUI. Existing GPU composition, audio routing, stream and record functionality retain their separate implementation status.

Start the feature using:

```bash
# Choose a unique 32-256 character ASCII secret for the session.
# Never commit this value, embed it in plugin source, or put it in screenshots.
OPENVIDEOMIX_CONTROL_TOKEN="<your-long-random-secret>" cargo run -p openvideomix-desktop --features local-control-api
```

The listener is **always** bound to `127.0.0.1:21873` and is completely disabled without the environment variable or with an invalid token. This version deliberately has **no LAN mode**. It never prints the token. Local browser requests with an `Origin` header are rejected.

The API uses a single `Authorization: Bearer ...` header. Examples (substitute a secure value):

```bash
curl --fail-with-body -H "Authorization: Bearer $OPENVIDEOMIX_CONTROL_TOKEN" \
  http://127.0.0.1:21873/v1/state

curl --fail-with-body -X POST \
  -H "Authorization: Bearer $OPENVIDEOMIX_CONTROL_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"action":"select_preview","args":{"input":2}}' \
  http://127.0.0.1:21873/v1/command

curl --fail-with-body -X POST \
  -H "Authorization: Bearer $OPENVIDEOMIX_CONTROL_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"action":"cut"}' \
  http://127.0.0.1:21873/v1/command
```

## Responses

`GET /v1/state`: latest validated production snapshot (`revision`, `program`, `preview`, `overlay1`, `master_volume`, `master_muted`).

`POST /v1/command`: the production snapshot after the command has been dispatched on the GUI thread, or a JSON `{"error":"..."}` with an HTTP error status.

- 400: command validation failed
- 401: missing/invalid bearer credentials
- 403: request carries browser Origin
- 413: input exceeds configured JSON body size
- 503: dispatcher unavailable/queue saturated
- 504: production GUI did not process request in 3 seconds

All requests are limited to an 8 KiB body. A bounded 64-item command queue prevents uncontrolled accumulation; all state changes remain serialized by the GUI. HTTP server and GUI threads are separate. The status endpoint is a snapshot (not yet a subscription), so this version does not provide animated tally feedback.

## Security and future compatibility

- Token must be provisioned and protected by the host OS. Environment variables can be visible to local privileged processes; this does not replace a full credential store.
- All local processes that know the bearer token can invoke **the permitted subset** of production commands. Do not embed token in an exported Stream Deck profile.
- No direct remote start/stop recording or streaming actions exist yet.
- Protocol version is `/v1`; future schemas must retain or version behavior. Clients should derive tally from `program` and `preview`, not from assumptions about GUI layout.
- An OpenAction/Stream Deck SDK plugin can issue HTTPS? **No:** this prototype is HTTP loopback only; never expose it on LAN or tunnel it without TLS and a separate security review.
- Plugin runtime packaging, reconnect, subscriptions and button feedback remain pending. The OpenAction Rust library is MIT and OpenDeck itself is GPLv3; do not link/copy OpenDeck code into this project.

## Verification

Before integration, run:

```bash
cargo check -p openvideomix-desktop --features local-control-api
cargo test -p openvideomix-desktop --features local-control-api
cargo clippy -p openvideomix-desktop --all-targets --features local-control-api -- -D warnings
```

Test on Linux, Windows, macOS Apple Silicon and Intel. Functional acceptance: requests with valid token select Preview and CUT, status reflects them, wrong token/browser origin cannot issue commands, absent token leaves no listener, and closing the app frees the port.

References: #13, #16, OpenAction https://github.com/OpenActionAPI/rust , OpenDeck https://github.com/nekename/OpenDeck .
