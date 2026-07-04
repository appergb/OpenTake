# opentake-project 总览

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md) · 本模块目录：[INDEX.md](INDEX.md)

## 一句话定位

`opentake-project` 是 OpenTake 的**工程持久化层**：把内存中的 `Timeline` + `MediaManifest` 落地为磁盘上的 `.opentake` 目录包，并负责两类导出——**自包含归档**（把分散素材收拢进工程内）与 **XMEML 4 / FCP7 XML 时间线交换**。它是一个纯 IO crate，只依赖领域层 `opentake-domain`。

### 依赖分层位置

```
opentake-domain        值语义叶子层（Timeline/Track/Clip/MediaManifest/MediaResolver）
   ▲
opentake-project ★本模块  .opentake 包读写 + archive 归档 + XMEML 导出 + 生成日志
   ▲
opentake-core          会话 / DI / 事件总线（命令路由层，调用本模块开关存工程）
   ▲
src-tauri / web        Tauri 壳 + React 只读镜像
```

依赖**只向下**：本 crate 仅依赖 `opentake-domain`，外部依赖只有 `serde` / `serde_json` / `thiserror`（见 [`Cargo.toml`](../../../crates/opentake-project/Cargo.toml)）。它被 `opentake-core` 装配进会话后，再经 `src-tauri` 命令暴露给前端。`Timeline` / `MediaManifest` / `MediaManifestEntry` / `MediaSource` 等值类型都来自 domain，本 crate 通过 `lib.rs` 把它们 re-export，使下游只需依赖 `opentake-project` 就能完成持久化工作。

## 职责边界

**做：**

- 定义并实现 `.opentake` 目录包的磁盘格式（[`Project::open`] / [`Project::save`]），含上游的读取容错分级（`project.json` 强制、`media.json` 严格、`generation-log.json` 宽松）。
- 原子写盘：每个 JSON 组件先写同目录临时文件再 `rename` 落位，崩溃不会留下半截 `project.json`。
- 自包含归档（[`archive`]）：把所有可解析的媒体引用拷进目标包的 `media/`，并把清单 source 改写为工程相对路径；对拍上游 `PalmierProjectExporter`。
- 时间线导出（[`export_xmeml`]）：把 `Timeline` 序列化为 XMEML 4（FCP7 XML），覆盖位置 / 裁剪 / 变速 / 音量 / 不透明度 / 变换 / 裁切 / 淡入淡出 / A·V 链接；对拍上游 `XMLExporter`。
- 生成日志类型（[`GenerationLog`] / [`GenerationLogEntry`]）：domain 层（刻意零 IO）省略的 AI 生成审计日志，含旧版「美元 → credits」迁移。
- 包内文件名契约（`layout`）与统一错误类型（`error`）。

**不做：**

- **不持权威状态**：本层无 `Timeline` 的所有权语义，撤销 / 重做 / 版本号在 [opentake-ops](../opentake-ops/INDEX.md)。
- **不做媒体解码**：缩略图抽帧 / 波形 / 转写 / 分辨率探测全归 `opentake-media`；本 crate 不依赖 FFmpeg，归档只做**字节拷贝**。
- **不做缩略图生成**：上游 `VideoProject.captureThumbnail` 的抽帧逻辑不在此；`save` 只在 [`Project::thumbnail`] 已被上层填好字节时才写 `thumbnail.jpg`。
- **不做最近工程注册表 / 示例工程下载**：上游 `ProjectRegistry` / `SampleProjectService` 当前**不在本 crate**（见下方完成状态）。
- **不管理 `media/` 与 `chat-sessions/` 目录的内容**：`save` 只写 JSON 组件（与持有的缩略图），这两个目录由媒体层与 agent 层 out-of-band 维护；归档时按整体拷贝搬运。
- **不做窗口 / 主屏 UI**：上游 Project 模块里的 AppKit / SwiftUI 部分全部归前端重建。

## 关键概念与数据流

### `.opentake` 包结构（`docs/architecture/ARCHITECTURE.md` §9）

```text
Name.opentake/
├── project.json         # Timeline（强制；缺失即报错）
├── media.json           # MediaManifest（严格解析；缺失则空清单）
├── generation-log.json  # GenerationLog（可选；解析失败降级为 None）
├── thumbnail.jpg        # 封面（可选）
├── media/               # 工程内素材（.project 相对路径指向此处）
└── chat-sessions/       # agent 对话历史，每会话一个 <session>.json
```

