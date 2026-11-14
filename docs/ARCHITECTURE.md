# Architecture reset

The 2026-11-14 branch is a clean product restart. Rust, egui and all prior runtime choices are **not inherited**.

## Evaluation criteria
1. OBS/libobs integration, licensing compatibility and maintainability.
2. Reliable Windows/Linux device capture and audio/video synchronization.
3. Low-latency Preview/Program switching, scene composition and Stinger support.
4. MP4 recording with robust crash handling and Adobe Premiere Pro interoperability.
5. RTMP stability, local API, modular automation and GPU acceleration.
6. Sustainable packaging, testability and open-source contribution workflow.

## Implementation milestones
- M0: validate licenses and choose stack with a short architecture decision record.
- M1: capture source, Preview and Program, CUT, MP4 output; benchmark sync and latency.
- M2: audio mixer, MIX/WIPE/Stinger, independent scene editor.
- M3: RTMP, show profiles, autosave/recovery, reporting, Stream Deck API.
- M4: PiP, Video Follows Audio, optional local AI directing.

No source tree or dependency manifest is included until the stack is selected. Do not present this branch as a functional application.
