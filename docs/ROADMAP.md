# Folla de ruta de OPENVIDEOMIX

Estado a 14 de novembro de 2026. **Prioridade inmediata: primeira Alpha executábel e verificable no PC grande con Windows 10 Pro**; manter Windows, Linux e macOS Intel/Apple Silicon como obxectivos arquitectónicos. Ergonomía inspirada en vMix. **Prototipo técnico; non apto para produción.**

## Prioridade P0 inmediata: primeira Alpha no PC grande con Windows

**Fito e criterios de aceptación:** [issue #46](https://github.com/dodemo/OPENVIDEOMIX/issues/46). Banco físico principal: Windows 10 Pro x64, RTX 4080 SUPER 16 GB, 96 GB RAM e 2 TB NVMe. Windows é **a primeira máquina de validación**, non unha substitución dos obxectivos Linux/macOS.

Orde obligatoria antes de ampliar o produto:

1. Revisar e integrar [PR #45](https://github.com/dodemo/OPENVIDEOMIX/pull/45) só se conserva CI verde no head exacto e supera revisión. Audio en directo con micrófono físico aínda non comprobado.
2. **Entregar unha compilación Windows x64 descargable (ZIP ou instalador), que abra por dobre clic sen Rust/Cargo no PC do usuario.** Resolver distribución e dependencias GStreamer/codec; verificar o que poida a CI Windows e comunicar os bloqueos.
3. Executar no PC o estudo sintético existente: catro fontes, Preview → CUT → Program → gravación WebM con audio a 48 kHz. Medir a cadencia de gravación real, fotogramas perdidos, colas, CPU/GPU e sincronía; non confundir caps nominais con FPS efectivos.
4. Conectar selección **real** de cámara/capturadora e micrófono WASAPI ao mesmo Program/gravación. Negociar caps e modo 1080p50 cando o dispositivo os soporte; sinalar modos non admitidos e fallos sen simulacións.
5. Consolidar un procedemento curto con probas de 60 segundos e, despois, 30 minutos, diagnóstico exportable sen datos persoais e comparación de rendemento reproducible.

As probas físicas requiren executar o programa no PC grande: **Codex Cloud non pode dalas por superadas sen evidencias do equipo**. Non prometer aceleración RTX/NVENC ou 50 FPS sostidos só por detectar a GPU. Respectar o perfil predeterminado 1920×1080 progresivo 50/1 + 48 kHz.

**Ata acadar esta Alpha, pospoñer** ampliación de MIX/WIPE/Stinger, RTMP, novos buses/AFV/PFL, melloras xenéricas da GUI e reescrituras GPU especulativas. Corrixir rendemento medido cando sexa bloqueo real. Conservar CI multiplataforma. Sen API de IA de pago nin servizos adicionais.

## Completado en `main` (alcance limitado)

- [x] Repositorio MIT e workspace Rust con `core` e escritorio `eframe/egui`.
- [x] Preview/Program e CUT; estudo de catro sinais sintéticos que alimenta o Program real e a gravación WebM, sen rendemento 1080p50 sostido acreditado.
- [x] Imaxes estáticas e overlay visual de demostración.
- [x] Compositor RGBA de referencia en CPU: CUT, FADE, WIPE e catro overlays con probas de píxeles; CUT ligado ao estudo sintético de escritorio. A GPU, MIX/WIPE e overlays de produción seguen pendentes.
- [x] Bus de ordes tipadas, API HTTP local autenticada opcional, MIDI Learn e complemento OpenAction para OpenDeck; hardware real sen validar.
- [x] CI base con probas nas plataformas dispoñibles; validar cada novo PR por separado.

## Contrato de vídeo predeterminado

- **1920 × 1080 progresivo, 50/1 fps exactos, audio 48 kHz** en proxectos novos.
- Preview, Program e futuras saídas deben usar 1080p50 por defecto; modos distintos só mediante configuración explícita.
- As cámaras deben anunciar as súas capacidades reais; sen coincidencia exacta, informar de conversión necesaria e non presentar outro modo como 1080p50 nativo.
- Medir FPS de captura e de saída independentemente da GUI. As probas a 640 × 360 / 30 fps non cumpren esta condición.

## Fontes de vídeo reais: traballo pendente tras abrir a Alpha no PC grande

- [x] [PR #22](https://github.com/dodemo/OPENVIDEOMIX/pull/22): ruta GStreamer de cámara predeterminada e fonte sintética, xa integrada.
- [x] [PR #23](https://github.com/dodemo/OPENVIDEOMIX/pull/23): varias sesións independentes e proba sintética concorrente, xa integrada.
- [ ] Enumerar cámaras por backend e seleccionar identificadores persistentes (Windows Media Foundation, Linux V4L2, macOS AVFoundation), sen asumir `/dev/videoN` fóra de Linux.
- [ ] Negociar modos reais 1080p50, timestamps, métricas de captura, desconexión e reconexión; validar OBSBOT Tiny 2 sobre hardware cando estea dispoñible.
- [ ] Captura de pantalla con permiso (Wayland/PipeWire portal, APIs Windows, ScreenCaptureKit) e ficheiros MP4/MKV con vídeo real.

## Motor de realización audiovisual

- [ ] Reloxo mestre PTS, colas acoutadas, sincronización, deriva, FPS real e latencia medida.
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
