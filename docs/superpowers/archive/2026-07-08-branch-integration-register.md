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
| `opentake-pr9` dirty worktree | In progress | reverse-clip uncommitted diff; branch head equals `origin/main` | Port useful reverse-clip changes; exclude `.claude` deletions |
| `backup/before-rollback-20260708-163646` | In progress | commits `0b72a10`, `1cfee93`, `9eceadb` | Port reverse-clip fixes and docs only |
| `fix/text-raster-alignment` | Pending | one old commit `89bf38c`, 154 behind / 1 ahead | Inspect after reverse clip |
| `test/render-pixel-diff` | Pending | one old commit `eb6e429`, 154 behind / 1 ahead | Inspect after text raster |
| `fix/91-media-library-rewrite` | Pending | one old commit `b9e4954`, 154 behind / 1 ahead | Inspect for non-regressive media-library pieces |
| `feat/save-clip-as-media` | Pending | one old commit `708fd44`, 154 behind / 1 ahead | Inspect after reverse and media surfaces |
| `feat/freeze-frame` | Pending | one old commit `da3e934`, 154 behind / 1 ahead | Inspect after source-frame mapping is stable |
| `feat/account-scaffold` | Pending | one old commit `4986716`, 154 behind / 1 ahead | Inspect after editing recovery |
| `feat/agent-chat-panel` | Pending | one old commit `dd9f224`, 154 behind / 1 ahead | Inspect last due wide surface |
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