> 与上游 `.palmier` 的差异：(1) 扩展名 `opentake`；(2) 对话目录由上游的 `chat/` 改名为 `chat-sessions/`（见 [`layout`](layout.md)）。包内的 `project.json` / `media.json` 在**字段 / 值级别**与上游 wire 兼容，但本 crate 写 pretty JSON，上游写 compact JSON，故空白与键序不同——不是字节级一致。

### 三类数据流

```
存工程：  core 持有 Timeline+Manifest ──► Project{...}.save() ──► 原子写 project.json/media.json/(gen-log)/(thumb)
开工程：  Project::open(path) ──► 分级解析三个 JSON ──► 交还 Timeline+Manifest+gen_log 给 core
归档：    archive(timeline, manifest, gen_log, src_bundle, dest) ──► 拷媒体到 dest/media/ + 改写 source + 搬附属
导出：    export_xmeml(timeline, manifest, project_base) ──► 一棵 XmlNode 树 ──► render 出 .xml 文本
```

`Project` 是一个轻量内存句柄（包路径 + 三个已解码组件 + 可选缩略图字节），**不把媒体加载进结构**。归档与导出都是**纯函数入口**：输入领域值，输出磁盘副本 / 文本，不依赖 `Project` 句柄。

## 对应上游 Swift 模块

> 详见 [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md) 的 `Project`（§"Project · mixed → needs-replacement"）与 `Export` 两节。

本 crate 是上游 **两个**模块的纯逻辑子集的合并端口：

| 本 crate | 上游 Swift | 上游文件 | 端口性质 |
|---|---|---|---|
| `bundle.rs`（`Project::open`/`save`） | `VideoProject` 的 `read`/`save`/`fileWrapper` | `Project/VideoProject.swift` | 去掉 NSDocument / FileWrapper，改普通目录读写 |
| `layout.rs` | `enum Project`（命名空间常量） | `Utilities/Constants.swift` | 文件名契约直译，`chat/` → `chat-sessions/` |
| `archive.rs`（`archive`） | `PalmierProjectExporter.export` + `Report` | `Export/PalmierProjectExporter.swift` | 1:1，含 Foundation 路径语义复刻 |
| `fcpxml.rs`（`export_xmeml`） | `XMLExporter` + `Builder` | `Export/XMLExporter.swift` | 1:1，含两处跨平台降级（源时码 / 文件存在性过滤） |
| `gen_log.rs`（`GenerationLog`） | `GenerationLog` / `GenerationLogEntry` | `Editor/ViewModel/EditorViewModel+Cost.swift` | 1:1，含美元 → credits 迁移 |
| `error.rs`（`ProjectError`） | 上游抛 `fileReadCorruptFile` 等 | （散落于 read/save） | 归一为一个 `thiserror` 枚举 |

上游 `Project` 模块里的 **NSDocument 生命周期 / 工程窗口 / 标题栏配件 / 缩略图抽帧 / 素材恢复 / FPS 重采样 / 设置不匹配对话框 / Home 主屏 / `ProjectRegistry` / `SampleProjectService`** 都**不在**本 crate——按移植策略分别归 ui-rebuild（前端）、`opentake-media`、`opentake-core`，或尚未实现。

## 完成状态：已实现 vs 计划中

> 对照 [ROADMAP.md](../../architecture/ROADMAP.md)（Phase 2 持久化、Phase 5 导出）、[PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) 与代码实测。

**已实现（代码在本 crate）：**

