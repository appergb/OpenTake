# 语义搜索模型恢复与下载大小提示

> 状态：generated · 阶段：implementation-backed · 日期：2026-09-06
> 工作树：OpenTake-generation · 分支：release/v1.0.0-beta.6
> 范围：Web 搜索界面与 IPC 类型接线；原生模型、下载与 worker 验证由主线负责。

## 目标与契约

本切片修复首次模型下载大小占位、损坏模型缺少修复入口，以及修复成功后当前查询不刷新的问题。保留现有搜索 hook、共享操作 token、Rust 只读镜像与 Files/Spoken 独立结果组。

主线确认的契约：

- `SearchResults.visualError?: string | null` 向后兼容；未安装模型继续空 Moments 且无 visualError。
- 模型验证损坏使用稳定 marker `SEARCH_MODEL_REPAIR_REQUIRED:`。查询返回直接前缀，索引 rejection 可能由 WorkerError 包装为 `model error: SEARCH_MODEL_REPAIR_REQUIRED: ...`，因此 Web 在整个错误消息中查找 marker。
- 普通推理/索引错误不带该 marker，不能误导用户重新下载模型。
- `searchModelStatus().bytes` 是 manifest 下载文件合计；成功 DTO 与现有 `downloadSearchModel`、`searchIndexStart(epoch, path)`、`searchQuery(query)` 调用约定不变。
- Tauri 查询/索引等待移出 UI 主线程、复用 bounded ORT worker 属于主线 Rust 工作，不在本切片修改。

## 实现

1. 读取既有 `searchModelStatus()`，下载 tooltip 使用实际合计按十进制 MB/GB 展示。1535824768 bytes 显示 `1.54 GB`。等待状态、零值、无效数值或状态读取失败时显示“暂时无法确定下载大小”，移除 380 MB 默认值。
2. 索引操作结算事件保留错误文字。含修复 marker 的错误导向“修复并重新下载模型”并调用 `downloadSearchModel`；普通索引失败显示实际错误，使用索引重试说明并继续调用 `searchIndexStart`。
3. 消费可选 visualError。即使所有素材已经建立索引，损坏查询也显示修复说明及入口；普通视觉推理错误显示失败提示。Files 和 Spoken 继续渲染，不把视觉失败显示为无匹配结果。
4. 同工程下载/索引成功后沿现有刷新通道读取最新索引状态，并递增查询 revision，重跑此时的查询文字；新查询仍使用 250ms debounce、请求序列与取消保护，成功后移除旧视觉错误。下载完成后若仍需建立索引，继续显示原“建立索引”按钮，不新增自动索引流程。
5. 保留共享下载/索引 operation token，重复修复点击不会重复下载。同工程卸载/重挂载可继续显示同一 pending 操作并接收完成通知；卸载后无订阅者时不发起刷新/查询。旧工程操作完成可使当前工程重新读取权威状态（原有行为），但不会重跑新工程查询、套用旧失败或自动建立索引。
6. manifest 大小返回、查询返回和操作后的查询重跑均检查现有生命周期/工程身份。修复时的新下载错误不会被旧 visualError 遮盖，网络失败可见且能重试下载。

## Read Set 与写集

已核对根 AGENTS.md、`docs/project/conventions.md`、既有 Web 路由与 MediaSearch hook/测试、SearchResults/SearchModelStatus/SearchIndexStatus 类型，以及 api.ts 中对应只读契约。只读参考上游 `MediaPanel/MediaTab/MediaTab+IndexStatus.swift`；核对主线新增 Rust visual_error DTO，仅阅读未修改。

本切片实际写入：

- `web/src/components/media/MediaSearch.tsx`
- `web/src/components/media/MediaSearch.test.ts`
- `web/src/lib/types.ts`（仅 SearchResults 的可选字段与说明）
- `web/src/i18n/dict.ts`（搜索恢复所需中英文文案，保留既有贴纸内容）
- 本记录

Sticker 组件及其测试保持冻结。没有修改 Rust、API 调用参数、依赖、测试配置、版本或其他 MD，没有 commit/push。

## 验证证据

TDD 过程：

- 最初新增 9 项用例均在旧实现失败，包括真实大小/未知大小、损坏索引修复、普通索引重试、完整索引下的 query 损坏与结果保留、操作完成重查及工程边界；最小接线后通过。
- 增补卸载/重挂载、卸载后无刷新、迟到旧查询与下载失败可见性用例；“旧 query 损坏错误遮盖新下载失败”先失败，修正显示优先级后通过。
- 收到主线 WorkerError 包装补充后，增加包装 marker 用例，先确认 startsWith 判断失败，再改为 includes 后通过。

最终执行：

```bash
pnpm --dir web test --run src/components/media/MediaSearch.test.ts src/components/media/MediaPanel.test.tsx
pnpm --dir web build
git diff --check -- web/src/components/media/MediaSearch.tsx web/src/components/media/MediaSearch.test.ts web/src/lib/types.ts web/src/i18n/dict.ts
```

结果：2 个测试文件 / 76 项通过；其中 MediaSearch 29 项（原 15 项 + 新增 14 项）。TypeScript 与 Vite 构建通过。构建仍有动态/静态 import 无法分块和大 chunk 的既有告警。本任务没有运行全量测试或 Rust 测试。

测试使用真实 React 搜索面板与 Zustand，替换外部媒体 API 和下载/索引 promise，以验证用户动作、显示、请求次数及异步隔离。没有下载真实 1.5GB 模型，也没有运行原生 GUI，不能据此声明主线 worker 或真模型恢复验收通过。

## 主线原生待验收

1. 模型未安装时下载提示来自 manifest 合计，下载/索引期间 UI 响应正常。
2. 同尺寸损坏模型：索引 worker 包装错误与完整索引下 query 校验错误均显示修复；Files/Spoken 保留。
3. 修复下载成功后当前查询自动重跑；仍缺索引时按现有按钮建立索引，再自动重查；恢复后的画面结果正确，错误提示消失。
4. 普通推理错误不触发重新下载；网络失败给出实际错误和下载重试；快速重复点击、跨项目以及切走再回的操作不会重复提交或显示旧工程错误。

这些 GUI/真实模型场景仍待主线验收；本记录不表示 Beta 6 已发布。
