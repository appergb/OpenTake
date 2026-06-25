# ort + SigLIP2 + tokenizers 视觉/口语搜索

> 「口语搜索」= 转写关键词检索(§6.4,`TranscriptSearch`)。本节聚焦**视觉语义搜索**(SigLIP2 双编码器),完整复刻 `Search/` 子树。模型:`siglip2-base-patch16-256`,dim=768,imageSize=256,contextLength=64(`SearchIndexConfig.swift:22-45`)。

## 5.1 双编码器 trait + Spec

```rust
// search/embedder.rs
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbedderSpec {       // = VisualEmbedder.Spec(VisualEmbedder.swift:7)
    pub model: String, pub version: i32,
    pub embedding_dim: usize,   // 768
    pub image_size: u32,        // 256
    pub context_length: usize,  // 64
    #[serde(default)] pub normalized: bool, // 模型图内是否已 L2 归一化(见 §0.8)
}

pub trait Embedder: Send + Sync {
    fn spec(&self) -> &EmbedderSpec;
    fn encode_image(&self, frame: &RgbaFrame) -> Result<Vec<f32>>; // 长度 = embedding_dim
    fn encode_text(&self, text: &str) -> Result<Vec<f32>>;
}
```
对齐 `VisualEmbedder.encode(image:)`/`encode(text:)`(`VisualEmbedder.swift:37-51`)与输出提取 `vector(from:dim:)`(`:53-61`,断言 `count == dim`,否则 `BadModelOutput`)。

## 5.2 图像预处理(squash-resize、黑底、256²)— 逐字复刻

`VisualEmbedder.pixelBuffer`(`VisualEmbedder.swift:63-87`)+ `MODULE-PORT-MAP` L858:
1. 目标 256×256(`image_size²`)。
2. **黑底**:先用黑(`gray:0, alpha:1`)填满整张——因为缓冲内存复用未清零,带 alpha 源必须在黑底上混合(`VisualEmbedder.swift:81-83`)。
3. **squash-resize**:直接拉伸到正方形,**不裁剪、不保宽高比**(`:84-85`,注释 "SigLIP preprocessing squash-resizes to a square (no aspect crop)")。
4. 像素布局上游是 BGRA premultipliedFirst + byteOrder32Little + sRGB;Rust 喂模型用 RGB f32 张量(NCHW),按 SigLIP 预处理的 mean/std 归一(来自模型预处理配置,通常 `[0.5,0.5,0.5]`/`[0.5,0.5,0.5]`,实施时以导出模型附带的 `preprocessor_config.json` 为准)。

```rust
// 预处理:RgbaFrame -> ndarray::Array4<f32> (1,3,256,256)
fn preprocess_image(frame: &RgbaFrame, size: u32, mean: [f32;3], std: [f32;3]) -> ndarray::Array4<f32>;
```
- squash-resize 用 `image::imageops::resize(.., Triangle)` 到精确 `size×size`(忽略宽高比),对齐 swscale squash(`docs/_analysis/02` L75 / MODULE-PORT-MAP L858「FFmpeg/swscale 直接 scale 到 256x256,忽略宽高比」)。
- 带 alpha 源:先 over 黑底合成再丢 alpha。

## 5.3 文本 tokenize(SigLIP,定长 64,右填 0)

`TextTokenizer`(`TextTokenizer.swift:4-24`):`AutoTokenizer.from(modelFolder:)`,`encode` → 截断到 `contextLength` → 右填 `padToken=0` 到定长(无 attention mask,匹配 Python 参考)。

```rust
// search/tokenizer.rs
pub struct SiglipTokenizer { inner: tokenizers::Tokenizer, context_length: usize }
impl SiglipTokenizer {
    pub fn from_folder(folder: &Path, context_length: usize) -> Result<Self>; // 读 tokenizer.json
    /// 截断到 context_length,右填 0 到定长 context_length。返回 i64(ort)/i32 视后端。
    pub fn tokenize(&self, text: &str) -> Vec<i64>;
}
```
- HF `tokenizers` crate 与 swift-transformers 同源(`docs/_analysis/02` 表 L82 / MODULE-PORT-MAP L881「tokenizers crate(HF Rust 原生,与 swift-transformers 同源)」)。
- 关闭 padding/truncation 的自动行为,手动 `prefix(64)` + 右填 0,逐字对齐 `TextTokenizer.swift:18-22`。

