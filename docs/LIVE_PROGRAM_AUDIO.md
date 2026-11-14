# Audio en directo → gravación Program (primeiro tramo, #44)

O escritorio con `gstreamer-capture,gstreamer-recording` permite seleccionar
explicitamente audio en directo no diálogo RECORD. O modo predeterminado
segue a ser **ton Rust xerado de 1 kHz**. Non hai buses, AFV, PFL nin monitorización
por altofalantes implementados neste tramo.

```sh
cargo run -p openvideomix-desktop --features gstreamer-capture,gstreamer-recording
```

Iniciar o estudo de proba (vídeo sintético 1080p50), abrir RECORD, marcar
«Captura de audio en directo GStreamer» e seleccionar:

| Fonte | Dispositivo | Evidencia |
| --- | --- | --- |
| `audiotestsrc` | baleiro | Sintética en directo, sen micrófono; probada automaticamente |
| `pulsesrc` | identificador de fonte PulseAudio ou baleiro | Candidato Linux, sen hardware validado |
| `wasapisrc` | identificador WASAPI ou baleiro | Candidato Windows, sen hardware validado |
| `osxaudiosrc` | índice Core Audio ou baleiro | Candidato macOS, sen hardware validado |

O elemento/plataforma e os plugins deben estar instalados. A selección acepta un
único elemento fonte e a súa propiedade `device` (texto ou enteiro validado),
non unha cadea arbitraria de pipeline. A falta de elemento, permisos, dispositivo,
negociación ou audio causa estado visible/error. Non se require ningún servizo,
clave ou API de pago. GStreamer/plugins manteñen as obrigas de licenza existentes.

A fonte negocia **48.000 Hz e dúas canles antes de `audioconvert`**. Só se converte
a representación a F32LE intercalada. Non hai `audioresample` nin conversión
silenciosa de canles/taxa. A negociación non proba a taxa nativa do ADC físico:
o backend do sistema pode efectuar conversións propias. Modos incompatibles
fallan explicitamente. Por exemplo, un micrófono só mono pode non negociar.

A ganancia/mute Master existentes aplícanse ao PCM antes da codificación. O
estado público `StudioView` e a GUI distinguen a etiqueta da fonte, mute, estado,
perdas e peak/RMS dBFS **calculados do PCM posterior á ganancia**; `AudioStatus`
tamén expón niveis anteriores á ganancia. Cero dixital é −∞ dBFS; falta de
medición é `None`. Os antigos medidores de demostración doutros buses non
representan este sinal. Non hai unha cualificación física implícita nas etiquetas.

## Contrato temporal e límites

- `AudioInput` é reutilizable sen GUI. Usa o mesmo `CaptureTimeline`, SystemClock,
  época e conversión PTS → segmento → running time + base time que o vídeo.
  Conserva offsets de arranque e conta DISCONT. Non resta o primeiro PTS de cada
  fonte por separado e non substitúe o reloxo de vídeo por un reloxo de audio.
- No escritorio, T0 é o tempo de captura do primeiro intervalo Program aceptado:
  PTS do tick menos os 100 ms de latencia de entrada configurados. Os índices
  posteriores son relativos ao mesmo tick; audio e vídeo comezan no mesmo T0.
  Unha fonte de audio que arranca máis tarde conserva o seu atraso como silencio
  **contado**, sen mover o vídeo. Non é calibración da latencia física.
- Colas: appsink de dous buffers sen descarte automático; canle de captura de
  16 buffers; cada bloque máximo 100 ms/4.800 mostras por canle. Cola chea descarta
  o bloque novo e incrementa `queue_drops`. O aliñador ten como máximo 128 bloques
  (4,92 MB de PCM no peor caso); non crece indefinidamente.
- O consumidor do encoder espera audio como máximo 100 ms por transacción;
  captura, compositor e GUI seguen independentes. O PCM ausente pasa a silencio
  e incrementa `silence_samples`. Máis dun segundo seguido de ausencia remata a
  sesión con erro e etiqueta de ficheiro parcial. Sen buffers durante 500 ms,
  a captura publica `Unavailable`; EOS/desconexión e erros son terminais.
- Colocación absoluta no índice de mostra máis próximo, erro de cuantización
  máximo 10,42 µs. Saltos positivos son ocos contados; os solapamentos recórtanse
  e cóntanse, sen sobreescribir mostras xa aceptadas. Un paso cara atrás superior
  a 240 mostras (5 ms) falla. Exponse o máximo paso entre bloques en mostras.
  Non hai PLL/resampling adaptativo: é reparación temporal acoutada por bloque,
  cola e ausencia continua, **non disciplina dos reloxos físicos**. Unha deriva
  sostida pode producir silencio/recortes audibles; precisa unha fase posterior.
