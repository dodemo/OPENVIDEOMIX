# AGENTS.md: Guía para ChatGPT Enterprise e Axentes de IA en OPENVIDEOMIX

Benvido/a. Este documento define o contexto, a arquitectura, as normas de enxeñaría e o fluxo de traballo para que modelos de linguaxe (especialmente **ChatGPT Enterprise**) poidan estudar o repositorio **OPENVIDEOMIX** e redactar **GitHub Issues** perfectamente accionables para que o axente local (**Antigravity**) os execute e resolva.

---

## 1. Visión e Obxectivo do Proxecto

**OPENVIDEOMIX** é unha alternativa aberta (Licenza MIT), modular e de alto rendemento a programas de realización en directo como vMix, OBS Studio e TriCaster.
- **Linguaxe**: Rust puro (con foco en seguridade de memoria, latencia mínima e concorrencia).
- **Interface Gráfica (GUI)**: `egui` + `eframe` (renderizado directo acelerado por GPU, 0% Chromium/Electron, fluidez máxima).
- **Plataforma de destino**: Linux e Windows (multiplataforma nativa).
- **Estándar de Cadencia**: **50.00 FPS PAL Broadcast** (20.0ms por cadro), con opcións para 59.94p e 60p.
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

## 3. Estado Actual da Implementación (v0.1.0-alpha)

- ✅ **Dobre Monitor Broadcast**: Monitor **PREVIEW** (recadro verde) e **PROGRAM** (recadro vermello ON AIR).
- ✅ **Transicións**: `CUT` instantáneo, `FADE` (500ms), `MERGE`, `WIPE` e fader **T-Bar** interactivo con interpolación en tempo real.
- ✅ **Explorador de Arquivos Nativo (`rfd`)**: O botón *"ENGADIR Rótulo PNG (Explorador)"* abre o explorador nativo do sistema operativo para seleccionar ficheiros PNG/WebP/JPG con transparencia.
- ✅ **Superposición con Transparencia Alfa (Canle `[OV1]`)**: Os gráficos e rótulos cargados superpóñense sobre o monitor de PROGRAM con canal alfa RGBA real.
- ✅ **Xestión de Entradas**: Botón `[✕]` en cada tarxeta para pechar ou eliminar entradas sen orfandade de estado.
- ✅ **Cadencia 50.00 FPS PAL Real**: Xerador de barras SMPTE con Timecode e indicador de movemento a 50Hz exactos (20ms/cadro).
- ✅ **Panel de Axustes (`F1` / Botón)**: Modais para Vídeo (Resolución e FPS), Son e Escáner de dispositivos `/dev/video*`.

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
- [ ] Non se rompe a compatibilidade cos 50 FPS nin coas entradas existentes.
```

---

## 5. Próximos Grandes Fitos Pendentes (Ideas para novas Issues)

1. **Captura Real de Webcams USB (V4L2 no Linux / MediaFoundation en Windows)**: Pasar da detección de `/dev/video*` ao fluxo continuo de fotogramas na GPU.
2. **Captura de Pantalla Real (Wayland Desktop Portal / PipeWire)**: Captura da fiestra ou monitor completo en directo.
3. **Reprodución de Vídeo MP4/MKV**: Reprodución de clips de vídeo con son integrado e bucle.
4. **Motor de Son Real**: Lectura do micrófono analóxico/USB con vúmetros de decibelios reais (dBFS).
5. **Gravación e Streaming (Outputs)**: Saída RTMP cara a servidores locais/YouTube e saída de Cámara Virtual (`v4l2loopback`).

---

## 6. Criterio para decisións de produto sen especificación explícita

Cando haxa unha dúbida funcional ou de interacción, **priorizar o comportamento e modelo operativo de vMix**; usar OBS Studio como referencia secundaria cando vMix non resolva o caso. Aplicar este criterio especialmente a entradas, Preview/Program, transicións, overlays, buses, AFV, configuración de fontes, atallos e saídas. A referencia é a experiencia e a función, **non unha copia de marcas nin recursos gráficos protexidos**. Non inferir que bibliotecas ou protocolos propietarios de vMix deben usarse: seleccionar cada implementación segundo soporte Linux/Windows/macOS, licenzas, accesibilidade, calidade e medicións reais. Documentar diferenzas deliberadas e poñer ao usuario só decisións de produto de calado importante.
