# GitHub-Codex coordination: restricted pilot

GitHub Issues are the task queue, pull requests are proposals and CI is evidence. User decisions can be gathered by voice and translated into issues, so the product designer need not relay repeated instructions to Codex. This first workflow is a **documentation-only pilot**, not an automated implementation of the full application.

## Disabled by default: cost and safety gate

`.github/workflows/codex-docs-pilot.yml` runs its Codex job **only** if the GitHub Actions repository variable `OPENVIDEOMIX_CODEX_ENABLED` has the exact value `true`. Leave it unset until the separate cost is approved. The workflow uses the `openai/codex-action` and a repository secret named `OPENAI_API_KEY`. An OpenAI **API** key is not included in the ChatGPT Business subscription; API requests are billed separately. GitHub-hosted Actions minutes may also incur charges. Define a spending limit before enabling; a 20-minute job timeout is not a financial cap. Never put keys in issues, commits, documentation or chat messages.

GitHub Settings > Actions > General > Workflow permissions must allow Actions to create pull requests if the draft-publishing job is to succeed. Keep manual merge review and branch protections in place.

## One issue, one draft PR

1. Create a narrowly scoped **documentation-only** issue, with desired correction, supporting evidence and verifiable acceptance criteria.
2. When ready for the pilot, add `codex-ready` to the issue. With the repository variable enabled, applying the label starts a run. Alternatively, use the workflow's manual `Run workflow` command with its issue number (the label is still mandatory). Both methods can consume API credit.
3. Codex runs in a workspace-restricted job with only `contents: read` and `issues: read`. It cannot make GitHub writes directly. Its output is checked against an allowlist (`README.md` and existing Markdown files in `docs/`) and transferred as a bounded patch to a separate publishing job.
4. The publisher opens or updates a **draft** pull request. No automatic merging, issue closing or repeat-until-done loop is configured.
5. Inspect the PR and CI, then close the issue only after evidence-backed acceptance. A green build does not prove real AV ingest, synchronization, 1080p50 or recording.

A concurrent-run limit prevents parallel pilot runs but not repeated billable runs. To stop new runs, set the variable to `false`; cancel any already-running workflow separately in GitHub Actions.

## Extend only after a verified successful trial

A future workflow may edit application code after a separate review of permissions, test gates, CI on Windows/Linux/macOS, dependency ordering among PRs and an enforceable budget. Do not broaden this pilot's allowlist to Rust files as a shortcut.

Official documentation: https://github.com/openai/codex-action and https://github.com/peter-evans/create-pull-request .
