# ort 推理 worker 通用接口(供进阶 AI 特性复用)

上游无此抽象(CoreML 直接在 `VisualEmbedder`)。`docs/ROADMAP.md` Phase 8 与 `docs/ADVANCED-FEATURES.md` B/C/D 层要求「统一 ort worker」承载:超分(Real-ESRGAN/SeedVR)、AI 抠像(RVM/BiRefNet)、运动追踪(CoTracker)、人声分离(Demucs)等。SigLIP2 的 `OrtEmbedder`(§5.7)是它的第一个使用者。

## 7.1 通用模型抽象

```rust
// ort_worker/mod.rs
/// 一个已加载的 ONNX 模型 + 其 IO 约定。线程安全,可被多任务共享。
pub struct OrtModel { session: ort::Session, io: IoSpec }
pub struct IoSpec { pub inputs: Vec<IoTensor>, pub outputs: Vec<IoTensor> }
pub struct IoTensor { pub name: String, pub dtype: TensorDType, pub shape: Vec<i64> } // -1=动态

impl OrtModel {
    pub fn load(path: &Path, ep: ExecutionProvider) -> Result<Self>;
    /// 多输入多输出推理;输入/输出按名映射 ndarray。
    pub fn run(&self, inputs: &[(&str, TensorRef<'_>)]) -> Result<HashMap<String, OwnedTensor>>;
}

pub enum ExecutionProvider { Cpu, CoreML, Cuda, DirectMl, Tensorrt } // 按平台可用性回退到 Cpu
```

## 7.2 worker(序列化 GPU/重负载,导出期让路)

```rust
/// 单后台执行器:把推理任务排队,序列化访问昂贵 EP,导出活跃时暂停(与 §7.7 共享暂停信号)。
pub struct OrtWorker { /* tokio mpsc + 单 worker */ }
impl OrtWorker {
    pub fn spawn(export_pause: ExportPause) -> Self;
    pub async fn submit<F, T>(&self, job: F) -> Result<T>
        where F: FnOnce(&OrtModelRegistry) -> Result<T> + Send + 'static, T: Send + 'static;
}
pub struct OrtModelRegistry { /* 按 key 懒加载并缓存 OrtModel,避免重复 load */ }
```
- **张量辅助**(`ort_worker/tensor.rs`):`ndarray ↔ ort::Value`、NCHW/NHWC 转换、mean/std 归一、`Array4<f32>` ↔ 图像。SigLIP 预处理(§5.2)即复用这里。
- **EP 回退**:首选平台 EP(CoreML/CUDA/DirectML),不可用回退 CPU,日志 `tracing::warn`。
- **复用点**:`OrtEmbedder`(§5.7)内部即一个 `OrtModel`(image)+ 一个 `OrtModel`(text);进阶特性各自定义自己的预处理/后处理,共用 `OrtModel::run` + `OrtWorker` 调度。

> 本 crate 只交付**框架 + SigLIP2 使用者**;具体进阶模型(Real-ESRGAN 等)在各自 Phase 8+ PR 落地,复用本接口。记此以明确「worker 通用接口」的交付边界 = §7.1/§7.2 + 至少一个真实使用者(SigLIP2)。

## 7.3 后台索引/转写调度 `IndexCoordinator`(替 `SearchIndexCoordinator`)

`Search/SearchIndexCoordinator.swift` + `MODULE-PORT-MAP` L864/L867。上游是 `@MainActor @Observable`;Rust 用 **tokio 单 worker 队列 + AtomicUsize 导出暂停 + 事件向前端推进度**(MODULE-PORT-MAP L881 (6))。**注意:UI 状态(进度/`@Observable`)属上层**;本 crate 提供调度内核 + 进度回调,UI 镜像在 `opentake-core`/前端。

```rust
// index_coordinator.rs
#[derive(Clone)] pub struct ExportPause(Arc<AtomicUsize>); // 引用计数,跨窗口
impl ExportPause {
    pub fn begin(&self); pub fn end(&self);      // exportDidBegin/End(:46-47)
    pub fn is_active(&self) -> bool;             // exportActive(:45)
    pub async fn wait_while_active(&self);       // 每 2s 轮询(:49-53)
}

pub struct IndexCoordinator { /* queue, failed set, single worker, loaded_indexes cache */ }
impl IndexCoordinator {
    pub fn new(export_pause: ExportPause, embedder: Arc<dyn Embedder>,
               transcriber: Arc<dyn Transcriber>, cache_root: PathBuf) -> Self;

    /// 入队需要(重)索引的素材(视觉 needsIndex 或 转写无磁盘缓存)。
    pub fn schedule(&self, asset: &opentake_domain::media::MediaAsset);
    pub fn sweep(&self, assets: &[opentake_domain::media::MediaAsset]);
    pub async fn cancel_all(&self);

    /// 查询:快照候选 → off-thread 加载/编码/排名 → 返回 Hit(视觉)。
    pub async fn search_visual(&self, query: &str, limit: usize,
        within: Option<&HashSet<String>>, assets: &[MediaAsset]) -> Vec<Hit>;

    pub fn progress(&self) -> IndexProgress; // {batch_total, batch_completed, current_fraction}
}
```
逐项对齐(`SearchIndexCoordinator.swift`):
- **schedule 条件**:enabled 且有 embedder 且 `!asset.is_generating`;id 不在 queue/failed;`needsVisual(video|image 且 VisualIndexer.needs_index)` 或 `needsTranscript(audio|video+hasAudio 且 转写无磁盘缓存)` 成立才入队;`batch_total+=1`;`ensure_worker`(`:107-124`)。
- **worker**:单个(`tokio::spawn`),`utility` 优先级;循环 dequeue,`export_pause.wait_while_active()` 每 2s 轮询(`:148-160`);`index_one`(`:178-221`)。
- **index_one**:需转写则视觉占进度 0.5 否则 1.0(`visualShare`,`:181-185`);`async let`/`tokio::join!` 并发跑转写(`TranscriptCache.transcript`)与视觉索引;视觉完成后置 `current_fraction = visualShare` 再 await 转写(`:189-214`)。失败(非取消)记 `failed`(`:217-220`)。
- **dequeue**:跳过已不存在的 id(`batch_completed+=1`);队列空 `reset_batch` 返回 None(`:162-170`)。
- **search**:main 快照候选 `(id,url)` + `loaded_indexes`;off-thread 算 key、命中内存缓存(key 相等)复用否则 `EmbeddingStore::load`、`encode_text(query)`、`VisualSearch::search`;回主合并 `loaded_indexes`;空 query → `[]`(`:225-257`)。
- **generation 让路**:`exportPause` 跨窗口引用计数(`ExportPauseCounter`,`:37-47`);导出开始/结束由 `opentake-render` 调 `begin/end`(对齐上游 `ExportService.isExporting.didSet`)。
