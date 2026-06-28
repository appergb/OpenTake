# library-index — 全局素材库 + 索引调度内核 + 缓存键

> 上级：[INDEX.md](INDEX.md) · [OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：`library.rs`、`index_coordinator.rs`、`cache_key.rs`、`error.rs`。`library.rs` 上游无对应（OpenTake 新增 #37/#104）；`index_coordinator.rs` 对应 `SearchIndexCoordinator`（调度部分）。

---

## 职责

四个基础设施件：

1. `cache_key.rs` — **统一内容身份缓存键**（缩略图/波形/转写/嵌入四处共用）。
2. `library.rs` — **跨项目全局素材库**：内容寻址去重 + 原子 manifest。
3. `index_coordinator.rs` — **后台索引/转写调度的可移植内核**（判断 + 导出暂停；运行时在 core）。
4. `error.rs` — `MediaError`（见下「错误类型」）。

---

## 缓存键 `cache_key.rs`

```rust
pub const KEY_HEX_LEN: usize = 32;
pub fn file_identity_key(path: &Path, prefix_chars: usize) -> Option<String>;
pub fn identity_hex(path: &str, mtime_secs: f64, size: u64, prefix_chars: usize) -> String;  // 纯核
```

- 键 = `SHA256("<path>|<mtime_secs_f64>|<size>")` 小写 hex，取**前 16 字节 = 32 hex 字符**。文件不存在/读不到 mtime|size → `None`（对齐上游 `guard let … else return nil`）。
- 统一上游三处同构实现（`MediaVisualCache.diskCacheKey` 取 16 字节、`EmbeddingStore.key`/`TranscriptCache.key` 取 32 hex 字符——最终都是 32 hex/16 字节熵）。**与上游同机缓存目录可互读**。
- **逐字节对齐点（`swift_double`）**：mtime 用 Swift `Double.description` 渲染——它对整数秒保留 `.0`（`1000.0`），而 Rust f64 `Display` 打印 `1000`（丢 `.0`）。`identity_hex` 对整数值补 `.0`，使 hash 种子字节与上游一致（单测固定 `"/a/b.mp4|1000.0|42"` → `c428ca2d60590827149ac76ecc8f743f`）。其余分数渲染两者一致。

---

## 全局素材库 `library.rs`

```rust
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry { pub id: String /*SHA-256 hex*/, pub kind: String /*JSON "type"*/,
    pub category: Option<String>, pub favorited_at: f64, pub source: Option<String>, pub thumb: Option<String> }
pub struct FavoriteRequest<'a> { pub source: &'a Path, pub kind: &'a str, pub category: Option<String>,
    pub favorited_at: f64, pub thumb: Option<String> }
pub struct LibraryStore { /* root + write_lock: Mutex<()> */ }

impl LibraryStore {
    pub fn new(root) -> Self;  pub fn open_default() -> Result<Self>;
    pub fn favorite(&self, req: FavoriteRequest) -> Result<LibraryEntry>;
    pub fn entries(&self) -> Result<Vec<LibraryEntry>>;
    pub fn entries_in_category(&self, cat: Option<&str>) -> Result<Vec<LibraryEntry>>;
    pub fn remove(&self, id: &str) -> Result<bool>;
    pub fn set_category(&self, id, category: Option<String>) -> Result<Option<LibraryEntry>>;
    pub fn rename_category(&self, from: &str, to: Option<String>) -> Result<usize>;
    pub fn stored_path(&self, id: &str) -> Result<Option<PathBuf>>;
}
```

设计（[ROADMAP.md](../../architecture/ROADMAP.md) 注：#37 = 跨项目全局库，区别于 #49/#91 每项目媒体）：
- **内容寻址去重**：id = 文件内容的 SHA-256 hex（`hash_hex`）；同字节内容只存一份。
- **copy-on-favorite**：收藏时把文件复制进库（原文件删除不影响）。
- **原子 manifest**：先写 `library.json.tmp` 再 rename（崩溃不留破碎 JSON）。
- **写锁序列化**：进程内 `Mutex` 防并发 favorite/remove 丢条目；锁中毒时 `into_inner()` 恢复。
- 磁盘布局：`<root>/library.json` + `<root>/files/<hash><ext>`（保留扩展名）。
- `LibraryEntry` 全 `camelCase`（`favoritedAt`），`kind` 序列化为 `type`。

> **历史 follow-up（客观记录）**：`remove(id)` 删不存在的 id **不报错、返回 `Ok(false)`**；删磁盘文件用 `let _ = std::fs::remove_file(path)`——**best-effort 吞错**（manifest 条目已移除，残留磁盘文件无害但不上报失败）。`stored_path` 按 hash 匹配 stem 遍历 `files/`，理论上 hash+ext 唯一无碰撞，但依赖目录遍历顺序，属潜在脆弱性而非已知 bug。（MEMORY.md 记的 "library.rs:322" 行号已随改动漂移，实际吞错点在 `remove` 内的文件删除。）

---

## 索引调度内核 `index_coordinator.rs`

```rust
pub struct ExportPause(/* Arc<AtomicUsize> */);
impl ExportPause { pub fn new(); pub fn begin(&self); pub fn end(&self); pub fn is_active(&self) -> bool; }

pub struct WorkNeeded { pub visual: bool, pub transcript: bool }
impl WorkNeeded { pub fn any(&self) -> bool; }
pub fn work_needed(cache_root: &Path, asset: &MediaAsset, spec: &EmbedderSpec) -> WorkNeeded;
pub fn visual_share(work: WorkNeeded) -> f64;   // 两任务并行 0.5，单任务 1.0
pub struct IndexProgress { pub batch_total: usize, pub batch_completed: usize, pub current_fraction: f64 }
```

- **`ExportPause`**：跨窗口 export 活跃**引用计数**（`AtomicUsize`，`SeqCst`）。render 导出时 `begin`/`end`（`end` 饱和不低于 0），后台索引 worker 轮询 `is_active()` 为 true 时让路（对齐 `ExportPauseCounter`，[SPEC.md](SPEC.md) §0.7）。`MediaEngine::export_pause()` 暴露共享句柄。
- **`work_needed`**（对齐 `SearchIndexCoordinator` 的 `needsVisual`/`needsTranscript`，`schedule`）：
  - asset 生成中（`is_generating()`）→ 全 false。
  - `visual` = （视频或图片）且 embedding 索引不 current（调 `search::indexer::needs_index`）。
  - `transcript` = （音频 或 视频+有音轨）且无磁盘缓存转写（调 `transcribe::cache` 检查）。
- **`visual_share`**：视觉+转写并行时各占进度 0.5，否则 1.0（对齐 `visualShare`）。

> **架构边界（明确分层）**：本模块是 **kernel**——`work_needed`/`visual_share`/`ExportPause` 是无 tokio/async 的纯可移植函数，单测友好。**tokio worker 队列（enqueue/dequeue/retry）、并发转写+视觉、失败集合、2s 轮询等待、索引快照查询**全部 deferred 给 [opentake-core](../opentake-core/INDEX.md)（[SPEC.md](SPEC.md) §7.7）。

---

## 错误类型 `error.rs`

`MediaError`（`thiserror`）把上游多个错误枚举（`ImageVideoError`/`NormalizeError`/`TranscriptionError`/`DownloadError`/`VisualEmbedder.ModelError`/`EmbeddingStore.StoreError`）收敛为一个边界类型：`Io`/`Json`/`Ffmpeg`/`NoTrack(kind,path)`/`Decode`/`Encode`/`UnsupportedLocale`/`Transcribe`/`ModelInstall`/`Checksum`/`StoreCorrupt`/`BadModelOutput`/`Cancelled`/`Other(anyhow)`。`pub type Result<T> = std::result::Result<T, MediaError>`。内部传播用 `anyhow` 经 `Other` 透传；`opentake-domain` 零 I/O，本 crate 是第一层允许 I/O 的 crate。

---

## 完成状态
缓存键 / 素材库存储 / 调度内核 / 错误类型**均已实现并全测**。素材库 Tauri 命令层在 `src-tauri/src/library.rs`（7 命令，#106）；**素材库前端（#37-C/#56）未做**。调度 worker 运行时在 core。

## 测试
- `cache_key`：稳定小写、各分量变化、`swift_double` 整数补 `.0`、整数秒 hash 固定值、真文件读取、缺失文件 `None`。
- `library`（约 15 条）：copy+写 manifest、同内容去重、不同内容分条、分类过滤、首次空 manifest、remove 删条目+文件、序列化往返、camelCase+`type` 键、set_category、rename_category 批量、默认目录路径。
- `index_coordinator`（约 9 条）：引用计数 begin/end/饱和、clone 共享、生成中短路、视频+音轨双任务、静音视频仅视觉、音频仅转写、图片仅视觉、转写已缓存跳过、visual_share 0.5/1.0 分支。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 相关：[semantic-search.md](semantic-search.md)（`needs_index`/取消/导出让路）· [transcribe.md](transcribe.md)（转写缓存检查）· [opentake-core](../opentake-core/INDEX.md)（worker 运行时）
- 模块文档树：[../INDEX.md](../INDEX.md) · docs 总目录：[../../INDEX.md](../../INDEX.md)
- 源码根：`../../../crates/opentake-media/src/`
