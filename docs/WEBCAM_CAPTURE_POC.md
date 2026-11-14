# Proba de captura de webcam (GStreamer)

## Estado

Esta é unha **proba de concepto**, non unha integración de produción:
- Recibe fotogramas reais dunha cámara predeterminada por `autovideosrc` e amósaos en **Preview**, **Program** e miniaturas da entrada nova.
- Captura **vídeo, non audio**. Un pipeline de mostra negociado a 640×360/30 fps RGBA utilízase para limitar a memoria e o custo de copia.
- O proceso de captura corre nun fío separado cunha cola acoutada de 3 fotogramas; descártanse fotogramas cando a UI non consegue consumilos. A GUI mostra a última textura dispoñible.
- O botón `Engadir webcam USB (proba)` tenta abrir a cámara predeterminada do sistema; **aínda non permite escoller un dispositivo nin abre varias cámaras simultaneamente**.
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

**Limitación da revisión actual:** o contorno remoto usado para editar GitHub non ten un compilador Rust nin GStreamer de desenvolvemento, polo que non se executaron as probas. A PR debe permanecer en borrador ata CI e hardware real.
