# ADR-0001 — Motor multimedia e arquitectura multiplataforma

- **Data:** 2026-11-14
- **Estado:** Proposed — require proof of concept and license review before adoption
- **Issue:** #3
- **Produtos de destino:** Windows (x86_64), Linux (x86_64), macOS (arm64 Apple Silicon + x86_64 Intel)
- **Perfil de referencia:** 1920×1080 progresivo 50/1 fps; audio PCM 48 kHz

## 1. Contexto e requisitos

OPENVIDEOMIX é unha consola de realización de vídeo en directo con entradas independentes, Preview/Program, transicións interpoladas, catro overlays, buses de audio con AFV/PFL e saídas de gravación, RTMP e, posteriormente, SRT/cámara virtual. A interface debe lembrar o modelo de operación de vMix sen copiar a súa identidade. Debe poder renderizar e codificar unha saída real; unha actualización de egui cada 20 ms non constitúe unha cadencia de saída de vídeo verificada.

O repositorio actual contén unha interface en egui con cámaras, clips e medidores principalmente simulados. Non existe aínda un pipeline de captura, mestura e codificación contrastado nos tres sistemas.

**Restricións:** software aberto, código propio MIT; sen dependencia de navegador/Electron; compatibilidade cos tres sistemas desde a primeira versión operativa; evita bloqueos por integración dependente dun fabricante; control explícito dos permisos do sistema e dos erros; accesibilidade con teclado e tecnoloxías asistivas.

## 2. Decisión proposta

**Conservar Rust para o modelo de estado e as ordes de realización, pero permitir adaptadores nativos en C/C++/Objective-C/Swift cando sexan necesarios.** Non se adopta a regra de “Rust puro” nin se xustifica unha reescritura sen comparar prototipos.

**GStreamer (bindings gstreamer-rs) é o primeiro candidato para captura, decodificación, negociación de formatos, sincronización e codificación**, non unha decisión irreversible. **wgpu** é o primeiro candidato para un compositor GPU independente de egui; a interop de memoria GPU de cero copias será unha hipótese que cómpre verificar para cada backend. **egui/eframe** continúa como UI provisional e substituíble; comparar contra alternativas como Qt/Slint mediante unha demostración de paneis desacoplables, múltiples monitores, render de vídeo, entrada de teclado e accesibilidade.

### Alternativas

| Opción | Vantaxes | Riscos / condicións | Decisión |
|---|---|---|---|
| GStreamer + Rust + wgpu | Pipeline multiplataforma, negociación de formatos, reloxos, hardware APIs e SDKs | Plugins con licenzas heteroxéneas, distribución, interop GPU e custo de integración | **Candidato A; probar primeiro** |
| FFmpeg + compositor propio | Decodificadores amplos, control directo de fotogramas | Maior responsabilidade sobre captura, reloxos, A/V sync e distribución/licenzas | Candidato B / respaldo |
| libobs como motor | Motor de produción maduro, capacidades reais de captura/emisión | GPLv2, arquitectura/plugin ABI e implicacións para proxecto MIT; non integrar sen revisión | Investigación separada |
| C++/Qt para todo | Integración nativa ampla e tooling maduro | Migración prematura e maior superficie de mantemento | Só reconsiderar ante medicións obxectivas |
| egui + eframe | Reutiliza a interface existente, menor fricción con Rust | Docks, a11y, acceso a texturas externas, varios monitores | UI provisional; benchmark |
| Qt ou Slint para UI | Ecosistemas alternativos | Custos de integración, licenza e ergonomía broadcast | Benchmark antes de decisión final |

### Contratos propostos (independentes da implementación)

- **ProductionState:** fontes, buses, overlays, tally, presets, transicións e estado de saídas; validación de invariantes.
- **CommandBus:** AddInput, RemoveInput, SelectPreview, Cut, TakeWithTransition, SetTBar, SetOverlay, SetGain, StartRecording, StartStreaming, SaveProject; resultado tipado / eventos, sen que a UI cambie o hardware directamente.
- **SourceAdapter:** discovery → permission_pending → opening → live → stalled → error → closed; formatos/capacidades, PTS, audio e vídeo; política de reconexión.
- **MediaFrame:** timestamp racional, duración, formato de cor, colorimetría, orientación, alpha, identificador de recurso GPU ou buffer CPU e propiedade explícita; prohibido asumir que compartir textura de GStreamer a wgpu será sempre zero-copy.
- **VideoCompositor:** Preview, Program, capas de overlay, máscaras de transición e unha saída de vídeo común para monitores/gravación/stream.
- **AudioGraph:** PCM 48 kHz, por-fonte gain/mute/AFV, Master/A/B/PFL e compensación de drift; medidores reais Peak/RMS dBFS.
- **OutputSink:** consumidor de fotogramas e audio con estados idle/starting/running/stopping/error e métricas fiables.
- **ProjectStore:** proxectos versionados, escritura atómica e migracións; hardware dispoñible non é requisito para abrir proxectos existentes.
- **UiAdapter:** subscrición a snapshots/eventos e envío de ordes; non é dono do clock de vídeo nin das decodificacións.

