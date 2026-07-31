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
/// 固定容量 admission channel + 一个专用线程；不随调用数增加 worker。
pub struct OrtWorker { /* sync_channel + one worker + live-key dedupe */ }
impl OrtWorker {
    pub fn spawn(export_pause: ExportPause, capacity: usize) -> Self;
    pub fn submit<T, F>(&self, request: JobRequest, job: F)
        -> Result<JobHandle<T>, WorkerError>;
    pub fn active_jobs(&self) -> usize;
    pub fn shutdown(&self) -> Result<(), WorkerError>;
}
pub enum JobState { Queued, Running, Cancelled, Completed, Failed }
pub enum JobPriority { Background, Interactive }
pub struct JobRequest { kind: JobKind, model_identity: String,
                        dedupe_key: String, priority: JobPriority }
pub struct OrtModelRegistry { /* 按 model_identity 懒加载并缓存，避免重复 load */ }
```
- **有界与顺序**:`try_send` 在容量满时返回 typed `QueueFull`；每个优先级内部按单调 sequence FIFO，连续 4 个 interactive 后强制服务一个 background，给出明确的 starvation bound。
- **取消与终态**:`JobHandle` 暴露 `state/cancel/wait/wait_until_running`；排队和运行中取消都收敛到 `Cancelled`。job error、model error 与 panic 收敛到 `Failed`，worker 捕获 panic 后继续服务下一项；`shutdown` 取消队列、join 唯一线程并保证 `active_jobs()==0`。
- **去重与结果**:同一 live `dedupe_key` 返回同一 shared typed result，不二次占队列/执行；终态前先移除 live key，因此失败可以立即用同 key 重试。
- **张量辅助**(`ort_worker/tensor.rs`):`ndarray ↔ ort::Value`、NCHW/NHWC 转换、mean/std 归一、`Array4<f32>` ↔ 图像。SigLIP 预处理(§5.2)即复用这里。
- **EP 回退**:首选平台 EP(CoreML/CUDA/DirectML),不可用回退 CPU,日志 `tracing::warn`。
- **复用点**:`OrtEmbedder`(§5.7)内部即一个 `OrtModel`(image)+ 一个 `OrtModel`(text);进阶特性各自定义自己的预处理/后处理,共用 `OrtModel::run` + `OrtWorker` 调度。

> 本 crate 只交付**框架 + SigLIP2 使用者**;具体进阶模型(Real-ESRGAN 等)在各自 Phase 8+ PR 落地,复用本接口。记此以明确「worker 通用接口」的交付边界 = §7.1/§7.2 + 至少一个真实使用者(SigLIP2)。

## 7.3 后台索引/转写调度 `IndexCoordinator`(替 `SearchIndexCoordinator`)

`Search/SearchIndexCoordinator.swift` + `MODULE-PORT-MAP` L864/L867。上游是 `@MainActor @Observable`;Rust 的生产拥有者是 Tauri `search_index_start` + `index_assets`，通过上面的固定容量单 worker、共享 `ExportPause` 和 `search://index` 事件实现。同步 Tauri 命令在调用线程等待 typed handle，真正的重负载只在唯一专用 worker 执行，重复命令共享结果而不是启动第二次索引。

```rust
// index_coordinator.rs
#[derive(Clone)] pub struct ExportPause(Arc<ExportPauseInner>); // 引用计数+Condvar,跨窗口
impl ExportPause {
    pub fn begin(&self); pub fn end(&self);      // exportDidBegin/End(:46-47)
    pub fn is_active(&self) -> bool;             // exportActive(:45)
    pub fn guard(&self) -> ExportPauseGuard;     // Drop 自动平衡 end
    pub fn wait_while_active(&self, cancelled: impl Fn() -> bool) -> bool;
}
```
逐项对齐(`SearchIndexCoordinator.swift`):
- **schedule 条件**:`search_index_start` 固化一次 manifest snapshot；视觉只取 `needs_index` 为真者，音频/有音轨视频只取无 fingerprint cache 者。job key 包含 cache root、每个 source fingerprint 与 SigLIP model/version；live duplicate 合并。
- **worker**:进程内一个 `OrtWorker`，容量 8；`OrtModelRegistry` 以 model/version/models-root 懒加载 SigLIP。每项开始和素材 batch boundary 都检查 cancellation 与 `ExportPause`，最终 holder drop 后 Condvar 立即唤醒（取消最多 20 ms 被观察），不再用 2 s 忙轮询。
- **index_one**:视觉与自动转写都在同一个 heavy worker 中串行，避免两个模型同时争用 GPU/CPU；单素材失败只记录并继续，取消则终止整个 job。embedding 由原子 store 写入，transcript 由 fingerprint cache 保存；重启重新 sweep，已完成项跳过，未完成项继续。
- **dequeue**:提交时的 owned snapshot 使项目切换不会混合素材；缺失/离线文件被该轮跳过，下一轮 source identity 变化后可重试。
- **search**:main 快照候选 `(id,url)` + `loaded_indexes`;off-thread 算 key、命中内存缓存(key 相等)复用否则 `EmbeddingStore::load`、`encode_text(query)`、`VisualSearch::search`;回主合并 `loaded_indexes`;空 query → `[]`(`:225-257`)。
- **generation 让路**:`ExportPause` 是跨窗口引用计数；播放/导出压力拥有者持有 `guard` 时不启动新 job，运行 job 在下一素材边界让行，嵌套 holder 仅在最后一个离开时恢复。

拥有测试:`search::tests::bounded_single_worker_cancels_skips_stale_and_yields_to_playback_export` 覆盖容量、优先级 FIFO、4-job 防饥饿、live-key 去重、排队/运行取消、model error、panic 后恢复、失败同 key 重试、model registry 单次加载、压力恢复、source/model identity 失效、shutdown 与 restart；`index_coordinator::tests::export_pause_ref_counts` 覆盖嵌套 guard、唤醒与取消。
