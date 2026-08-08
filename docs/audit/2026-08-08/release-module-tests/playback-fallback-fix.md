# Playback capability fallback 修复证据

- 日期：2026-08-08（Asia/Shanghai）
- 范围：Web 播放能力握手与回退；未修改 playback transport、asset、proxy、scheduler、Rust 命令注册或 release workflow
- 结论：独立测试报告中的 `--no-default-features` 前端回退 blocker 已有自动化 RED/GREEN 闭环

## 修复行为

1. `get_preview_endpoint` 现同时是播放 feature 能力握手：调用成功才标记 Rust engine available；仅精确的 `Command get_preview_endpoint not found` 被归一为可缓存的 `null`，不再产生 unhandled rejection。成功或能力缺失的探测 Promise 会复用，`Preview` 与 app-level engine 不会重复发起 IPC；瞬态/非能力错误则原样抛出并清除 probe cache，下一次调用可重试。
2. app-level engine 在握手完成前不启动 native playback；结果不可用时，可由 WebKit 完整表达的时间线走 legacy `<video>` 路由，不会调用 `playback_start`。需要 compositor 而无 Rust 能力的时间线仍由现有 route 显示 unsupported，避免静默丢效果。
3. `Preview` 使用同一能力结果选择 Rust/WebKit 表面并获取 frame endpoint。默认 feature 开启时仍得到 endpoint 并走既有 Rust route。
4. Tauri 2.11.x 缺失命令的精确 plain-string 形式 `Command <name> not found` 仅在 `<name>` 为 `get_preview_endpoint` 或 `playback_start/pause/stop/seek` 时被视为 fallback-safe；其他缺失命令或任意错误不会被吞掉。

## TDD 证据

### RED

命令（`web/`）：

```bash
pnpm exec vitest run src/lib/api.playbackCapability.test.ts src/components/preview/nativePlaybackSession.test.ts src/components/preview/previewEngine.test.ts src/components/preview/Preview.test.tsx
```

实际结果：exit 1；4 files failed，5 tests failed / 42 passed。五个失败签名与 blocker 一致：

- 缺失 `get_preview_endpoint` 仍 reject，而非解析为 unavailable。
- playback unknown-command 判定 helper 尚不存在。
- native controller 对 `Command playback_start not found` 返回不回退。
- endpoint capability 为 `null` 时 engine 仍调用了 `playback_start` 1 次。
- endpoint capability 为 unavailable 时双视频轨 Preview 仍渲染 Rust 表面，没有 WebKit `<video>`。

### GREEN

同一命令实际结果：exit 0；4 files passed，47/47 tests passed。

发布关键组稳定性重放：同一 focused 命令连续三次均为 47/47，`pass@1 = 1.00`、`pass@3 = 1.00`、`pass^3 = 3/3`。

### Final Web review HIGH：probe 错误分类与重试

reviewer 指出首版把所有 endpoint probe rejection 都转成并永久缓存为 `null`，会把瞬态 IPC/后端启动错误误报为编译时能力缺失。

RED 命令（`web/`）：

```bash
pnpm exec vitest run src/lib/api.playbackCapability.test.ts
```

实际 RED：exit 1；1 file failed，2 failed / 2 passed。失败签名分别证明：

- 结构化瞬态 `{ code: "engine", message: "preview server is starting" }` 被错误 resolve 为 `null`。
- `Command playback_start not found` 也被 endpoint probe 错误 resolve 为 `null`。

最小修复后同一 focused 命令连续三次均为 exit 0、4/4：精确 endpoint missing 连续调用只发起 1 次 IPC 并缓存 `null`；两类非 capability 错误原样 reject，随后调用可重试成功且总共发起 2 次 IPC。`pass@1 = 1.00`、`pass@3 = 1.00`、`pass^3 = 3/3`。

## 回归与构建

- `pnpm exec vitest run src/components/preview src/lib/api.playbackCapability.test.ts src/lib/api.test.ts`：exit 0；21 files，171/171 tests passed。
- `pnpm test`：exit 0；138 files，1147/1147 tests passed。
- `pnpm build`：exit 0；TypeScript project build 和 Vite production build 成功。Vite 仍报告仓库既有的 dynamic-import/chunk-size warnings，无新增编译错误。
- 未触碰 Rust，因此本修复没有重跑 Rust default/no-default checks；原独立报告已证明两种 Rust 配置可编译。

Final Web review HIGH 修复后的最终重放：

- `pnpm exec vitest run src/lib/api*.test.ts src/components/preview`：exit 0；33 files，206/206 tests passed。
- `pnpm test`：exit 0；138 files，1154/1154 tests passed。
- `pnpm build`：exit 0；TypeScript + Vite production build 成功；仅有上述既有 warnings。

## 修改文件

- `web/src/lib/api.ts`
- `web/src/lib/api.playbackCapability.test.ts`
- `web/src/components/preview/nativePlaybackSession.ts`
- `web/src/components/preview/nativePlaybackSession.test.ts`
- `web/src/components/preview/previewEngine.ts`
- `web/src/components/preview/previewEngine.test.ts`
- `web/src/components/preview/Preview.tsx`
- `web/src/components/preview/Preview.test.tsx`

`Preview.tsx` 与 `Preview.test.tsx` 在本任务前已有他人未提交的 accessibility/poster scheduler 改动；本修复只叠加能力握手/route 相关 hunk，未回滚或改写这些现有改动。

## Concern

- 自动化测试精确模拟了 Tauri 2.11.x 的 missing-command rejection，但本任务未打包并手工启动真实 `--no-default-features` GUI；候选包实机验收仍属独立发布门禁。