- ✅ `.opentake` 包读写：`Project::open` / `save` / `save_to` / `new`，分级容错 + 原子写（[`bundle.rs`](../../../crates/opentake-project/src/bundle.rs)）。
- ✅ 自包含归档：`archive` 全流程，含按 Swift `standardizedFileURL` 语义的纯词法去重、`URL.pathExtension` 语义的扩展名提取、collision 重命名、附属文件搬运（[`archive.rs`](../../../crates/opentake-project/src/archive.rs)，含 unix 符号链接对拍测试）。
- ✅ XMEML 4 / FCP7 XML 导出：`export_xmeml` 全流程，覆盖位置 / 裁剪 / 变速 / 音量（静态 + 关键帧）/ 不透明度 / 变换 / 裁切 / 淡变 / A·V 链接 / NTSC 判定 / SMPTE 时码（[`fcpxml.rs`](../../../crates/opentake-project/src/fcpxml.rs)）。
- ✅ 生成日志：`GenerationLog` / `GenerationLogEntry`，含 `version` 缺省 = 1、美元 → credits 迁移、`total_credits`（[`gen_log.rs`](../../../crates/opentake-project/src/gen_log.rs)）。
- ✅ 各文件均带 `#[cfg(test)]` 单测（gen_log / archive / fcpxml 内含大量对拍用例）。

**计划中 / 不在本 crate（代码暂无或归他处）：**

- 🔄 **新建即落盘 + 自动保存**：上游"新建先选盘再 `save`"与 `autosavesInPlace` 等价物（[PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) P0-1 / P1-1 / P1-14）。`Project::save` 已具备能力，但触发时机（`session.rs` 落盘、tokio 防抖自动保存）在 `opentake-core` / `src-tauri`，且依赖前端先选定路径。
- 🔄 **缩略图生成**：上游 `captureThumbnail` 的抽帧→JPEG（GAP P2-1）。`save` 只接受现成字节；抽帧逻辑待落到 `opentake-media`（FFmpeg seek 单帧）。
- 🔄 **最近工程注册表 `ProjectRegistry`**：JSON 持久化 + 废纸篓删除 + 挂起变更队列。本 crate **未实现**；前端有 recents store 雏形（GAP P2-2/P2-3）。
- 🔄 **示例工程 `SampleProjectService`**：依赖闭源 Convex 后端；按移植策略归 cloud-rebuild，本 crate **未实现**。
- 🔄 **FPS 重采样 / 分辨率自动适配 / 设置不匹配判定**（`applyTimelineSettings` / `checkProjectSettings`）：纯算术，按移植图归领域/会话层，本 crate **不含**。
- 🔄 **导出接线**：`export_xmeml` 是纯逻辑且**已实现**，但成片视频导出（H.264/H.265/ProRes，逐帧合成）在 `src-tauri/src/export.rs` + `opentake-render`/`opentake-media`，不在本 crate（ROADMAP Phase 5）。

## 移植铁律（本模块重点）

1. **serde 向后兼容是第一铁律**：所有序列化模型加 `#[serde(default)]` + `Option<T>`，保证**读旧工程不破坏**。新增字段必须有缺省值；缺失键降级而非报错。`media.json` 的 `version` 缺省按 1（结构体默认 2，但缺省回退 1），`generation-log.json` 的 `version` 默认与回退**都是 1**——二者不同，别混。
2. **读取容错分级**对齐上游：`project.json` 缺失 = 硬错（`fileReadCorruptFile` 等价）；`media.json` 在场则严格解析、缺失则空清单；`generation-log.json` 用 `try?` 等价的宽松解析，失败降级为 `None`。
3. **旧字段迁移逐位复刻**：`GenerationLogEntry` 无 `costCredits` 但有旧 `cost`（美元 float）时，`costCredits = ceil(cost * 100)`（Swift `.rounded(.up)`，向上取整，永不截断）；二者同在时 `costCredits` 优先。Transform 的旧 `x/y → centerX/centerY` 迁移在 domain 层处理。
4. **一切以整数帧为单位**：导出里 `secondsToFrame` 用截断 `Int(s*fps)` 而非四舍五入（[`fcpxml.rs`](../../../crates/opentake-project/src/fcpxml.rs) 的 `seconds_to_frame`）；`source_frames_consumed` 的 round 方向与上游一致。
5. **路径语义对齐 Foundation**：归档去重用**纯词法** `standardize`（不 stat、不解符号链接，两条指向同一文件的符号链接各拷一份）；扩展名提取用 Swift `URL.pathExtension` 规则而非 `Path::extension`（`foo. mp4`、`..mp4` 均判**无扩展名**）。
6. **日期是 Apple 参考日秒**：`created_at` 是 `f64`（Apple-reference-date 秒，`JSONEncoder` 默认 `Date` 编码），墙钟换算归上层。

---

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md) · 本模块目录：[INDEX.md](INDEX.md)
