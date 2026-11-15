# OPENVIDEOMIX — Product Design and Issue-Driven Development Architecture

**Document type:** consolidated technical specification and engineering handoff  
**Revision:** 1.0 — 15 November 2026  
**Repository:** https://github.com/dodemo/OPENVIDEOMIX  
**Repository baseline inspected:** `e397fe646343c4480bd94db701b1ea301547f8af`  
**Language:** technical English; the exact Galician participant-calibration text is preserved as a product requirement.

## Contents

1. [Purpose, Scope, and Authority](#1-purpose-scope-and-authority)
2. [Product Definition](#2-product-definition)
3. [Functional Product Design](#3-functional-product-design)
4. [Media Engine Architecture](#4-media-engine-architecture)
5. [External Controls and Device Integration](#5-external-controls-and-device-integration)
6. [Mandatory Multimodal Auto-Director](#6-mandatory-multimodal-auto-director)
7. [Repository Baseline and Known Implementation Limits](#7-repository-baseline-and-known-implementation-limits)
8. [First Runnable Alpha and Release Architecture](#8-first-runnable-alpha-and-release-architecture)
9. [GitHub Repository and Engineering Workflow](#9-github-repository-and-engineering-workflow)
10. [Intended OpenHands Development System](#10-intended-openhands-development-system)
11. [OpenHands Acceptance Tests and Previous Attempt Status](#11-openhands-acceptance-tests-and-previous-attempt-status)
12. [CI, Testing, Performance, and Engineering Quality](#12-ci-testing-performance-and-engineering-quality)
13. [Delivery Roadmap and Issue Map](#13-delivery-roadmap-and-issue-map)
14. [Handoff Constraints and Open Decisions](#14-handoff-constraints-and-open-decisions)
15. [Source Register](#15-source-register)

## 1. Purpose, Scope, and Authority

This document consolidates the product design of OPENVIDEOMIX and the intended GitHub Issues → OpenHands → local Ollama → Draft Pull Request development system. It is suitable for handoff to an implementation environment, including Antigravity, without making ChatGPT a required component.

The owner has stopped the previous OpenHands configuration attempt and intends to reassess development using Antigravity. The OpenHands architecture is retained here because it is part of the requested design record; documenting it does **not** authorize restarting the stopped automation or declare it operational. An Antigravity workflow and an OpenHands workflow must not run concurrently against the same task or checkout.

The product and the development system are separate systems:

- **OPENVIDEOMIX** is the native audiovisual application used by a live-production operator.
- **The development system** reads authorized engineering tasks, changes the repository, runs checks, and publishes reviewable Draft PRs.
- **The runtime auto-director** is an optional operating mode of the audiovisual product, mandatory on its agreed roadmap. It is not the coding agent or the issue coordinator.

This is a design and evidence consolidation, not a new installation, execution report, source-code change, or authorization to process the issue backlog.

### 1.1 Evidence terminology

| Classification | Meaning |
|---|---|
| Required | Explicit owner requirement or recorded product decision. |
| Existing | Present in the inspected repository, within the stated limits. |
| Recorded evidence | A retained test result in repository documentation or artifacts; not rerun for this document. |
| Reported | A result described in the owner's supplied local-session reports/screenshots; not independently inspected here. |
| Proposed | An architectural contract or implementation policy requiring validation. |
| NOT TESTED / Unverified | No sufficient evidence establishes the capability. |

Historical files contain stale implementation descriptions and superseded instructions. More specific owner decisions take precedence: no visible T-Bar/TAKE, mandatory first-version Stinger and internet streaming, first physical qualification on Windows, all three operating systems retained, and no automatic PR merging in the coding-automation design.

## 2. Product Definition

OPENVIDEOMIX is an open-source native live video production switcher. Its primary operational reference is **vMix**; OBS Studio is the secondary reference for source management, scene composition, and editing workflows. Implement original code and assets without copying proprietary branding, artwork, or implementation.

The application targets solo operators and production teams performing live switching, graphics insertion, audio mixing, Program recording, and internet streaming. It must produce actual synchronized audiovisual output, rather than a console mockup or disconnected media-library demonstrations.

The product name is **OPENVIDEOMIX**, without a suffix or “+”. The project's own source code currently uses the **MIT** license. That does not establish the license of every distributed dependency, codec, model, or native binary.

### 2.1 Mandatory platforms

| Platform | Architectural requirement |
|---|---|
| Windows | Native desktop application; Windows x64 is the first physical Alpha qualification target. |
| Linux | Native desktop application; explicitly handle Wayland and relevant X11 behavior. |
| macOS | Native application on Apple Silicon and Intel, including permissions, signing, and distribution requirements. |

Windows-first physical validation must not remove Linux/macOS architecture or CI coverage. The audiovisual product must not require an Electron/browser runtime. OpenHands Agent Canvas being an Electron desktop application is a separate development-tool concern.

### 2.2 Default audiovisual contract

- Resolution: **1920 × 1080**.
- Scan mode: **progressive**.
- Frame rate: **50/1 frames per second**, represented rationally; nominal period **20 ms**.
- Working audio sample rate: **48,000 Hz**.
- Additional explicitly selected rate families: `25/1`, `60/1`, and `60000/1001`.
- Preview, Program, composition, recording, and streaming share the requested production profile unless the operator explicitly chooses another configuration.

Do not silently substitute 1080p30, 720p, or 59.94 fps. Distinguish requested format, negotiated source format, configured conversion, actual delivered frames, and encoded output. GUI refresh rate, texture-update rate, encoder caps, and a container header do not prove sustained 1080p50.

Reduced-resolution synthetic tests, including 640 × 360 at 30 fps, are development fixtures only. At 50/1 with 48 kHz audio, one video interval corresponds to **960 audio sample frames per channel**. Other rate combinations require exact cumulative sample accounting rather than repeated rounding.

## 3. Functional Product Design

### 3.1 Independent inputs and Preview/Program

Each input is an independently configured source with a stable logical identity, lifecycle, capabilities, status, thumbnail, audio properties, and tally. Source type must not be inferred from an operating-system-specific path.

- Preview and Program are separate live monitors.
- Preview selection must not immediately alter on-air Program.
- CUT performs an immediate frame-boundary switch with defined Preview/Program semantics.
- Program is the actual composed signal supplied to recording and streaming.
- Source health and tally must reflect engine state, with text or icons as well as color.
- Initial operating target: at least **four simultaneous configurable video inputs**.

### 3.2 Source families

| Family | Intended capability and boundary |
|---|---|
| USB/UVC cameras | Enumerate physical devices, persist identity, negotiate modes, and recover from removal/reconnection. |
| HDMI/SDI capture | Use compatible capture devices and supported platform/vendor interfaces; qualify actual hardware. |
| Display/window capture | Use OS permission and selection flows; handle cancellation and revocation. |
| Media files | Real MP4/MKV playback, decoding, play/pause, seeking, looping, duration, and errors. |
| Still images and graphics | PNG/WebP/JPG as appropriate; alpha where supported; editable titles and lower thirds. |
| Network inputs | RTSP, SRT, and NDI are intended integrations; validate SDKs, licenses, latency, and recovery individually. |
| Synthetic sources | Explicitly labeled test patterns for deterministic tests and diagnostics. |

RTMP is required as an output protocol. Network input protocol breadth remains capability-specific; do not infer that implementing RTMP output implements network ingest.

Missing codecs, permissions, disconnected devices, unsupported formats, and stale frames require explicit states. A camera that supports 1080p50 only through compressed MJPEG/H.264 must not be treated as equivalent to an uncompressed mode without measuring bandwidth and decode cost.

### 3.3 Transitions

The final primary controls are **CUT**, **MIX**, and **WIPE**. There is **no visible T-Bar and no TAKE button** in the final agreed interface.

- MIX/WIPE duration presets: **0.5 s, 1 s, 2 s**, plus a custom duration.
- WIPE: selectable pattern, direction, and softness.
- Transition state includes source/target, type, monotonic start, duration, continuous progress, completion, and interruption policy.
- Core transition progress remains continuous and deterministic for testing, Stinger preview, and API use. Removing the visible T-Bar must not reduce transitions to delayed CUTs.
- Source disappearance, source removal, repeated commands, same-source routing, interruption, and completion require defined behavior.
- MERGE and additional advanced transitions require separate specifications; they must not delay the first usable Windows Alpha.

### 3.4 Stinger

Stinger is a **first-version product requirement**, even though it follows the basic media/recording foundations in implementation order.

Required behavior:

1. Load a short video/animation or image sequence, preferably with alpha.
2. Permit a fully opaque cover at the switch point.
3. Display an interactive timeline with frame stepping.
4. Let the operator select the exact background-switch frame visually.
5. Preview the complete transition before use on Program.
6. Validate decoding, timing, alpha, asset availability, and interruption behavior.

### 3.5 Four global overlay channels

Provide **OV1–OV4** independently of the background input. Assign graphics, titles, video, or live sources to overlays; support alpha, positioning, crop/rectangle, z-order, visibility, and appropriate timing controls.

Overlays must persist correctly across background transitions. Their composition must appear in the actual Program output and encoded file, not only in a GUI preview. Reference alpha tests should verify ordering and blend semantics.

### 3.6 Scene editor and graphics

Provide an OBS-like scene editor in a separate window, optionally on another monitor. Editing a scene must not alter on-air Program until the operator explicitly applies the changes.

Support scene creation, duplication, renaming, ordering, layers, positioning, resizing, visibility, locking, z-order, source reuse versus independent copies, and editable graphics with font, size, color, and positioning controls. Snapshot/apply semantics must prevent partial edits from leaking to Program.

### 3.7 Audio

The audio system must process real captured/decoded PCM at 48 kHz with timestamps and bounded buffering.

- Per-input gain, level, mute, and appropriate solo behavior.
- **AFV — Audio Follows Video**, with explicit transition behavior.
- Independent **Master, Bus A, and Bus B** routing.
- Headphone **PFL/AFL** monitoring without unintended effects on Master or streaming.
- Real Peak/RMS dBFS meters and clipping indications.
- Defined A/V synchronization and clock-drift handling.
- Device selection, permissions, delayed starts, unplug/replug, overflow, and error states.
- Feedback/loopback protection and nonblocking real-time callbacks.

Meter design references include a normal region up to approximately −18 dBFS, a warning region approaching −3 dBFS, and a clipping warning. Calibration and meter ballistics require explicit implementation decisions. Never display animated meters as captured measurements when audio is unavailable.

Generated test tone, captured microphone audio, decoded file audio, muted signal, unavailable signal, and error must be distinguishable in the UI and diagnostics.

### 3.8 Outputs

**Program recording is the first output priority.** Recording and streaming consume the same composed video and synchronized mixed-audio output contract.

Required first-version outcomes:

- Playable recordings containing actual Program video and audio.
- RTMP internet streaming with accurate status and recoverable errors.
- Explicit resolution, frame-rate, bitrate, codec, keyframe, and audio settings.
- Encoder capability negotiation and an explicit, measured CPU fallback where allowed.
- File no-overwrite protection; disk-full, inaccessible destination, encoder, muxer, and network errors.
- Bounded stop/finalization and truthful saved/error status.

Hardware encoder candidates include NVENC, Intel Quick Sync, AMD AMF, VA-API, and VideoToolbox as applicable. GPU presence or plugin discovery does not establish working hardware encoding.

The existing proof of concept writes **CPU VP8/Vorbis WebM**. MP4/hardware-encoded recording remains an intended output capability subject to implementation and distribution constraints. SRT output and virtual-camera output follow the recording/RTMP baseline; a Linux `v4l2loopback` path is not a portable Windows/macOS virtual-camera implementation.

### 3.9 Projects and operational UX

Use versioned project files with atomic writes and migration tests. Persist sources, stable identities, rack order, scenes, transitions, overlays, audio routing, shortcuts, and control profiles as appropriate.

Opening a project with missing hardware or media must remain possible, with explicit unresolved-source states. Do not replay an old recording/stream-start request merely because the project or application restarts. Restore configuration separately from resuming an active broadcast operation.

The console should support dense input racks, audio panels, detached windows, multiple monitors, HiDPI, keyboard navigation, visible focus, scalable text, accessible names, and color-independent status. Layout qualification includes 1366 × 768, 1920 × 1080, and dual-monitor use. Platform-specific shortcuts and accessibility behavior must be respected.

## 4. Media Engine Architecture

### 4.1 Implementation choices and unresolved decisions

Rust is the current core and desktop implementation language. Native C/C++/Objective-C/Swift bridges are permitted when justified by device APIs, performance, maintenance, and licensing. A wholesale rewrite requires measured benefit.

| Layer | Existing or candidate choice | Decision boundary |
|---|---|---|
| Application state/control | Rust typed state and commands | Existing foundation; preserve and extend. |
| Capture/decode/encode | GStreamer / gstreamer-rs | Active integration; audit platform support and shipped plugins. |
| Production GPU composition | wgpu with native backend interop | Candidate, not a validated production implementation. |
| GUI | eframe/egui | Existing provisional UI; benchmark before final commitment. |
| Alternative media engine | FFmpeg-based pipeline | Alternative requiring comparison of capture, clock, sync, and distribution costs. |
| libobs | Investigation only | Block adoption/distribution until explicit GPL and architecture review. |
| Alternative GUI | Qt or Slint evaluation | Only replace the existing UI on evidence; keep media contracts independent. |

The relevant ADR remains **Proposed**, not Accepted. Zero-copy interop is a per-backend hypothesis to test, not a universal promise.

### 4.2 Logical signal path

```text
Physical / file / network / synthetic sources
    → platform SourceAdapters and decoders
    → common-clock timestamp mapping and bounded ingress queues
    → Preview / Program routing and scene state
    → production compositor + OV1–OV4 + transition state
    → timestamped Program video ──────────────────────────┐
                                                        ├→ recording / streaming sinks
Audio capture / decoding → timestamp mapping → AudioGraph┘
                              ↓
                  independent metering / monitoring

GUI / keyboard / MIDI / OpenDeck / authenticated API / auto-director
    → validated typed command bus → production state → events / tally
```

GUI monitoring subscribes to production output. It does not own the production clock, and downsampled GUI textures must not become the recording source.

### 4.3 Proposed interface responsibilities

The names below describe logical contracts, not a claim that every interface already exists as a crate or public type.

| Contract | Responsibility |
|---|---|
| ProductionState | Validated sources, buses, routes, overlays, transitions, tally, and output states. |
| CommandBus | Typed commands, validation, serialization of state changes, results, and events. |
| SourceAdapter | Discovery, permission, open/close, live/stalled/error lifecycle, formats, timestamps, and reconnect. |
| MediaFrame | Rational timestamp/duration, color format/colorimetry, orientation, alpha convention, ownership, CPU buffer or opaque GPU resource. |
| VideoCompositor | Actual Preview/Program composition, transitions, and four overlay layers. |
| AudioGraph | Captured/decoded PCM, gains, routing, AFV/PFL, metering, synchronization, and drift correction. |
| OutputSink | Recording/stream lifecycle, bounded delivery, codec negotiation, finalization, and metrics. |
| ProjectStore | Versioned serialization, atomic writes, migrations, and missing-resource handling. |
| UiAdapter | Render snapshots/events and submit commands without owning media processing. |

### 4.4 Timing, buffering, and failure behavior

- Use an explicit monotonic media clock and rational rates.
- Preserve original source PTS and mapped common-clock PTS; keep DTS distinct where encoding requires it.
- Derive deadlines from absolute frame indices; do not accumulate rounded periods.
- Map independent source clocks and correct audio drift explicitly.
- Use bounded queues with documented overflow/backpressure policy.
- Count late input, rejected timestamps, held frames, missed deadlines, output queue drops, delivered packets, and encoded frames separately.
- Never compress missing video intervals into a shorter timeline to conceal loss.
- Avoid rendering a burst of stale frames after a missed deadline.
- Keep UI, capture callbacks, audio processing, composition, and encoder supervision appropriately separated.
- Preserve partial recording files and report failure accurately when finalization cannot complete.

The CPU reference compositor uses unpremultiplied RGBA8 output, premultiplied-alpha interpolation for FADE, a reference horizontal WIPE, and ordered overlays. Production color management, linear-light behavior, YUV conversion, scaling/crop, HDR, and interlacing rules need explicit accepted specifications and differential tests.

### 4.5 Platform adapters

| Capability | Windows | Linux | macOS |
|---|---|---|---|
| Camera | Media Foundation / supported GStreamer providers | V4L2/UVC | AVFoundation |
| Screen/window | Windows Graphics Capture candidate | XDG Desktop Portal + PipeWire on Wayland; qualify X11 path | ScreenCaptureKit |
| Audio | WASAPI | PipeWire/ALSA | Core Audio |
| GPU | Direct3D / wgpu | Vulkan / wgpu; assess DMABUF | Metal / wgpu |
| Hardware encoding | NVENC/QSV/AMF or supported system APIs | VA-API/NVENC/QSV as supported | VideoToolbox |

These are integration targets. Each needs build, permission, runtime, disconnect, sleep/wake, and actual media qualification. Generic `autovideosrc` discovery is not proof of persistent physical-device selection.

## 5. External Controls and Device Integration

### 5.1 Shared control plane

GUI, keyboard, OpenDeck, MIDI, authenticated API, and the runtime auto-director must use the same command validation and production state. No controller may bypass routing invariants by mutating GUI fields or media objects directly.

Commands should cover Preview selection, CUT/MIX/WIPE, overlays, audio parameters, output control, project operations, and supported PTZ presets. Capability-gate commands that are not implemented. Publish acknowledgments, state revisions, tally, errors, online/offline status, and output lifecycle events.

### 5.2 OpenDeck / OpenAction / Stream Deck

Use a standalone **OpenAction / Stream Deck SDK-compatible `.sdPlugin`** with a compatible host such as OpenDeck. OPENVIDEOMIX should not duplicate the host's device-driver stack.

Existing proof-of-concept actions: CUT, Select Preview, and Toggle Overlay 1. Continuous tally subscription, richer audio/output controls, reconnect behavior, macros/pages, signing, and physical compatibility remain separate acceptance items.

OpenDeck's GPLv3 implementation is not approved for copying/linking into the MIT application. Interface with documented protocols and review dependency licenses. Do not promise compatibility with every Elgato or third-party deck without testing.

### 5.3 USB MIDI

Support cross-platform MIDI enumeration, MIDI Learn, persistent logical-device mapping, buttons/notes, CC knobs/faders, normalization, debouncing, dead zones, inversion, and **soft takeover/pickup**. Support feedback/LED/motor behavior only when the specific device provides it. Device disconnect must not interrupt production.

The existing adapter supports a limited MIDI 1.0 Note/CC path through `midir`, bounded callback delivery, JSON profiles, and pickup logic. Automatic profile loading, persistent hardware binding, reconnection, relative encoders, SysEx/extended message handling, and bidirectional feedback are not established by that implementation.

The Mingzhe controller identified by ASIN **B0FV93GS5Z** is a target example, not hardware-qualified support. Verify its actual USB-MIDI behavior before shipping a device-specific preset.

### 5.4 Existing local control API

The opt-in `local-control-api` feature currently binds to **`127.0.0.1:21873`** and requires `OPENVIDEOMIX_CONTROL_TOKEN`, a 32–256-character ASCII credential. It uses Bearer authorization and rejects browser requests carrying an Origin header.

- `GET /v1/state`: current validated production snapshot.
- `POST /v1/command`: supported command dispatch and resulting state/error.
- Existing limits: 8 KiB request body, 64-item bounded queue, 3-second dispatch timeout.
- Existing status classes include validation, authentication, origin rejection, oversized body, unavailable/saturated dispatcher, and timeout.

Current implementation dispatches through the GUI thread; that is an existing prototype constraint, not the target ownership model for a production media clock. There is no current LAN mode, WebSocket subscription, complete vMix protocol compatibility, or implemented remote record/stream command set implied by this API.

Future LAN access requires explicit enablement, authentication, transport security appropriate to the exposure, and origin/permission/rate-limit review. Keep development-tool API credentials separate from this product control credential.

### 5.5 Restricted vMix shortcut adapter

An existing translator maps a subset of shortcut names such as `Cut`, `PreviewInput`, `OverlayInput1In/Off`, and `AudioOn/Off` to typed commands. Rack positions are not persistent InputIds. Unsupported functions must fail explicitly. This is not a full vMix HTTP/TCP server or scripting implementation.

### 5.6 OBSBOT Tiny 2

Treat Tiny 2 first as a generic UVC/UAC camera with a **fixed-camera profile**, not as a prerequisite for proprietary PTZ integration. Intended behavior includes stable identity, real mode negotiation, static framing across transitions, reconnect handling, and optional saved pan/tilt/zoom where verified.

1080p50 compressed-mode support is described in issue #10, but actual firmware, USB topology, exposed audio channels, permissions, decode cost, and multicamera bandwidth require physical qualification. Selecting a source must not silently enable tracking or move the camera. Vendor SDK/PTZ/AI tracking support is optional, feature-gated, and subject to license and hardware checks.

## 6. Mandatory Multimodal Auto-Director

Issue **#47** records mandatory P1 work after the usable Windows Alpha and real physical capture foundation. It is not discarded as a future virtual-camera-only feature.

### 6.1 Detection and switching rules

- Combine visible face/mouth/lip activity with voice activity and calibrated vocal characteristics.
- Do not replace visual confirmation with “choose the loudest microphone.”
- Associate each participant with a validated profile and configured physical or virtual shot.
- Voice without a sufficiently matching visible mouth selects the configured **wide/safety shot**.
- Simultaneous/ambiguous speech follows the profile's safe-shot or listening-shot policy.
- Use confidence thresholds, confirmation duration, hysteresis, minimum shot duration, and anti-oscillation policy.
- Initial decision-latency objective: **≤500 ms**, subject to physical measurement; not an achieved figure.

### 6.2 Absolute manual priority

Any manual production intervention from GUI, keyboard, MIDI, OpenDeck, or API suspends AUTO immediately. Show **MANUAL / AUTO / AUTO SUSPENDED** and the reason. **Only an explicit operator command re-enables AUTO; never a timer.** All automatic decisions still pass through the shared command bus.

### 6.3 Calibration and human approval

Provide an approximately 20-second neutral spoken calibration with synchronized mouth video and audio. Preserve this exact required UI text:

> Hoxe pola mañá mirei arredor con calma. Pronuncio pausas curtas e longas. Pecho e abro os labios con naturalidade, sen présa, mentres falo dun día común, sen emocións fortes nin ruído.

After capture, the profile is **PROVISIONAL / NOT ELIGIBLE FOR AUTO**. The operator must review the recording visually and audibly and explicitly confirm identity/source association before it becomes **VALIDATED / ELIGIBLE FOR AUTO**. Support retry, correction, rejection, recalibration, and duplicate/misassociation detection. Do not overwrite a validated calibration without review.

Store participants, titles, camera/shot identities, audio association, safety shot, confidence/timing parameters, calibration status, and decision policies in versioned show profiles. Obtain participant awareness/consent, minimize retained biometric material, support deletion, and keep voice/face data local. No automatic cloud upload or paid inference API.

### 6.4 Resource isolation and future virtual cameras

Perform inference off composition/audio threads using bounded analysis queues and proxy frames. Verify inference GPU/resource cost separately from rendering and encoding; never silently degrade Program 1080p50.

Issue **#2** covers future multiple crops from 4K/180°/360° cameras, dewarping, tracking, and reframing. Crop resolution and latency need measurement. This extension is not a prerequisite for #47 using multiple physical cameras.

## 7. Repository Baseline and Known Implementation Limits

### 7.1 Existing repository structure

```text
OPENVIDEOMIX/
├── Cargo.toml
├── crates/core/src/
│   ├── lib.rs, commands.rs, video_profile.rs
│   ├── media_timebase.rs, output_schedule.rs
│   ├── camera.rs, camera_registry.rs
│   ├── reference_compositor.rs, reference_output.rs
│   └── midi.rs, vmix_compat.rs
├── apps/openvideomix-desktop/src/
│   ├── main.rs, lib.rs, app.rs
│   ├── capture.rs, capture_devices.rs, capture_output.rs
│   ├── output_worker.rs, desktop_production.rs
│   ├── program_recording.rs
│   └── midi_input.rs, remote_control.rs
├── apps/openvideomix-opendeck/   # Separate Rust application/workspace
├── .github/workflows/
├── .github/ISSUE_TEMPLATE/
├── docs/                       # Design, ADRs, procedures, retained evidence
├── scripts/                    # Existing verification utilities
├── AGENTS.md
└── LICENSE
```

Do not describe proposed media/audio/output crates as already present. Logical separation may initially use modules; reorganize only when it solves a demonstrated engineering problem.

### 7.2 Existing versus incomplete

| Area | Baseline capability | Remaining qualification/work |
|---|---|---|
| Typed production commands | Shared state/dispatch foundation | Extend with actual production capabilities and events. |
| CPU compositor | Pixel-tested CUT, FADE, WIPE, four overlays | GPU engine, color rules, real sources, full desktop integration. |
| Capture | Optional GStreamer capture and independent synthetic sessions | Persistent physical selection, permissions, multicamera hardware modes. |
| Desktop vertical slice | Four synthetic sources → Preview → CUT → CPU Program → WebM/test tone | Sustained cadence, physical inputs/audio, GPU/encoder integration. |
| MIDI/API/OpenAction | Limited optional implementations | Physical devices, feedback, persistence/reconnect, release packaging. |
| Audio | Generated test-tone recording in baseline; separate live-audio work referenced by #44/#45 | Physical WASAPI/audio validation, full buses/AFV/PFL and drift handling. |
| Streaming/Stinger/scene editor | Requirements | No complete validated implementation established here. |
| Offline distributions | Required | Clean-install and offline end-to-end qualification pending. |

PR #45 must be inspected at its current head before use. Historical reports of passing CI do not establish review, merge, physical microphone support, or inclusion in the inspected baseline.

### 7.3 Retained media evidence

The repository records a short Linux synthetic desktop test under Xvfb/software OpenGL with GStreamer 1.26.2 and a four-CPU quota:

| Measurement | Recorded result |
|---|---|
| Video codec / dimensions | VP8 / 1920 × 1080 |
| Nominal rate | 50/1 |
| Container duration | 12.46 s |
| Decoded video frames | 214 |
| Effective file frame rate | Approximately **17.17 fps** |
| Missing video slots | **409** |
| Maximum video PTS gap | **200 ms** |
| Audio | Vorbis, 48 kHz, stereo, generated tone |
| Decoded audio sample frames | 598,592 |
| Audio padding relative to video end | Approximately 10.7 ms |
| Program switch evidence | Decoded pixel change from bars to red |

These results demonstrate a functional synthetic recording path and a significant unresolved performance gap. They do **not** qualify real-time 1080p50, physical A/V sync, Windows/macOS media runtime, GPU composition, or NVENC.

Recording SHA-256: `8bb39cb080938a18d56d127f937f6e7e545298009fe60835765678e54ebe0407`.

### 7.4 Existing reference-worker boundaries

The paced CPU worker uses rational deadlines and a common GStreamer clock epoch. The desktop test path documents a 100 ms lookback, 12-frame source history, a 500 ms maximum hold, independent monitor/record consumers, a two-packet recording delivery queue, bounded encoder waiting, and bounded EOS finalization. These are current test-path choices, not approved universal production latency targets.

The recorder's explicit frame-index API preserves gaps and requires PCM for the corresponding elapsed interval. Do not replace it with contiguous frame numbering that hides dropped time. GUI monitoring is downsampled while the recorded Program remains full resolution.

## 8. First Runnable Alpha and Release Architecture

### 8.1 Windows qualification bench

Owner-described target: **Windows 11 x64, NVIDIA RTX 4080 SUPER with 16 GB VRAM, 96 GB RAM, and a 2 TB NVMe SSD**. Older issue text mentions Windows 10; verify the actual installed OS/build and drivers rather than inheriting contradictory historical descriptions.

The first Alpha must be downloadable, extractable/installable, and launchable by double-click without Rust/Cargo. First qualify synthetic Program recording, then physical camera and WASAPI microphone paths when available. Run a 60-second baseline before a 30-minute endurance test.

### 8.2 Mandatory self-contained offline distribution

After the initial artifact download/transfer, shipped local functionality must work on an offline target computer without installing GStreamer, codecs, FFmpeg, Python, Rust, Ollama, OpenCode, or other development tools separately.

Bundle all legally redistributable, actually required application-local runtime libraries, plugins, codecs/muxers, transitive native dependencies, assets, redistributable runtimes, and any model weights required by enabled shipped AI features. No first-launch component download. Internet streaming naturally requires a network destination; that does not justify online dependency installation for local operation.

Use private loader/plugin paths, not machine-wide PATH changes. Apply equivalent packaging principles to Linux/macOS. OS permissions, device drivers, and compatible GPU drivers remain platform prerequisites and must be documented honestly.

### 8.3 Release gates

- Versioned artifacts linked to exact commits; reproducible dependency resolution.
- Clean-machine, network-disabled launch and synthetic recording validation.
- Actual bundled binary/plugin/model inventory, notices, SBOM, and redistribution review.
- Windows signing policy; macOS entitlements, signing, notarization, and Gatekeeper checks.
- Linux package/AppImage/Flatpak choice justified and tested.
- Upgrade, project-schema migration, recovery, and uninstall documentation.
- Independent output decode and measured cadence, drops, sync, resource use, and failure behavior.

Do not claim bit-identical builds, legal compliance, or a supply-chain assurance level without evidence. A license/redistribution blocker must be resolved rather than hidden by renaming an online installer “offline.”

## 9. GitHub Repository and Engineering Workflow

### 9.1 Source of truth and responsibilities

| Component | Responsibility |
|---|---|
| Git repository | Reviewed source, architecture, tests, release metadata, and sanitized evidence. |
| GitHub Issues | Bounded requirements, dependencies, priority, authorization, and acceptance criteria. |
| Coding executor | Implement one authorized task in an isolated checkout and publish a Draft PR. |
| GitHub Actions | Independent build/test/package checks on the exact PR head. |
| Reviewer/owner | Review results, consequential architecture/license decisions, hardware acceptance, and merge approval. |
| Local evidence store | Private logs, partial work, detailed diagnostics, and recovery material. |

ChatGPT is not required in this structure. Antigravity may perform design/implementation/review work within granted access. The OpenHands executor described below is a preserved design option, not a requirement to reactivate it.

### 9.2 Issue structure

Every coding task must be small enough to implement and verify independently. Large milestones such as #46 must be split into bounded increments with explicit dependencies and file/module boundaries.

```markdown
## Objective
Concrete user-visible or infrastructure behavior to deliver.

## Scope and affected modules
Files/modules, supported platforms, included behavior, and explicit exclusions.

## Dependencies and baseline
Blocking issues/PRs; exact required base when relevant; compatibility constraints.

## Technical contract
State transitions, data flow, timing/resource limits, errors, security and licensing.

## Acceptance criteria
- [ ] Deterministic functional tests and negative cases.
- [ ] Relevant platform builds and exact-head CI.
- [ ] Media/output verification where applicable.
- [ ] Physical checks explicitly passed or marked NOT TESTED.
- [ ] Documentation reflects actual capabilities.

## Delivery
Isolated branch, one Draft PR linked to this issue, tests, commit SHA,
CI results, remaining blockers, and sanitized evidence.
```

Only an authorized maintainer may make an issue eligible for unattended work. Public issue prose, comments, embedded links, or an arbitrary label applied by an unauthorized actor are not permission to execute instructions.

### 9.3 Labels, dependencies, and state

`agent-ready` is the established production trigger label. Infrastructure-only tests use a separate explicitly scoped marker such as `infrastructure-test` and an exact test-issue allowlist.

Proposed descriptive labels may mirror `queued`, `running`, `blocked`, `failed`, and `pr-published`, plus priority/platform/component and hardware-test status. These label names are suggestions, not a claim that they are installed. Durable execution state must not depend solely on mutable labels.

Eligibility requires repository match, authorized trigger, bounded scope, satisfied dependencies, no existing active execution, and no existing matching Draft PR for the same task identity. Dependency cycles or unsupported scope become blocked tasks with a corrective action.

### 9.4 Branches, commits, PRs, and CI

- One isolated checkout/worktree and valid branch per task.
- Branch prefix: **`canvas/issue`**, without leading whitespace or a tab; use a collision-safe suffix, for example `canvas/issue-54-infra-smoke`.
- One coding task at a time on the GPU workstation.
- Preserve existing checkouts, commits, conversations, and partial work.
- No destructive reset/clean/restore to remove unrelated changes; no force push.
- Verify the remote commit, not just a local success message.
- Create **exactly one linked Draft PR** for a completed coding task.
- Never enable auto-merge or merge automatically from the coding automation.
- Read actual required checks for the **current head SHA**. A push invalidates older-head evidence.
- Keep agent completion, PR publication, CI conclusion, review, merge, and issue closure as distinct facts.

For active unfinished work, prefer a PR body that references the issue without causing premature closure. A closing keyword is appropriate only when its merge semantics match the completed scope; opening a Draft PR is not issue completion.

### 9.5 Study → Build → Check → Refine

1. **Study:** inspect existing code, the bounded issue, dependencies, public operational references, and objective acceptance conditions.
2. **Build:** make the smallest coherent implementation using established interfaces.
3. **Check:** run deterministic checks, operate the real application where access permits, and decode/measure outputs independently.
4. **Refine:** correct measured mismatches and regressions; repeat affected checks; publish evidence and limitations.

This method does not require adopting Craft Anything software or adding another service. Do not replace functional development with endless reference prototypes, infrastructure scripts, or status reports.

## 10. Intended OpenHands Development System

### 10.1 Required unattended sequence

```text
Authorized GitHub issue
    → persistent coordinator detects eligible task
    → Wake-on-LAN wakes Windows workstation when required
    → workstation readiness and resource checks
    → official OpenHands execution with LOCAL Ollama
    → isolated checkout / task branch / verification / commit / push
    → exactly one linked Draft PR
    → coordinator records actual CI for the exact head
    → next eligible task, or idle policy permits workstation suspension
```

“Unattended” means the system does not ask the owner to approve routine commands, copy/paste between agents, transfer logs, or open each issue manually. Provision access and narrow permissions once, then operate within that scope. Missing credentials, physical actions, unresolved licensing, or unavailable access must produce a durable blocked state and a useful notification, not an endless approval loop.

### 10.2 Computer roles

| Machine/service | Intended responsibility | Outstanding evidence |
|---|---|---|
| miniPC | Always-available coordination, persistent queue, WOL, outcome/CI observation, existing notifications. | OS, coordinator installation, SSH identity/auth, autostart, LAN reachability and WOL are unverified. |
| Windows GPU workstation | Native OpenHands tools, repository worktrees, local Ollama, builds, physical media tests. | Complete unattended lifecycle, GPU qualification, OS-reboot autostart and idle suspension were not demonstrated. |
| GitHub | Authorized issues, branches, Draft PRs, hosted CI and artifacts. | Repository policies and each exact PR head must be inspected. |

Use supported OpenHands components where available. The mapping of the miniPC coordinator to the official OpenHands automation runtime is **not yet proven**. In particular, scheduling a Windows desktop automation every 15 minutes cannot wake its own suspended host. Do not silently replace the missing external coordinator with an unrelated custom orchestration platform or a duplicate production scanner.

Keep the OpenHands API and Ollama loopback-only. A supported secure control mechanism or tightly scoped authenticated host connection is required for remote dispatch. Do not assume a `127.0.0.1` endpoint on the miniPC or in a cloud environment refers to Windows. WOL packets must reach the target LAN through an appropriate authorized local sender; they are not a proof of SSH/API readiness.

### 10.3 Official executor and model configuration

The intended default is the official **OpenHands Agent Canvas GitHub Issue to PR** automation with the native OpenHands agent and an explicitly selected local Ollama profile.

| Setting | Intended value / requirement |
|---|---|
| Repository | `dodemo/OPENVIDEOMIX` only |
| Production trigger | Authorized `agent-ready` issues |
| Poll interval | 15 minutes, as previously planned; does not replace supervision |
| Global coding concurrency | 1 |
| Branch prefix | `canvas/issue` |
| PR type | Draft only |
| Merge | Never automatic |
| LLM profile | Existing `ollama-devstral`, after validation |
| Documented model candidate | `openai/devstral-small-2:24b-instruct-2512-q4_K_M` through an OpenAI-compatible provider |
| Local API base | `http://127.0.0.1:11434/v1` when both processes run on the same Windows host |
| API placeholder | A nonsecret local placeholder, if the client requires one; not a paid-provider credential |
| Provider fallback | Fail closed; no paid/cloud model switch, overages, or new billable APIs |

The `openai/` compatibility prefix and `/v1` route describe the API format; they do not imply OpenAI-hosted inference. Verify the actual endpoint, model tag, tool support, context configuration, output limits, and process behavior.

The earlier **OpenCode ACP 2.x** path failed during connection and is not a validated production dependency. It may remain an optional later integration, but the native OpenHands path avoids that extra protocol/process layer. Do not run the retired custom launcher/verifier or REPARADA/DIAGNOSTICO variants as additional production automations.

### 10.4 Execution-environment qualification

Run the following checks from a real OpenHands tool execution, not only from an unrelated PowerShell window:

- Actual native OS, shell, current directory, environment, user identity, and filesystem access.
- Git and GitHub CLI discovery, versions, PATH, and child-process inheritance.
- Repository identity and isolated checkout; valid native Windows paths, never `/workspace/...` on a native Windows host.
- Required MSVC developer environment, Rust, GStreamer development/runtime, and selected plugins for the task.
- Scoped credential availability and repository permissions without printing values.
- Actual model-generated tool execution, expected stdout/stderr, exit code, and artifact creation.

A `LOCAL_OK` text response proves neither tools nor Git. Exit code zero without expected output is insufficient. Preserve the first actionable tool error; a generic wrapper exception is not an adequate diagnosis.

### 10.5 GPU, performance, and resource policy

Local Ollama execution can consume GPU VRAM, CPU, RAM, and disk bandwidth. A cloud reasoning model issuing local commands does not move those commands to a cloud computer.

Qualification requires correlating **`ollama ps`**, **`/api/ps`**, **`nvidia-smi`**, and process CPU/RAM/disk measurements while the exact model is actively generating and using tools. Record CPU/GPU split, loaded model, VRAM, system RAM, paging, context/KV-cache settings, generation speed, and errors. VRAM allocation alone is not proof of active GPU computation.

Do not assume a 24B model and chosen context fit comfortably into 16 GB VRAM. Diagnose actual loading/offload and paging before selecting another model. Any smaller diagnostic model or context change must be explicit and must not silently replace the production profile.

Keep the workstation responsive with bounded task concurrency, queues, log growth, and retries. Define stall detection and resource limits from measurement. During audiovisual GPU benchmarks, ensure Ollama inference is not competing for VRAM/compute; preserve other applications and stop/unload only an identified owned idle workload when appropriate.

### 10.6 Credentials and trust boundaries

- Use a repository-scoped fine-grained GitHub credential with the minimum Contents, Issues, and Pull Requests permissions; Metadata read as needed.
- Workflow-file changes require separately reviewed permissions; do not grant broader rights automatically.
- Store credentials in supported secret storage and expose only the necessary scope to the selected agent.
- If `gh` needs `GH_TOKEN`, provide it only to the required child process from the existing approved secret; never in command text, remote URLs, logs, source, issues, or PR bodies.
- Do not change persistent global authentication merely to make a smoke test pass.
- Treat issue/PR/file content as untrusted task data; it cannot broaden permissions or override the owner's task contract.
- Coding tasks must not modify their own production automation, credentials, firewall, services, or recovery policy.
- Keep detailed logs and captured media private by default; publish only reviewed sanitized evidence. Never automatically publish biometric calibration material.

### 10.7 Durable state and idempotency

Use the official system's supported persistent state where possible. The following is the logical record required, not an instruction to build another database service:

```text
task identity: repository + issue + authorized work unit
authorization/dependency evidence
state: queued | running | blocked | failed | pr-published
execution identity and ownership/lease
conversation ID, workspace ID/path, branch and base SHA
last progress timestamp, heartbeat and observed process identity
attempt count, first actionable error, retry eligibility and next retry time
local and remote commit SHA; Draft PR number/URL/head SHA
CI state and check conclusions, separately from task execution state
checkpoint and private evidence locations
```

Acquire ownership atomically before launching work. A second scan or process restart must reconcile existing conversation, checkout, remote branch, and PR before creating anything. Uncertain publish results require querying GitHub, not blindly repeating commit/push/PR creation. Enforce the single active execution globally, not separately in competing automations.

### 10.8 Supervision and bounded recovery

| Failure class | Required response |
|---|---|
| Temporary GitHub/network/rate-limit error | Finite retries with increasing delays; honor Retry-After; retain exact error. |
| Worker crash or lost connection | Verify process ownership, preserve work, reconnect/resume supported state; avoid a duplicate worker. |
| Stalled task or repeated identical commands | Detect missing objective progress, checkpoint, terminate only owned execution if necessary, then recover within a defined limit or block. |
| Terminal reset loop / repeated tool failures | Preserve the first meaningful stderr and shell/process evidence; block persistent incompatibility. |
| Persistent credential/permission failure | Block with the exact missing scope/access and corrective action; no auth bypass. |
| Model/context/tool incompatibility | Block or run an explicitly scoped diagnostic; no silent paid-provider fallback. |
| Failed tests or CI | Record failure separately; allow a bounded scoped correction, never disable tests or weaken CI. |
| Ambiguous push/PR response | Reconcile remote state and reuse the existing result. |
| No miniPC access / physical device unavailable | Mark affected tests NOT TESTED and preserve the blocker. |

Recovery limits and timeouts must be configured and tested before production enablement. Their final numeric values were not agreed or verified in the previous attempt; do not invent a claim that “three retries” or a particular stall timeout is already active. Persistent failures stop the affected task with evidence. No infinite reset loops, uncontrolled scans, or arbitrary infrastructure changes by the coding LLM.

Use the existing notification mechanism if one is available; no additional messaging service is required by this design.

### 10.9 Autostart, restart continuity, wake, and suspension

After one-time provisioning, normal reboot must not require reopening apps, re-entering credentials, changing PATH, manually starting a terminal, or reconfiguring the queue.

Required startup order: connectivity/secure secret access → Ollama readiness → native OpenHands readiness → persistent-state reconciliation → eligible-task processing. The miniPC coordinator must start automatically as well.

Prefer supported services or scheduled startup facilities. Exact service names and startup triggers remain implementation details to verify. If Agent Canvas requires an interactive user session, **start-at-logon is not start-at-boot**: explicitly resolve whether execution before login is supported. Do not silently enable Windows auto-login or store a plaintext account password.

On restart, reconcile paused/running tasks, preserve partial work, and resume or block safely without duplicate agents, commits, or PRs. Test both application/coordinator restart and a full OS reboot; one does not prove the other.

Keep Windows awake while an owned task, build, Git operation, encoder/required child process, checkpoint, or result publication is active. Permit idle suspension only after state and ownership checks confirm no required local work remains. CI observation can continue on the miniPC after Windows work is safely complete. Test actual WOL, readiness delay, sleep/wake, and subsequent state recovery on the physical computers.

### 10.10 Stop and rollback contract

An emergency stop first disables new dispatch and production scanning, then pauses/cancels owned conversations through supported controls, preserves work and evidence, and releases owned resources after checkpointing. Stopping a maintenance CLI does not automatically stop OpenHands, Ollama, or child processes.

Before changes, back up affected OpenHands settings/state, a consistent SQLite snapshot, relevant Electron state, automation/profile definitions, startup configuration, and previous environment-variable presence/value. Protect secret-bearing backups for the owner and SYSTEM on Windows; keep per-attempt immutable directories.

Verify hashes, SQLite integrity and foreign keys, and an isolated restore of the actual snapshot. Restore tooling must preserve the current state before replacing it, use a specifically selected backup, and leave automations disabled until reviewed. Production restoration and autostart rollback need their own tests; a synthetic or isolated restore does not prove a live restore.

## 11. OpenHands Acceptance Tests and Previous Attempt Status

### 11.1 Required test matrix

| ID | Test | Concrete pass evidence |
|---|---|---|
| A01 | Detect one authorized infrastructure issue | Scanner result identifies the exact allowed issue and authorization. |
| A02 | Execute real tools | Actual OpenHands tool events, expected output, and exit codes. |
| A03 | Correct repository and isolated checkout | Repository identity, separate checkout and branch/base evidence. |
| A04 | Small test edit, verify, commit, push | Exact fixture contents, verification, local commit and matching remote SHA. |
| A05 | Exactly one linked Draft PR | GitHub URL, draft flag, issue linkage and expected head SHA. |
| A06 | Actual CI recorded | Required check conclusions for that exact head, stored separately from agent completion. |
| A07 | Second scan creates no duplicate work | Same execution/workspace/PR; no second agent or duplicate PR. |
| A08 | Harmless controlled interruption | Expected bounded recovery or blocked state, preserved partial work and diagnostics. |
| A09 | Coordinator restart preserves state | Durable task identities and ownership reconciled without duplicate execution. |
| A10 | WOL and idle suspension | Actual miniPC/Windows wake, readiness, awake-during-work and idle-sleep evidence. |
| A11 | Full Windows reboot and autostart | No manual application launch or reconfiguration; safe queue continuity. |
| A12 | Local-model/GPU qualification | Exact endpoint/model plus correlated GPU/CPU/memory observations during real work. |

A01–A10 consolidate the original infrastructure acceptance contract. A11 and A12 make the later explicit restart and GPU requirements separately visible. Keep normal issue processing disabled until required software tests pass; inaccessible hardware requirements remain explicit blockers or NOT TESTED, never silently “passed.”

### 11.2 Explicit infrastructure fixture

Issue **#54** is the isolated acceptance issue. Its recorded scope is a single file, `docs/infrastructure/openhands-smoke-test.txt`, containing:

```text
Harmless OpenHands infrastructure acceptance fixture.
```

Verify exact content and `git diff --check`, then commit, push the test branch, and open exactly one linked Draft PR. No product feature work, CI changes, service/credential changes, or unrelated checkout modification belongs in that coding task.

Issue #49 is a product-placeholder-label task, not the infrastructure fixture. PR #50 explicitly described itself as created through ChatGPT rather than the local OpenHands agent; it cannot prove this pipeline.

### 11.3 Reported partial results and unresolved failures

Owner-supplied local reports described protected backups, 3,374 verified files, SQLite checks, isolated restore, loopback OpenHands startup, local tool/authentication checks, whitespace correction, single-task rejection with HTTP 429, and retained state after an application restart. These are reported partial results, not newly rerun acceptance results.

The same attempt also showed API validation HTTP 422, tool exits including 128 without sufficient visible cause, terminal interruption failures, a 24B model repeatedly failing, a smaller 14B/16K diagnostic configuration, and a test-script duplicate-conversation guard. Repeated screenshots showed no published PR. A “Successful” issue sweep means the scan completed, not that coding, pushing, PR creation, or CI passed.

No complete evidence package in this handoff establishes A01–A12 as passed end to end. GPU use, full Windows reboot/autostart, authenticated miniPC access, WOL, idle suspension, a successful OpenHands-created fixture PR, and its exact-head CI remain unverified here. The owner stopped the attempt; do not silently resume it.

## 12. CI, Testing, Performance, and Engineering Quality

### 12.1 Existing CI foundation

The inspected Rust quality workflow includes base jobs on `ubuntu-24.04`, `windows-2022`, `macos-15`, and `macos-15-intel`, plus Linux GStreamer capture/recording/all-feature checks. Additional workflows cover production core, camera core, reference compositor, MIDI, local control API, UI status, and OpenDeck build/package tests.

This is workflow configuration, not a fresh statement that every current check is green. Base cross-platform Rust builds do not qualify optional Windows/macOS GStreamer runtime or physical hardware.

### 12.2 Required validation layers

1. Unit/contract tests: rational timing, commands/state, alpha/pixels, source identity, MIDI mapping, serialization, transitions.
2. Integration tests: real GStreamer synthetic sources, bounded queues, Program tap, audio timestamps, output finalization and decode.
3. E2E tests: actual desktop source selection, Preview/CUT/Program, recording/streaming state and output.
4. GUI/accessibility tests: keyboard, focus, HiDPI, multiwindow, non-color status, platform accessibility.
5. Physical tests: cameras, capture cards, microphones, USB controllers, GPU encoders, multiple USB sources, sleep/wake, A/V sync.
6. Release tests: clean/offline install, dependencies, licenses, upgrade/migration, signing, recovery and uninstall.
7. Automation tests: the separate A01–A12 matrix above.

Representative repository checks, adapted to the relevant features and installed native dependencies:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build --manifest-path apps/openvideomix-opendeck/Cargo.toml --release
```

Use release-build performance measurements. Independently decode recordings with FFmpeg/FFprobe or suitable bundled verification. Record actual capture/composition/delivery/encode cadence, drops, PTS gaps, latency, A/V offset/drift, CPU/GPU, VRAM/RAM, paging, disk and network behavior. Freeze exact builds/configurations for comparisons. Do not invent unmeasured latency/drop thresholds; agree measurable release limits before qualification.

### 12.3 Governance and release documentation

Issue #33 tracks CONTRIBUTING, SECURITY, CHANGELOG, PR templates, release checklist, API/project schema policy, lockfiles for distributable binaries, dependency advisory/license review, SBOM/notices, artifact provenance, and verified branch protection. These are requirements; not all are present.

Track dependency resolution with appropriate lockfiles and `--locked` once established. Review actual shipped GStreamer plugins/codecs, native libraries, SDKs, inference weights and licenses. Keep ADR status explicit. Never suppress tests/advisories simply to produce green CI or assert that repository MIT licensing settles every distribution obligation.

## 13. Delivery Roadmap and Issue Map

### 13.1 Implementation order

| Stage | Deliverable | Exit condition |
|---|---|---|
| 0 | Reconcile architecture, code baseline and execution environment | One coherent source of requirements; no competing agent writers. |
| 1 | First runnable Windows Alpha (#46) | Self-contained downloadable build; synthetic Preview/CUT/Program recording; measured results. |
| 2 | Physical source/audio vertical slice (#6, #44/#45, #7) | Selected physical camera/microphone, common clock, genuine levels and decoded synchronized recording. |
| 3 | Qualified media engine and controls (#3–#8, #13/#14) | Measured GPU/output behavior, real transitions/overlays, audio routing, controller integration. |
| 4 | Complete agreed first-version UX/output | Scene editor, editable graphics, Stinger, RTMP, projects, accessibility and platform qualification. |
| 5 | Multimodal auto-director (#47) | Validated calibration, manual override, safe fallbacks, physical latency/resource evidence. |
| 6 | Release qualification (#33) | Offline packages, licenses, security, docs, clean install and cross-platform evidence. |

Stages express dependency order, not permission to omit Stinger/streaming or to indefinitely postpone required auto-direction. Work may be divided into independent reviewed increments, but a useful integrated audiovisual application remains the goal. Do not start GPU rewrites or broad GUI polishing before identifying a measured Alpha blocker.

### 13.2 Referenced issue inventory

| Issue | Design responsibility |
|---|---|
| #2 | Future high-resolution/panoramic virtual crops and auto-framing. |
| #3 | Media architecture, clock, licensing and platform decisions. |
| #4 | Native operator console, accessibility and layout. |
| #5 | Actual CUT/MIX/WIPE, overlays and first-version Stinger. |
| #6 | Real camera/screen/file sources and lifecycle. |
| #7 | Audio graph, metering, buses, AFV/PFL and sync. |
| #8 | Outputs, projects, CI, packaging and truthful operational status. |
| #10 | OBSBOT Tiny 2 fixed camera; optional PTZ/tracking. |
| #13 | OpenDeck/OpenAction/Stream Deck controls. |
| #14 | USB MIDI, mapping and pickup. |
| #33 | Engineering documentation, reproducible releases, licenses and security. |
| #38 | Reconcile stale capture documentation. |
| #40 | Historical Antigravity coordination feasibility pilot; not proof of an installed workflow. |
| #41 | Historical audit and implementation-state reconciliation. |
| #44 / PR #45 | Live captured audio into timestamped Program recording; inspect actual head/status. |
| #46 | First self-contained Windows Alpha and physical qualification path. |
| #47 | Mandatory multimodal auto-director, profiles and manual priority. |
| #49 | Product demo-label smoke task; not the infrastructure acceptance fixture. |
| #54 | Isolated OpenHands/Ollama infrastructure acceptance fixture. |

An issue remaining open is not automatic authorization for execution. Read current scope, dependencies, labels, PRs and checks before starting it. Historical overnight authorizations do not override the later stop instruction.

## 14. Handoff Constraints and Open Decisions

### 14.1 Constraints to preserve in any executor

- Continue the existing repository; do not discard working code or history.
- Keep product requirements separate from coding-tool preferences.
- Use one executor per coding task and one writer per checkout.
- Preserve Windows/Linux/macOS and the exact default media profile.
- Prefer measurable integrated user functionality over unrelated infrastructure growth.
- Distinguish synthetic tests, physical evidence, CI success, and production readiness.
- No automatic merge, paid API fallback, credential exposure, or public private-log upload.
- No claiming remote control or physical hardware access until actually demonstrated.
- Unattended operation must have bounded supervision, safe restart, and useful blocked-state evidence.

### 14.2 Decisions still requiring evidence or explicit resolution

1. Production compositor/GPU interop and final GUI stack, based on cross-platform measurements.
2. Exact codec/plugin/model distribution inventory and any unresolved licensing obligations.
3. Measured sustained performance, A/V sync, endurance and resource limits on each target platform.
4. Current PR #45 integration status and a complete captured-audio path.
5. Antigravity execution capabilities, local/remote access, authentication, lifecycle and subscribed-entitlement boundaries; do not infer them from the tool's name.
6. If OpenHands is reconsidered: supported miniPC coordination/dispatch, authenticated access, pre-login versus post-login startup, WOL and suspension, bounded recovery settings, and a complete acceptance evidence package.
7. Current disposition of all maintenance-created scripts, test conversations, fixture branch/PR, disabled variants, and backups. Preserve them; no automatic cleanup or reactivation.

## 15. Source Register

Repository file links below are pinned to the inspected baseline. Issue links are mutable requirements and were read during preparation on 15 November 2026. Local-session statements are identified as reported evidence rather than independently verified computer state.

### 15.1 Product and architecture sources

- [README — implementation overview](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/README.md)
- [AGENTS — repository conventions and task boundaries](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/AGENTS.md)
- [Product direction and final UX decisions](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/PRODUCT_DIRECTION_2026-11-14.md)
- [Architecture](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/ARCHITECTURE.md)
- [ADR-0001 — proposed cross-platform media architecture](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/adr/0001-cross-platform-media-architecture.md)
- [Roadmap](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/ROADMAP.md)
- [vMix/OBS product analysis — historical details reconciled above](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/VMIX_OBS_ANALYSIS.md)
- [Desktop Program test and retained measurements](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/DESKTOP_PROGRAM_TEST.md)
- [Independent recording verification JSON](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/evidence/desktop-program-2026-11-14/file-verification.json)
- [Reference compositor](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/REFERENCE_COMPOSITOR.md)
- [Capture-to-output ingress](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/CAPTURE_OUTPUT_INGRESS.md)
- [Paced reference output](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/PACED_REFERENCE_OUTPUT.md)
- [Program recording contract](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/PROGRAM_RECORDING_POC.md)
- [Windows hardware procedure — not executed evidence](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/WINDOWS_HARDWARE_TEST.md)
- [Control API](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/LOCAL_CONTROL_API.md)
- [MIDI proof of concept](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/NATIVE_MIDI_POC.md)
- [Restricted vMix shortcut adapter](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/VMIX_SHORTCUT_COMPATIBILITY.md)
- [Standalone OpenAction plugin](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/apps/openvideomix-opendeck/README.md)

### 15.2 Development-system and requirement sources

- [Agent Canvas workflow — prepared, not end-to-end validated](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/docs/AGENT_CANVAS_WORKFLOW.md)
- [Base and optional-media CI configuration](https://github.com/dodemo/OPENVIDEOMIX/blob/e397fe646343c4480bd94db701b1ea301547f8af/.github/workflows/rust-quality.yml)
- [#3 — architecture](https://github.com/dodemo/OPENVIDEOMIX/issues/3)
- [#4 — console](https://github.com/dodemo/OPENVIDEOMIX/issues/4)
- [#5 — transitions and Stinger](https://github.com/dodemo/OPENVIDEOMIX/issues/5)
- [#6 — real sources](https://github.com/dodemo/OPENVIDEOMIX/issues/6)
- [#7 — audio](https://github.com/dodemo/OPENVIDEOMIX/issues/7)
- [#8 — outputs and projects](https://github.com/dodemo/OPENVIDEOMIX/issues/8)
- [#10 — OBSBOT](https://github.com/dodemo/OPENVIDEOMIX/issues/10)
- [#13 — OpenDeck](https://github.com/dodemo/OPENVIDEOMIX/issues/13)
- [#14 — MIDI](https://github.com/dodemo/OPENVIDEOMIX/issues/14)
- [#33 — release and engineering gates](https://github.com/dodemo/OPENVIDEOMIX/issues/33)
- [#44 — live audio slice](https://github.com/dodemo/OPENVIDEOMIX/issues/44)
- [#46 — self-contained Windows Alpha](https://github.com/dodemo/OPENVIDEOMIX/issues/46)
- [#47 — auto-director and exact calibration requirements](https://github.com/dodemo/OPENVIDEOMIX/issues/47)
- [#2 — future virtual cameras](https://github.com/dodemo/OPENVIDEOMIX/issues/2)
- [#54 — harmless infrastructure fixture](https://github.com/dodemo/OPENVIDEOMIX/issues/54)

### 15.3 Conflict-resolution notes

- Historical T-Bar/TAKE references are superseded for the visible final UI; continuous internal transition progress remains required.
- Older statements that capture PRs #22/#23 are unmerged are stale relative to the baseline overview and integration record.
- Broad historical autonomous goals mentioning automatic integration do not override the later **Draft PR only, no auto-merge** automation contract.
- Historical OpenCode ACP setup instructions do not override the later native OpenHands fallback after ACP connection failure.
- Early “AI is future-only” wording must not erase mandatory #47; only the panoramic/virtual-crop extension #2 remains separately future-scoped.
- Application-restart tests do not satisfy full OS reboot/autostart requirements.
- Reported maintenance progress does not override the final stop instruction or establish an operational unattended system.

---

**Completion rule:** OPENVIDEOMIX is complete only when its agreed audiovisual capabilities are implemented, packaged, and supported by appropriate executable and physical evidence. The development automation is complete only when its independent acceptance matrix passes. Neither completion may be inferred from the other.
