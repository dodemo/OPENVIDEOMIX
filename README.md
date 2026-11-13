# OPENVIDEOMIX

> **Suite aberta, moderna e de alto rendemento para realización e produción audiovisual en directo, escrita en Rust.**

[![Licenza MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Windows-lightgrey.svg)]()
[![Standard](https://img.shields.io/badge/standard-50.00%20FPS%20PAL-success.svg)]()

---

## 🎬 Visión Xeral

**OPENVIDEOMIX** é un mesturador e conmutador de vídeo en directo inspirado nas capacidades profesionais de programas como **vMix**, **OBS Studio** e **TriCaster**, deseñado dende cero con tecnoloxías abertas baixo **Licenza MIT**.

Construído con **Rust nativo e a biblioteca gráfica `egui`**, OPENVIDEOMIX ofrece un rendemento de 60/50 FPS estables acelerados directamente pola GPU, sen consumir recursos en capas web pesadas (0% Electron, 0% Chromium).

---

## ✨ Características Principais (v0.1.0-alpha)

- **Dobre Monitor Broadcast**:
  - **PREVIEW (PVW)**: Previsualización en recadro verde coa fonte seguinte en entrar.
  - **PROGRAM (PGM)**: Emisión activa en recadro vermello (*ON AIR*).
- **Transicións Profesionais**:
  - Conmutación instantánea `CUT`.
  - Transición temporalizada `FADE 500ms`.
  - Transicións xeométricas `MERGE` e `WIPE`.
  - Control deslizante **T-Bar Fader** con interpolación e corte automático ao acadar o extremo.
- **Xestión de Entradas con Explorador Nativo**:
  - Botón **`🖼️ ENGADIR Rótulo PNG`**: Abre o explorador de ficheiros nativo do sistema operativo (`rfd`) para cargar calquera imaxe PNG, WebP ou JPG.
  - Detección de dimensións reais e decodificación en memoria con canle alfa RGBA.
- **Canles de Superposición con Transparencia Alfa (`[OV1]`)**:
  - Os rótulos e imaxes con transparencia aplícanse como capa superposta (*Lower Third*) sobre a emisión de Program sen cortar o vídeo de fondo.
- **Cadencia 50.00 FPS PAL Real**:
  - Temporizador de refresco exacto a 20.0ms por cadro (estándar europeo de televisión PAL / TVG / EBU).
  - Xerador de barras de cor SMPTE cun reloxo de *Timecode* continuo e indicador de movemento a 50Hz.
- **Xestión de Entradas**: Botón `[✕]` para eliminar e pechar entradas de xeito limpo.
- **Panel de Axustes (`F1` / Botón)**:
  - Configuración de resolución (Full HD 1080p, HD 720p, 4K UHD) e framerates (50p, 59.94p, 60p, 25p, 30p).
  - Escáner de dispositivos de vídeo USB (`/dev/video*`).
- **Atallos de Teclado**:
  - `Espazo`: CUT instantáneo.
  - `Enter`: FADE.
  - `1` a `6`: Selección rápida de fontes a Preview.
  - `F1`: Abrir/pechar panel de Axustes.

---

## 🏗️ Estrutura do Workspace

```text
OPENVIDEOMIX/
├── apps/
│   └── openvideomix-desktop/       # Aplicación de escritorio principal (eframe + egui)
├── crates/
│   └── core/                        # Motor lóxico: inputs, mixer state, buses de son
├── docs/
│   ├── ARCHITECTURE.md              # Documento de arquitectura de sistemas
│   └── ROADMAP.md                   # Fases de desenvolvemento e tarefas pendentes
├── .github/
│   └── ISSUE_TEMPLATE/              # Modelos para reporte de erros e novas funcións
├── AGENTS.md                        # Guía de contexto para ChatGPT Enterprise e LLMs
└── LICENSE                          # Licenza MIT
```

---

## 🚀 Como Executar

```bash
# Clonar o repositorio
git clone https://github.com/dodemo/OPENVIDEOMIX.git
cd OPENVIDEOMIX

# Compilar e arrincar a consola de realización
cargo run --bin openvideomix
```

---

## 🤖 Guía para Desenvolvemento con ChatGPT e Axentes de IA

Consulta o documento [AGENTS.md](AGENTS.md) para ver como ChatGPT Enterprise pode interactuar con este repositorio, estudar o código e xerar **GitHub Issues** perfectamente formatadas para o axente local **Antigravity**.

---

## 📄 Licenza

Distribuído baixo a licenza **MIT**. Consulta o ficheiro [LICENSE](LICENSE) para máis detalles.