## 5.4 视觉去重抽帧 `FrameSampler`(luma 8×8 + 镜头边界 + 覆盖下限)

`Search/Indexing/FrameSampler.swift` + `MODULE-PORT-MAP` L854。**纯算法 + ffmpeg 解帧**。

```rust
// search/frame_sampler.rs
pub const SAMPLER_VERSION: i32 = 1; // FrameSampler.swift:8

pub struct SamplerOptions {        // FrameSampler.Options(:10-16)
    pub candidate_interval: f64,   // 2.0
    pub coverage_floor: f64,       // 8.0
    pub promote_diff: f32,         // 12.0
    pub max_size: (u32, u32),      // (512,512)
    pub high_res_edge: u32,        // 3000
}
impl Default for SamplerOptions { /* 上游默认值 */ }

pub struct SampledFrame { pub time_secs: f64, pub image: RgbaFrame, pub is_new_shot: bool }

/// 流式产出视觉上不同的帧。FFmpeg 解帧替代 AVAssetImageGenerator。
pub fn sample_frames(path: &Path, duration_secs: f64, opts: &SamplerOptions)
    -> Result<impl Iterator<Item = Result<SampledFrame>>>;
```
算法(逐步对齐 `FrameSampler.sample`,`:40-90`):
1. 取首条视频流;若 `max(|w|,|h|) ≥ high_res_edge(3000)` 则 `interval *= 2`(2.0→4.0)(`:48-52`)。
2. 候选时间:`stride(from: interval/2, to: duration, by: interval)`(严格 `< duration`);为空则 `[duration/2]`(`:62-64`)。
3. 解帧:`max_size=512²`、`apply_rotation=true`、tolerance `max(interval/2, 1.0)`(`:54-60`)。
4. 每成功帧:`t = actual_secs`;丢 `t ≤ lastTime`(去重,`:74`);算 8×8 luma grid;有上一 grid → `is_new_shot = meanDiff > promote_diff(12)`,否则首帧 `is_new_shot=true`(`:78-84`);更新 `lastGrid`(**用所有解码帧更新**)。
5. 保留:`is_new_shot || t - lastKeptTime ≥ coverage_floor(8.0)`;满足则 `lastKeptTime=t` 并产出(`:86-88`)。注意 **luma 用所有帧更新,但 lastKeptTime 只在被保留时推进**(`MODULE-PORT-MAP` L854 末句)。

```rust
// LumaGrid(FrameSampler.swift:94-117)
pub const LUMA_CELLS: usize = 8;
/// 8×8 下采样,每格 Rec.601 luma = 0.299R + 0.587G + 0.114B(对 sRGB 字节)。
pub fn luma_grid(frame: &RgbaFrame) -> [f32; 64];
pub fn luma_mean_diff(a: &[f32;64], b: &[f32;64]) -> f32; // L1 平均差
```
- 系数 `.299/.587/.114` 逐字照搬(`:108`);8×8 下采样用高质量插值(`interpolationQuality=.high`,`:105`)。`meanDiff` = `Σ|a-b| / 64`(`:112-116`)。

## 5.5 索引器 `VisualIndexer`(帧→embedding→store,幂等)

`Search/Indexing/VisualIndexer.swift` + `MODULE-PORT-MAP` L856。

