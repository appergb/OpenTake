# bundle + archive — 工程包读写与自包含归档

> 上级：本模块目录 [INDEX.md](INDEX.md)

覆盖两个源文件：[`bundle.rs`](../../../crates/opentake-project/src/bundle.rs)（`.opentake` 包的内存句柄与读写）+ [`archive.rs`](../../../crates/opentake-project/src/archive.rs)（把分散素材收拢成自包含包）。二者共用 [`layout`](layout.md) 的文件名契约与 [`error`](#错误类型) 的错误类型。

---

## 一、bundle.rs — `.opentake` 包读写

### 职责

把一个 `.opentake` 目录在「内存 [`Project`] 句柄」与「磁盘三件 JSON + 缩略图」之间双向搬运。端口自上游 `VideoProject` 的持久化部分（`Project/VideoProject.swift` 的 `read` / `save` / `fileWrapper`），**去掉 AppKit 的 NSDocument / FileWrapper 机制**——包就是普通目录，按路径读写。

### 关键类型

- **`Project`** — 打开的工程句柄。字段：
  - `bundle_path: PathBuf` — 包目录绝对路径（`…/Name.opentake`）。
  - `timeline: Timeline` — `project.json` 的内容。
  - `manifest: MediaManifest` — `media.json`，缺失时为空清单。
  - `generation_log: Option<GenerationLog>` — `generation-log.json`，缺失或解析失败时 `None`。
  - `thumbnail: Option<Vec<u8>>` — 待下次 `save` 写入的 JPEG 字节；`None` 则不动磁盘上已有的 `thumbnail.jpg`。
  - **句柄不加载媒体**：`media/` 下的素材、`chat-sessions/` 对话、磁盘缩略图都留在盘上，不进内存。

### 关键函数与算法

- **`Project::new(path)`** — 构造一个空工程（`Timeline::new()` + 空清单），尚未落盘。
- **`Project::open(path)`** — 打开包，**读取容错分级**严格对齐上游 `read(from:)`：
  - 路径非目录 → `ProjectError::NotABundle`。
  - `project.json` 缺失 → `ProjectError::MissingTimeline`（上游抛 `fileReadCorruptFile`）；解析失败 → `ProjectError::Json`。
  - `media.json` **在场时严格**解析（失败即错），缺失则 `MediaManifest::new()`。
  - `generation-log.json` **宽松**解析：读失败或解析失败都降级为 `None`（上游 `try?`）。
- **`Project::save()` / `save_to(bundle)`** — 写盘。`save` 写回 `self.bundle_path`，`save_to` 写到指定目录（归档 staging 用，不改 `self`）。语义：
  - 必要时建目录；**总是**写 `project.json` + `media.json`；持有日志时写 `generation-log.json`；`thumbnail` 有值时写 `thumbnail.jpg`。
  - **从不**创建 / 删除 `media/` 与 `chat-sessions/`——它们由媒体层与 agent 层管理。
- **原子写 `write_bytes_atomic`** — 先写同目录临时文件（名 `.<file>.<pid>.<counter>.tmp`）再 `rename` 落位；失败清理临时文件。临时名唯一性靠 pid + 进程内原子计数器，避免拉入 RNG 依赖。这复刻了架构注「先组装内存快照再原子写盘」，崩溃不留半截文件。
- **`write_json_atomic`** — `serde_json::to_vec_pretty`（**pretty**，与上游 compact 的差异来源）后走原子写。

### 不变量与上游对齐

- 读取分级与上游 `read` **逐分支一致**：强制 / 严格 / 宽松三档，错误类型一一对应。
- 写出**字段 / 值级**兼容上游 `.palmier`，但**非字节级**（pretty vs compact）。
- `save` 的「只管 JSON + 缩略图，不碰素材 / 对话目录」边界，对应上游 fileWrapper 对 `media/` 取现有目录整体纳入、不重写素材的行为。
- 简化掉了上游的主线程快照时序约束（Swift `captureSaveSnapshot` 必须主线程先跑），Rust 同步序列化即可，但保留「快照 + 原子落位」精神。

---

## 二、archive.rs — 自包含归档

### 职责

写出一个**自包含** `.opentake` 包：把每个**可解析**的媒体引用拷进新包的 `media/`，并把清单里该条的 `source` 改写为包相对路径 `media/<file>`；解析不到源文件的悬空引用**原样保留**并计入 `missing`。端口自上游 `PalmierProjectExporter.export`（`Export/PalmierProjectExporter.swift`），行为 **1:1 对拍**。

### 关键类型

- **`ArchiveReport`** — 归档结果（对拍上游 `Report`）：`collected`（原 external 现已内联的 id 列表）、`copied_internal`（已是 `.project` 的拷贝数）、`missing: Vec<MissingMedia>`、`total_bytes`（拷入新包的总字节）。
- **`MissingMedia`** — `{ id, name }`，源文件找不到的条目。

### 关键函数与算法

- **`archive(timeline, manifest, generation_log, source_bundle, dest_bundle) -> Result<ArchiveReport>`** — 纯函数入口（不依赖 `Project` 句柄）：
  1. **remove-then-land**：`dest_bundle` 已存在则先 `remove_dir_all`，复刻上游"原子替换"，避免残留旧 `media/` / `thumbnail.jpg`。
  2. 建 `dest/media/`，逐条 manifest entry：解析源路径（`.external` → 绝对路径；`.project` → 拼 `source_bundle`）→ 不存在则记 `missing` 且保留原 entry → 存在则按**去重键**决定拷贝或复用。
  3. 拷贝后把 entry 的 `source` 改写为 `MediaSource::Project { relative_path: "media/<file>" }`，external 来源额外计入 `collected`。
  4. 写 `project.json` / `media.json`（用改写后的清单）/ `generation-log.json`；最后从 `source_bundle` 搬运 `thumbnail.jpg` 与整个 `chat-sessions/`（present-only）。
- **去重 `standardize`（关键坑）** — 用 `source` 的**纯词法**标准化路径作 dedup key，精确复刻 Swift `URL.standardizedFileURL.path`：折叠 `.`、词法解析 `..`（根上的 `..` 被吸收，相对路径开头的 `..` 保留）、合并重复分隔符，**全程不碰文件系统、不解符号链接**。因此**两条指向同一物理文件的不同符号链接会各拷一份**（key 不同），与上游一致——有专门的 unix 对拍测试 `two_symlinks_to_one_file_are_not_deduped`。
- **命名 `filename_for`** — `.project` 保留原文件名；`.external` 变 `import-<id 前 8 字符>.<ext>`。
- **扩展名 `path_extension`（关键坑）** — 复刻 Swift `URL.pathExtension` / `NSString.pathExtension` 语义而非 Rust `Path::extension`：仅当①有 `.` ②末段非空 ③末段不含 ASCII 空格（**但允许** tab/换行）④`.` 前缀含至少一个非 `.` 字符，才算扩展名。故 `foo. mp4`、`..mp4`、`.hidden`、`trailing.` 均判**无扩展名**。`split_extension` 用同一规则做 collision 重命名的 base/ext 切分。
- **collision `unique_path`** — 重名则追加 `-1`、`-2`…（扩展名保留）。

### 不变量与上游对齐

- 去重、命名、扩展名、collision、remove-then-land、附属搬运全部 **1:1 对拍** Foundation 行为（源文件 doc 注明每一处与 upstream 的对应点，测试覆盖符号链接 / 异常文件名 / 美元迁移等边角）。
- 归档只做**字节拷贝**（`fs::copy`），不解码、不转码——媒体感知归 `opentake-media`。

---

## 错误类型

[`error.rs`](../../../crates/opentake-project/src/error.rs) 定义 `ProjectError`（`thiserror`）+ `Result<T>` 别名，本 crate 全部 IO / 序列化失败归一到它：

- `MissingTimeline { file, bundle }` — 缺强制 `project.json`（对拍上游 `fileReadCorruptFile`）。
- `NotABundle(PathBuf)` — 给的路径不是目录。
- `Io { path, source }` — 文件系统操作失败，`path` 记录涉事路径。
- `Json { file, source }` — 某组件 JSON 解析 / 序列化失败，`file` 记录是哪个组件。

边界层（`src-tauri`）按代码风格把它转成 `Err(String)` 给前端。

## 与其他子系统的关系

- 依赖 [`layout`](layout.md) 取所有包内文件 / 目录路径。
- `bundle` 与 `archive` 都序列化 [`gen-log`](gen-log.md) 的 `GenerationLog`；`archive` 改写的 `MediaSource` 来自 domain。
- `archive` 的源解析与 [`fcpxml-export`](fcpxml-export.md) 用的 `MediaResolver` 是同一套 `.external`/`.project` 定位规则（前者直接拼路径，后者经 domain 的 resolver）。
- 被 `opentake-core` 的会话层调用以开 / 存工程；归档 / 导出经 `src-tauri` 命令暴露。

---

> 上级：本模块目录 [INDEX.md](INDEX.md)
