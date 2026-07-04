# whisper-rs 转写(word/segment 时间戳,TranscriptionResult 模型)

对应 `Transcription/{Transcription,TranscriptCache,TranscriptSearch}.swift`。上游用 macOS 26 `SpeechAnalyzer`/`SpeechTranscriber`;跨平台换 **whisper-rs**(`docs/_analysis/02` 表 L79、MODULE-PORT-MAP L1211)。**上层算法(数据模型 / offsetting / 缓存 / filter / 关键词搜索 / locale 匹配)逐行复刻,只换 ASR 后端。**

## 6.1 数据模型(逐行 1:1 port)

`Transcription.swift:5-39`:
```rust
// transcribe/mod.rs
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TranscriptionWord { pub text: String, pub start: Option<f64>, pub end: Option<f64> }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TranscriptionSegment { pub text: String, pub start: f64, pub end: f64 }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: Option<String>,
    pub words: Vec<TranscriptionWord>,
    pub segments: Vec<TranscriptionSegment>,
}
impl TranscriptionResult {
    /// 把时间码移回源时间(转写截取区间后)。offset==0 原样返回。
    pub fn offsetting(&self, offset: f64) -> TranscriptionResult; // Transcription.swift:26-38
}
```
- `offsetting`:`offset==0` 直接 clone;否则 words 的 `start/end` 各 `+offset`(None 保持 None),segments 的 `start/end` `+offset`(`:26-38`)。
- JSON 字段保持同名(`text/language/words/segments/start/end`),与上游 `TranscriptCache` 磁盘 JSON 互读。

## 6.2 转写后端 trait + whisper 实现

```rust
// transcribe/mod.rs
pub struct TranscribeOptions {
    pub censor_profanity: bool,        // 上游 etiquetteReplacements(:121);whisper 无内建 → 见下
    pub preferred_language: Option<String>, // bcp47/iso639;whisper language 参数
    pub source_range: Option<(f64, f64)>,   // 绝对秒;先抽 PCM 再 offsetting
}
pub trait Transcriber: Send + Sync {
    fn transcribe_pcm(&self, pcm: &PcmBuffer, opts: &TranscribeOptions) -> Result<TranscriptionResult>;
}

// transcribe/whisper.rs
pub struct WhisperTranscriber { ctx: whisper_rs::WhisperContext /* 模型路径 */ }
impl Transcriber for WhisperTranscriber { /* full params + token timestamps */ }
```
顶层便捷函数(对齐 `Transcription.transcribe*`,`:65-200`):
```rust
pub fn transcribe_file(path: &Path, t: &dyn Transcriber, opts: &TranscribeOptions) -> Result<TranscriptionResult>;
pub fn transcribe_video_audio(path: &Path, t: &dyn Transcriber, opts: &TranscribeOptions) -> Result<TranscriptionResult>;
```
实现要点:
- **音频抽取**:`extract_pcm(path, &PcmSpec{16000,1,F32}, opts.source_range)`(§2.3);whisper 吃 16k mono f32。对齐 `extractAudioTrack`(`Transcription.swift:203-280`)。
- **range → offsetting**:有 `source_range` 时,抽截取段 → 转写 → `result.offsetting(lower)`(`Transcription.swift:65-70/92-98`)。
- **word/segment 时间戳**:whisper-rs 开启 `set_token_timestamps(true)`;遍历 segments 取 `start_t/end_t`(厘秒→秒);遍历 tokens 取 token 级时间组装 `TranscriptionWord`(过滤特殊 token / 空白)。对齐 `decodeResults`(`:284-322`):每个 segment = 一条 `TranscriptionSegment`(text trim + start/end),每个非空 token run = 一条 `TranscriptionWord`(trim + 可空 start/end)。`fullText` = 所有 segment 文本拼接后 trim(`:317`)。
- **language**:whisper 自动检测或用 `preferred_language`;回填 `result.language`(对齐 `locale.identifier(.bcp47)`,`:320`)。
- **profanity**:whisper 无 `etiquetteReplacements`;`censor_profanity=true` 时用**可选脏词表后处理**(MODULE-PORT-MAP L1211 (3)),默认关闭。
- **不要求逐 token 对齐 Apple**:下游字幕「词中点落区间计数」「短语重叠归属」只需秒值精度(MODULE-PORT-MAP L1211 (3) 末句)。

