# OPENVIDEOMIX OpenAction / OpenDeck plugin

This standalone MIT-licensed Rust plugin targets [OpenAction](https://openaction.amankhanna.me/) and the backward-compatible Stream Deck SDK. It is **not** part of the broadcast application's Cargo workspace to avoid leaking external dependencies into the core engine.

## Actions

- **CUT (`com.openvideomix.control.cut`):** switches Preview to Program immediately.
- **Select Preview (`com.openvideomix.control.preview`):** selects a source by its numeric ID as Preview.
- **Select Program (`com.openvideomix.control.program`):** cuts directly to a selected numeric source ID on Program.
- **Toggle Overlay 1 (`com.openvideomix.control.overlay1`):** toggles one source as overlay 1 over Program.

The numeric source number is configured in the plugin property inspector (`pi/input.html`). Icon artwork is original SVG. All actions dispatch through the authenticated localhost API (`/v1/command`) to the validated broadcast command dispatcher.

## Control API Endpoints

The desktop application provides an opt-in loopback API on `127.0.0.1:21873`:

| Endpoint | Method | Auth | Description |
|---|---|---|---|
| `/v1/status` | GET | Bearer | Lightweight switcher health, version, revision and active input IDs |
| `/v1/state` | GET | Bearer | Full production snapshot (program, preview, overlay1, volume, mute) |
| `/v1/command` | POST | Bearer | Dispatches a typed `ProductionCommand` (Cut, SelectPreview, SelectProgram, ToggleOverlay1, etc.) |

### Security Model

- **Localhost only:** The server binds strictly to `127.0.0.1`.
- **Bearer token authentication:** A 32–256 character ASCII token is required in `Authorization: Bearer <TOKEN>`. Tokens are checked in constant time using `subtle::ConstantTimeEq`.
- **Browser origin rejection:** Any request with an `Origin` header is rejected with `403 Forbidden` to prevent cross-origin browser exploitation.
- **Payload size limit:** Restricted to 8 KiB maximum request body.

## Hardware Support Matrix

| Controller / Host | Platform | Status | Notes |
|---|---|---|---|
| OpenDeck (v0.2+) | Linux, Windows, macOS | Compatible | Primary target host; runs `.sdPlugin` directly |
| Elgato Stream Deck (15 keys) | Windows, macOS | Compatible | Standard keypad profile via OpenAction compatibility |
| Elgato Stream Deck MK.2 | Windows, macOS | Compatible | Standard keypad profile |
| Elgato Stream Deck XL (32 keys) | Windows, macOS | Compatible | Standard keypad profile |
| Elgato Stream Deck Mini (6 keys) | Windows, macOS | Compatible | Standard keypad profile |
| Elgato Stream Deck + | Windows, macOS | Partially tested | Keypad actions work; rotary encoder dials planned |
| Loupedeck / Razer Stream Controller | All | Planned | Requires MIDI or native transport bridge |

## Requirements and limitations

- The desktop application must be compiled with `--features local-control-api` and be running.
- The desktop app and **the OpenDeck plugin process** must receive the same `OPENVIDEOMIX_CONTROL_TOKEN` environment variable (32–256 ASCII characters). Launching OpenDeck normally will not automatically provide it unless exported in the session environment. No token is stored in exported profiles, plugin properties, or source code.
- Both processes must run on the **same computer**; the API listens on `127.0.0.1` only.
- The plugin shows `OFFLINE` when the local API is inaccessible or rejects a command.
- The plugin is not a general Stream Deck driver: OpenDeck or another compatible host manages hardware.

## Build

```bash
cargo build --manifest-path apps/openvideomix-opendeck/Cargo.toml --release
```

Package the corresponding binary and all files under
`assets/com.openvideomix.control.sdPlugin/` into a directory named `com.openvideomix.control.sdPlugin`.
Rename each executable to the target-triple-specific name listed in `manifest.json`:
- Windows: `openvideomix-opendeck-x86_64-pc-windows-msvc.exe`
- Linux: `openvideomix-opendeck-x86_64-unknown-linux-gnu`
- macOS (Apple Silicon): `openvideomix-opendeck-aarch64-apple-darwin`
- macOS (Intel): `openvideomix-opendeck-x86_64-apple-darwin`

Install the resulting folder in the OpenDeck plugins directory, then restart the host.

## Automated test bundles

The GitHub Actions workflow `.github/workflows/opendeck-package.yml` builds a development `.zip` containing the `.sdPlugin` directory and the native executable for Windows x86_64, Linux x86_64, macOS Intel or macOS Apple Silicon.
