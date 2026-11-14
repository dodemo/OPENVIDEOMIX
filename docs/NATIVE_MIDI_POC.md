# Proba de entrada MIDI nativa

## Obxectivo

Permitir que controladores USB MIDI estándar invoquen as mesmas ordes que a interface gráfica. A referencia de interacción é vMix. O controlador Mingzhe B0FV93GS5Z só se poderá declarar compatible tras comprobar que expón un porto MIDI estándar e as mensaxes que emite.

## Compilación

```bash
cargo run -p openvideomix-desktop --features midi-input
```

Requírese ALSA de desenvolvemento en Linux. macOS utiliza CoreMIDI; Windows utiliza o backend compatible proporcionado por midir. Non se realizaron probas físicas.

## Proba manual

1. Abrir **Axustes > MIDI**.
2. Conectar o controlador e premer **Actualizar portos MIDI**.
3. Escoller o porto e premer **Conectar**.
4. Seleccionar **Aprender CUT** e premer un botón MIDI.
5. Seleccionar **Aprender volume Master** e mover un fader ata atravesar o volume actual. O soft takeover debe evitar saltos de nivel.
6. Seleccionar unha entrada mediante **PVW #…** e aprender o botón MIDI correspondente.
7. Desconectar o dispositivo e verificar que a GUI segue operativa.

## Estado e limitacións

- Primeira implementación optativa para MIDI 1.0 Note On/Off e Control Change de tres bytes.
- Os callback MIDI non modifican directamente o estado de realización. Depositan eventos nunha cola limitada; a GUI despáchaos ao núcleo tipado.
- MIDI Learn pode gardar e cargar perfís JSON versionados a través do explorador de ficheiros. Os perfís son portables, non se cargan automaticamente no arranque; aínda faltan a reconexión automática, a asignación persistente a un dispositivo físico e o tratamento dos cambios de índice do porto.
- Non se soportan aínda running status, SysEx, MIDI 2.0, codificadores relativos, bancos, control motorizado nin feedback LED.
- Os cambios de volume MIDI utilizan soft takeover; necesitan probas de hardware, tolerancias e conciliación con outros controladores. A importación de perfís valida versión, tamaño máximo, duplicados e límites MIDI antes de substituír os mapeos activos.
- OBSBOT PTZ é un subsistema independente e non é requisito para a recepción MIDI.
- Non existe aínda un complemento OpenDeck; deberá chamar ao mesmo motor de ordes.
- O subsistema de audio actual é un prototipo sen mestura real. Un fader cambia o valor no estado, non un sinal de son operativo.
- A compilación e as probas automatizadas de todos os sistemas son condición de integración; non dar por compatible un dispositivo antes das probas.

## Seguridade e fiabilidade

Un porto MIDI local non debe poder abrir dispositivos arbitrarios, provocar bloqueo dos fíos de audio/vídeo nin modificar valores con datos inválidos. A función de aprendizaxe require unha selección explícita. Antes da primeira versión operativa cómpre filtrar eventos de reloxo, reconexións e valores fóra de rango, e probar os cambios de banco sen salto de ganancia.

Relacionado: issues #13, #14 e PR #16.
