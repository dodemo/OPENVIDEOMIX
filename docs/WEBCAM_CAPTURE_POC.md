# Proba de captura de webcam (GStreamer)

## Estado

Esta é unha **proba de concepto**, non unha integración de produción:
- Recibe fotogramas reais dunha cámara predeterminada por `autovideosrc` e amósaos en **Preview**, **Program** e miniaturas da entrada nova.
- Captura **vídeo, non audio**. Un pipeline de mostra negociado a 640×360/30 fps RGBA utilízase para limitar a memoria e o custo de copia.
- O proceso de captura corre nun fío separado cunha cola acoutada de 3 fotogramas; descártanse fotogramas cando a UI non consegue consumilos. A GUI mostra a última textura dispoñible.
- O botón `Engadir webcam USB (proba)` tenta abrir a cámara predeterminada do sistema. O botón `Engadir sinal sintético GStreamer` utiliza un xerador de vídeo en movemento (`videotestsrc`) que percorre exactamente o mesmo pipeline RGBA e as mesmas texturas de Preview/Program. A proba sintética **non** acredita que funcione unha cámara USB. Agora admítense varias sesións de captura independentes, incluídas dúas ou máis fontes sintéticas. As fontes USB aínda usan `autovideosrc` e, sen selección explícita de dispositivo, poden intentar abrir a mesma webcam: **isto non demostra compatibilidade con varias webcams físicas**.
- Os dispositivos/cámara simulados iniciais da aplicación seguen sendo marcadores de posición, sen emisión real.
- Non hai gravación nin emisión, nin transicións GPU reais, nin captura 1080p50 garantida.
- A interface de control PTZ e seguimento OBSBOT continúa fóra de alcance.

## Compilación

Requírese instalar previamente **GStreamer de desenvolvemento e runtime**, e os complementos apropiados para cada plataforma, con headers e configuración `pkg-config` accesibles ao compilador.

```bash
cargo run -p openvideomix-desktop --features gstreamer-capture
```

Sen o feature, o programa mantén a interface habitual e explica que a captura USB non está compilada, sen necesitar bibliotecas nativas GStreamer. Para despregar Windows e macOS haberá que distribuír ou documentar as bibliotecas/plugins e axustar a carga do runtime; non está resolto nesta PR.

## Validación necesaria

1. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`.
2. Compilar con e sen feature nos tres sistemas operativos. Comprobar que o plugin `autovideosrc`, `videoconvert`, `videoscale`, `videorate` e `appsink` están instalados.
3. Conectar unha cámara UVC; engadila, seleccionar a entrada en Preview e trasladala a Program. Confirmar movemento real e estado de erro cando se desconecta.
4. Comprobar que saír da aplicación ou eliminar a fonte pecha o pipeline e devolve o dispositivo ao sistema.
5. A lectura actual emprega `GstVideoFrame` e respecta o `stride` das filas para RGBA. Verificar buffers con padding de filas, erros de metadatos, orientación e diferentes drivers; non se admite aínda un formato distinto de RGBA nin cópias GPU sen paso por CPU.
6. Medir latencia de extremo a extremo, uso CPU/memoria, drops e fotogramas realmente recibidos, non os repintados da GUI.
7. Antes de activar 1080p50 e OBSBOT Tiny 2, enumerar capacidades reais e permitir seleccionar cámara, resolución e codec MJPEG/H.264. Evitar abrir `autovideosrc` para todas as entradas.
8. Revisar licenzas de GStreamer e plugins redistribuídos, permisos (especialmente macOS), instaladores e ruta de plugins.

## Dependencias técnicas

- [GStreamer autovideosrc](https://gstreamer.freedesktop.org/documentation/autodetect/autovideosrc.html)
- [gstreamer-rs AppSink](https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer_app/struct.AppSink.html)
- Issues #3, #6, #10

**Validación local (2026-11-14):** executados en Linux `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` e `cargo test --workspace --all-features`: 28 probas do núcleo e 11 do escritorio superadas. Non hai hardware físico validado; manter os gates de CI multiplataforma.

## Validación da seguinte etapa: varias fontes

A rama dependente `feat/concurrent-gstreamer-inputs` conserva unha sesión de `CameraSession` por `InputId`, non pecha unha fonte ao engadir outra e libera o seu pipeline ao eliminar a entrada. A proba `two_synthetic_sources_capture_concurrently` abre dúas fontes `videotestsrc` simultáneas, comproba fotogramas RGBA en ambas e confirma que pechar unha non detén a outra. Esta proba necesita GStreamer no CI Linux e non substitúe probas con dúas cámaras USB reais. Antes de produción deberán implementarse identificadores físicos persistentes, enumeración de modos e sincronización por timestamps.

## Timestamps de captura (prototipo)

Cada `CameraFrame` transporta agora `pts_ns` e `duration_ns` opcionais, copiados dos metadatos de `GstBuffer`. A proba de fonte sintética comproba que se recibiu un PTS. O valor é relativo ao pipeline de cada fonte: **non é un reloxo global compartido entre dúas cámaras**, non acredita sincronía A/V e non debe compararse directamente con PTS doutro pipeline sen mapear as bases temporais. Un dispositivo real pode non proporcionar PTS; neste caso o campo é `None`. A GUI segue amosando texturas sen usar estes tempos para compoñer a saída broadcast.

## Mapeo ao reloxo común de captura

`capture_pts_ns` é un campo adicional: todas as sesións utilizan explicitamente
un `GstSystemClock` compartido e unha única época do proceso. A transformación é
`segment.to_running_time(PTS) + pipeline.base_time - epoch`, con aritmética
comprobada. Respecta o segmento (non só o PTS cru), conserva a separación temporal
entre sesións iniciadas en momentos diferentes e deixa `None` cando faltan
metadatos, o PTS está fóra do segmento ou hai overflow/underflow. `pts_ns` segue
conservando o valor local orixinal descrito arriba.

A GUI distingue PTS local e común na notificación inicial; ningún é unha medida
de FPS nin do instante físico de exposición. Dúas probas verifican segmentos e
metadatos inválidos, e fontes sintéticas iniciadas separadamente, mantendo o
offset real e a continuidade ao pechar a primeira. Un gardián RAII devolve o
pipeline a NULL tamén nas saídas temperás por erro de inicio.

Este mapeo aínda non corrixe deriva de dispositivos, non calibra latencia USB,
non establece sincronía A/V e non conecta as colas ao compositor GPU ou gravador.
O reloxo de audio mestre e a sincronización física seguen pendentes. A ruta de
captura experimental mantén 640×360/30 explicitamente; non substitúe o perfil
predeterminado 1080p50 da produción nin acredita 1080p50 medido.
