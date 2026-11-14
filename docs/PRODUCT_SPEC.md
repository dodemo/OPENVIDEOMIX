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

## Confirmed interview-production decisions (14 November 2026, continued design)

### Camera allocation and safety
- **Exactly one physical camera per participant, plus one mandatory dedicated physical wide-shot camera**. Participant count varies. The wide shot is **not** a composite scene; do not reinterpret it as one. The general source count limit of four for the initial release must be reconciled with this requirement (three participants + wide camera at most if that limit remains).
- Participant faces are assumed visible in their assigned cameras during the show. If speech is detected but no matching moving mouth can be identified reliably, cut to the physical wide-shot camera. If two or more participants speak simultaneously, use wide shot or an operator-configured listening shot; wide shot is the safe default. After a single stable speaker returns, allow an approximately 500 ms configurable settling interval before another automatic cut.
- Manual operator selection immediately suspends AUTO and switches to MANUAL. **Never resume AUTO without an explicit operator action**. AI may continue analysis in MANUAL but cannot switch Program.
- If a configured camera loses its signal, its input tile stays visible and shows a prominent **red SEN SINAL / NO SIGNAL warning**. Never silently remove it.

### Automatic input layout and localization
- Follow the established vMix-style fixed main layout: Preview left and Program right at the top, switching controls between/below them, configured source tiles at the bottom, audio and other controls in consistent locations. Main panels are not freely rearrangeable; the separate scene editor remains independently movable.
- Show **only configured sources**, with no empty placeholder inputs. Automatically resize/reflow thumbnails for two, three or four sources; allow reordering sources and save their order per show. Disconnected-but-configured sources remain visible with warning.
- Complete Galician (gl) and English (en) interfaces from first release, with first-launch language selection and changeable global preference. Translate menus, notifications, wizard, help and reports; design for future locales.

### Supervised audiovisual calibration and profiles
- Each participant reads a neutral Galician text for **at most 20 seconds**, facing camera with synchronized microphone audio. The system proposes a provisional voice/face/lip association; the **technician must verify the visible person, mouth and voice and explicitly confirm the participant's name** before the profile can control AUTO.
- Calibration is a bootstrap, not proof of biometric identity. Use speaker embeddings plus face tracking and audio-visual active-speaker synchronization; mouth motion alone cannot establish speech. Continue validation during natural conversation, detect contradictions, allow retake and operator corrections, and flag duplicate identities for manual review.
- Offer both show-only ephemeral profiles and reusable participant profiles, with explicit authorization before persistence. No automatic identity merges. Minimize stored biometric data and provide deletion controls; assess consent and applicable privacy obligations before shipping.
- In case of uncertainty, preserve operator control and prefer wide shot rather than an arbitrary participant camera. Voice-only matching without matching mouth is insufficient to select a close-up.

### External audio and initial output simplicity
- Main production audio usually arrives as **one external stereo mix** from a mixing console over USB or analog line input (via sound card/interface); also support embedded camera audio and optional music as separate input sources to the internal mixer. Prevent doubled audio and provide audio/video sync adjustment.
- First version records **one switched Program video track and one final stereo mixed audio track** in MP4 (H.264/AAC 48 kHz default). **Do not require separate voice/music tracks in v1**. Preserve internal source separation for potential later multitrack recording and postproduction.
- Automatic speaker recognition must work from a mixed stereo feed plus video evidence; performance and latency require empirical tests, especially with overlapping voices.

### Pending validation and design risks
- Confirm the initial camera/source ceiling in light of the mandatory wide camera and variable participant count.
- Measure active-speaker detection reliability with stereo mix, overlapping speech, low light, different angles, microphone bleed and lip occlusion. The 20-second sample and ~0.5-second end-to-end target are design goals, **not verified guarantees**.
- Confirm robust fallback for no-signal on the mandatory wide camera, and distinguish operator-visible errors from live Program output.
- Confirm cross-platform audio device handling, sync drift, hardware capture and recording continuity under GPU load.

### Calibration reading example (Galician, estimated under 20 seconds)
> Bo día. Vou probar o micrófono e a cámara. Papá prepara pan, mamá bebe auga e Pepe mira pola ventá. Agora conto: un, dous, tres, catro e cinco. A miña voz e os meus labios móvense mentres falo con naturalidade.

This is a sample for evaluation; do not hard-code a person's name into a participant identity. Technician supplies and confirms the participant name separately.
