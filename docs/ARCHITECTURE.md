# Arquitectura de OPENVIDEOMIX

OPENVIDEOMIX é unha plataforma de produción e realización audiovisual en directo de código aberto (Licenza MIT), deseñada para ofrecer unha experiencia fluída e profesional similar a vMix, con soporte nativo para Linux e Windows.

---

## 1. Decisións Tecnolóxicas Clave

- **Linguaxe Principal**: Rust (seguridade de memoria, concorrencia sen bloqueos e latencia mínima).
- **Interface Gráfica (UI)**: `egui` (renderizado nativo por GPU con `eframe`/`wgpu`, idéntico ao motor de interface de *Craft Anything*).
- **Motor de Vídeo e Procesamento**: Aproveitamento do ecosistema de `libobs` (motor de OBS Studio) e canalizacións de alto rendemento para captura, filtros, codificación por hardware (NVENC, VAAPI, AMF, QuickSync) e emisión (RTMP, SRT, NDI, cámara virtual).
- **Compatibilidade**: Multiplataforma estrita (Linux e Windows) dende a fase inicial.
- **Intelixencia Artificial**:
  - *No desenvolvemento*: Axentes colaborativos (Antigravity + OpenAI/Codex + LLMs locais en PCgrande).
  - *No programa en execución*: Módulo de copiloto de realización (subtitulación en directo con Whisper, conmutación intelixente de cámaras por detección de voz, control por linguaxe natural).

---

## 2. Mapa de Módulos (Workspace de Rust)

```text
OPENVIDEOMIX/
├── apps/
│   ├── openvideomix-desktop/    # Aplicación principal de escritorio con interface egui
│   └── openvideomix-cli/        # Ferramenta de liña de comandos e control headless
├── crates/
│   ├── core/                    # Tipos centrais, bus de eventos e sincronización (timecode)
│   ├── video-engine/            # Abstracción de mestura, escalado, capas e transicións
│   ├── audio-engine/            # Buses de son (Master, Aux, Headphones) e efectos
│   ├── inputs/                  # Xestión de entradas (cámaras, NDI, ficheiros, xanelas, captura)
│   ├── outputs/                 # Emisión (RTMP/SRT), gravación local e cámara virtual
│   ├── ai-assistant/            # Conector con LLMs (PCgrande / OpenAI) e modelos de son/visión
│   └── ui-components/           # Compoñentes visuais en egui (Preview/Program, fader T-bar, vúmetros)
├── docs/                        # Documentación técnica e guías de desenvolvemento
└── LICENSE                      # Licenza MIT
```

---

## 3. Estratexia de Colaboración con Modelos de IA (LLMs)

1. **Antigravity (Orquestrador no espazo de traballo)**:
   - Mantemento do repositorio, estrutura dos crates, compilación e resolución de dependencias locais.
2. **PCgrande (LLMs locais)**:
   - Procesamento sen límite de tokens nin tarifas para tarefas continuas, xeración de probas unitarias, transcrición e inferencia local de Whisper ou detección de escenas.
3. **ChatGPT Enterprise / Codex**:
   - Deseño de algoritmos críticos, shaders de transición (Wipe, Stinger, DVE 3D), revisión de seguridade e refactorización de alto nivel.
