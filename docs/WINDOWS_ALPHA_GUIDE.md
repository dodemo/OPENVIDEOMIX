# Guía de Instalación e Probas de OPENVIDEOMIX Alpha en Windows 11

Esta guía explica como descargar, iniciar e probar a primeira **Alpha autónoma de OPENVIDEOMIX** no PC de probas con **Windows 11 e tarxeta gráfica NVIDIA RTX 4080 SUPER**.

---

## 1. Descarga e Descompresión

1. Descarga o paquete ZIP `OPENVIDEOMIX-Windows-Alpha-x64-*.zip` desde os artefactos de GitHub Actions ou a versión de lanzamento.
2. Comproba que o SHA256 coincida co ficheiro `.zip.sha256`:
   ```powershell
   Get-FileHash OPENVIDEOMIX-Windows-Alpha-x64-*.zip -Algorithm SHA256
   ```
3. Extrae o arquivo nun cartafol local (por exemplo, `C:\OPENVIDEOMIX\`).
   > [!IMPORTANT]
   > Esta distribución inclúe o runtime de GStreamer e todas as dependencias necesarias. **NON require** instalar Rust, Cargo, Python, Visual Studio nin paquetes de códecs adicionais.

---

## 2. Estrutura do Cartafol

```text
C:\OPENVIDEOMIX\
├── openvideomix.exe             # Executable principal
├── INICIAR-OPENVIDEOMIX.cmd     # Guión de inicio rápido con contorna configurada
├── build-manifest.json          # Metadatos da compilación, versión, hash e licenzas
├── LICENSE                      # Licenza MIT
├── LEEME.txt                    # Instrucións rápidas en galego
├── bin/                         # DLLs de runtime (GStreamer, GLib, etc.)
└── lib/
    └── gstreamer-1.0/           # Plugins multimedia (vp8, vorbis, matroska, testsrc)
```

---

## 3. Inicio da Aplicación

Podes iniciar o programa de dúas maneiras:
1. **Opción recomendada**: Fai dobre clic en `INICIAR-OPENVIDEOMIX.cmd`.
2. **Dobre clic directo**: Fai dobre clic en `openvideomix.exe`. O binario detecta automaticamente as carpetas `lib/gstreamer-1.0` e `bin` adxacentes e configura o runtime de forma autónoma.

---

## 4. Probas de Realización e Gravación

### Paso 1: Estudo Sintético 1080p50
- Ao abrir, verás o rack de entradas con sinais de proba a 1080p50.
- Selecciona unha entrada para **Preview**.
- Preme **CUT** (ou a barra de espazo) para trocar Preview e **Program**. O sinal composto actualízase de inmediato.
- Proba as transicións **MIX** e **WIPE**.

### Paso 2: Gravación de Program
- Preme **GRAVAR TESTE** na barra superior.
- Confirma a ruta de saída do ficheiro `.webm` e o xerador de audio (ton de proba a 48 kHz).
- Preme **Iniciar gravación de proba**.
- Observa o contador de gravación en tempo real. Realiza varios cambios con **CUT** durante a gravación.
- Preme **PARAR GRAVACIÓN** para pechar o ficheiro de vídeo de xeito limpo.

### Paso 3: Exportación de Diagnósticos Sanitizados
- Preme o botón **📋 DIAGNÓSTICO** na barra superior (ou na xanela de axustes).
- Abrirase a xanela de diagnóstico que amosa:
  - Versión do motor e plataforma detectada.
  - Estado do runtime GStreamer e plugins multimedia dispoñibles.
  - Estado de cada entrada do rack.
  - Métricas de saída: fotogramas codificados, ocos/drops e duración.
- Preme **💾 Gardar Markdown...** ou **💾 Gardar JSON...** para gardar o informe.
  > [!NOTE]
  > O informe está **redactado automaticamente**: non contén contrasinais, tokens, nomes de usuario persoais nin rutas privadas. Podes pegalo directamente nunha issue de GitHub.

---

## 5. Verificación Independente do Ficheiro Gravado

Podes verificar o ficheiro `.webm` xerado usando `ffprobe`:
```powershell
ffprobe -show_streams -show_format C:\OPENVIDEOMIX\recordings\openvideomix-*.webm
```
Verifica:
- Fluxo de vídeo: códec VP8, 1920×1080, cadencia nominal 50 FPS.
- Fluxo de son: códec Vorbis, frecuencia de mostraxe 48000 Hz, estéreo.

---

## 6. Probas de Resistencia Recomendadas

1. **Proba base de 60 segundos**: Comprobar que non se perden fotogramas durante cambios normais de plano.
2. **Proba de resistencia de 30 minutos**: Comprobar estabilidade de memoria RAM, uso de GPU e sincronización entre vídeo e son tras media hora de realización continuada.
