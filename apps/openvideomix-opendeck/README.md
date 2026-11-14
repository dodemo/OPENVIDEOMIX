# OPENVIDEOMIX OpenAction / OpenDeck plugin (proof of concept)

This standalone MIT-licensed Rust plugin targets [OpenAction](https://openaction.amankhanna.me/) and the backward-compatible Stream Deck SDK. It is **not** part of the broadcast application's Cargo workspace.

## Actions

- **CUT:** switches Preview to Program immediately.
- **Select Preview:** selects a source by its numeric ID.
- **Toggle Overlay 1:** toggles one source as overlay 1.

The source number is configured in the plugin property inspector. Icon artwork is original. All actions share the validated desktop command dispatcher in PR #16 through the local API in PR #18.

## Requirements and limitations

- The desktop application must be compiled with `--features local-control-api` and be running.
- The desktop app and **the OpenDeck plugin process** must receive the same `OPENVIDEOMIX_CONTROL_TOKEN` environment variable (32–256 ASCII characters). Launching OpenDeck normally will not automatically provide it. No token is stored in exported profiles, plugin properties, or source code.
- Both processes must run on the **same computer**; the API listens on `127.0.0.1` only.
- These are initial key-up actions; there is no continuous tally/state subscription, streaming/recording action, dynamic input list, remote LAN support or multi-action transactional guarantee yet.
- The plugin shows `OFFLINE` when the local API is inaccessible or rejects a command.
- The plugin is not a general Stream Deck driver: OpenDeck or another compatible host manages hardware.
- **No guarantee of Elgato Stream Deck compatibility** until tested in OpenDeck and official Stream Deck software. OpenAction extends the SDK, but the features used here are limited to conventional keypad events and settings.
- Native binary packaging/signing/notarization is still to do, especially on macOS. No binary is shipped by this PR.

## Build

```bash
cargo build --manifest-path apps/openvideomix-opendeck/Cargo.toml --release
```

Package the corresponding binary and all files under
`assets/com.openvideomix.control.sdPlugin/` into a directory named `com.openvideomix.control.sdPlugin`.
Rename each executable to the target-triple-specific name listed in `manifest.json`, for example `openvideomix-opendeck-x86_64-pc-windows-msvc.exe` on Windows or `openvideomix-opendeck-aarch64-apple-darwin` on Apple Silicon. Install the resulting folder in the OpenDeck plugins directory, then restart the host. OpenDeck's settings provide a shortcut to that folder.

## Testing before inclusion in a release

1. Check compilation on Windows, Linux, macOS Intel and Apple Silicon, including Rust Edition 2024 requirements.
2. Validate `manifest.json` against the current OpenAction/Stream Deck specification; verify that all icon, inspector and executable paths exist in packaged bundles.
3. Test CUT and Preview with the real desktop app, including invalid source IDs, missing credentials and app shutdown.
4. Check input selection inspector and profile persistence in both OpenDeck and official Stream Deck software.
5. Ensure real on-air state is reflected correctly before adding LED/color feedback; the current title only reports command dispatch success or failure, **not** continuous tally.
6. Improve authentication/credential provisioning for end users; environment variables are an integration placeholder and do not provide one-click setup.
7. Audit plugin licenses and package signatures for each release.

Related PRs: #16 (typed production commands), #18 (localhost API), issue #13 (control surfaces).

Official reference: https://openaction.amankhanna.me/manifest.html

## Automated test bundles

The GitHub Actions workflow `.github/workflows/opendeck-package.yml` builds a separate development `.zip` containing the `.sdPlugin` directory and the native executable for Windows x86_64, Linux x86_64, macOS Intel or macOS Apple Silicon. The outputs are attached to the workflow as test artifacts. **They are not signed or notarized, are not release builds, and are not automatically installed.** A complete signed/notarized distribution remains a release gate.

For macOS Apple Silicon and Intel the OpenAction `CodePaths` map selects different executables. Official Stream Deck versions that rely only on legacy `CodePathMac` require an architecture-compatible launcher or universal binary before release.
