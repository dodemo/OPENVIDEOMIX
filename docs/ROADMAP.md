# Folla de ruta de OPENVIDEOMIX

Estado a 14 de novembro de 2026. Prioridade: realización nativa con ergonomía próxima a vMix e compatibilidade Windows, Linux e macOS (Intel/Apple Silicon). **Prototipo técnico; non apto para produción.**

## Completado en `main` (alcance limitado)

- [x] Repositorio MIT e workspace Rust con `core` e escritorio `eframe/egui`.
- [x] Preview/Program e CUT de estado (non saída de vídeo broadcast).
- [x] Imaxes estáticas e overlay visual de demostración.
- [x] Compositor RGBA de referencia en CPU: CUT, FADE, WIPE e catro overlays con probas de píxeles; **non conectado** ao escritorio ou saída de produción.
- [x] Bus de ordes tipadas, API HTTP local autenticada opcional, MIDI Learn e complemento OpenAction para OpenDeck; hardware real sen validar.
- [x] CI base con probas nas plataformas dispoñibles; validar cada novo PR por separado.

## Fase inmediata: sinal de vídeo verificable

- [ ] Integrar [PR #22](https://github.com/dodemo/OPENVIDEOMIX/pull/22): GStreamer, cámara predeterminada e fonte sintética, **só tras CI completa**.
- [ ] Integrar [PR #23](https://github.com/dodemo/OPENVIDEOMIX/pull/23), dependente da #22: varias sesións independentes e proba sintética concorrente.
- [ ] Enumerar cámaras por backend e seleccionar identificadores persistentes (Windows Media Foundation, Linux V4L2, macOS AVFoundation), sen asumir `/dev/videoN` fóra de Linux.
- [ ] Negociar modos reais 1080p50, timestamps, métricas de captura, desconexión e reconexión; validar OBSBOT Tiny 2 sobre hardware cando estea dispoñible.
- [ ] Captura de pantalla con permiso (Wayland/PipeWire portal, APIs Windows, ScreenCaptureKit) e ficheiros MP4/MKV con vídeo real.

## Motor de realización audiovisual

- [ ] Reloj mestre PTS, colas acoutadas, sincronización, deriva, FPS real e latencia medida.
- [ ] Compositor GPU independente de egui e conectado á saída: CUT, FADE, WIPE, T-Bar, overlays 1–4, despois Stinger e MERGE.
- [ ] Audio de 48 kHz, gain/mute, AFV, buses Master/A/B, PFL, vúmetros dBFS reais e sincronización A/V.
- [ ] Saída audiovisual común: gravación reproducible e RTMP real; despois SRT e cámara virtual.
- [ ] Gardar/restaurar proxectos, formatos de erro recuperables, empaquetado e permisos para os tres sistemas; asinar e notarizar macOS.

## Controis e automatización

- [ ] Completar feedback de tally e subscricións para OpenDeck/Stream Deck, macros e reconexión.
- [ ] Probas físicas de control MIDI e OpenDeck, sen atribuír soporte a modelos non verificados.
- [ ] Funcións de IA opt-in só despois da estabilidade do fluxo principal.

## Criterios de aceptación

`cargo fmt --check`, `cargo clippy`, `cargo test` e compilación das features relevantes en Windows, Linux e macOS. Os proxectos con fontes reais deben medir o fluxo audiovisual, non inferir 50 fps polo repintado de interface. Non chamar «ON AIR», «gravando» ou «audio real» a simulacións. Non fusionar PR dependentes fóra de orde.
