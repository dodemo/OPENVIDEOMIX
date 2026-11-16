# Skylos Quality & Security Gate en OPENVIDEOMIX

Este documento describe a integración de [Skylos](https://github.com/duriantaco/skylos) en **OPENVIDEOMIX** para garantir a integridade, seguridade e calidade do código xerado por axentes de IA e desenvolvedores.

---

## 1. Configuración (`skylos.toml`)

No directorio raíz do proxecto configúrase `skylos.toml`:

```toml
[tool.skylos]
exclude = [
    "target",
    "dist",
    "bin",
    "recordings",
    ".git",
]
confidence = 50

[tool.skylos.done]
test_command = "cargo test --workspace"

[tool.skylos.gate]
block_security = true
block_secrets = true
block_ai_defects = true
```

---

## 2. Comprobacións Automatizadas

1. **Tests e Integridade (`skylos done --base main`)**:
   - Executa `cargo test --workspace`.
   - Verifica que ningún axente borrase, omitise ou relaxase tests unitarios existentes.
   - Asegura que ningún linter nin comprobación estática fose silenciada sen xustificación documentada.

2. **Auditoría Estática de Seguridade (SAST)**:
   - Detección de *Path Traversal* (`SKY-D215`): impide que rutas de ficheiros non confiables alcancen o disco sen validación.
   - Detección de filtracións de segredos (`SKY-S101`): bloquea claves de API, tokens e certificados hardcodeados.

3. **Calidade de Scripts**:
   - Vixía a complexidade ciclomática e cognitiva (`SKY-Q301`, `SKY-Q306`) e o tamaño máximo de funcións.

---

## 3. Workflow de GitHub Actions Recomendado

Para activar o gardián de calidade en cada Pull Request, pódese engadir o seguinte ficheiro en `.github/workflows/skylos.yml` (require permiso `workflow` no token de GitHub):

```yaml
name: Skylos Quality Gate

on:
  pull_request:
  push:
    branches: ["main"]

permissions:
  contents: read

jobs:
  skylos:
    name: Skylos Quality & Security Gate
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - name: Checkout repository
        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2
        with:
          fetch-depth: 0
          persist-credentials: false

      - name: Setup Python
        uses: actions/setup-python@42375524e23c412d93fb67b49958b491fce71c38 # v5.4.0
        with:
          python-version: '3.12'

      - name: Install Skylos
        run: python -I -m pip install skylos==5.6.0

      - name: Run Skylos Analysis
        run: |
          pr_base_ref=""
          if [ "${{ github.event_name }}" = "pull_request" ]; then
            pr_base_ref="origin/${GITHUB_BASE_REF:-main}"
          fi
          if [ -n "$pr_base_ref" ]; then
            skylos . --danger --quality --ai-defects --secrets --sca --diff "$pr_base_ref" --format concise
          else
            skylos . --danger --quality --ai-defects --secrets --sca --format concise
          fi
```
