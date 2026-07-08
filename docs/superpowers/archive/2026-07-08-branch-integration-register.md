# Branch Integration Register

## Policy

"Merge all branches" means integrate all still-relevant active branch work onto
`recovery/superpowers-integration-20260708-v2` without regressing current
`origin/main`.

Direct stale branch merges are rejected when the branch would delete current
main-line work. Selective replay is the integration method.

## Current Base

| Ref | SHA | Meaning |
|---|---|---|
| `origin/main` | `ac50dc8` | Canonical integration base |
| `recovery/superpowers-integration-20260708-v2` | `942518e` | Recovery branch with approved Superpowers design |
| `backup/before-rollback-20260708-163646` | `9eceadb` | Evidence source for reverse clip and prior planning |

## Queue

| Source | Status | Evidence | Action |
|---|---|---|---|
| `opentake-pr9` dirty worktree | Deferred | reverse-clip uncommitted diff; branch head equals `origin/main` | Defer until reverse-clip replay task resumes; then port useful changes and exclude `.claude` deletions |
| `backup/before-rollback-20260708-163646` | Deferred | commits `0b72a10`, `1cfee93`, `9eceadb` | Defer until reverse-clip replay task resumes; then port reverse-clip fixes and docs only |
| `fix/text-raster-alignment` | Deferred | one old commit `89bf38c`, 154 behind / 1 ahead | Defer until reverse clip is settled; then inspect and replay text raster work |
| `test/render-pixel-diff` | Deferred | one old commit `eb6e429`, 154 behind / 1 ahead | Defer until text raster is settled; then inspect and replay pixel diff work |
| `fix/91-media-library-rewrite` | Deferred | one old commit `b9e4954`, 154 behind / 1 ahead | Defer until reverse clip and media surfaces are stable; then inspect for non-regressive pieces |
| `feat/save-clip-as-media` | Deferred | one old commit `708fd44`, 154 behind / 1 ahead | Defer until reverse and media surfaces are stable; then inspect save-clip-as-media work |
| `feat/freeze-frame` | Deferred | one old commit `da3e934`, 154 behind / 1 ahead | Defer until source-frame mapping is stable; then inspect and replay freeze-frame work |
| `feat/account-scaffold` | Deferred | one old commit `4986716`, 154 behind / 1 ahead | Defer until editing recovery is settled; then inspect and replay account scaffold work |
| `feat/agent-chat-panel` | Deferred | one old commit `dd9f224`, 154 behind / 1 ahead | Defer until the wide-surface tasks are done; then inspect and replay chat panel work |
| `feat/generative-ui` | No-op | branch head equals `origin/main` | No functional delta at discovery |
| `feat/inspector-ai-edit-tab` | No-op | branch head equals `origin/main` | No functional delta at discovery |
| `feat/proxy-media` | No-op | branch head equals `origin/main` | No functional delta at discovery |

## Evidence Commands

- `git status --short --branch`
- `git worktree list --porcelain`
- `git rev-list --left-right --count origin/main...<branch>`
- `git log --oneline --no-merges origin/main..<branch>`
- `git diff --name-status origin/main..<branch>`
- `git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/opentake-pr9 diff --stat`
- `git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/opentake-pr9 diff --name-status`