- `ProgramRecorder::write_slot_at` permite `None` para vídeo ausente: grava todo
  o PCM, incluídos ocos finais, sen inventar imaxes. Os saltos de paquetes de
  vídeo conservan o seu índice e todo o intervalo PCM acumulado. Cada transacción
  está limitada a un segundo. As estatísticas distinguen slots e imaxes reais.
- STOP pecha ambas as pistas por EOS e agarda a finalización. O final da gravación
  é o último slot consumido; os paquetes aínda na cola ao procesar STOP non se
  gravan. Nunca se declara gardado un erro/timeout. Drop cancela captura e
  xunta o seu fío; o pipeline volve a NULL mesmo en saídas de erro.

As superficies usan `ProductionCommand::StartLiveRecording { path, factory,
device }`, `StartTestRecording` e `StopRecording`. A mesma validación e a
aprobación local do cartafol de destino aplícanse ás intencións externas.

## Study → Build → Check → Refine

Estudáronse #7/#41, a arquitectura vMix/OBS, CaptureTimeline, OutputWorker,
DesktopProduction e ProgramRecorder. Reutilizáronse estes servizos e o perfil
predeterminado 1920×1080 progresivo 50/1 con 48 kHz, sen engadir outro motor.

```sh
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p openvideomix-desktop --all-features --test live_audio -- --nocapture
```

A suite nativa `live_audio` captura fontes GStreamer **en directo independentes**,
compón Program e finaliza WebM. Un pipeline de descodificación separado verifica
PCM, niveis, PTS, negociación e duración. A proba reducida 640×360/30 dura 600 ms,
con atraso deliberado de audio, dous ocos intermedios e dous finais: 14 imaxes,
18 slots e 28.800 mostras/canle. Nunha execución Cloud Linux, a pista de audio
rematou a 604,33 ms (inclúe padding Vorbis), peak −11,79 e RMS −15,80 dBFS tras
ganancia 0,5. Son medicións **sintéticas**, non sincronía física. Os timestamps
WebM teñen cuantización de milisegundos e tolérase un bloque final do códec.

Outra proba usa o servizo DesktopProduction real co perfil predeterminado, a
captura de vídeo sintético e a selección explícita de PCM GStreamer. Mantense
a proba existente de descodificación 1080p50. As probas de fallo inclúen EOS con
cola chea, fonte inexistente, modo 44,1 kHz incompatible, parada, overflow,
underflow, DISCONT, ganancia/mute, límites de memoria/aritmética e simulación
±1.000 ppm. A deriva positiva insire 43 mostras de silencio en 48.000;
a negativa recorta cinco mostras en dous bloques de 100 ms. Non se ocultaron
perdas para facer coincidir duracións.

A CI existente executa a suite Linux con todas as features e as compilacións
base Windows/Linux/macOS Intel/Apple Silicon. **Non valida os adaptadores
GStreamer opcionais nin permisos físicos de Windows/macOS.**

## Proba física repetible (pendente)

1. Nun posto físico, instalar GStreamer cos plugins da plataforma, conceder
   permiso de micrófono e comprobar a identidade e os modos da interface.
   Confirmar 48 kHz estéreo; rexistrar calquera conversión que faga o sistema.
2. Executar o comando anterior, iniciar o estudo de vídeo sintético, escoller
   a fonte/dispositivo real e gravar 60 s nun cartafol aprobado. Comprobar que
   a etiqueta é capturada, que falar cambia os niveis medidos e que mute produce
   PCM cero. Desconectar o micrófono: debe aparecer erro/ausencia e non «gardado».
3. Repetir con sinal de referencia e ganancia coñecida. Inspeccionar con
   `ffprobe -show_streams -show_packets ficheiro.webm` e descodificar ambas as
   pistas; comparar duración, timestamps, silencio, perdas e pasos máximos.
4. Para medir sincronía **física**, usar unha cámara real coa mesma época de
   captura e unha referencia flash/clap visible/audible. A GUI deste tramo segue
   a usar vídeo sintético: ese ensaio por si só **non mide latencia cámara-micrófono**.
   Medir offset inicial/final e deriva nunha gravación longa por plataforma.

Non se executou esta proba física en Cloud: non hai micrófono nin cámara real.
Non se acredita captura física, sincronía A/V medida nin 50 fps sostidos.
