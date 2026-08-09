# P1 派生资源调度修复证据

修复时间：2026-08-08（Asia/Shanghai）
工作树 / HEAD 基线：`OpenTake-generation` / `f9398a9302e57690caa3ea21ff995fdd0ce97706`
修复范围：前端 waveform、素材卡/搜索缩略图、选中视频 preview poster 的统一 admission control；未修改 Rust、asset protocol、CSP 或 accessible-name UI。

## 1. RED → GREEN

### RED

先新增 `web/src/lib/derivedResourceScheduler.test.ts`，未实现调度器时运行：

```text
pnpm exec vitest run src/lib/derivedResourceScheduler.test.ts --reporter=default
Exit 1
Test Files 1 failed
Error: Cannot find module './derivedResourceScheduler'
```

测试先定义了旧实现不具备的契约：共享 active/pending 上限、跨调用点同键 single-flight、订阅独立取消、排队取消、project epoch 失效、preview latest-wins、AbortSignal、交互优先级。

### GREEN

实现和接线后的验证：

| 命令 | 结果 |
|---|---:|
| scheduler 单测 | **10 passed** / 0 failed |
| MediaPanel + MediaSearch + Preview + scheduler focused 集 | 6 files / **81 passed** / 0 failed |
| `pnpm test -- --run --reporter=default` | **136 files / 1132 passed** / 0 failed |
| `pnpm exec tsc -b --pretty false` | Exit 0 |
| `pnpm build` | Exit 0；`tsc -b` + Vite production build 成功 |
| `git diff --check` | Exit 0 |

测试仍有仓库既有 React `act(...)` warning；build 仍有既有 dynamic-import/chunk-size warning。两者均未造成失败。项目没有安装 Prettier/ESLint/Biome 可执行依赖，`pnpm exec prettier --check ...` 因 command not found 未运行；TypeScript build 是本次格式外的强制静态检查。

## 2. 实现语义

统一调度器：`web/src/lib/derivedResourceScheduler.ts`。

- 全局共享 `maxActive = 4`、`maxPending = 64`。waveform、卡片 thumbnail、搜索 thumbnail、preview poster 不再各自放大 FFmpeg fan-out。
- pending 达到硬上限后，普通请求不再无界增长闭包队列；交互请求会先汰汰低优先级排队项，队列全为交互项时汰汰最旧项，因此仍遵守 64 的硬上限且新选 preview 不会永久丢失。无可汰汰项时才返回 `admitted=false` + `null`。
- identity = `projectEpoch + resource key`；相同 identity 只运行一个底层 Promise，多个订阅者共享结果。
- 单个订阅者取消只结束自己的 Promise；仍有其他订阅者时不误伤共享任务。
- 最后一个订阅者取消：queued 任务从队列中真实删除；普通 active 任务仅解除订阅者，保留同键物理 flight 直到结算，因而 React StrictMode/快速卸载→重挂不会并发启动第二份同键工作。该无订阅者 flight 不计入 publishable `inFlight`，但仍占 active 槽到物理 Promise 结束。
- `activateProject(newEpoch)` 原子失效旧 epoch 的 queued/active 订阅；旧结果不会发布到新项目。
- `latestGroup="preview-poster"` 保证新选中素材使旧 poster 失效；project/latest 失效会对 active entry 发出 AbortSignal 并禁止旧结果发布。
- priority 为 `interactive > visible > background`；preview poster 可越过已排队 waveform，队列满时也会抢占低优先级/最旧同级交互项；已进入 Rust 的物理任务不抢占。
- settled entry 在通知订阅者前从同键 Map 删除，避免完成与新请求之间的“附着到已结算 Promise”竞态。

## 3. 调用点接线

### MediaPanel

- 删除原 `activeThumbnailRequests + pendingThumbnailRequests + mediaThumbnailInFlight` 独立队列。
- 素材卡 thumbnail 使用统一 scheduler、项目 epoch、source-aware key 和原 256 项 LRU；IntersectionObserver admission 仍保留。
- `AudioWaveform` 使用 background 优先级、source-aware key；素材卡只在进入 160px 预取视口后才 admission，unmount/prop/epoch 变化取消订阅并拒绝旧结果写回。

组件测试证据：同时挂载 6 个冷波形，仅 4 次 `getWaveform` 立即开始、2 个 pending；unmount 后 pending 从 2→0、publishable inFlight 从 6→0。4 个已进入 Rust 模拟工作的 active 槽在底层 Promise 完成后回到 0。另有 A epoch 结果晚到不覆盖 B epoch，以及已 disconnect 的旧 MediaCard `IntersectionObserver` callback 不能反向切回旧 epoch/启动缩略图的回归测试。

### MediaSearch

