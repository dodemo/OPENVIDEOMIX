# Changelog

Tódolos cambios notables neste proxecto quedarán documentados neste arquivo seguindo o formato [Keep a Changelog](https://keepachangelog.com/es-ES/1.0.0/) e adheríndose a [Semantic Versioning](https://semver.org/lang/es/).

---

## [Unreleased]

### Engadido
- **Compositor de referencia (Issue #5)**: Soporte para transición `MIX` e cortinillas xeométricas `WipeLeftToRight`, `WipeRightToLeft`, `WipeTopToBottom` e `WipeBottomToTop`.
- **Superposición con coordenadas (`overlay_at`)**: Capacidade para posicionar capas de menor resolución en coordenadas arbitrárias con recorte (*clipping*) automático.
- **Son en directo 48 kHz PCM (Issue #44 / PR #45)**: Captura e aliñamento temporal de son estéreo a 48 kHz na gravación de Program.
- **Distribución e CI de Windows x64 (Issue #46 / PR #51 e #52)**: Workflow automatizado para empaquetado portable `.zip` e verificación MSVC con GStreamer.
- **Gobernanza e Seguridade (Issue #33)**: Políticas de contribución (`CONTRIBUTING.md`), seguridade (`SECURITY.md`) e control de dependencias reproducibles.

### Cambiado
- **Interface de usuario (Issue #4)**: Eliminación do fader T-Bar visible e botón TAKE da pantalla principal, substituíndoos por controis limpos de `CUT`, `MIX`, `WIPE` con presets de tempo (0.5s, 1.0s, 2.0s).
- **Etiquetas de cámara honestas (Issue #49 / PR #50)**: Substitución de rutas ríxidas tipo `/dev/video0` por descricións neutras e verdadeiras en Windows e macOS.

---

## [0.1.0-alpha] - 2026-11-14

### Engadido
- Núcleo de realización broadcast nativo en Rust con licenza MIT.
- Consola de dobre monitor con PREVIEW e PROGRAM.
- Perfil audiovisual estándar predeterminado: 1920 × 1080 progressive a 50.00 FPS con son a 48 kHz.
- Bus de comandos unificado e tipado para control compartido mediante GUI, MIDI e API de rede local.
- Integración opcional de controladores USB MIDI con *soft takeover*.
- Complemento independente OpenAction / Stream Deck.
- Gravador WebM de Program de referencia con probas sintéticas.
