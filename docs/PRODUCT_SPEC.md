# OPENVIDEOMIX — functional specification

Date: 2026-11-14. Status: product-design baseline, **not implemented**.

## Purpose and platforms
- Live video switching and production, primarily interviews; not a nonlinear editor.
- Windows and Linux; open source; use OBS/libobs technology where technically and legally feasible.
- Four simultaneous video sources in initial release, extensible architecture. Capture: HDMI/SDI, USB, NDI/SRT/RTSP, files, windows/screens and web pages, subject to implementation feasibility.

## Switching and composition
- Distinct Preview and Program monitors. Large CUT, MIX, WIPE buttons; MIX/WIPE presets 0.5, 1, 2 seconds and custom durations.
- Configurable Stinger with transparent animation and exact switching frame.
- Separate scene editor window, potentially on a second display; editing must not affect live Program until taken. Scene duplication, source reuse or isolation, layers, hide/lock, z-order and transforms.
- Titles/lower thirds, playlists (manual/automatic), PiP with presets and free placement. Pinning a PiP overrides automatic removal.
- PiP appears in clean-feed recording; clean feed excludes graphics by default, configurable per show.

## Audio and automatic directing
- Independent audio mixer with gain, mute, meters and optional processing; audio does not have to follow selected video.
- Video Follows Audio: microphone triggers, hold and minimum cut intervals, reaction/wide shots, operator override, AUTO/MANUAL mode.
- Later/advanced local AI: speaker diarization and voice signatures, face recognition, lip-sync association, identity-to-camera matching, participant calibration and configurable lower-third frequency. Target overall broadcast latency around 0.5 s is a **hypothesis requiring measurement**, not a guarantee.
- Interview setup asks participant count; participant identities are unique, can be merged manually, added or removed live.

## Recording and streaming
- Record without streaming or do both. Single switched video plus final mixed audio; no isolated microphone tracks initially.
- MP4 only (not MKV), default H.264 + AAC 48 kHz, constant frame rate; prioritize Adobe Premiere Pro compatibility. Crash-resilient MP4 strategy and OBS Hybrid MP4 compatibility require validation.
- Default suggested 1080p/25 fps/~15 Mb/s; configurable resolution, fps, codec and bitrate, saved per show.
- Recording segments by time or size, 15 minutes default, with seamless handover to validate. Timestamped files in a show-specific directory. Future Premiere timeline/project export.
- Estimate required disk space from expected duration and bitrate plus 20% margin. Warn on low space; never delete files automatically. Audible operator-only alerts never enter Program/recording.
- Recording Start/Stop default; optional Pause per show. Separate REC and LIVE indicators/timers; red border around Program only when live.
- One RTMP destination initially; secure handling of URL/key. Verify connection before starting, then require explicit confirmation. Optional configurable countdown. Warn if streaming without recording and offer immediate Record action. Confirm stopping stream while recording. Separate RTMP audio mute deferred.
- Rehearsal mode with no recording or streaming; RTMP requires confirmation.

## Show management and operator workflow
- Startup: open show, new show, duplicate show or resume persistent draft. New-show wizard: name, format, resolution/fps, sources, audio, recording, RTMP and AI. Interview template first.
- Autosave changes with version recovery. Abandoned drafts persist until manually deleted. On duplicate show, offer blank RTMP credentials (default) or copy, warning that renaming a destination does not create a new endpoint.
- Show switching prohibited while recording or streaming. Shortcut defaults customizable per show; optional global set. Stream Deck integration through local API/OpenDeck, editable ordered macros; Companion later.
- Operator selected at show entry; required selection may be a saved habitual operator or 'Convidado'. No mid-show operator switching or repeat confirmation at record start.
- Quick notes with automatic timestamp and keyboard shortcut.
- Check for updates at launch; do not install or otherwise change the application while recording/streaming.

## Reports and event history
- Generate TXT and CSV report after any recording **or** stream, even without recording; show a completion summary and reminder if unseen.
- Include recording/stream start/end times, duration, technical settings, clean-feed/PiP flags, operator, pause/resume counts and reasons when known. Do not log countdown start/end in the report.
- Basic event history for record/stream starts/stops and scene changes, capped at 1000 entries by default; export TXT/CSV. Delete history only via Export & Maintenance, with confirmation, not the main interface.

## Future ideas
- Virtual crops from one 4K/180°/360° source, including tracking and dewarping (tracked separately).
- Premiere timeline export, Companion, advanced AI features and per-output audio control.

## Interpretation
Product choices here are requirements or design intentions, not claims of working code. Unresolved feasibility, licensing, platform and latency questions must be tested before implementation.
