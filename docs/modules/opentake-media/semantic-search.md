# semantic-search — SigLIP2 视觉语义搜索 + 通用 ONNX 推理面

> 上级：[INDEX.md](INDEX.md) · [OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：`search/{mod,config,embedder,ort_embedder,tokenizer,frame_sampler,indexer,embed_store,ranker,model_download}.rs`、`ort_worker/{mod,tensor}.rs`。对应上游 `Search/` 子树（CoreML → ONNX Runtime）。行级算法见 [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md)「Search」节。

---

## 职责

「按内容搜素材」的视觉语义侧（口语侧见 [transcribe.md](transcribe.md)）：用 **SigLIP2 双编码器**给素材帧生成 768 维 embedding 并幂等落盘，文本查询编码后与帧矩阵点积排名。模型 `siglip2-base-patch16-256`。外加一个**通用 ONNX 推理面 `ort_worker`**，供超分/抠像/追踪等进阶 AI 特性复用（[ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md) B 层）。

**设计纪律**：预处理 / tokenize / 抽帧判定 / 索引累积 / 存储 / 排名全是**纯函数**，可全单测；真实 ONNX 后端藏在 feature 后，**默认 build 用 mock、离线无 ML 链接**。

---

## 常量 `config.rs`（逐字照搬上游 `SearchIndexConfig`）

```rust
pub const MODEL_NAME: &str = "siglip2-base-patch16-256";
pub const MODEL_VERSION: i32 = 1;
pub const EMBEDDING_DIM: usize = 768;
pub const IMAGE_SIZE: u32 = 256;
pub const CONTEXT_LENGTH: usize = 64;
pub const SIGLIP_MEAN: [f32;3] = [0.5,0.5,0.5];
pub const SIGLIP_STD:  [f32;3] = [0.5,0.5,0.5];
pub const VISUAL_MATCH_COSINE_FLOOR: f32 = 0.05;  // 绝对余弦下限
pub const RELATIVE_CUTOFF: f32 = 0.85;            // 相对截断
pub const SEARCH_LIMIT: usize = 20;
```

---

## 双编码器 `embedder.rs` / `ort_embedder.rs`

```rust
pub struct EmbedderSpec { pub model: String, pub version: i32, pub embedding_dim: usize,
                          pub image_size: u32, pub context_length: usize, pub normalized: bool }
pub trait Embedder: Send + Sync {
    fn spec(&self) -> &EmbedderSpec;
    fn encode_image(&self, frame: &RgbaFrame) -> Result<Vec<f32>>;  // len == dim
    fn encode_text(&self, text: &str) -> Result<Vec<f32>>;
}
```

### 图像预处理（纯函数 `preprocess_image`，逐字复刻 `VisualEmbedder.pixelBuffer`）
1. **黑底合成**：带 alpha 源先 over 黑底（`RgbaFrame::black`），再丢 alpha——因上游缓冲未清零需黑底混合。
2. **squash-resize**：直接拉伸到 `256×256`，**不裁剪、不保宽高比**（Triangle 滤镜）。注意与 [decode.md](decode.md) 的 `fit_within`（等比、不放大）是不同函数。
3. **归一为 NCHW f32**（1,3,256,256）：`/255` 后 `(v-mean[c])/std[c]`；黑→`-1.0`、白→`+1.0`。

### tokenize `tokenizer.rs`（SigLIP，定长 64，右填 0）
```rust
pub const PAD_TOKEN: i64 = 0;
pub struct SiglipTokenizer { /* HF tokenizers + context_length */ }
pub fn pad_or_truncate(ids: &[u32], len: usize) -> Vec<i64>;  // 截断到 len，右填 0
```
- HF `tokenizers` crate（与上游 swift-transformers 同源）；**手动**截断到 64 + 右填 0，**关闭** tokenizer 自动 padding/truncation——SigLIP 训练无 attention mask，必须与 Python 参考严格一致。

### ort 后端 `ort_embedder.rs`（feature `ort-backend`）
```rust
pub struct OrtEmbedder { image: Mutex<Session>, text: Mutex<Session>, tokenizer: SiglipTokenizer, spec, io: IoNames }
pub struct IoNames { pub image_input, image_output, text_input, text_output: String }  // 默认 "image"/"embedding"/"tokens"/"embedding"
```
- 图像输入 NCHW f32、文本输入 `(1,64)` int64；输出断言 `len == dim`，否则 `BadModelOutput`。
- **L2 归一化开关**：`spec.normalized` 默认 `false`（上游模型图内已归一化），裸点积即等价余弦；当前 `finalize` 仅做长度校验（标定路径保留为后续，[SPEC.md](SPEC.md) §0.8 风险）。务必复用上游同一份导出权重转 ONNX。

---

## 视觉去重抽帧 `frame_sampler.rs`

```rust
pub const SAMPLER_VERSION: i32 = 1;   // 参与缓存失效判定
pub const LUMA_CELLS: usize = 8;
pub struct SamplerOptions { pub candidate_interval: f64 /*2.0*/, pub coverage_floor: f64 /*8.0*/,
    pub promote_diff: f32 /*12.0*/, pub max_size: (u32,u32) /*(512,512)*/, pub high_res_edge: u32 /*3000*/ }
pub struct SampledFrame { pub time_secs: f64, pub image: RgbaFrame, pub is_new_shot: bool }
pub fn sample_frames(path, duration_secs, opts) -> Result<impl Iterator<Item = Result<SampledFrame>>>;
pub fn luma_grid(frame) -> [f32;64];          // 8×8 Rec.601 luma
pub fn luma_mean_diff(a,b) -> f32;            // L1 平均差 = Σ|a-b|/64
```

算法（逐步对齐 `FrameSampler.sample`）：
1. 若 `max(|w|,|h|) ≥ high_res_edge(3000)` 则 `interval *= 2`（2.0→4.0）。
2. 候选时间：`stride(from: interval/2, to: duration, by: interval)`（严格 `< duration`）；为空则 `[duration/2]`。
3. 解帧：`max_size=512²`、`apply_rotation=true`、tolerance `max(interval/2, 1.0)`。
4. 每帧：`t = actual_secs`，丢 `t ≤ last_time`（去重）；算 8×8 luma grid；有上一 grid 则 `is_new_shot = mean_diff > promote_diff(12)`，否则首帧 `is_new_shot=true`。
5. 保留：`is_new_shot || t - last_kept_time ≥ coverage_floor(8.0)`；保留时推进 `last_kept_time`。
   - **关键不变量**：`luma grid` 用**所有解码帧**更新，`last_kept_time` 只在**被保留**时推进（由 `ShotDetector` 状态机维护）。
- `luma_grid`：8×8 平均池，每格 Rec.601 `0.299R + 0.587G + 0.114B`（对 sRGB 字节，不做 gamma 线性化），系数逐字照搬。

---

## 索引器 `indexer.rs`（幂等）

```rust
pub fn needs_index(cache_root, path, spec) -> bool;
pub fn index_video(path, duration_secs, embedder, opts, on_progress, cancel) -> Result<()>;
pub fn index_image(cache_root, path, image, embedder, cancel) -> Result<()>;
pub fn accumulate_rows(frames: &[(f64, bool)], duration: f64) -> Vec<Row>;  // 纯函数
pub struct CancelToken(/* Arc<AtomicBool> */);
```

- **shot 累积**（`accumulate_rows`）：维护 `shot_starts`；遇 `is_new_shot` 则 push（**第一个镜头起点强制 0.0**，无论首帧实际时间，其余为该帧 time）；`row.shot_end = 下一镜头起点 or duration`。
- **幂等**：`needs_index` 用 `(model, model_version, sampler_version)` 三元组判断，已 current 直接返回。
- **图像**：单 embedding，`Row{time:0, shot_start:0, shot_end:0}`（零长 shot）；解码失败仍写 `count=0` 索引（标记已处理，避免反复重试）。
- **导出让路 + 取消**：每帧前 `cancel.check()` 与等待导出（[library-index.md](library-index.md) 的 `ExportPause`）。

---

## 嵌入存储 `embed_store.rs`（`PALMEMB1` 二进制，逐字节复刻）

```rust
#[serde(rename_all = "camelCase")]
pub struct Header { pub model: String, pub model_version: i32, pub sampler_version: i32, pub dim: usize, pub count: usize }
pub struct Row { pub time: f64, pub shot_start: f64, pub shot_end: f64 }
pub struct AssetIndex { pub header: Header, pub rows: Vec<Row>, pub vectors: Vec<f32> } // count*dim, f32 内存
```

布局（little-endian、无对齐）：
```
magic "PALMEMB1"  (8 bytes ASCII)
u32 headerLen     (4 bytes LE)
JSON(Header)      (headerLen bytes)
count 行，每行 rowBytes = 24 + dim*2：
    f64 time / f64 shotStart / f64 shotEnd  (各 8 bytes LE)
    dim × f16  (每个 2 bytes LE)   # half crate：落盘 f16，内存 f32
```
- `dim=768 ⇒ rowBytes = 24 + 1536 = 1560`。
- **严格校验**：`total == 8 + 4 + headerLen + count*rowBytes`，多/少字节 → `StoreCorrupt`。
- 写 **atomic**（临时文件 → rename）；文件 `<cache_root>/Embeddings/<key>.embed`（`key = file_identity_key(path,32)`）。
- `is_current`：`model && model_version && sampler_version` 全等，任一不符即重索引。

---

## 排名 `ranker.rs`（纯函数）

```rust
pub struct Hit { pub asset_id: String, pub time: f64, pub shot_start: f64, pub shot_end: f64, pub score: f32 }
pub fn rank(query: &[f32], indexes, limit, relative_cutoff, min_score) -> Vec<Hit>;
```

对每个 `AssetIndex`（`dim` 不符或 `count==0` 跳过）：
1. **矩阵·向量**：`vectors`（count×dim 行主序）· `query` 得每帧分数（手写点积；上游用 `cblas_sgemv`）。
2. **best-per-shot**：按 `row.shot_start` 分组，每 shot 只留最高分（同分保留先出现）。
3. 全局 hits 按 score 降序；先 `min_score`（默认 0.05）绝对过滤。
4. **截断顺序关键**：`top = 最高分`（≤0 返回空）；`floor = top * relative_cutoff(0.85)`；**先 `prefix(limit)` 再 filter `≥ floor`**——最终条数 `≤ limit`。

---

## 通用推理面 `ort_worker/`

- `mod.rs`：`ExecutionProvider`（Cpu/CoreML/Cuda/DirectMl/TensorRt，`platform_default()` 按平台选 CoreML/DirectMl/Cpu）+ `IoTensor`/`IoSpec`（输入输出张量描述）+ `OrtModel`（`Session` 的 Mutex 包装，feature `ort-backend`）。
- `tensor.rs`：`frame_to_hwc`（RGBA→HWC f32 [0,1] 丢 alpha）、`hwc_to_nchw_normalized`（按 mean/std 归一）、`mean_pool`（token 级输出平均）。
- 用途：SigLIP2 与后续超分/抠像/追踪/补帧的统一 ONNX 推理通道（[ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md) §B/§54）。

---

## 模型下载 `model_download.rs`

```rust
pub struct Manifest { pub model, version, embedding_dim, image_size, context_length,
                      image_encoder: ManifestFile, text_encoder, tokenizer }  // ManifestFile{name, sha256, bytes}
pub fn install_dir(models_dir, m) -> PathBuf;       // <models_dir>/<model>-v<version>/
pub fn installed(models_dir, m) -> Option<InstalledModel>;
pub fn verify_sha256(path, expected) -> Result<()>; // 1MiB 流式
pub async fn install(models_dir, m, base_url, on_progress) -> Result<...>;  // feature model-download
```
- 幂等下载 image/text encoder + tokenizer → 逐个 **SHA-256 流式校验**（1MiB 块）→ tokenizer.zip 解压单顶层目录 → 原子 rename 到最终位置 → 写 spec.json。`installed` 按三件 + `tokenizer/tokenizer.json` 存在性判定。
- 相比上游去掉了 `MLModel.compileModel`（ONNX 无需编译）。
- ⚠️ **占位待填**：`Manifest` 的 sha256/bytes 当前为空字符串/0，待实际 ONNX 资产托管后填实（[ROADMAP.md](../../architecture/ROADMAP.md) Phase 8）。

---

## feature 与完成状态
```toml
ort-backend    = ["ort", "ndarray"]       # 默认 build 不含；启用后真实 SigLIP2 推理
model-download = ["reqwest", "zip", ...]  # 启用后下载
```
全链路纯函数 + mock **已实现并全测**；真实 ort 推理 + 模型托管属 Phase 8 计划中。改任何烧印常量（promoteDiff/coverageFloor/dim/imageSize…）须两侧同步。

## 测试
预处理（黑→-1/白→+1/squash/alpha 合成）、pad_or_truncate、候选时间（stride/回退/零 duration）、luma_grid（黑/白/Rec.601）、ShotDetector 状态机（首帧/去重/镜头切/覆盖下限/grid 总更新）、accumulate_rows（首镜头归零/链接/幂等）、PALMEMB1 往返（f16 量化/版本校验/多字节拒绝）、排名（点积排序/best-per-shot/limit-then-floor/空索引）、install_dir/installed/SHA256、ort_worker 张量互转；端到端 `index_then_rank_finds_brightest_match`（mock 流）。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 相关：[decode.md](decode.md) · [transcribe.md](transcribe.md)（口语侧）· [library-index.md](library-index.md)（调度内核）· [ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md)
- 模块文档树：[../INDEX.md](../INDEX.md) · docs 总目录：[../../INDEX.md](../../INDEX.md)
- 源码根：`../../../crates/opentake-media/src/`
