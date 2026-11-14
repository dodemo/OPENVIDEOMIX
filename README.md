# OPENVIDEOMIX

**Mesturador de vídeo en desenvolvemento**, inspirado prioritariamente no fluxo de realización de [vMix](https://www.vmix.com/) e complementariamente en [OBS Studio](https://obsproject.com/). Código propio en Rust, con licenza MIT.

**Plataformas obxectivo: Windows, Linux e macOS (Intel e Apple Silicon).** As tres teñen igual prioridade arquitectónica. O escritorio actual usa `eframe/egui`; a elección definitiva da interface e do compositor debe verificarse co rendemento real.

> **Estado: prototipo técnico, non apto para emisións en produción.** A interface inclúe elementos de demostración. Non existe aínda un fluxo completo de captura → composición → audio → codificación → gravación/emisión. Ningún indicador de refresco da GUI acredita 1080p50 de saída.

## Perfil audiovisual predeterminado

**O perfil de produción predeterminado é 1920 × 1080 progresivo a 50/1 FPS (1080p50), con audio a 48 kHz.** Preview, Program, composición e futuras saídas de gravación/emisión deben respectar este perfil salvo modificación explícita do usuario.

Un dispositivo que anuncie só 1080p30 ou unha resolución inferior non se tratará como fonte 1080p50 nativa. A aplicación debe indicar a necesidade de conversión ou permitir seleccionar outro modo compatible; non se debe efectuar unha degradación silenciosa nin confundir FPS solicitados con FPS medidos. As fontes sintéticas e probas reducidas a 640 × 360 / 30 FPS son exclusivamente probas de captura e non cumpren os criterios de aceptación de produción.

## Funcións dispoñibles en `main`

| Área | Que existe hoxe | Limitación importante |
| --- | --- | --- |
| Preview / Program | Consola de dobre monitor, selección de entradas e CUT lóxico, co estado centralizado | Non constitúe unha saída broadcast codificada |
| Imaxes | Apertura de PNG, WebP e JPG e presentación na interface; unha superposición con alfa en Program | Non hai aínda compositor de saída real |
| Cámaras USB/UVC | Modelo de capacidades, formatos e perfís fixos, con probas de portabilidade | A captura efectiva por GStreamer segue nunha PR separada |
| Transicións | Compositor **de referencia de CPU**, con CUT, FADE, WIPE e ata catro overlays, probado píxel a píxel | O compositor de referencia **non está conectado** ao escritorio nin usa a GPU. FADE/WIPE/T-Bar da GUI están desactivados |
| Control de realización | Comandos tipados compartidos por GUI, MIDI e API; tradutor restrinxido de nomes de atallos de vMix | Non é compatible con toda a API de vMix |
| MIDI | Adaptador nativo opcional, MIDI Learn, perfís JSON e *soft takeover* | Compilación/CI verificadas; falta proba de controladores físicos |
| OpenDeck / Stream Deck | Complemento OpenAction separado: CUT, seleccionar Preview e alternar Overlay 1 | Paquetes de proba para as catro plataformas; falta validación cun dispositivo e instalador distribuíble |
| API local | Servidor HTTP opt-in autenticado, só en `127.0.0.1:21873` | Sen exposición á LAN, WebSocket nin feedback continuo de tally |
| Audio, gravación e streaming | Modelos de estado e elementos de demostración | **Non implementados como saídas reais** |

O tempo da interface, o contador de FPS e os medidores de audio actuais poden ser simulados. **Non indican cadencia audiovisual medida, mestura de son real, emisión nin gravación.** Os botóns de saída e as transicións non implementadas están desactivados.

## Probas de vídeo en desenvolvemento

A [PR #22](https://github.com/dodemo/OPENVIDEOMIX/pull/22) introduce unha primeira ruta opt-in de captura USB predeterminada e vídeo sintético mediante GStreamer, presentados como texturas na GUI. A [PR #23](https://github.com/dodemo/OPENVIDEOMIX/pull/23), dependente da anterior, incorpora varias sesións de vídeo independentes e probas con dúas fontes sintéticas. **Estas funcións non forman parte de `main` mentres as PR non estean fusionadas.**

Ningunha destas probas garante aínda selección individual de varias webcams físicas, 1080p50, sincronización A/V nin rendemento de produción. A compatibilidade coa OBSBOT Tiny 2 está prevista, non validada sobre hardware real.

## Compilar e executar

Precísase unha versión de Rust compatible coa edición indicada no `Cargo.toml` e as dependencias nativas da plataforma.

```bash
git clone https://github.com/dodemo/OPENVIDEOMIX.git
cd OPENVIDEOMIX
cargo run -p openvideomix-desktop
```

Funcións opcionais xa integradas:

```bash
cargo run -p openvideomix-desktop --features midi-input,local-control-api
```

A API local só arranca se está configurada `OPENVIDEOMIX_CONTROL_TOKEN` cun valor ASCII de 32 a 256 caracteres. Non gardes o token en perfís exportados nin o publiques no repositorio. O complemento OpenDeck está no proxecto independente `apps/openvideomix-opendeck` e necesita que a aplicación e o proceso do host reciban a mesma credencial. Consulta [o seu README](apps/openvideomix-opendeck/README.md).

Nas PR experimentais de captura, a función adicional é `gstreamer-capture`, que require bibliotecas e complementos GStreamer dispoñibles no sistema. Non se debe anunciar como dispoñible en `main` antes de superar a revisión.

## Arquitectura e desenvolvemento

- `crates/core/`: estado do mesturador, ordes de realización, MIDI, capacidades de cámaras e compositor RGBA de referencia.
- `apps/openvideomix-desktop/`: interface de escritorio e adaptadores opcionais.
- `apps/openvideomix-opendeck/`: complemento OpenAction/Stream Deck con compilación independente.
- `docs/`: [arquitectura](docs/ARCHITECTURE.md), [folla de ruta](docs/ROADMAP.md), [referencia do compositor](docs/REFERENCE_COMPOSITOR.md), [API local](docs/LOCAL_CONTROL_API.md) e [compatibilidade de atallos de vMix](docs/VMIX_SHORTCUT_COMPATIBILITY.md).
- `AGENTS.md`: convencións e instrucións para os axentes de desenvolvemento.

A orde técnica de traballo é verificar captura e selección de cámaras, compositor GPU con reloxo de vídeo, audio sincronizado, gravación/codificación/streaming, empaquetado e probas físicas multiplataforma. Priorizar a experiencia operativa de **vMix**, e recorrer a **OBS** cando vMix non ofreza unha referencia apropiada.

## Licenza e dependencias

O **código propio** de OPENVIDEOMIX publícase baixo [MIT](LICENSE). Os códecs, bibliotecas multimedia, complementos GStreamer e paquetes distribuídos deben pasar unha auditoría específica de licenzas e requisitos de distribución antes dunha versión pública.

## Estado das probas

GitHub Actions comproba código de Rust e funcións opcionais en Windows, Linux e macOS Intel/Apple Silicon. Unha CI verde indica compilación e probas automatizadas do alcance declarado; **non substitúe** probas de latencia, reloxo, sincronización, dispositivos físicos ou estabilidade nunha realización en directo.
