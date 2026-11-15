# LOCALIZATION MATRIX: SPREADSHEET-BASED COMMUNITY TRANSLATION SPECIFICATION

> **Milestone:** Issue #70 (`feat(i18n): Cross-platform Localization System`)  
> **Status:** Architectural Specification  
> **Target System:** OPENVIDEOMIX  

---

## 1. Visión e Obxectivo / Executive Overview

Para democratizar a tradución de OPENVIDEOMIX e permitir que calquera persoa da comunidade poida engadir un novo idioma **sen tocar código nin entender programación**, o sistema de internacionalización (i18n) estrutúrase coma unha **Matriz de Tradución en Folla de Cálculo (*Localization Spreadsheet / Matrix*)**.

O termo técnico estándar deste formato é **Matriz Tabular de Localización (Tabular i18n Matrix)** en formato **CSV / TSV / ODS / Google Sheets**.

---

## 2. Estrutura da Matriz de Idiomas (Folla de Cálculo)

Na folla de cálculo:
- **Cada fila** representa un termo, texto da interface ou comando do programa identificado pola súa clave única (`Key`).
- **A columna A** contén a clave técnica invariable.
- **A columna B** contén o texto de referencia canónico (en inglés).
- **A columna C** contén o idioma principal de desenvolvemento (Galego).
- **Cada nova columna adicional** representa un novo idioma achegado pola comunidade (Español, Portugués, Francés, Catalán, Euskera, etc.).

### Exemplo da Matriz de Tradución (`assets/locales/translations.csv`):

| Key (Clave) | Description (Contexto) | en (English - Base) | gl (Galego - Predeterminado) | es (Español) | pt (Português) | fr (Français) |
|---|---|---|---|---|---|---|
| `app.title` | Título da fiestra principal | Openvideomix Studio | Openvideomix Studio | Openvideomix Studio | Openvideomix Studio | Openvideomix Studio |
| `btn.cut` | Botón de corte directo | CUT | CORTE | CORTE | CORTE | COUPE |
| `btn.fade` | Botón de transición fundida | FADE | FUNDIDO | FUNDIDO | FUNDIDO | FONDU |
| `btn.take` | Botón de transición auto | TAKE | PASO | PASAR | ENTRAR | ENVOYER |
| `bus.master` | Nome do bus mestre | Master | Mestre | Maestro | Principal | Général |
| `bus.phones` | Canle de auriculares | Headphones | Auriculares | Auriculares | Fones de ouvido | Casque |
| `status.on_air` | Indicador de emisión | ON AIR | NO AIRE | EN EL AIRE | NO AR | EN DIRECT |
| `status.live` | Indicador de vivo | LIVE | EN DIRECTO | EN DIRECTO | AO VIVO | EN DIRECT |
| `status.recording` | Indicador de gravación | RECORDING | GRAVANDO | GRABANDO | GRAVANDO | ENREGISTREMENT |
| `input.add` | Engadir nova fonte | Add Input | Engadir Entrada | Añadir Entrada | Adicionar Entrada | Ajouter Entrée |
| `input.camera` | Tipo de entrada cámara | Camera | Cámara | Cámara | Câmera | Caméra |
| `input.clock` | Capa de reloxo | Production Clock | Reloxo de Produción | Reloj de Producción | Relógio de Produção | Horloge de Production |
| `input.countdown`| Capa de cronómetro | Countdown Timer | Cronómetro / Conta Atrás | Cuenta Atrás | Temporizador / Regressiva | Compte à rebours |
| `input.lower_third`| Rótulo terzo inferior | Lower Third | Terzo Inferior / Rótulo | Faldón / Rótulo | Terço Inferior | Synthé |
| `ai.auto_mode` | Modo de realización IA | Auto Director | Realización Automática | Realización Automática | Auto Direção | Réalisation Automatique |
| `ai.suspended` | Estado tras corte manual | AUTO SUSPENDED | AUTO SUSPENDIDO | AUTO SUSPENDIDO | AUTO SUSPENSO | AUTO SUSPENDU |

---

## 3. Como pode a comunidade achegar un novo idioma? / Community Workflow

1. **Descargar ou abrir a folla:** Calquera persoa abre o ficheiro `translations.csv` en LibreOffice Calc, Microsoft Excel, Google Sheets ou calquera editor sinxelo.
2. **Engadir unha nova columna:** No encabezamento da primeira fila engade o código ISO do novo idioma (por exemplo, `it` para italiano ou `ca` para catalán).
3. **Traducir as filas:** Vai traducindo as celas da súa columna respectando as claves da columna A.
4. **Enviar a mellora:** Comparte o ficheiro ou fai un Pull Request en GitHub.
5. **Carga en tempo real:** OPENVIDEOMIX detecta automaticamente a nova columna no ficheiro e engade ese idioma de inmediato ao desplegable de selección da interface, **sen necesidade de recompilar nin reprogramar o software**.

---

## 4. Xestor de Tradución Integrado na Interface (In-App Translation Editor)

Nas opcións de OPENVIDEOMIX (`Configuración -> Idiomas`):
- O usuario pode **ver a táboa de tradución en vivo**.
- Hai un botón **`[➕ Engadir Novo Idioma]`** onde o usuario escribe o nome e o código da lingua, e a aplicación crea a nova columna na táboa para que a poida cubrir directamente dende a interface gráfica.
- Hai un botón **`[📥 Importar / Exportar Folla CSV]`** para facilitar o intercambio coas ferramentas ofimáticas habituais.

---

## 5. Resistencia a Fallos (Graceful Fallback)
Se un idioma da comunidade aínda non ten traducidas todas as celas (deixa algunha cela en branco):
- O motor comproba a existencia da tradución no idioma seleccionado.
- Se a cela está baleira ou ausente, recorre automaticamente ao **galego** ou ao **inglés** canónico como respaldo de seguridade, garantindo que **a interface nunca falle, non mostre textos rotos nin se peche con erro**.