### Tempo e sincronización

- Cadencias como fraccións `50/1`, `60000/1001`, `60/1`, `25/1`, non números decimais ou etiquetas de texto.
- Un reloxo multimedia monotónico rexistrará timestamps de captura/descodificación, instantes de composición e tempo de codificación; alinear audio e vídeo, incluído tratamento de drift.
- Bounded frame queues con política documentada para atrasos; métricas de fotogramas descartados ou duplicados **na saída**, non só na GUI.
- Definir os puntos de sincronización e latencia; evitar bloqueos en fíos de audio, vídeo e UI.
- Overlays e T-Bar en composición real: FADE e WIPE non poden chamar `cut()` agás na finalización correspondente.

## 3. Adaptadores por sistema

| Capacidade | Windows | Linux | macOS |
|---|---|---|---|
| Cámara | Media Foundation | V4L2/UVC | AVFoundation |
| Pantalla/xanela | Windows Graphics Capture (avaliar) | XDG Desktop Portal + PipeWire para Wayland | ScreenCaptureKit con permiso |
| Audio I/O | WASAPI | PipeWire/ALSA | Core Audio |
| GPU | Direct3D / wgpu | Vulkan / wgpu (avaliar DMABUF) | Metal / wgpu |
| Codificador por hardware | Media Foundation, NVENC/QSV/AMF segundo dispositivo | VA-API, NVENC/QSV segundo dispositivo | VideoToolbox |
| Distribución | instalador asinado, dependencias declaradas | paquete/AppImage/Flatpak avaliados | .app/.dmg firmado e notarizado, frameworks e entitlements |

Os backends enumerados son obxectivos de integración, non capacidades actualmente verificadas. A compatibilidade Intel/Apple Silicon, o macOS mínimo e os permisos exactos quedarán fixados despois das probas reais.

## 4. Licenzas e distribución

O código propio continúa MIT. GStreamer é LGPL, pero a licenza **efectiva** depende tamén de plugins, códecs e bibliotecas externas. Construír unha listaxe de todos os binarios e plugins redistribuídos, súas licenzas efectivas, posibles patentes, mecanismo de enlazado e obrigas de distribución. Non incluír por defecto plugins dependentes da GPL sen revisión. O software MIT non implica que toda a distribución poida describirse simplemente como MIT.

A integración con libobs, distribuído baixo GPLv2, está **bloqueada** ata resolución explícita de compatibilidade de licenza e modelo de distribución. Revisar con asesoramento xurídico se fose necesario.

## 5. Fitos para resolver esta ADR

1. **PoC A — soporte triple sistema:** un xerador de patróns e vídeo real (cámara ou ficheiro), con timestamps, visualización GPU independente da GUI e saída codificada nun ficheiro reproducible, en Windows/Linux/macOS.
2. **PoC B — GPU:** medir copia CPU vs interop GPU, calidade e latencia de composición de dúas fontes e overlay alpha, contemplando Metal e Direct3D.
3. **PoC C — audio:** fonte de son real, medidor Peak/RMS e A/V sync durante captura/gravación.
4. **PoC D — UI:** comparativa visual egui fronte a polo menos unha alternativa, con paneis dockables, dúas pantallas 16:9, 20 entradas, mesturador, HiDPI e navegación con teclado; verificar VoiceOver, lector de pantalla en Windows e AT-SPI.
5. **Paquetización:** executar prototipo desde instalación limpa nos tres SO; licenzas e dependencias explícitas; permisos macOS, firma/notarización.
6. **Benchmark documentado:** equipos, SO e GPU, versións das dependencias, FPS realmente codificados, drops, latencia, uso CPU/GPU, memoria e fallo/reconexión. Establecer límites antes de pasar á implementación xeral.

**Criterio de saída:** escoller tecnoloxías e fixar interfaces estables só tras reproducir estas probas. Ningunha promesa de “50 FPS exactos” nin de zero-copy baseada unicamente en documentación.

## 6. Fontes oficiais

- GStreamer: https://gstreamer.freedesktop.org/features/
- Documentación sobre licenzas GStreamer: https://gstreamer.freedesktop.org/documentation/frequently-asked-questions/general.html
- Despregamento macOS: https://gstreamer.freedesktop.org/documentation/deploying/mac-osx.html
- GStreamer Rust: https://gstreamer.freedesktop.org/documentation/rust/
- Apple ScreenCaptureKit: https://developer.apple.com/documentation/screencapturekit
- Apple AVFoundation: https://developer.apple.com/av-foundation/
- OBS GPL: https://obsproject.com/help
- egui: https://github.com/emilk/egui
