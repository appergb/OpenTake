# semantic-search — SigLIP2 视觉语义搜索 + 通用 ONNX 推理面

> 上级：[INDEX.md](INDEX.md) · [OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：`search/{mod,config,embedder,ort_embedder,tokenizer,frame_sampler,indexer,embed_store,ranker,model_download}.rs`、`ort_worker/{mod,tensor}.rs`。对应上游 `Search/` 子树（CoreML → ONNX Runtime）。行级算法见 [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md)「Search」节。

---

## 职责

「按内容搜素材」的视觉语义侧（口语侧见 [transcribe.md](transcribe.md)）：用 **SigLIP2 双编码器**给素材帧生成 768 维 embedding 并幂等落盘，文本查询编码后与帧矩阵点积排名。模型 `siglip2-base-patch16-256`。外加一个**通用 ONNX 推理面 `ort_worker`**，供超分/抠像/追踪等进阶 AI 特性复用（[ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md) B 层）。

**设计纪律**：预处理 / tokenize / 抽帧判定 / 索引累积 / 存储 / 排名全是**纯函数**，可全单测；真实 ONNX 后端藏在 feature 后，**默认 build 用 mock、离线无 ML 链接**。

---

## 常量 `config.rs`（检索阈值对齐上游，模型缓存版本按实际 ONNX 契约升级）

```rust
pub const MODEL_NAME: &str = "siglip2-base-patch16-256";
pub const MODEL_VERSION: i32 = 2;
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
- HF `tokenizers` 在特殊 token 后处理前按 context length 截断，保留长查询末尾 EOS，再右填 0 到 64；模型只接收 input_ids，不传 attention mask。通用 `pad_or_truncate` 仍保留，真实 tokenizer 路径先完成上述约束。

### ort 后端 `ort_embedder.rs`（feature `ort-backend`）
```rust
pub struct OrtEmbedder { image: Mutex<Session>, text: Mutex<Session>, tokenizer: SiglipTokenizer, spec, io: IoNames }
pub struct IoNames { pub image_input, image_output, text_input, text_output: String }  // 默认 "pixel_values"/"pooler_output"/"input_ids"/"pooler_output"
```
- 图像输入 NCHW f32、文本输入 `(1,64)` int64；输出校验维度、有限值及非零范数。Windows 使用同一锁定 Tract 引擎绑定固定输入并重推导动态维度，保留 dtype/rank/静态尺寸及 `[1,768]` 输出约束；macOS/Linux 使用原生 ORT。
- **L2 归一化**：`spec.normalized` 表示图输出是否已经归一化。当前固定 ONNX 图输出未归一化，因此该值为 `false`，`finalize` 在校验后执行 L2 归一化；随后点积才可按余弦阈值排名。更换权重或 I/O 契约须重新实测并更新缓存版本。

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

- `mod.rs`：`ExecutionProvider`（Cpu/CoreML/Cuda/DirectMl/TensorRt，`platform_default()` 在 macOS 选 CoreML、Windows/Linux 选 CPU；Windows 由纯 Rust tract 执行）+ `IoTensor`/`IoSpec`（输入输出张量描述）+ `OrtModel`（`Session` 的 Mutex 包装，feature `ort-backend`）。
- `tensor.rs`：`frame_to_hwc`（RGBA→HWC f32 [0,1] 丢 alpha）、`hwc_to_nchw_normalized`（按 mean/std 归一）、`mean_pool`（token 级输出平均）。
- 用途：SigLIP2 与后续超分/抠像/追踪/补帧的统一 ONNX 推理通道（[ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md) §B/§54）。

---

## 模型下载 `model_download.rs`

```rust
pub struct Manifest { pub model, version, embedding_dim, image_size, context_length,
                      image_encoder: ManifestFile, text_encoder, tokenizer }  // ManifestFile{name, sha256, bytes}
pub fn install_dir(models_dir, m) -> PathBuf;       // <models_dir>/<model>-v<version>/
pub fn installed(models_dir, m) -> Option<InstalledModel>; // 快速检查回执及长度
pub fn verify_installed(models_dir, m) -> Result<InstalledModel>; // 加载前重验实际 SHA-256
pub fn install_from_directory(models_dir, m, source_dir) -> Result<InstalledModel>;
pub fn verify_sha256(path, expected) -> Result<()>; // 1MiB 流式
pub async fn install(models_dir, m, base_url, on_progress) -> Result<...>;  // feature model-download
```
- 按固定 revision、长度和 SHA-256 下载两个 encoder 与 tokenizer，支持安全相对子目录。当前直接使用 tokenizer.json，保留旧 ZIP 兼容。先在临时目录完整校验并写 spec/manifest 回执，再发布安装目录，失败时清理或恢复旧目录。`installed` 核对当前回执和准确长度，加载前 `verify_installed` 重新流式验 hash；离线安装走相同校验。
- 相比上游去掉了 `MLModel.compileModel`（ONNX 无需编译）。
- **模型安装修复已完成并验证（2026-09-06）**：原空 sha256/bytes 占位已替换为固定 revision 资产清单，约 1.5 GB 文件已逐一校验。macOS 真实 Rust 路径已验证离线安装、图文 embedding、归一化、PALMEMB1 f16 往返和排名；Windows/MSVC 与 macOS 原生资格流程已通过真实安装、图文推理和五条检索断言；当前新包 UI 验收另见总记录。详见[模型专项审计](../../audit/2026-09-06/semantic-search-model.md)，完整 revision/校验值由审计链接的知识记录统一维护。

---

## feature 与完成状态
```toml
ort-backend    = ["dep:ort", "dep:ort-tract", "dep:tract-onnx"] # 依赖按平台激活；默认不启用
model-download = ["reqwest", "zip", ...]  # 启用后下载
```
模型安装与真实后端纵向链路已实现并验证：正式 Cargo media search 为 84 passed / 1 ignored，Tauri search 为 14 passed；引用实际产品模块并运行真实模型的 Rust harness 为 72 passed / 0 ignored，包含离线安装、图文 embedding 与排名。后者不是 Python 推理替代，也不是已执行正式 Cargo opt-in 命令；复现方式与证据见[专项审计](../../audit/2026-09-06/semantic-search-model.md)。三组测试覆盖有重叠，不相加为独立测试数。后续正式平台流程已在 `64dce59` 上于 macOS 与 Windows 各执行同一真实模型 Cargo 用例（1 passed/0 ignored），见 [平台资格结果](https://github.com/appergb/OpenTake/actions/runs/34043674623)。新包 UI 与最终发布状态按总验收记录维护。改任何烧印常量（promoteDiff/coverageFloor/dim/imageSize…）须两侧同步。

## 测试
预处理（黑→-1/白→+1/squash/alpha 合成）、pad_or_truncate、候选时间（stride/回退/零 duration）、luma_grid（黑/白/Rec.601）、ShotDetector 状态机（首帧/去重/镜头切/覆盖下限/grid 总更新）、accumulate_rows（首镜头归零/链接/幂等）、PALMEMB1 往返（f16 量化/版本校验/多字节拒绝）、排名（点积排序/best-per-shot/limit-then-floor/空索引）、install_dir/installed/SHA256、ort_worker 张量互转；端到端 `index_then_rank_finds_brightest_match`（mock 流）。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 相关：[decode.md](decode.md) · [transcribe.md](transcribe.md)（口语侧）· [library-index.md](library-index.md)（调度内核）· [ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md)
- 模块文档树：[../INDEX.md](../INDEX.md) · docs 总目录：[../../INDEX.md](../../INDEX.md)
- 源码根：`../../../crates/opentake-media/src/`
