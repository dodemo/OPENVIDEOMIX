# OPENVIDEOMIX

**Mesturador de vídeo en desenvolvemento**, inspirado prioritariamente no fluxo de realización de [vMix](https://www.vmix.com/) e complementariamente en [OBS Studio](https://obsproject.com/). Código propio en Rust, con licenza MIT.

**Plataformas obxectivo: Windows, Linux e macOS (Intel e Apple Silicon).** As tres teñen igual prioridade arquitectónica. O escritorio actual usa `eframe/egui`; a elección definitiva da interface e do compositor debe verificarse co rendemento real.

> **Estado: prototipo técnico, non apto para emisións en produción.** A interface inclúe elementos de demostración. Esta rama incorpora un percorrido de vídeo sintético → Preview → CUT → Program real → WebM con ton de proba. O rendemento de gravación 1080p50 e o audio capturado seguen pendentes. Ningún indicador de refresco da GUI acredita 1080p50 de saída.

## Perfil audiovisual predeterminado

**O perfil de produción predeterminado é 1920 × 1080 progresivo a 50/1 FPS (1080p50), con audio a 48 kHz.** Preview, Program, composición e futuras saídas de gravación/emisión deben respectar este perfil salvo modificación explícita do usuario.

Un dispositivo que anuncie só 1080p30 ou unha resolución inferior non se tratará como fonte 1080p50 nativa. A aplicación debe indicar a necesidade de conversión ou permitir seleccionar outro modo compatible; non se debe efectuar unha degradación silenciosa nin confundir FPS solicitados con FPS medidos. As fontes sintéticas e probas reducidas a 640 × 360 / 30 FPS son exclusivamente probas de captura e non cumpren os criterios de aceptación de produción.

## Funcións desta revisión de desenvolvemento

| Área | Que existe hoxe | Limitación importante |
| --- | --- | --- |
| Preview / Program | Consola de dobre monitor e CUT centralizado; estudo opt-in con catro sinais de proba e Program CPU real | O percorrido sintético grava WebM, pero perde fotogramas; non está cualificado para emisión |
| Imaxes | Apertura de PNG, WebP e JPG e presentación na interface; unha superposición con alfa en Program | Non hai aínda compositor de saída real |
| Cámaras USB/UVC | Captura opt-in con GStreamer da cámara predeterminada e fonte sintética; modelos de capacidades | Sesións independentes e detección asíncrona; selección física persistente e varias webcams reais pendentes |
| Transicións | Compositor **de referencia de CPU**, con CUT, FADE, WIPE e ata catro overlays, probado píxel a píxel | CUT conectado ao estudo sintético; GPU e MIX/WIPE do escritorio pendentes. Sen T-Bar/TAKE |
| Control de realización | Comandos tipados compartidos por GUI, MIDI e API; tradutor restrinxido de nomes de atallos de vMix | Non é compatible con toda a API de vMix |
| MIDI | Adaptador nativo opcional, MIDI Learn, perfís JSON e *soft takeover* | Compilación/CI verificadas; falta proba de controladores físicos |
| OpenDeck / Stream Deck | Complemento OpenAction separado: CUT, seleccionar Preview e alternar Overlay 1 | Paquetes de proba para as catro plataformas; falta validación cun dispositivo e instalador distribuíble |
| API local | Servidor HTTP opt-in autenticado, só en `127.0.0.1:21873` | Sen exposición á LAN, WebSocket nin feedback continuo de tally |
| Audio, gravación e streaming | Gravación opt-in de Program real en WebM desde o escritorio, con ton xerado e ocos temporais conservados | Sen audio capturado/mesturado, 1080p50 sostido nin streaming |

Os indicadores de demostración da interface non acreditan cadencia de saída. Os medidores de audio quedan a cero e indican ausencia de captura. A gravación de proba ten estado e contas reais; a emisión e as transicións non implementadas seguen desactivadas.

## Probas de vídeo en desenvolvemento

A [PR #22](https://github.com/dodemo/OPENVIDEOMIX/pull/22), xa integrada en `main`, introduce unha primeira ruta opt-in de captura USB predeterminada e vídeo sintético mediante GStreamer, presentados como texturas na GUI. A [PR #23](https://github.com/dodemo/OPENVIDEOMIX/pull/23) tamén está integrada en `main`: incorpora sesións independentes e probas con fontes sintéticas. Esta rama conecta catro sinais de proba co Program e a gravación; as PR encadeadas deben completar a súa integración antes de describir toda a ruta como dispoñible en `main`.

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

A captura experimental integrada utiliza a función `gstreamer-capture`, que require bibliotecas e complementos GStreamer dispoñibles no sistema. A súa dispoñibilidade non acredita selección persistente, audio nin 1080p50 real.

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

## Proba audiovisual desde o escritorio

```sh
cargo run --release -p openvideomix-desktop --features gstreamer-capture,gstreamer-recording
```

Inicia o estudo de catro sinais, selecciona PVW T1–T4, preme CUT e usa GRAVAR TESTE.
Grava o Program composto con ton de proba de 1 kHz, non micrófono. Consulta
[o procedemento, vídeo real e límites medidos](docs/DESKTOP_PROGRAM_TEST.md).

## Gravación headless en desenvolvemento

O feature opt-in `gstreamer-recording` incorpora un gravador WebM de Program
RGBA composto e PCM estéreo a 48 kHz, cun exemplo reproducible independente da
GUI e probas de descodificación. Usa o perfil predeterminado 1920×1080 a 50/1,
CPU VP8/Vorbis, colas acoutadas e finalización EOS. **É unha proba técnica**:
o exemplo emprega o compositor de referencia en CPU e son xerado; o feature illado non conecta o motor de audio ou o compositor GPU. A combinación
con `gstreamer-capture` activa a gravación de proba do estudo sintético descrito arriba.
Consulta [o alcance e as probas](docs/PROGRAM_RECORDING_POC.md).