```rust
// search/indexer.rs
pub fn needs_index(path: &Path, spec: &EmbedderSpec) -> bool;

pub fn index_video(path: &Path, duration_secs: f64, embedder: &dyn Embedder,
    opts: &SamplerOptions, on_progress: Option<&dyn Fn(f64)>,
    cancel: &CancelToken) -> Result<()>;

pub fn index_image(path: &Path, embedder: &dyn Embedder, cancel: &CancelToken) -> Result<()>;
```
视频累积算法(`VisualIndexer.index`,`:15-51` + `MODULE-PORT-MAP` L856):
- 维护 `shot_starts: Vec<f64>`。每遇 `is_new_shot`:`push(if empty {0.0} else {frame.time})` —— **第一个镜头起点强制 0**(无论首帧实际时间)(`:34-36`)。
- 每帧:`vectors += encode_image(frame)`;`times.push(t)`;`shot_indices.push(shot_starts.len()-1)`(`:37-39`)。
- Row:`shotStart = shot_starts[shot]`;`shotEnd = if shot+1 < len {shot_starts[shot+1]} else {duration}`(`:43-49`)。
- 进度:`min(t/duration, 1)`(`:40`)。
- **导出让路 + 取消**:每帧前 `cancel.check()?` 与 `wait_while_export_active()`(`:32-33`,见 §7.7)。
图像(`indexImage`,`:54-67`):解码到 512 长边缩略图(`:69-77`)→ 单 embedding,`Row{time:0, shotStart:0, shotEnd:0}`(零长 shot)。
保存:构造 `Header{model,modelVersion,samplerVersion,dim,count}` 写 `EmbeddingStore`(`:79-86`)。

## 5.6 嵌入存储 `EmbeddingStore`(PALMEMB1 二进制,f16 落盘 / f32 内存)— 逐字节复刻

`Search/Indexing/EmbeddingStore.swift` + `MODULE-PORT-MAP` L861/L924。**精确格式,可与上游互读**。

```rust
// search/embed_store.rs
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Clone)]
pub struct Header { pub model: String, pub model_version: i32, pub sampler_version: i32,
                    pub dim: usize, pub count: usize }   // JSON 字段 camelCase
pub struct Row { pub time: f64, pub shot_start: f64, pub shot_end: f64 }
pub struct AssetIndex { pub header: Header, pub rows: Vec<Row>, pub vectors: Vec<f32> } // count*dim, f32

pub const MAGIC: &[u8;8] = b"PALMEMB1";

pub fn key(path: &Path) -> Option<String>;             // file_identity_key(path, 32)
pub fn header(cache_root: &Path, key: &str) -> Option<Header>;
pub fn is_current(cache_root: &Path, key: &str, model: &str, mv: i32, sv: i32) -> bool;
pub fn load(cache_root: &Path, key: &str) -> Result<AssetIndex>;
pub fn save(cache_root: &Path, key: &str, header: &Header, rows: &[Row], vectors: &[f32]) -> Result<()>;
pub fn clear_all(cache_root: &Path) -> Result<()>;
```
布局(`EmbeddingStore.swift:30/63-115` + `MODULE-PORT-MAP` L861):
```
magic "PALMEMB1" (8 bytes ASCII)
u32 headerLen     (4 bytes, little-endian)
JSON(Header)      (headerLen bytes)
count 行,每行 rowBytes = 3*8 + dim*2 = 24 + dim*2:
    f64 time      (8 bytes LE)
    f64 shotStart (8 bytes LE)
    f64 shotEnd   (8 bytes LE)
    dim × f16     (每个 2 bytes LE)        # half crate f16→f32 读 / f32→f16 写
```
- `dim=768` ⇒ `rowBytes = 24 + 1536 = 1560`(`MODULE-PORT-MAP` L861)。
- **严格校验**:`total == 8 + 4 + headerLen + count*rowBytes`,否则 `StoreCorrupt`(`EmbeddingStore.swift:69/74`)。
- 全部 **little-endian、无对齐**(`loadUnaligned`,`:79-92`);用 `byteorder` LE。
- 写 **atomic**(`:114`,先写临时再 rename)。
- 文件:`<cache_root>/Embeddings/<key>.embed`(`:32-46`)。
- `is_current`:`header.model==model && model_version==mv && sampler_version==sv`(`:58-61`);任一不符即需重索引。
- 内存向量 f32 连续(供 §5.8 矩阵·向量),落盘 f16(`AssetIndex` 注释 `:24`)。

## 5.7 推理后端实现(ort 默认 / candle 仅备选)

