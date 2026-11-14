/goal Complete OPENVIDEOMIX as a usable, tested, cross-platform, open-source live video production switcher inspired primarily by vMix. Continue autonomously through implementation, testing, integration and release. Do not stop after a plan, PR, checkpoint or status report. Stop only when the first-release acceptance criteria are verified, or when an unavoidable external blocker or execution budget prevents further useful work.

## Repository and existing work

Repository: https://github.com/dodemo/OPENVIDEOMIX

Continue the EXISTING project and Codex development history. Do not restart, rewrite or replace working components without evidence.

Read AGENTS.md, README.md, docs/PRODUCT_DIRECTION_2026-11-14.md, docs/ARCHITECTURE.md, the roadmap, all active issues, PRs and CI results.

Use the latest GitHub state, not historical assumptions. In particular, inspect PR #36, its dependency chain and docs/DEVELOPMENT_CHECKPOINT_2026-11-14.md.

The project is named OPENVIDEOMIX, never OPENVIDEOMIX+.

## Product definition and completion criteria

Build a real audiovisual switcher, NOT merely a GUI mockup, documentation project, disconnected libraries or an endless series of CPU reference prototypes.

Required first-release capabilities:

- Native Windows, Linux and macOS support, including Intel and Apple Silicon.
- Default 1920 × 1080 progressive, exactly 50/1 FPS, with 48 kHz audio.
- At least four simultaneous configurable video inputs.
- Real camera capture, USB/HDMI capture devices, screen/window capture, media files and graphics.
- Independent live Preview and Program monitors.
- Functional CUT, MIX and WIPE transitions; four independent overlays.
- First-release Stinger with alpha, visual timeline, frame stepping and exact switch-frame selection.
- A vMix-inspired operator console with source thumbnails, audio controls and editable graphics.
- Separate scene editor with changes applied to Program only when explicitly requested.
- No visible T-Bar or TAKE button in the final UI.
- Real audio mixing, gain, mute, AFV, PFL, dBFS metering and A/V synchronization.
- Recording of the ACTUAL composed Program signal into playable audiovisual files.
- RTMP streaming with truthful status and recoverable errors.
- Stream Deck/OpenDeck/OpenAction and USB MIDI control, including MIDI Learn.
- Saved projects, documentation, installers and auditable dependency licenses.

Prefer vMix operational behavior when product decisions are unclear. Use OBS as the secondary reference. Use original implementation and assets.

## Craft Anything methodology — mandatory

Apply the clean-room Study → Build → Check → Refine/Ready workflow from https://github.com/storytold/craft-anything.

For EVERY functional milestone:

1. STUDY: inspect the relevant observable vMix behavior, public documentation, interactions and UI references. Identify an objective acceptance checklist. Do not copy proprietary code, branding or artwork.
2. BUILD: implement the equivalent behavior in OPENVIDEOMIX, reusing the existing architecture.
3. CHECK: launch the actual application where possible, interact with it, inspect screenshots and compare real behavior against the reference and acceptance checklist. Verify media outputs independently using executable tests, FFprobe or suitable tools.
4. REFINE: fix mismatches and regressions, repeat tests and visual comparisons until the milestone passes. Then document evidence and proceed.

Evaluate Craft Anything's actual tooling and licenses before integrating it. Its METHOD is required; installing its software is optional and must not delay implementation.

Never mark a milestone complete because code compiles or a UI resembles vMix. Require functional evidence.

## Immediate priorities

First complete the timestamp/gap-aware recording work identified by PR #36, preserving output PTS and synchronized audio intervals.

Then prioritize an END-TO-END functional vertical slice:

real camera or test video → Preview → actual CUT → Program → synchronized, playable recording.

Make this usable from the desktop console, not only headless tests. Continue with the real-time GPU compositor, live audio graph, additional transitions, streaming and remaining first-release features.

Avoid endless low-level infrastructure work without integrating it into a usable application. Prefer the next milestone that produces demonstrable end-user functionality.

## Autonomous workflow

Maintain a dependency-aware backlog from GitHub Issues and PRs.

For each increment: inspect → implement → test → review → commit → push → PR → CI → integrate when safe → update documentation → select the next actionable increment.

Run formatting, compilation, Clippy, unit, integration and relevant cross-platform tests. Add deterministic media tests and measured performance checks.

Do not merge failing or pending required CI, force-push, disable safeguards, expose secrets or claim physical hardware validation without actual testing.

When CI is pending, work on independent tasks. When one implementation fails, diagnose and retry with a different justified approach. Do not ask me to choose routine technical solutions.

You are authorized to make normal engineering decisions and manage repository changes within existing permissions. Escalate only missing external credentials, unavailable physical hardware, unresolved consequential licensing decisions or genuinely irreversible actions.

Implement issue #33's professional documentation, license, dependency-security and release requirements as part of the release process.

My physical test workstation is Windows with an RTX 4080 SUPER 16 GB, 96 GB RAM and a 2 TB NVMe SSD. See issue #8. Prepare repeatable hardware test procedures; never pretend to have tested this PC remotely.

## Continuity and supervision

Use a native persistent Codex Goal when supported. If unavailable, investigate a single thread-attached recurring continuation automation, with no overlapping runs, using only authorized facilities.

Do not silently create external paid services or unsupported self-running agents.

Persist the exact current state, completed work, evidence, blockers and next executable action in the repository, updating AGENTS.md and the development checkpoint as needed.

If a session reaches a limit, save committed progress and a complete resume instruction. Never label an unfinished project COMPLETE.

Do not ask me "Should I continue?" and do not require repeated confirmations for ordinary development work.

Send concise progress summaries in Galician. These are informational, not requests for permission.

## Final acceptance

Declare the first release complete only when OPENVIDEOMIX can be installed, launched and operated as a basic vMix-style live switcher, with real capture, switching, synchronized audio, recording, streaming, transitions, control integration and validated platform behavior.

Defer explicitly future-only enhancements such as AI-directed virtual cameras rather than letting them delay the first release.

BEGIN NOW. Execute the next actionable task. After completing it, automatically inspect the remaining acceptance criteria and continue wherever the available Codex execution lifecycle permits.