- Moment/Spoken `HitThumbnail` 经统一 scheduler，key 包含 project epoch、media id、source path、source second。
- query/项目变化或 unmount 时取消每个命中订阅。

组件测试证据：冷搜索返回 40 个不同时间点时，仅 4 次 `generateThumbnail` 立即开始、36 个 pending；unmount 后 pending 36→0、publishable inFlight 40→0。

### Preview

- `MediaPreview` poster 使用 interactive priority + `latestGroup="preview-poster"`。
- item/path/epoch 变化或 unmount 取消旧订阅，poster state 先清空。

组件测试证据：old→current 快速切换后，old poster 晚到不写入 `<video poster>`；current poster 才发布。

## 4. 代码评审追加边界

独立 code-review Agent 指出的 3 个 P1/HIGH 均已转为回归测试并修复：

1. active 最后订阅者取消后立即重订阅同键：现复用原物理 flight，底层 run 只调用 1 次。
2. cleanup 后晚到的旧 MediaCard IO callback：现由 cleanup `cancelled` + 当前 store epoch 双重防卫，scheduler epoch 保持在新项目。
3. 队列满时 interactive/latest 丢弃：现优先汰汰低级项；全交互队列时汰汰最旧同级项，新 poster 保持 admitted/latest-wins。

### 4.1 TypeScript 强制复审 RED → GREEN

后续 TypeScript reviewer 又找到 1 个 HIGH 和 1 个 MEDIUM：

- HIGH：旧 Map 只用 `projectEpoch + string key` 定位，随后直接将 `Entry<unknown>` 断言为 `Entry<T>`。同 raw key 先请求字符串、再请求数组时，第二个 `run` 不会执行，声明为数组的 Promise 会静默收到字符串。RED 稳定显示 `numberRun` 期望 1 次、实际 0 次。
- MEDIUM：active A 被 latest-group B 取代时，旧实现立即删除 A 的 physical entry；当 A→B→A 快速切换时，旧 A 仍在占 Rust/FFmpeg 槽位，却会再启一份 A。RED 稳定显示 `duplicateARun` 被调用 1 次。

GREEN 实现：

- scheduler 现只接受自身定义的 invariant typed kind token（thumbnail/search thumbnail/preview poster/waveform）；identity 加入不可冲突的 kind id，key 与结果类型在编译期和运行时都绑定。`@ts-expect-error` 契约证明 waveform token 不能声称 image-path 结果类型；同 raw key 但不同 kind 会各执行一次并返回正确类型。
- latest-group 放弃 active entry 时仍结算旧订阅并发出 AbortSignal，但保留 keyed physical entry 直到底层 Promise 结算；A 回切时先取消 B，再重挂旧 A，因而 A 底层 `run` 仍只有 1 次。

复修后共享工作树门禁：

```text
pnpm exec vitest run src/lib/derivedResourceScheduler.test.ts
13 passed / 0 failed

pnpm exec tsc --noEmit -p tsconfig.json
exit 0

pnpm test
137 files / 1143 passed / 0 failed

pnpm build
exit 0
```

## 5. 修改文件

- `web/src/lib/derivedResourceScheduler.ts`（新增）
- `web/src/lib/derivedResourceScheduler.test.ts`（新增）
- `web/src/components/media/MediaPanel.tsx`
- `web/src/components/media/MediaPanel.test.tsx`
- `web/src/components/media/MediaSearch.tsx`
- `web/src/components/media/MediaSearch.test.ts`
- `web/src/components/preview/Preview.tsx`
- `web/src/components/preview/Preview.poster.test.tsx`（新增）
- 本证据文档

没有修改 `web/src/lib/asset.ts`、`src-tauri/src/safe_asset_protocol.rs`、`src-tauri/src/lib.rs`，也没有触碰共享工作树中其他 Agent 的 Library、Export 或 Rust 修复。

## 6. 残留限制

判定：**P2 能力限制，不阻断本次前端 P1 有界化**。

现有 `generateThumbnail`、`getWaveform`、`previewPoster` Tauri command 不接收 cancel token / operation id。调度器可以：

- 真正删除尚未开始的 pending；
- 在 project/latest 失效时将 active 标为不可发布并发出 AbortSignal；普通 unmount 则保留同键 flight 供立即重挂复用；
- 以全局 4 active 上限约束仍在运行的 Rust/FFmpeg；

但不能物理 kill 已经进入 Rust 的 FFmpeg。要实现 active hard-cancel，需另行扩展 Rust command/cancel API，并保证 child kill+wait、缓存 staging/rename 与 project epoch 一致性。主 Agent 已确认本轮把它记录为 P2，不要求越权修改 Rust。

另外，本任务**没有顺手修复 MediaSearch accessible name**；搜索框仍只有 placeholder 的 UI 问题留给 UI/accessibility 修复线，避免扩大本 P1 资源调度补丁。
