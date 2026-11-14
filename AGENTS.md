# AGENTS.md: Guía para ChatGPT Enterprise e Axentes de IA en OPENVIDEOMIX

Benvido/a. Este documento define o contexto, a arquitectura, as normas de enxeñaría e o fluxo de traballo para que modelos de linguaxe (especialmente **ChatGPT Enterprise**) poidan estudar o repositorio **OPENVIDEOMIX** e redactar **GitHub Issues** perfectamente accionables para que o axente local (**Antigravity**) os execute e resolva.

---

## 1. Visión e Obxectivo do Proxecto

**OPENVIDEOMIX** é unha alternativa aberta (Licenza MIT), modular e de alto rendemento a programas de realización en directo como vMix, OBS Studio e TriCaster.
- **Linguaxe**: Rust como implementación actual; pontes nativas xustificadas por probas e licenzas son admisibles.
- **Interface Gráfica (GUI)**: `egui` + `eframe` na versión actual; medir rendemento antes de considerala definitiva e manter o compositor independente.
- **Plataforma de destino**: Windows, Linux e macOS (Intel e Apple Silicon), obrigatorias desde o inicio.
- **Estándar obxectivo**: vídeo 50/1 fps (20 ms por fotograma), ademais de 25/1, 60000/1001 e 60/1. A cadencia da GUI non acredita cadencia de saída.
- **Predeterminado obrigatorio**: proxectos novos, Preview, Program, compositor e saídas deben solicitar **1920×1080 progresivo a 50/1 FPS**, con audio a **48 kHz**. Os formatos alternativos son opcións explícitas, non degradacións silenciosas. Verificar modos físicos e medir a cadencia real antes de declarar 1080p50 operativo. As probas sintéticas a 640×360/30 son só desenvolvemento.
- **Estudo Completo de vMix e OBS**: Consulta obrigatoriamente [docs/VMIX_OBS_ANALYSIS.md](docs/VMIX_OBS_ANALYSIS.md) para comprender o paradigma de *Inputs*, as canles de *Overlay 1-4*, o bus de son e os 5 alicerces da realización televisiva.

---

## 2. Mapa do Código Actual (Workspace)

```text
OPENVIDEOMIX/
├── apps/
│   └── openvideomix-desktop/       # Aplicación de escritorio principal
│       ├── Cargo.toml               # Dependencias: eframe, egui, image, rfd, chrono, tokio
│       └── src/
│           ├── main.rs              # Punto de entrada de eframe nativo
│           └── app.rs               # Interface de estudo broadcast completa
├── crates/
│   └── core/                        # Núcleo do motor (sen dependencias gráficas)
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs               # Modelos: VideoInput, MixerState, AudioBus, TransitionType
├── docs/
│   ├── ARCHITECTURE.md              # Decisións arquitectónicas xerais
│   └── ROADMAP.md                   # Fases de desenvolvemento e fitos
├── AGENTS.md                        # Esta guía para axentes de IA
└── LICENSE                          # Licenza MIT
```

---

## 3. Estado da implementación (verificar sempre o README e a CI)

- **Operativo no núcleo:** estados e ordes tipadas de realización, CUT lóxico, compositor RGBA de referencia en CPU e probas de píxeles.
- **GUI:** Preview/Program, imaxes PNG/WebP/JPG e un overlay de demostración. A GUI non constitúe unha saída audiovisual codificada.
- **MIDI:** MIDI Learn, perfís e adaptación opcional; falta proba con hardware físico.
- **OpenDeck:** complemento OpenAction independente e API local autenticada; falta validación sobre dispositivos reais e feedback continuo.
- **Captura real:** #22/#23 están integradas en `main`; sesións GStreamer independentes. Esta rama engade un estudo sintético de catro fontes → CUT → Program CPU real → WebM con ton de proba. Ver `docs/DESKTOP_PROGRAM_TEST.md` para evidencias e perdas; non inferir cámaras físicas ou 1080p50 sostido.
- **Non operativo en produción:** compositor GPU, audio capturado/mesturado, gravación 1080p50 sostida, RTMP, sincronización física A/V e PTZ. A gravación de proba declara os ocos reais; os medidores quedan a cero sen captura de audio. A GUI aínda conserva indicadores de demostración.
- **Licenzas:** GStreamer require auditoría de plugins/códecs; libobs non é unha dependencia aprobada do proxecto MIT.

