---
id: capabilities.ledger
title: OpenTake Capability Ledger
summary: 首轮上游对齐和安装版 UI 实测的模块级状态；详细证据按能力 ID 继续累积。
kind: engineering
status: draft
content_stage: partial-implementation
scope:
  - all-modules
triggers:
  - parity
  - 上游对齐
  - 功能完成度
  - 真实验收
read_when:
  - 修改公共能力或判断模块是否完成
skip_when:
  - 仅查看单个内部函数
priority: must
freshness_class: project
last_verified: 2026-08-21T13:12:31+08:00
owners:
  - OpenTake-generation
source_of_truth:
  - ./requirements.json
  - ../audit/2026-08-21/full-desktop-functional-matrix.md
related:
  prerequisites:
    - ./README.md
  next:
    - ../superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md
supersedes: []
tags:
  - capabilities
  - parity
  - evidence
---

# 首轮状态

这不是最终完成声明，而是 2026-08-21 的可恢复基线。机器可读条目见 [`requirements.json`](./requirements.json)；安装版 UI 场景见 [`full-desktop-functional-matrix.md`](../audit/2026-08-21/full-desktop-functional-matrix.md)。首轮上游 agent 按实际代码和两条定向测试落锚，未把历史 handoff 当成当前证据。

## 已有代码链路

- Models、Timeline、Track、Clip、Keyframe、Transform、TextStyle：domain crate 已有实现，需继续做完整行为级枚举。
- EditCommand、事务边界、undo/redo、split/trim/move/ripple：ops/core 已有上游对齐实现；`split_clip_distributes_keyframes_at_cut` 通过。
- Inspector、字幕/转写/搜索、Agent/MCP、Motion Studio opaque MP4：代码和对应测试入口已存在；仍需安装版场景证据才能升级为 verified。

## 首轮明确缺口

| ID | 优先级 | 当前状态 | 最小实施切片 | 验收 |
|---|---:|---|---|---|
| `OT-WINDOW-SMALL-SCREEN` | P0 | verified | Tauri 安全初始尺寸 + monitor-aware standard/compact 裁剪 + Home/Editor min-size 回归 | 1066×666、1280×720、1331×768 contract 测试；安装版紧凑/标准切换与 Home/编辑器截图 |
| `UP-MEDIA-IMPORT-AND-FOLDERS` | P0 | verified | 文件/文件夹导入、跳过 unsupported、relink 和预览均已通过安装版验收 | 新构建 app 中 MP4、PNG、双文件多选 Open 可用；导入数 1→3；文件夹导入出现目录并提示跳过 64；relink 恢复离线媒体 |
| `UP-TIMELINE-UI` | P0 | partial | 继续补齐真实选区、拖拽、删除和撤销；分割有效前置条件已验证 | 新构建 app 选中 `sample-text-0`、播放头帧 15 后 `⌘K` 成功新增 UUID 片段并启用撤销；其它时间轴操作仍待矩阵化 |
| `UP-PREVIEW-PLAYBACK` | P0 | partial | 复制媒体 fixture 做暂停/恢复/seek/尾帧/导出对照 | preview/export 同帧语义，音画与错误边界可证 |
| `UP-PREVIEW-TABS` | P1 | verified | `previewTabIds + previewTabHistory + activeTabId`，含 legacy 归一和删除清理 | 安装版同时显示 Timeline/两个素材 tab；关闭第二个回退第一个；91 个定向测试通过 |
| `UP-MEDIA-VIEW-MODES` | P1 | verified | folder/flat/grouped 三态投影、网格/列表密度、文件夹导航和音频导入入口已通过测试及安装版验收 | 搜索、选择、拖拽和预览沿用同一 MediaItem ID 链路，继续纳入后续模块化 QA |
| `OT-MOTION-ALPHA` | P1 | partial | alpha-safe 输出和 render resolver | 透明 motion 覆盖底图、预览/导出一致 |
| `UP-ACCOUNT-CLOUD-BOUNDARY` | P0 | blocked | 先定义 BYOK/provider 替代还是追同构云契约，再补 capability gate | 未授权时不广告/不发送；有凭据时生成→媒体→落轨闭环 |
| `UP-SETTINGS-TAXONOMY` | P2 | partial | 明确 models/agent/storage/general/account 语义映射 | 设置入口和上游语义一一可解释 |

## 真实 UI 证据摘要

- 约 `1331×768`：标准窗口配置偏大，最近项目区域宽度控制弱。
- 紧凑档约 `1066×666`：可见、可操作，当前切换行为通过。
- 新建/打开项目可弹原生 sheet 并取消；示例项目可打开；播放头会推进；选中文本片段能联动 Inspector；Agent/Motion/导出/帮助入口可达。
- 首轮导入图片时选中后 Open 仍禁用，导入失败阻塞素材预览闭环；时间线分割在帧 0/片段外无变化，但在有效选区和帧 15 前置条件下已实测新增片段并启用撤销。
- 本轮修复后，文件导入 MP4/PNG/双文件多选均能点 Open，导入后素材预览可见；文件夹导入成功出现目录并提示跳过 unsupported；relink 成功恢复离线媒体。
- Preview tabs 已在安装版打开两个素材并验证关闭回退；当前 Preview/Media slice 的定向测试为 4 files / 91 tests passed。
- 曾出现 AX 更新但截图未重绘的黑屏瞬间；关闭/重开后页面正常，后续以窗口重绘时序风险记录，不把瞬态截图直接等同于产品黑屏。

## 定向验证

