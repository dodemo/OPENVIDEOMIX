# Análise Técnica Comparativa: vMix vs OBS Studio e Obxectivos de OPENVIDEOMIX

Este documento recolle o estudo en profundidade dos dous grandes referentes da produción de vídeo en directo (**vMix** e **OBS Studio**) para definir con máxima claridade os obxectivos e a arquitectura funcional de **OPENVIDEOMIX**.

Está deseñado para que tanto **ChatGPT Enterprise** como **Antigravity** teñan exactamente a mesma visión compartida do que o software debe facer.

---

## 1. Por que vMix é o estándar de ouro en televisión e directos corporativos?

Mentres que **OBS Studio** foi concibido principalmente para *streamers* de videoxogos (modelo baseado en "Escenas" que conteñen conxuntos de "Fontes"), **vMix** está deseñado para a **realización broadcast profesional** (modelo baseado en "Entradas Independentes" ou *Inputs*).

### Diferenza de Paradigma:
| Característica | OBS Studio | vMix | OPENVIDEOMIX (A nosa meta) |
|---|---|---|---|
| **Paradigma Principal** | Escenas e Fontes compartidas | **Entradas Independentes (Inputs)** | **Entradas Independentes estilo vMix** |
| **Transicións** | Globais entre Escenas | **Directas entre calquera Entrada** | **Directas entre Entradas (CUT/FADE/T-Bar)** |
| **Superposicións (Overlays)** | Fontes dentro da mesma escena | **Canles de Superposición (Overlay 1 a 4)** con alfa independente | **Canles Overlay 1-4 independentes con alfa** |
| **Xestión de Son** | Mesturador global compartido | **Buses independentes (Master, A, B, Auriculares)** + AFV | **Buses Master, A, B e AFV** |
| **Cadencia Broadcast** | 60p / 30p streaming web | **50.00p PAL / 59.94p NTSC / 60p** | **50.00 FPS PAL Broadcast nativo (20ms)** |
| **Conmutación** | Studio Mode (Preview / Program) | **Preview / Program con Tally directo** | **Preview / Program con Tally vermello/verde** |

---

## 2. Os 5 Alicerces Fundamentais que OPENVIDEOMIX debe cumprir

### Alicerce 1: O Paradigma de Entradas (*Inputs Rack*)
- Cada entrada na grella inferior é un obxecto autónomo:
  - Ten o seu propio volume, ganancia e estado de mute.
  - Pode ter activado o modo **AFV (Audio Follows Video)**: o micrófono ou son desa entrada só se abre cando esa entrada pasa a Program.
  - Ten o seu propio estado de **Tally** (bordo verde se está en Preview, bordo vermello se está no aire en Program).
  - Admite calquera formato: Cámara web USB (V4L2), fluxo de rede (NDI, SRT, RTMP), captura de pantalla/xanela, clip de vídeo (MP4/MKV) ou imaxe con transparencia (PNG).

### Alicerce 2: A Matriz de Superposición (*Overlay Channels 1, 2, 3 e 4*)
- O realizador non ten que cambiar a cámara principal para amosar un rótulo ou logotipo.
- Existen 4 canles de superposición globais (`[OV1]`, `[OV2]`, `[OV3]`, `[OV4]`).
- Calquera entrada (un PNG cun rótulo, un vídeo cun grafismo animado, unha segunda cámara en Picture-in-Picture) pódese asignar a unha canle de Overlay.
- Ao premer no botón `OV1`, o grafismo superponse con **transparencia alfa real** sobre o monitor de PROGRAM sen alterar a fonte de fondo.

### Alicerce 3: O Motor de Mestura e Cadencia Broadcast (50.00 FPS PAL)
- Na televisión europea e galega (TVG / EBU), o estándar obrigatorio é **50.00 FPS PAL** (cadencia exacta de 20.0 milisegundos por fotograma).
- O motor de renderizado sobre GPU (`egui` + `wgpu`) debe manter un reloxo de sincronización estrito sen fluctuacións (*jitter*), garantindo que non se produzan saltos de fotogramas nin perda de sincronía entre o son e o vídeo.

### Alicerce 4: O Motor de Son Multicanle
- **Buses virtuais**:
  - **Master**: A mestura final que sae cara á emisión ou gravación.
  - **Bus A / Bus B**: Para enviar audio independente cara a tradución simultánea, monitores de plató ou liñas de intercom.
  - **Auriculares (Cue / PFL)**: Para que o realizador poida escoitar unha fonte en Preview antes de metela no aire.
- Vúmetros con niveis dBFS reais (escala verde ata -18 dB, amarela ata -3 dB e vermella para avisar de saturación/clipping).

### Alicerce 5: Entradas e Saídas Reais (I/O)
- **Captura de Entrada**:
  - V4L2 en Linux para capturar webcams USB e tarxetas HDMI/SDI (Magewell, Cam Link, DeckLink).
  - Portal de escritorio (Wayland / PipeWire) para capturar calquera pantalla ou fiestra.
  - Ficheiros multimedia con decodificación nativa.
- **Saídas (Outputs)**:
  - **Cámara Virtual (`v4l2loopback`)**: Para que calquera programa de videoconferencia (Zoom, Teams, Google Meet) ou navegador poida recibir a saída final de OPENVIDEOMIX coma se fose unha cámara web.
  - **Emisión en directo**: RTMP/SRT cara a YouTube, Twitch ou servidores locais.
  - **Gravación local**: Gravación directa a disco en formato MP4 con aceleración por hardware (NVENC / VAAPI).

---

## 3. Como debe ChatGPT Enterprise xerar as tarefas (Issues)

Cando lle pidas a ChatGPT que deseñe ou especifique a seguinte función:
1. ChatGPT debe consultar este documento para entender en que alicerce encaixa a tarefa.
2. ChatGPT debe xerar a Issue en GitHub asegurándose de que:
   - A lóxica sexa modular en Rust puro.
   - Non se introduzan dependencias externas innecesarias nin pesadas.
   - Respecte os 50.00 FPS de cadencia.
   - Forneza os pasos de proba para que Antigravity poida validar o código antes de pechar a issue.
