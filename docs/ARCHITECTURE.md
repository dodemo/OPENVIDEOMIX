# Arquitectura de OPENVIDEOMIX

OPENVIDEOMIX é un mesturador de vídeo en desenvolvemento, inspirado na operativa de vMix e, como referencia secundaria, OBS Studio. O código propio distribúese baixo licenza MIT.

> Estado: prototipo técnico. Non hai aínda un pipeline completo e verificable de captura → composición audiovisual → codificación → gravación ou emisión. Non se pode empregar en produción.

## Plataformas e implementación

- **Windows, Linux e macOS** son plataformas obrigatorias e de primeira clase. macOS inclúe Intel e Apple Silicon.
- **Rust** é a linguaxe actual do núcleo e da GUI, pero pódense empregar pontes nativas C/C++/Swift/Objective-C cando se xustifiquen por medicións e licenzas.
- **eframe/egui** fornece a GUI actual; a súa adecuación para varios monitores de vídeo e paneis desacoplables require medicións. O estado de produción e o compositor non deben depender de tipos de egui.
- **GStreamer** é o candidato actual de captura/decodificación. A integración de captura está nas PR #22 e #23, non en `main` ata a súa aprobación. A biblioteca e os seus plugins deben pasar unha auditoría de licenzas e distribución por plataforma.
- **libobs** é unha alternativa para avaliar, non unha dependencia seleccionada. Non incorporar código GPL a un proxecto MIT sen decisión explícita de arquitectura e licenzas.
- **Compositor**: existe un compositor RGBA de referencia en CPU, con probas de píxeles, pero non está conectado ao fluxo de vídeo do escritorio nin á saída codificada. Requírese unha implementación de produción acelerada por GPU e desacoplada da GUI.

## Módulos reais do repositorio

- `crates/core/`: estado, ordes tipadas, MIDI, capacidades de cámara e compositor de referencia.
- `apps/openvideomix-desktop/`: interface e integracións opcionais de captura, MIDI e API local.
- `apps/openvideomix-opendeck/`: complemento OpenAction para controladores compatibles.
- `docs/`: contratos, investigación, decisións e criterios de aceptación.

Non asumir a existencia de crates futuros como `video-engine`, `audio-engine` ou `outputs` ata que se incorporen realmente ao repositorio.

## Modelo de tempo e sinal

A cadencia de referencia é **50/1 fps** (20 ms por fotograma), cunha familia adicional 25/1, 60/1 e 60000/1001. O repintado da GUI non é un reloxo broadcast. O motor debe transportar timestamps PTS, definir un reloxo mestre, colas acoutadas, política de descarte/backpressure, sincronización A/V e métricas observables. Audio a 48 kHz cando corresponda, con control de deriva.

As fontes teñen identidade estable, ciclo de vida independente e erros recuperables. Debe haber enumeración específica para Windows (Media Foundation), Linux (V4L2, PipeWire/portais) e macOS (AVFoundation, ScreenCaptureKit). `autovideosrc` non equivale a selección persistente de cámaras nin garante varias webcams físicas.

O compositor de produción debe implementar CUT, FADE, WIPE, overlays 1–4 e T-Bar na saída real, non só na interface; as transicións máis complexas, incluídos Stinger e MERGE, deben definirse e probarse separadamente.

## Control externo e seguranza

GUI, teclado, API local autenticada, MIDI e OpenDeck deben converxer no mesmo bus de ordes tipadas. O estado de tally, erros e saídas debe reflectir operacións reais. O complemento OpenAction é independente do host OpenDeck; non reutilizar código GPL sen revisión xurídica. Acceso LAN só por configuración expresa e autenticación.

## Fases e probas de aceptación

1. Validar e integrar en orde a captura sintética/USB da PR #22 e as sesións concorrentes da PR #23, con CI multiplataforma.
2. Enumerar e seleccionar cámaras por identificador físico persistente e negociar modos reais, incluíndo 1080p50 cando o hardware o permita.
3. Integrar timestamps, compositor GPU independente da GUI e monitorización real.
4. Mesturar audio, medir dBFS, sincronizar e conectar unha saída audiovisual común a gravación e RTMP.
5. Completar proxectos gardados, empaquetado, permisos, sinatura/notarización en macOS e probas físicas nos tres sistemas.

Non afirmar que 50 Hz na interface implica saída a 50 fps; non describir medidores simulados como reais; non presentar gravación, emisión ou PTZ como operativos sen proba de extremo a extremo. Para o estado da implementación, consultar o README, as PR abertas e as comprobacións de CI.

## IA e ferramentas de desenvolvemento

Os axentes (Codex, Antigravity e modelos locais) poden axudar na análise, implementación e probas, pero non forman parte do camiño crítico do sinal audiovisual. As funcións de IA en tempo de execución son opcionais e posteriores á estabilización do motor de realización.
