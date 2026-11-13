# ROADMAP de OPENVIDEOMIX

Este documento recolle as fases de desenvolvemento, os fitos (*milestones*) e o estado das funcións.

---

## Fase 1: Fundação do Sistema e Interface (Completada ✅)
- [x] Repositorio con licenza aberta MIT en GitHub.
- [x] Arquitectura de workspace modular en Rust (`core`, `ai-assistant`, `desktop`).
- [x] Interface en `egui` con dobre monitor **Preview / Program**, T-Bar interactivo, mesturador de son e grella de entradas.
- [x] Compilación limpa en Linux e Windows.

---

## Fase 2: Captura Real de Vídeo e Pipeline Multimedia (En Curso 🔄)
- [ ] **Módulo de Captura Local (V4L2 / UVC)**: Detección e lectura real de cámaras web USB e tarxetas capturadoras.
- [ ] **Pipeline de Renderizado**: Decodificación eficiente de cadros e envío ás texturas de `egui`.
- [ ] **Xerador de Patróns de Proba**: Barras SMPTE reais, reloxo de timecode e degradados en movemento.
- [ ] **Xestión de Ficheiros de Vídeo**: Reprodución en bucle de clips MP4/MKV.

---

## Fase 3: Motor de Son de Baixa Latencia
- [ ] Integración con PipeWire / ALSA en Linux e WASAPI en Windows.
- [ ] Vúmetros con niveis dBFS reais (en lugar de simulados).
- [ ] Enrutamento de audio por canle (Audio Follows Video - AFV).

---

## Fase 4: Gravación e Emisión (Outputs)
- [ ] Saída por Cámara Virtual (`v4l2loopback` en Linux / DirectShow en Windows).
- [ ] Emisión RTMP/SRT cara a plataformas (YouTube, Twitch, servidores propios).
- [ ] Gravación local acelerada por hardware (NVENC, VAAPI).

---

## Fase 5: Conectividade cos LLMs e Automatización
- [ ] Conexión HTTP/WebSocket co servidor de LLMs do **PCgrande** (Ollama / vLLM).
- [ ] Integración opcional coa API de OpenAI Enterprise / Codex.
- [ ] Transcrición de voz en tempo real (Whisper local) e subtítulos automáticos.