O estado válido é o comprobado no código e CI, non o indicado por capturas de pantalla ou documentación antiga. Consultar [README.md](README.md) e [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## 4. Como debe ChatGPT redactar as GitHub Issues

Cando o usuario che pida crear ou especificar unha nova función ou corrección de erro, crea unha **GitHub Issue** seguindo este formato estruturado:

### Formato Recomendado de Issue:

```markdown
### 🎯 Obxectivo
Descrición breve e clara da función ou corrección que se quere acadar.

### 🛠️ Crate e Módulos Afectados
- `crates/core` ou `apps/openvideomix-desktop`

### 📋 Especificación Técnica
- Lóxica que debe cumprir o código en Rust.
- Tipos de datos, estruturas ou eventos necesarios.
- Bibliotecas recomendadas do ecosistema Rust (sempre de licenza aberta MIT/Apache).

### ✅ Criterios de Aceptación (DoD)
- [ ] O código compila sen avisos (`cargo check`).
- [ ] A función pódese probar visualmente na interface de `egui`.
- [ ] Non se rompe o reloxo de vídeo nin as entradas existentes; demostrar cadencia de saída medida cando a función o requira.
- [ ] CI Windows, Linux e macOS (arm64 e Intel cando dispoñible), e probas sen hardware para fontes e estados.
```

---

## 5. Próximos Grandes Fitos Pendentes (Ideas para novas Issues)

1. **Captura Real de Webcams USB (V4L2 no Linux / MediaFoundation en Windows)**: Pasar da detección de `/dev/video*` ao fluxo continuo de fotogramas na GPU.
2. **Captura de Pantalla Real (Wayland Desktop Portal / PipeWire)**: Captura da fiestra ou monitor completo en directo.
3. **Reprodución de Vídeo MP4/MKV**: Reprodución de clips de vídeo con son integrado e bucle.
4. **Motor de Son Real**: Lectura do micrófono analóxico/USB con vúmetros de decibelios reais (dBFS).
5. **Gravación e Streaming (Outputs)**: Saída RTMP cara a servidores locais/YouTube e saída de Cámara Virtual (`v4l2loopback`).

---

## 6. Norma obrigatoria: superficies de control externo

OPENVIDEOMIX debe soportar **Stream Deck/OpenDeck e controladores USB MIDI**, en Windows, Linux e macOS. Ver issues [#13](https://github.com/dodemo/OPENVIDEOMIX/issues/13) e [#14](https://github.com/dodemo/OPENVIDEOMIX/issues/14).

- **Referencia de produto:** ante dúbidas, imitar primeiro as funcións e a ergonomía de vMix; OBS é a referencia secundaria. Non copiar recursos gráficos ou marca alleos.
- **Stream Deck:** dar prioridade a un complemento OpenAction/Stream Deck SDK compatible con [OpenDeck](https://github.com/nekename/OpenDeck), como aplicación anfitrioa autónoma. Non copiar nin vincular código GPL de OpenDeck á aplicación MIT sen revisión de licenzas.
- **MIDI:** incluír controladores estándar mediante un backend multiplataforma (candidato `midir`); MIDI Learn, botóns Note/CC, potenciómetros, faders, mapeos persistentes, soft takeover e reconexión. O Mingzhe B0FV93GS5Z é un dispositivo de destino; o seu protocolo e identificadores non están verificados con hardware.
- **Arquitectura:** GUI, teclado, OpenDeck, MIDI e API de rede executan exactamente as mesmas ordes tipadas sobre o núcleo; estado de tally, audio e gravación/streaming emítese por eventos. A API remota require autenticación, control de orixe e opción explícita de exposición LAN. Non bloquear os fíos en tempo real.
- **Calidade:** probas sen hardware con controladores simulados, probas físicas posteriores, indicadores accesibles sen depender só da cor, control de permisos, detección de desconexión, perfís e documentación de plataformas.

## Active autonomous assignment (2026-11-14, owner-confirmed)

**P0: Deliver the first working OPENVIDEOMIX Alpha on the owner's Windows 11 PC grande** — [issue #46](https://github.com/dodemo/OPENVIDEOMIX/issues/46). Keep Windows, Linux and macOS as target platforms. The first runtime environment is a physical Windows 11 machine with RTX 4080 SUPER, 96 GB RAM, Ollama and working OpenCode. The owner has verified that base `cargo test --workspace` passes with MSVC Build Tools 2026 and that the GUI opens; the first desktop run was without GStreamer features. Do not claim actual live capture or 50 FPS from those two results.

For local OpenCode execution, use the concrete task [docs/LOCAL_WINDOWS_ALPHA_AGENT.md](docs/LOCAL_WINDOWS_ALPHA_AGENT.md) and the one-click `START-OPENVIDEOMIX-AGENT.cmd` launcher. **Implement and test**, not just inspect or write plans. Keep work on a feature branch, use the shared MSVC environment, record progress and errors in gitignored `local-results/`, and do not run paid AI services or publish private diagnostics without explicit permission. Do not run as Administrator or install system software without permission.

Deliver a versioned Windows executable/ZIP with its own media runtime and codecs, no first-run downloads, and independently decoded AV evidence. Windows x64 is the first hardware test bench, **not the only platform**. Real camera/mic validation, 50/1 FPS sustained and an offline clean-VM installation require explicit evidence and must remain PENDING until done. Preserve 1920x1080 progressive at 50/1 and 48 kHz, and maintain cross-platform CI.

**P1 mandatory after first usable Alpha:** [issue #47](https://github.com/dodemo/OPENVIDEOMIX/issues/47), multimodal mouth+voice active-speaker detection and per-program production profiles; manual operation suspends AUTO until explicit reactivation. [#2](https://github.com/dodemo/OPENVIDEOMIX/issues/2) is a future extension for virtual 4K/180/360 camera crops.

The old [#43](https://github.com/dodemo/OPENVIDEOMIX/issues/43) PR-integration assignment is **completed/closed**, not the next task. [#45](https://github.com/dodemo/OPENVIDEOMIX/pull/45) adds live audio but is still Draft as of this update; review current head and CI before any merge. The old `codex/continuation-lock` serialization applied to the historical recurring cloud Codex workflow; it must not be created as a precondition for the local Windows agent. Never overlap agent writers on the same local working tree, force-push, or merge without verified applicable CI and review.
