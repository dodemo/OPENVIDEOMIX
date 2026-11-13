# OPENVIDEOMIX

Open-source, high-performance live video production and mixing platform written in Rust.

## Visión xeral / Overview

**OPENVIDEOMIX** é unha suite aberta, moderna e modular deseñada para produción audiovisual en directo, mestura de vídeo e son multicámara, grafismo en tempo real e emisión en streaming.

Inspirado nos estándares da emisión profesional (conmutación Preview/Program, buses de son independentes, xestión de entradas NDI/SRT/WebRTC/HDMI/SDI e gráficos superpostos), OPENVIDEOMIX constrúese dende cero cunha arquitectura limpa e aberta baixo licenza **MIT**.

## Arquitectura Modular

```text
crates/
  openvideomix-core         # Tipos base, reloxo de sincronización, contratos de entrada/saída
  openvideomix-compositor   # Motor de composición por GPU (wgpu/Vulkan/DirectX)
  openvideomix-audio        # Mesturador de son de baixa latencia e buses virtuais
  openvideomix-inputs       # Captura de vídeo (V4L2, DeckLink, NDI, SRT, cámaras web)
  openvideomix-outputs      # Gravación e emisión (FFmpeg, RTMP, SRT, NDI, Cámara virtual)
  openvideomix-ui           # Interface de usuario (UI de escritorio e panel web)
  openvideomix-agent        # Conectores para automatización e control mediante LLMs
```

## Licenza

Distribuído baixo a licenza MIT. Consulta o ficheiro [LICENSE](LICENSE) para máis detalles.
