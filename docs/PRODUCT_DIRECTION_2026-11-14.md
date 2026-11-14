# Product direction and implementation priorities (2026-11-14)

Status: design decisions and research tasks; **not** a decision to replace the current engine.

## Product vision
OPENVIDEOMIX aims to be an open-source live-production switcher inspired primarily by vMix, using OBS Studio as a reference for capture, sources, scenes and composition. Target solo operators and teams on Windows, Linux and macOS (Intel and Apple Silicon). Existing Rust/egui prototype is a starting point, not a binding UX specification.

## Confirmed interaction decisions
- Separate Preview and Program; three prominent immediate-action buttons **CUT**, **MIX**, **WIPE**, with no TAKE button or T-Bar in the redesigned interface.
- MIX/WIPE durations: 0.5, 1, 2 seconds presets plus custom duration.
- WIPE pattern/direction/softness selection; classic broadcast transitions.
- **Stinger required in first version** as an additional configurable transition: load short animation/video or image sequence, preferably with alpha transparency; permit fully opaque cover at the switching point. Provide interactive timeline, frame stepping, visual selection of the exact switch frame, and full transition preview.
- OBS-like scene editor in a separate window, optionally on a second monitor; changes must not alter on-air Program until explicitly applied. Scene create/duplicate/rename/reorder, layers, positioning, resizing, visibility, locking, z-order, source reuse or independent copies.
- Sources: USB webcams; HDMI/SDI through capture devices; NDI, SRT and RTSP network inputs; full-display and window capture; video files; graphics. Initial target: up to four simultaneous video sources.
- Editable titles/graphics with font selection, size, color and positioning.
- MVP outcomes: live switching, titles/graphics, **Program recording (top priority)** and internet streaming.

## Architecture evaluation: libobs, not yet approved for adoption
1. Prototype libobs integration with the existing Rust/egui architecture on Windows, Linux and macOS; assess FFI safety, frame transfer, latency, GPU interop, device enumeration, capture, recording and streaming.
2. Compare with the active GStreamer capture candidate and a native GPU compositor, retaining the existing Rust engine plus selected open components; record benchmarks, maintenance and packaging impact.
3. **Licensing gate:** repository currently carries an MIT LICENSE. OBS/libobs is GPL-licensed; directly incorporating GPL code may impose GPL obligations on the combined distributed program. Obtain an explicit licensing decision before implementing/distributing libobs integration. Do not assume MIT-only distribution remains possible.
4. Preserve the current prototype until a validated migration plan exists.

## Prioritized module backlog
P0: verified 1920x1080 progressive video at exactly 50/1 FPS as the default production profile, with 48 kHz audio; real sources and capture (USB, HDMI/SDI capture devices, display/window, video files), Preview/Program routing, scene compositing, audio sync, reliable Program recording, CUT/MIX/WIPE, editable basic titles, tests.
P1: streaming, network sources (RTSP/SRT/NDI, checking SDK and licensing), scene editor isolation, full Stinger editor/player with alpha and exact switch frame, audio mixer and monitoring.
P2: advanced title templates/animations, browser sources, playlists, richer transitions, automation/control surfaces, advanced routing and overlays.

These are **planning priorities**, not promises that P1 features can be omitted from the agreed first release: Stinger and internet streaming remain first-version requirements; the phase order must be adjusted to meet that acceptance criterion.

## Craft Anything-inspired workflow
Investigate the GitHub Craft Anything project and its licensing/integration options before adopting it. Desired workflow: inspect existing functionality -> specify small changes -> implement with Codex/Antigravity -> compile and test -> visually validate actual output -> compare against acceptance criteria -> iterate -> document results. Prefer reproducible test scenes and short, reviewable PRs. Do not claim integration is working before a proof of concept.

## Next decisions
- Audit current code and docs against the new UX requirements.
- Create individual actionable issues with acceptance criteria, cross-platform validation and explicit licensing dependencies.
- Continue implementation without prompting for routine choices. Escalate only genuine product conflicts that cannot be resolved from the recorded decisions.

## Implementation status guardrail

These are product direction requirements, not an assertion that the current prototype implements them. As of 2026-11-14, real capture is in draft PRs, while audio, actual Program recording/streaming, GPU composition and Stinger editing remain unfinished. The redesigned CUT/MIX/WIPE workflow without a T-Bar is an explicit future UI requirement and must not be confused with the existing demonstrator console. Native video support and output are not validated at 1080p50 until hardware and measured output tests pass.