`VisualEmbedder` 用 CoreML(`VisualEmbedder.swift:1/29-35`),跨平台不可移植 → `docs/_analysis/02` 表 L80 / MODULE-PORT-MAP L881:**ort(ONNX Runtime)+ tokenizers**,candle 仅作可选回退。

```rust
// search/embedder.rs(默认实现)
#[cfg(feature = "ort-backend")]
pub struct OrtEmbedder { image: ort::Session, text: ort::Session,
                         tok: SiglipTokenizer, spec: EmbedderSpec }
#[cfg(feature = "ort-backend")]
impl Embedder for OrtEmbedder { /* encode_image/encode_text */ }
```
- **输入/输出名**:上游 CoreML 用 `"image"`/`"tokens"` 输入、`"embedding"` 输出(`VisualEmbedder.swift:39/48/54`)。ONNX 导出可能用不同名(如 `pixel_values`/`input_ids`/`image_embeds`),实施时按导出图实际名绑定,并在 `EmbedderSpec` 外补 `io_names` 配置或硬编码到 `OrtEmbedder::new`。
- **图像输入**:NCHW f32(1,3,256,256),mean/std 见 §5.2。
- **文本输入**:int64 (1,64),右填 0(§5.3)。
- **输出**:f32 (1,768);断言 `len==dim`(对齐 `vector(from:)` 的 `count==dim` 断言)。
- **L2 归一化**:若 `spec.normalized==false`(上游默认,模型内已归一)则**不**额外归一;否则 `v /= ‖v‖₂`。务必与导出模型一致(§0.8 风险)。
- **候选后端 candle**(`candle-backend` feature):`candle-transformers` 有 SigLIP 实现可加载 safetensors,纯 Rust 无 C++ 依赖;作为 ort 不可用平台的回退。两后端必须产出**同一向量**(同权重/同预处理),用「同图同文 → 余弦 > 0.999」单测交叉验证。

## 5.8 排名 `VisualSearch`(矩阵·向量 + best-per-shot + 截断)— 纯函数

`Search/Query/VisualSearch.swift`(`cblas_sgemv`)+ `MODULE-PORT-MAP` L863。上游用 Accelerate BLAS;Rust 用 `ndarray`(`gemv`)或手写点积。

```rust
// search/ranker.rs
#[derive(Clone, PartialEq, Debug)]
pub struct Hit { pub asset_id: String, pub time: f64,
                 pub shot_start: f64, pub shot_end: f64, pub score: f32 }

pub fn search(query: &[f32], indexes: &[(String, AssetIndex)],
    limit: usize,            // 20
    relative_cutoff: f32,    // 0.85
    min_score: Option<f32>,  // visualMatchCosineFloor 0.05
) -> Vec<Hit>;
```
算法(逐步,`VisualSearch.search` `:16-56` + `MODULE-PORT-MAP` L863):
1. 每个 `(asset_id, index)`:若 `dim != query.len() || count==0` 跳过(`:24`)。
2. `scores = vectors(count×dim, row-major) · query`(`cblas_sgemv(RowMajor,NoTrans,M=count,N=dim,α=1,A=vectors,lda=dim,x=query,β=0,y=scores)`,`:27-32`)。Rust:`Array2::from_shape(vectors).dot(&query_vec)`。
3. **best-per-shot**:按 `row.shotStart` 分组,只留最高分帧;**同分保留先出现**(`existing.score >= score` 则跳过,`:34-39`)。Rust 用 `HashMap<OrderedFloat<f64>, (usize, f32)>`(或对 `shot_start` 量化为 bits key)。
4. 每 shot 最佳 → `Hit`(`:40-47`);全局 `sort_by score desc`(`:49`)。
5. 若 `min_score`:先 `filter(score >= min_score)`(`:50-52`)。
6. `top = hits[0].score`;`top <= 0` → 返回空(`:53`)。
7. `floor = top * relative_cutoff`;返回 **`hits.prefix(limit)` 再 `filter(score >= floor)`**(顺序关键:先截 limit 再过 floor,最终 ≤ limit,`:54-55`)。