| 命令/场景 | 结果 | 说明 |
|---|---|---|
| `cargo test -p opentake-ops split_clip_distributes_keyframes_at_cut` | 通过 | 上游 split 关键帧边界对拍锚点 |
| `cargo test -p opentake-agent --test advertised_tool_acceptance` | 通过 | 工具广告面锚点 |
| `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage.json pnpm exec vitest run src/store/settingsStore.test.ts src/components/home/HomeView.visual.test.ts src/components/shell/SplitPane.interaction.test.tsx` | 3 files / 22 tests 通过 | Node 26 需要显式 localStorage 文件 |
| `pnpm test`（无 Node localStorage 参数） | 15 files / 125 tests 初始化失败 | 环境假红：`localStorage` 未提供，不作产品失败结论 |

## Task 2 新鲜验收证据

- 提交范围：`ff6177e8`（主实现）、`2235c638`（SplitPane 测试）、`32c8c3c`（review 修复）、`70e56c9`（Settings contract 修复）。
- 受影响 Web 回归：
  `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-final.json pnpm exec vitest run src/store/settingsStore.test.ts src/components/home/HomeView.visual.test.ts src/components/shell/SplitPane.interaction.test.tsx src/components/settings/SettingsView.visual.test.ts src/components/settings/SettingsView.interaction.test.tsx`
  → 5 files / 50 tests passed。
- Web build：`pnpm build` → exit 0；保留既有 dynamic-import / large-chunk warnings。
- Rust 格式：`cargo fmt --all -- --check` → exit 0。
- 上游 split 锚点：`cargo test -p opentake-ops split_clip_distributes_keyframes_at_cut` → 1 passed。
- Release app：`web/node_modules/.bin/tauri build` 已完成 release 编译并生成 `/Users/trip/TRUE 开发/PRIMARY-CN/OpenTake-generation/target/release/bundle/macos/OpenTake.app`；DMG `bundle_dmg.sh` 无输出后手动停止，不能声明 DMG 完成。
- 安装版二进制：最终 token 修复后的 `.app` 已复制到 `/Applications/OpenTake.app`，SHA-256 `02150854e418cd3c3dedad97d905f972ddad99e3a23d287ed1fcac42b15b40a0`。
- Computer Use：新构建首次进入约 `1066×666` 紧凑档；Home/编辑器/预览/时间轴/Inspector/设置可见；切换标准档时窗口铺满约 `1331×768` 工作区、无白边，再切回紧凑档恢复约 `1066×666`。

## 当前未闭环的全量门禁

- 全量 `NODE_OPTIONS=--localstorage-file=... pnpm test` 为 147/149 test files passed，2 个失败来自未改动的 `uiStore.persistence.test.ts` 和一次并行 localStorage 共享导致的 `TitleBar.interaction.test.tsx`；TitleBar 单文件重跑已 21/21，通过的 uiStore 单文件仍失败，需作为独立 Node 26/存储持久化问题处理。
- 文件导入定向回归：`mediaActions` + `MediaPanel` 为 2 files / 59 tests passed；Preview/媒体/Store 定向回归为 4 files / 91 tests passed；两条切片均已安装版验证。
- 主线 fresh verification（2026-08-21 12:09 +08:00）：Preview/Store/Media 4 files / 91 tests passed；MediaActions/MediaPanel 2 files / 59 tests passed；`pnpm build` exit 0；仅保留既有 Vite chunk warnings。
- 全量 Web 串行门禁（2026-08-21 12:20 +08:00）：`NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-serial-final.json pnpm exec vitest run --maxWorkers=1` → 149 files / 1391 tests passed；Node 26 并行共享 localStorage file 会产生假失败，因此本地门禁使用单 worker。

## Media View Modes 新鲜验收证据

- 实现提交：`745a190`（folder/flat/grouped UI）与 `5b1f45d`（空字符串根节点、悬挂 folderId fail-soft）。
- 聚焦测试：`NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-media-modes-final.json pnpm exec vitest run src/lib/mediaViewModes.test.ts src/lib/folderTree.test.ts src/components/media/MediaPanel.test.tsx` → 3 files / 64 tests passed。
- 安装版：重新构建并复制 `/Applications/OpenTake.app`，当前二进制 SHA-256 为 `fb46fbcaa96ddd10d8e760160b47674c6f233e55effd88eca6d3d8b552092a51`。
- Computer Use：菜单 `媒体组织方式` 可访问地提供“文件夹 / 平铺 / 分组”；带真实文件夹导入后，文件夹模式显示目录卡片，平铺模式显示 37 个媒体且隐藏目录卡片，分组模式显示“全部 / editor-core-after-fix-assets”分组；分组网格/列表均可用；进入目录后面包屑、返回上级及 17 个目录内项目可见；音频“导入”有同一模式入口，“我的/收藏”保持任务型空态。
- Preview token 修复：`tokenUsage.test.ts` 曾捕获未声明的 `--space-2xs`，已改为现有 `--space-xxs`；token、Preview、Store 48 tests 和 Web build 通过。
- 最终原生包：`web/node_modules/.bin/tauri build --bundles app` exit 0，release `.app` 完成生成；最终包 Computer Use 再次验证两个媒体 Preview tabs、关闭回退和紧凑窗口。
- `cargo tauri build` 的历史文档入口不可用（没有全局 cargo-tauri）；本轮使用项目本地 Tauri CLI。DMG 尾脚本仍需单独排查。

## Related Documents

- Parent index: [Capabilities README](./README.md)
- Machine manifest: [requirements.json](./requirements.json)
- Desktop matrix: [2026-08-21 functional matrix](../audit/2026-08-21/full-desktop-functional-matrix.md)
- Long-term plan: [Full UI and upstream convergence](../superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md)

## Change Log

- `2026-08-21T10:06:00+08:00` — 写入首轮 UI 真机巡检和上游 parity audit；状态均保守记录为 implemented/partial/blocked。
