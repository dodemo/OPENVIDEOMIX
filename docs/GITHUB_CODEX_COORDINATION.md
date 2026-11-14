# GitHub and Codex coordination (staged pilot)

GitHub Issues are the task queue, PRs are proposals, and CI is evidence. Voice conversations can be translated into issues without the product owner becoming a messenger between tools. The first implementation is intentionally limited to documentation changes. It does **not** automate the entire development backlog.

## Activation and cost gate

The workflow `.github/workflows/codex-docs-pilot.yml` is disabled unless the repository variable `OPENVIDEOMIX_CODEX_ENABLED` is set to the exact value `true`. Do not set it without an explicit cost decision. ChatGPT Business subscription credits **cannot** authenticate this GitHub Action; it requires an OpenAI API key stored as `OPENAI_API_KEY` in GitHub Actions repository secrets, with independent OpenAI API billing. GitHub Actions runner minutes may also be billed. Set a separate API spend limit before activation. Never paste credentials into an issue, chat, workflow file, or repository.

For the first run, also verify Settings > Actions > General allows GitHub Actions to create pull requests. Use the least repository permissions; this workflow separates read-only Codex execution from the write-enabled PR publisher. Keep branch protections and manual merge review in place.

## One task at a time

1. Write a **small** issue with objective, evidence and acceptance criteria. Add the label `codex-ready` only after the issue is safe and scoped for the pilot. This label can later trigger the workflow automatically when the activation variable is enabled.
2. With API billing and the variable enabled, adding `codex-ready` to an open issue starts one run. Alternatively run `OPENVIDEOMIX Codex documentation pilot` from the GitHub Actions tab, supplying the issue number. Re-adding the label or rerunning manually can consume further API credit.
3. The action checks that the issue is open and labeled, gives Codex a read-only GitHub token and an isolated workspace, and rejects changes outside existing `README.md` and `docs