> ⚠️ whisper 词级时间戳精度低于 Apple `audioTimeRange`;`TranscriptionWord.start/end` 为 `Option` 已容许缺失。模型权重(ggml/gguf,如 `base`/`small` 多语种)由 §5.9 同款下载器或单独 catalog 管理(记入 T8.0)。

## 6.3 转写缓存 `TranscriptCache`(内存 LRU=4 + 磁盘 JSON + filter)

`Transcription/TranscriptCache.swift`。上游是 actor;Rust 用 `tokio::sync::Mutex` 包内存表 + 同步磁盘读路径。

```rust
// transcribe/cache.rs
pub struct TranscriptCache { /* memory: HashMap<String, TranscriptionResult>(max 4) */ }
impl TranscriptCache {
    pub async fn transcript(&self, path: &Path, is_video: bool, range: Option<(f64,f64)>,
        t: &dyn Transcriber) -> Result<TranscriptionResult>;

    /// 半开重叠过滤(段 + 词),段文本以空格 join 为整体 text。
    pub fn filter(r: &TranscriptionResult, range: (f64,f64)) -> TranscriptionResult; // :29-39

    pub fn has_cached_on_disk(cache_root: &Path, path: &Path) -> bool;   // :70-73
    pub fn cached_on_disk(cache_root: &Path, path: &Path) -> Option<TranscriptionResult>; // :76-80
}
```
逐项(`TranscriptCache.swift`):
- **只缓存全量转写**;窗口请求通过 `filter` 全量缓存得到(`:4-5/12-27`)。命中(内存或磁盘)且有 range → 返回 `filter(full, range)`;否则若有 range 直接转写该 range(不缓存,`:17-21`);否则全量转写并缓存(`:22-26`)。
- **filter**:段 `end > lower && start < upper`;词需 `start/end` 非空且 `end > lower && start < upper`;`text = segments.map(text).join(" ")`(`:29-39`)。
- **内存 LRU**:`max=4`,满则 `removeAll()`(粗暴清空,逐字照搬 `:57-60`)。
- **磁盘**:`<cache_root>/Transcripts/<key>.json`,key=`file_identity_key(path,32)`(`:62-88`)。`cached_on_disk`/`has_cached_on_disk` 同步读(供索引调度器判断,§7.7)。

## 6.4 关键词搜索 `TranscriptSearch`(口语搜索,纯函数)

`Transcription/TranscriptSearch.swift` + `MODULE-PORT-MAP` L684/L1211 (1)。

```rust
// transcribe/search.rs
#[derive(Clone, PartialEq, Debug)]
pub struct SpokenHit { pub asset_id: String, pub start: f64, pub end: f64, pub text: String }

/// 在磁盘缓存转写中,对每素材的 segments 做「所有词项 AND 子串、大小写/变音不敏感」命中。
pub fn search(cache_root: &Path, query: &str, assets: &[(String, PathBuf)], limit: usize) -> Vec<SpokenHit>;
pub fn terms(query: &str) -> Vec<String>;          // 分词 + 去边缘标点(:27-32)
pub fn matches(text: &str, terms: &[String]) -> bool; // 全词 AND 子串(:34-36)
```
- `terms`:按空白分词,每词去**边缘**标点(`"budget," → "budget"`),去空(`:27-32`)。
- `matches`:`terms.all(|t| text.contains_ci_diacritic_insensitive(t))`(`:34-36`)。**变音不敏感**用 `unicode-normalization` NFD 去组合标记 + 大小写折叠后 `contains`(MODULE-PORT-MAP L1211 (1))。
- 遍历素材:读 `cached_on_disk`,对每条 `segment` 命中即收 `SpokenHit{asset_id,start,end,text}`,达 `limit(20)` 立即返回(`:12-25`)。

## 6.5 locale 匹配(纯逻辑,照搬)

`Transcription.swift:72-90`(`supportedLocales`/`bestSupportedLocale`/`matchLocale`):
```rust
// transcribe/locale.rs
/// 语言码优先、地区次之。candidates 用 [preferred] 或 [系统语言…, 当前]。
pub fn match_locale(candidates: &[&str], supported: &[&str]) -> Option<String>; // :81-90
pub fn best_supported_locale(supported: &[&str]) -> Option<String>; // 系统首选 + current(:76-79)
```
- whisper 支持语言集近乎固定(不像 Apple 需运行时查询),`supported` 可来自 whisper 静态语言表;`match_locale` 逻辑(同语言码下选同地区否则首个)逐行照搬(`:82-89`)。用于把用户/系统语言映射到 whisper `language` 参数(MODULE-PORT-MAP L1211 (3))。
