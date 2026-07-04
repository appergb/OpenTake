# layout — 工程包文件名契约

> 上级：本模块目录 [INDEX.md](INDEX.md)

源文件：[`layout.rs`](../../../crates/opentake-project/src/layout.rs)。

## 职责

集中定义 `.opentake` 目录包内**所有固定文件 / 目录名**及其路径拼接函数，是整个工程格式的单一契约源。[`bundle`](bundle-archive.md) 与 [`archive`](bundle-archive.md) 都从这里取路径，不在别处硬编码文件名。端口自上游 `enum Project` 的命名空间常量（`Utilities/Constants.swift`）。

## 关键常量

| 常量 | 值 | 含义 |
|---|---|---|
| `BUNDLE_EXTENSION` | `"opentake"` | 工程目录扩展名（不含点）。上游为 `"palmier"`。 |
| `TIMELINE_FILE` | `"project.json"` | 序列化的 `Timeline`（强制存在）。 |
| `MANIFEST_FILE` | `"media.json"` | 序列化的 `MediaManifest`。 |
| `GENERATION_LOG_FILE` | `"generation-log.json"` | 序列化的 `GenerationLog`（可选）。 |
| `THUMBNAIL_FILE` | `"thumbnail.jpg"` | JPEG 封面（可选）。 |
| `MEDIA_DIR` | `"media"` | 工程内素材目录；`.project` 相对路径按约定指向此处。 |
| `CHAT_SESSIONS_DIR` | `"chat-sessions"` | agent 对话目录，每会话一个 `<session>.json`。 |

## 关键函数

一组把包根 `&Path` 拼成绝对路径的纯函数，无 IO：

- `timeline_path(bundle)` / `manifest_path(bundle)` / `generation_log_path(bundle)` / `thumbnail_path(bundle)` — 各组件文件路径。
- `media_dir(bundle)` / `chat_sessions_dir(bundle)` — 两个子目录路径。

## 不变量与上游对齐

- 文件名与上游 `enum Project` **逐项一致**，使上游导出的 `project.json` / `media.json` 能被本 crate 直接打开（字段 / 值级兼容，见 [OVERVIEW](OVERVIEW.md)）。
- **唯一刻意差异**：对话目录从上游的 `chat/`（`ChatSessionStore.dirName`）改名为 `chat-sessions/`（依 `docs/architecture/ARCHITECTURE.md` §9）。源码注明：若将来需迁移老 `.palmier` 包，readers 应把 `chat/` 当 legacy 回退——但当前**未实现**该迁移。
- 这里**不含**上游 `Constants.swift` 里的 `typeIdentifier`（`io.palmier.project` UTI）、`storageDirectory`（`~/Documents/Palmier Pro`）、`registryFilename` 等：UTI 是 macOS 文件包机制，OpenTake 用普通目录无需；存储目录 / 注册表属"新建落盘 + 最近工程"范畴，归 `opentake-core` / `src-tauri`（见 [PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) P0-1，**计划中**）。

## 与其他子系统的关系

- 被 [`bundle`](bundle-archive.md)（开 / 存工程）与 [`archive`](bundle-archive.md)（拷贝目标布局、搬运附属）调用。
- `MEDIA_DIR` 的"`.project` 相对路径指向 `media/`"约定，与 [`fcpxml-export`](fcpxml-export.md) / `archive` 里 `MediaSource::Project` 的解析规则配套。

---

> 上级：本模块目录 [INDEX.md](INDEX.md)