> 纯函数 + 全单测:多素材、单镜头去重、minScore 过滤、relativeCutoff 顺序、空结果、dim 不匹配跳过。

## 5.9 模型下载/校验/安装 `ModelDownloader`(reqwest+sha2+zip)

`Search/Models/ModelDownloader.swift` + `MODULE-PORT-MAP` L927。需求降低:**ONNX/safetensors 无需编译步骤**(去掉 `MLModel.compileModel`)。

```rust
// search/model_download.rs
pub struct ManifestFile { pub name: String, pub sha256: String, pub bytes: i64 }
pub struct Manifest {
    pub model: String, pub version: i32,
    pub embedding_dim: usize, pub image_size: u32, pub context_length: usize,
    pub image_encoder: ManifestFile, pub text_encoder: ManifestFile, pub tokenizer: ManifestFile,
}
pub struct InstalledModel { pub image_encoder: PathBuf, pub text_encoder: PathBuf,
                            pub tokenizer_folder: PathBuf, pub spec: EmbedderSpec }

pub fn installed(models_dir: &Path, m: &Manifest) -> Option<InstalledModel>;
pub async fn install(models_dir: &Path, m: &Manifest, base_url: &str,
    on_progress: impl Fn(f64)) -> Result<InstalledModel>;
pub fn verify_sha256(path: &Path, expected: &str) -> Result<()>;
```
- **安装目录**:`<app_support>/OpenTake/Models/<model>-v<version>/{image_encoder.onnx, text_encoder.onnx, tokenizer/, spec.json}`(对齐 `ModelDownloader.swift:46-64`,把 `.mlmodelc` 换成 `.onnx`/`.safetensors`)。跨平台用 `dirs`/Tauri `app_data_dir`(不再硬编码 `~/Library/Application Support`)。
- **进度**:三文件按 bytes 加权 0..1(`:79-99`)。
- **校验**:流式 SHA256(1 MiB 分块)对比 manifest(`:146-155`),不符 `Checksum`。
- **解压**:`zip` crate 替 `/usr/bin/ditto`;每 zip 恰好一个顶层条目(`:157-172`)。
- **幂等**:已安装直接返回(`:72`);全部 staged 后原子 move 到安装目录(`:101-113`)。
- **idempotent install + 安装完整性**:三文件都存在且 tokenizer 含 `tokenizer.json` 才算已装(`:54-64`)。

> ⚠️ **模型来源调整**:上游托管 `huggingface.co/palmier-io/siglip2-base-coreml`(CoreML zip,`SearchIndexConfig.swift:6`)。OpenTake 需**自托管或指向 ONNX/safetensors 版**的 SigLIP2-base-patch16-256(转换或复用现成 ONNX 导出)。`Manifest` 的 sha256/bytes 重新计算;`config.rs` 的 `manifest` 常量替换为 ONNX 版三文件。**这是本节唯一需要外部资产准备的点**(模型转换/托管),记入 §8 实施清单 T8.0。

## 5.10 配置 `SearchIndexConfig` 等价

`Search/SearchIndexConfig.swift`:
```rust
// search/config.rs
pub const VISUAL_MATCH_COSINE_FLOOR: f32 = 0.05;   // :4
pub const RELATIVE_CUTOFF: f32 = 0.85;             // VisualSearch.swift:19 默认
pub const SEARCH_LIMIT: usize = 20;                // 多处默认
pub fn enabled() -> bool;  // 默认 true;Tauri Store/settings.json 持久化(替 UserDefaults)
pub fn manifest() -> Manifest; // siglip2-base-patch16-256, v1, dim768, size256, ctx64(ONNX 版)
pub fn base_url() -> String;   // 自托管;DEBUG 可被环境变量覆盖(替 UserDefaults override)
```
- `enabled` 默认 true,键语义照搬(`SearchIndexConfig.swift:8-11`);存储后端换 Tauri Store/`settings.json`(`MODULE-PORT-MAP` L940 (1))。
- `base_url` DEBUG 覆盖用环境变量替 `UserDefaults "searchIndexModelBaseURL"`(`:13-20`)。
