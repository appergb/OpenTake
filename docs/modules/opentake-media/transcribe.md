# transcribe — 转写：whisper 后端 + locale + 缓存 + 转写内搜索

> 上级：[INDEX.md](INDEX.md) · [OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：`transcribe/{mod,whisper,locale,cache,search}.rs`。对应上游 `Transcription` / `TranscriptCache` / `TranscriptSearch`（macOS Speech → whisper.cpp）。

---

## 职责

把音视频转成带时间戳的文字，并提供缓存与转写内关键词检索：

- `mod.rs` — `Transcriber` trait（后端协议）+ 数据模型 + `transcribe_file` 编排 + 时间码回移。
- `whisper.rs` — whisper.cpp 后端（`whisper-backend` feature）。
- `locale.rs` — BCP-47 语言/区域匹配（纯逻辑）。
- `cache.rs` — 内存 LRU + 磁盘 JSON 双层缓存 + range 过滤。
- `search.rs` — AND 子串 + NFD 折叠的转写内搜索（对应 MCP `search_media` 口语侧）。

时间单位全程**秒（`f64`）**（[OVERVIEW.md](OVERVIEW.md) §6 第 1 条）。

---

## 数据模型与编排 `mod.rs`

```rust
pub struct TranscriptionWord    { pub text: String, pub start: Option<f64>, pub end: Option<f64> }
pub struct TranscriptionSegment { pub text: String, pub start: f64, pub end: f64 }
pub struct TranscriptionResult  { pub text: String, pub language: Option<String>,
                                  pub words: Vec<TranscriptionWord>, pub segments: Vec<TranscriptionSegment> }
pub struct TranscribeOptions    { pub censor_profanity: bool, pub preferred_language: Option<String>,
                                  pub source_range: Option<(f64,f64)> }

pub trait Transcriber: Send + Sync {
    fn transcribe_pcm(&self, pcm: &PcmBuffer, opts: &TranscribeOptions) -> Result<TranscriptionResult>;
}

pub fn whisper_pcm_spec() -> PcmSpec;   // 16000 / 1 / F32
pub fn transcribe_file(path, transcriber: &dyn Transcriber, opts: &TranscribeOptions) -> Result<TranscriptionResult>;
impl TranscriptionResult { pub fn offsetting(self, offset: f64) -> Self; }
```

- **`transcribe_file`** 编排：`extract_pcm(path, whisper_pcm_spec(), opts.source_range)` → `transcriber.transcribe_pcm` → 若有 `source_range` 则 `offsetting(lower)` 把时间码移回源时间 → 返回。
- **`offsetting`** 给所有 word/segment 的 start/end 加偏移（窗口转写时把局部时间码还原为源绝对时间，对齐 `Transcription.swift` 的 `offsetting(by:)`）。

---

## whisper 后端 `whisper.rs`（feature `whisper-backend`）

整个文件在 `#[cfg(feature = "whisper-backend")]` 之后——**默认 build 不链接 whisper.cpp**，离线可测。

```rust
pub struct WhisperTranscriber { /* … */ }
impl WhisperTranscriber {
    pub fn from_model_path(path) -> Result<Self>;   // 加载 ggml/gguf 模型
    pub fn with_threads(n) -> Self;                 // 默认 CPU 核心数
}
impl Transcriber for WhisperTranscriber { /* transcribe_pcm */ }
```

要点：
- 输入 16k 单声道 f32（来自 `extract_pcm`）。
- whisper params：`set_token_timestamps(true)`（词级时标）、`set_suppress_blank(true)`、`set_translate(false)`、`set_print_special(false)`。
- **厘秒→秒**：whisper 输出厘秒（1/100s），`cs_to_secs(cs) = cs / 100.0`。
- 逐 segment 抽 `text.trim()` + t0/t1；逐 token 抽 word（start/end 可 `None`）。
- **特殊 token 过滤**：跳过空、`[_…]` 形式、`<|…|>` 形式。

---

## locale 匹配 `locale.rs`（纯逻辑）

BCP-47 解析 + 选择目标 locale：
- 语言短码 = 首个 `-`/`_` 前的部分小写（`en-US`→`en`，`zh-Hans-CN`→`zh`）。
- 区域短码 = 首个区域格式段（script 忽略）：2 字母 → 大写（`US`），3 位数字 → 原样（`419`），其余 `None`（`zh-Hans-CN`→`CN`，`es-419`→`419`）。
- `match_locale(candidates, supported)`：逐候选找 supported 中**同语言**条目，优先区域匹配、否则取首个同语言，**返回 supported 中的实际 ID**。
- `best_supported_locale(preferred, current, supported)`：候选 = `preferred + [current]` 后调 `match_locale`。

例：`en-US` vs `[en-GB, en-US]` → `en-US`；`en` → `en-GB`（首个同语言）；`en-AU` → `en-GB`（AU 不支持回退）。

---

## 缓存 `cache.rs`

```rust
pub struct TranscriptCache { /* 内存 LRU + cache_root */ }
impl TranscriptCache {
    pub fn transcript(&self, path, is_video, range: Option<(f64,f64)>, transcriber: &dyn Transcriber) -> Result<TranscriptionResult>;
}
```

- **只缓存整文件转写**；窗口请求（`range`）→ 对缓存的整文件做 `filter`（不单独转写、不缓存窗口）。
- 目录：`<cache_root>/Transcripts/<key>.json`（`key = file_identity_key(path,32)`，与上游互读）。
- **内存 LRU 容量 = 4（`MEMORY_MAX`）**；满时**整体清空**（对齐上游 wholesale clear 行为）。
- 流程：无 range → 内存命中 / 磁盘命中（提升进内存）/ miss 则 `transcribe_file` + 内存&磁盘 store；有 range → 整文件已缓存则 `filter`，否则 `transcribe_file(range)` 不缓存。
- `filter(transcript, (lower, upper))`（半开 `[lower, upper)`，对齐 `TranscriptCache.swift:29-39`）：segment 保留 `end > lower && start < upper`；word 须 start/end 都 `Some` 且同条件；`text` 由存活段空格拼接。

---

## 转写内搜索 `search.rs`

```rust
pub struct SpokenHit { pub asset_id: String, pub start: f64, pub end: f64, pub text: String }
pub fn search(cache_root, query, assets: &[(String, PathBuf)], limit) -> Vec<SpokenHit>;
```

- `terms(query)`：按空格切，逐词 `trim` 掉 ASCII 标点/非字母数字，过滤空词（内部标点保留：`a-b` 不拆）。
- `fold(s)`：**NFD 分解 → 剥离 combining marks（`0x0300..0x036F` 等区段）→ 小写**——大小写 + 变音不敏感（`café` ≡ `cafe`）。
- `matches(text, terms)`：`terms` 非空且 `fold(text)` 包含**每个** `fold(term)`（**AND 逻辑**）。
- `search`：对每个 `(asset_id, path)` 读磁盘缓存转写，逐 segment `matches` 命中即 push `SpokenHit`，达 `limit` 提前返回。

---

## 完成状态
数据模型 / locale / 缓存 / 搜索（纯逻辑）**已实现并全测**；whisper 真实后端在 feature 后，模型托管与端到端验证属 [ROADMAP.md](../../architecture/ROADMAP.md) Phase 8。与 ADVANCED-FEATURES 的「智能剪口播」（词级转写 + 静音检测 → Rust 内算 ripple）配套，对应 MCP `get_transcript` / `tighten_silences`（静音侧见 [analysis.md](analysis.md)）。

## 测试
`cs_to_secs`（厘秒换算）、locale（多组语言/区域/回退）、cache（MockTranscriber 命中/磁盘预种/filter 半开边界，无 ffmpeg 依赖）、search（terms 切分/fold 变音/AND/limit 截断）。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 相关：[decode.md](decode.md)（`extract_pcm` 16k mono）· [semantic-search.md](semantic-search.md)（`search_media` 视觉侧）· [analysis.md](analysis.md)
- 模块文档树：[../INDEX.md](../INDEX.md) · docs 总目录：[../../INDEX.md](../../INDEX.md)
- 源码根：`../../../crates/opentake-media/src/`
