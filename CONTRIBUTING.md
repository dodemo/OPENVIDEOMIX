# Guía de Contribución para OPENVIDEOMIX

Grazas polo teu interese en contribuír a **OPENVIDEOMIX**, o mesturador de vídeo nativo en directo de código aberto (MIT).

---

## 1. Principios do Proxecto

1. **Rendemento e Tempo Real**:
   - O perfil predeterminado obrigatorio é **1920 × 1080 progressive a 50/1 FPS (20 ms por cadro)** con audio PCM a **48 kHz**.
   - As taxas de refresco da interface gráfica (egui) non acreditan cadencia de saída broadcast.
2. **Multiplataforma Nativa**:
   - Windows 11 x64, Linux (Wayland e X11) e macOS (Apple Silicon e Intel).
   - Non se permite envolver aplicacións web en navegadores nin Electron para o motor audiovisual.
3. **Distribución 100% Autocontida (*Offline*)**:
   - Os paquetes distribuíbles deben conter todas as DLLs e recursos necesarios sen esixir instalacións previas de GStreamer, Python ou compiladores ao usuario final.

---

## 2. Fluxo de Traballo e Ramas

Seguimos a metodoloxía **Study → Build → Check → Refine**:

1. **Ramas illadas por tarefa**:
   - Nomea a túa rama co prefixo `canvas/issue-<id>-<descricion>` (ou `feat/...`, `fix/...`).
   - Traballa nunha única tarefa acoutada por rama.
2. **Probas locais obrigatorias antes de abrir Pull Request**:
   ```bash
   cargo fmt --all -- --check
   cargo check --workspace
   cargo test --workspace
   cargo clippy --workspace --all-targets -- -D warnings
   git diff --check
   ```
3. **Pull Requests**:
   - Abre sempre a túa PR como **Draft PR** inicialmente.
   - Vincula a Issue correspondente (`Closes #ID` ou `Relates to #ID`).
   - Todos os checks de CI de GitHub Actions deben estar en **verde** antes de solicitar revisión.
   - **Non se permite o *auto-merge* automático.** A revisión e integración require a aprobación dun mantedor.

---

## 3. Estrutura do Repositorio

- `crates/core/`: Lóxica pura do motor de realización sen dependencias de interface nin renderizado gráfico directo.
- `apps/openvideomix-desktop/`: Aplicación de escritorio nativa (`egui`/`eframe`) con adaptadores de captura, son e gravación.
- `apps/openvideomix-opendeck/`: Complemento independente OpenAction / Stream Deck.
- `docs/`: Documentación de arquitectura, contratos audiovisuais e ADRs.

---

## 4. Licenzas e Dependencias Alleas

- O código orixinal do proxecto é **MIT**.
- Non se admiten dependencias con licenzas incompatibles (GPL viral) dentro do executábel MIT nin códecs con riscos de patentes comerciais precompilados sen autorización.
- Toda nova dependencia en `Cargo.toml` debe xustificarse tecnicamente.
