# Local autonomous Windows 11 Alpha agent — OPENVIDEOMIX

**Active assignment: [#46](https://github.com/dodemo/OPENVIDEOMIX/issues/46).** This is an execution brief, not a request to write another plan. Use **Study → Build → Check → Refine**. Work locally on the owner's Windows 11 x64 PC grande (RTX 4080 SUPER, 96 GB RAM). Use the existing `ollama/devstral-small-2:24b-instruct-2512-q4_K_M` via OpenCode. Never use separately billed AI services, API keys or cloud coding quotas.

## Confirmed facts
- This PC successfully built and ran **base Rust workspace tests** after activating MSVC Build Tools 2026 + Windows 11 SDK using `vcvars64.bat`. The Windows desktop GUI also launched, but the build **did not enable GStreamer features**.
- The screenshot showed /dev/video0 and V4L2 Linux placeholders, literal `\n`, disabled capture/recording, and truncated demo text. The 60 Hz UI and nominal 50 fps are **not measured output FPS**.
- OpenCode 2.3.0 and Ollama local already work; repo is `%USERPROFILE%\source\OPENVIDEOMIX`. Do not reinstall them.
- `main` has #25/#34/#32/#35/#36/#37/#42 merged. [PR #45](https://github.com/dodemo/OPENVIDEOMIX/pull/45) adds live audio but is **Draft/not merged** at initial assignment. Do not act on the old #43 AGENTS instruction; #43 is closed. Check the current Git state.
- The user requires **fully OFFLINE self-contained installers**, including the actual used media libraries/codecs/plugins. A consumer must not need Rust, Python, GStreamer, FFmpeg, Ollama or OpenCode installed separately. Development dependencies are allowed on this workstation but must NOT be prerequisites for end users.

## The work, in order
1. **Inspect once**, then execute. Read current `AGENTS.md`, `README.md`, `docs/ROADMAP.md`, `docs/WINDOWS_HARDWARE_TEST.md`, `docs/DESKTOP_PROGRAM_TEST.md`, and the current Cargo features. Record current HEAD. Check Git status. The independent runner safely resumes the existing `feat/windows-local-alpha` branch or creates `local/windows-alpha-work` from a clean main/isolated bootstrap worktree. Never discard uncommitted changes. Do not reset/force-push/overwrite. Avoid repeated audits.
2. **Make the Windows development media environment reproducible** without changing machine-wide PATH: find official trusted GStreamer x64 MSVC runtime/development archives/installers and required plugins; inspect installed vs missing components. Obtain them only from official sources, verify signatures/checksums where available, use a private development directory or user-scoped installation. Do not run elevation/UAC, third-party download scripts or untrusted installers automatically. If permission/elevation or a license decision is essential, record one exact human action and mark BLOCKED. FFmpeg/ffprobe may be installed for development validation only, not required on end-user machines.
3. **Compile and run real GStreamer features** under MSVC: `cargo check --workspace --all-features`, `cargo test --workspace --all-features`, `cargo build --release -p openvideomix-desktop --features gstreamer-capture,gstreamer-recording`. Fix narrow Windows portability errors, then rerun tests. Do not turn off warnings, tests, features or permissions to obtain a green result. Use logs/return codes rather than assumptions.
4. **Validate media end-to-end** using available local automated tests: four independent 1920x1080 progressive 50/1 synthetic GStreamer sources → Preview/CUT → actual Program → WebM with 48 kHz stereo PCM. Check produced output independently with decoded frames/PTS and PCM duration, report dropped slots and *effective* FPS. GUI mouse interactions and physical camera/microphone tests can be marked PENDING unless a working, authorized automation interface is available; CLI tests are not proof of visual UX.
5. **Create a versioned portable Windows x64 ZIP prototype**, with application binary, required GStreamer/codec/plugin DLLs and assets co-located under app-private paths. Build should launch on a clean offline Windows 11 system without system GStreamer/Python/Rust/FFmpeg installations. Identify codec and third-party notices/licensing obligations; do NOT claim fully distributable until audited and offline clean-install validated. No internet downloads at first run. Record SHA256 and artifact path. A locally generated ZIP is not a signed or published release.
6. **Instrument a repeatable local validation command** and sanitized report capturing basic timings/ FPS/PTS gaps, encoded frame count, queue drops, CPU/GPU figures if genuinely measured, logs and exact binary commit. No personal path, hostname, tokens, video/audio biometric data or device IDs in publicly shared reports. A future physical 60-second and 30-minute test must be clearly marked PENDING if not executed.
7. Keep Windows/Linux/macOS CI architecture intact. Work in small testable commits on the feature branch. Do NOT push, merge, modify GitHub settings, contact paid APIs, install system-wide software or destroy unrelated state without explicit owner authorization.

## Local model selection and objective diagnostics

**Do not claim the initial model is the best without a benchmark on this computer.** The Windows launcher currently selects `ollama/devstral-small-2:24b-instruct-2512-q4_K_M` intentionally: it is a code-specialized agentic 24B model with tool support. At approximately 15 GB of quantized weights, it has very little GPU-memory headroom on a 16 GB RTX 4080 SUPER, especially for larger context windows; watch actual offload/CPU use and tokens/s. The model weights on disk are **not** the whole VRAM requirement.

Already installed alternatives (no extra downloads):
- `ollama/qwen3.5:9b-q4_K_M` (~6.6 GB): first candidate for fast diagnostics, concise fixes and longer GPU-resident contexts.
- `ollama/qwen3-coder-next:q4_K_M` (~51 GB): coder-specific 80B MoE with low active parameter count, but needs substantial RAM/CPU offload with this 16 GB GPU. Treat as an optional **measured** experiment, not an automatic switch for every task.
- `ollama/devstral-small-2:24b-instruct-2512-q4_K_M`: initial deep code-edit candidate.

Measure `ollama ps` (CPU/GPU placement), `nvidia-smi` (VRAM), wall-clock duration, token generation rate where exposed, tool-call failures, number of iterations, and actual Cargo/tests passing. For a meaningful A/B test, use the **same bounded task and acceptance criteria** on separate clean temporary worktrees or read-only tasks; don't edit the active feature branch concurrently. Do not benchmark models while another agent is modifying the repo. Prefer the model yielding valid fixes per hour, not the model with the largest parameter count.

**Important for media performance**: do not measure 1080p50 GPU composition/encoding while Ollama still occupies most of the RTX VRAM. Before a **separate reproducible GPU media benchmark**, explicitly stop/unload the local LLM and verify GPU memory is actually free (`ollama ps` / `nvidia-smi`); the agent may reload afterward. Keep model measurement distinct from application-performance measurement. Never attribute an apparent software bottleneck to the application without checking GPU contention.

For unattended operation, separate **agent generation** from a deterministic test runner that checks exit codes, produces independent media decode results and records hard pass/fail; have the agent read the report and implement the next narrow fix. A model's narrative cannot replace these acceptance checks. Publish only an owner-approved, sanitized report to GitHub; the local agent has no automatic access to ChatGPT's current conversation and ChatGPT cannot inspect the physical Windows session unless those diagnostics are shared through an authorized connector. Do not install a GitHub Actions self-hosted runner on the owner's personal PC for a public repository without a reviewed security design.

## Independent CraftAnything-style verification loop (P0)

`START-OPENVIDEOMIX-AGENT.cmd` activates this workstation's MSVC environment, selects **local Ollama Devstral** by default, and runs the standard-library-only `scripts/local_alpha_loop.py`. The launcher requires developer Python 3.10+, not end-user Python. It is a **developer tool only**; packaged OPENVIDEOMIX must run offline with no Python, GStreamer or Rust installation.

The Python executor, not the LLM, is responsible for:

- Preventing a second instance from entering the same worktree.
- Running `cargo fmt --all -- --check` and `cargo test --workspace` and recording real exit codes/logs. If the GStreamer development environment is genuinely present, run `cargo test --workspace --all-features` and build the release media-enabled desktop. If absent, mark those checks **BLOCKED**, never PASS.
- Limiting each agent attempt to 45 minutes and the overall session to at most three edits/verification rounds; stopping on repeated unchanged code/tests, an explicit blocker, process failure, or timeout.
- Writing private `local-results/WINDOWS_ALPHA_RESULTS.json`, `WINDOWS_ALPHA_REPORT.md` and `SANITIZED_RESULTS.json`. Logs, device details, local paths and recordings stay private and **are not automatically uploaded**.
- Marking physical capture, GPU-verified 50fps, AV synchronization and clean offline installation `NOT_TESTED` until instrumented/independent evidence exists.
- Rechecking the **integrity of its own verifier and safety policy** after each LLM session. If a model edits these protected files, stop.
- Only then ask OpenCode to implement the **next narrow failing task** in its separate Alpha work branch. Always leave an evidence-backed checkpoint. No git push/merge.

Model prompting uses the project specification and the **actual failed tests** from `WINDOWS_ALPHA_RESULTS.json`. The model does not control PASS/FAIL or declare release qualification. The runner is not a security sandbox: use a normal non-admin Windows account and do not grant access to secrets or privileged services.

**One-time use:** after the prior OpenCode task is fully stopped, start the verified launcher in a clean worktree. Optionally set `OPENVIDEOMIX_MODEL=ollama/qwen3.5:9b-q4_K_M` for a later isolated comparison; do not run two coding agents simultaneously. `python scripts/local_alpha_loop.py --checks-only` is a **read-only verification mode** (aside from local ignored result files). Do not run more agents until the existing test log and worktree are understood.

## Execution discipline

- The agent must **make code changes**, not repeatedly audit the repository or rewrite roadmaps.
- Read independent `local-results/WINDOWS_ALPHA_RESULTS.json` for check failures before every attempt; avoid repeating the same approach when results are unchanged.
- Record **a single clear blocker**, when indispensable, in `local-results/AGENT_BLOCKER.txt`. Do **not** invent a PASS verdict or write `agent-state.txt`: the independent executor owns the state.
- Do not install system-wide software or request elevation in unattended operation. Do not run Git push, merge, reset, or destructive operations. Do not commit or share raw biometric, camera/microphone, hostname, username, token or path information.
- Work in small testable commits on the local feature branch; the user must separately authorize publishing code and sanitized telemetry. Current GitHub issue #46 and PR #45 status must be checked at the start, not repeatedly during every attempt.
- Never assert that nominal 50 fps, GPU presence, GStreamer compilation, or a synthetic test proves physical 1080p50 broadcast reliability.
