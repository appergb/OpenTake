//! Generic ONNX Runtime worker — a reusable inference surface for advanced AI
//! features (super-resolution, matting, tracking, …). Upstream had no such
//! abstraction (CoreML was used inline); this is the cross-platform foundation
//! the SigLIP2 embedder and later models share (SPEC §7).
//!
//! The execution-provider enum, tensor helpers, and IO spec are always
//! available; the actual `OrtModel` (a loaded `ort::Session`) is behind feature
//! `ort-backend`. The worker serializes heavy inference and yields to active
//! exports via [`ExportPause`].

pub mod tensor;

pub use tensor::{frame_to_hwc, hwc_to_nchw_normalized, mean_pool};

/// Execution provider preference; the loader falls back to CPU when an
/// accelerator is unavailable. Windows ships the pure-Rust tract CPU backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionProvider {
    Cpu,
    CoreML,
    Cuda,
    DirectMl,
    TensorRt,
}

impl ExecutionProvider {
    /// The platform-preferred provider (CoreML on macOS, CPU tract on Windows,
    /// CPU on Linux), used as the first choice before CPU fallback.
    pub fn platform_default() -> Self {
        #[cfg(target_os = "macos")]
        {
            ExecutionProvider::CoreML
        }
        #[cfg(target_os = "windows")]
        {
            ExecutionProvider::Cpu
        }
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        {
            ExecutionProvider::Cpu
        }
    }
}

/// Dtype of a model IO tensor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TensorDType {
    F32,
    I64,
    I32,
    U8,
}

/// One IO tensor's declared spec (`-1` for dynamic dims).
#[derive(Clone, Debug, PartialEq)]
pub struct IoTensor {
    pub name: String,
    pub dtype: TensorDType,
    pub shape: Vec<i64>,
}

/// A model's full IO contract.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IoSpec {
    pub inputs: Vec<IoTensor>,
    pub outputs: Vec<IoTensor>,
}

use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::index_coordinator::ExportPause;
use crate::search::CancelToken;

/// The production operation class carried with every queued job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    Index,
    Transcribe,
    Search,
}

/// Scheduling priority. Playback/export does not enter this queue: its shared
/// [`ExportPause`] gate prevents new jobs from starting at batch boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobPriority {
    Background,
    Interactive,
}

/// Stable identity used for observability, prioritisation, and deduplication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobRequest {
    pub kind: JobKind,
    pub model_identity: String,
    pub dedupe_key: String,
    pub priority: JobPriority,
}

impl JobRequest {
    pub fn new(
        kind: JobKind,
        model_identity: impl Into<String>,
        dedupe_key: impl Into<String>,
        priority: JobPriority,
    ) -> Self {
        Self {
            kind,
            model_identity: model_identity.into(),
            dedupe_key: dedupe_key.into(),
            priority,
        }
    }
}

/// Consumer-visible lifecycle. Every accepted job reaches exactly one terminal
/// state even when its task returns an error or panics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Running,
    Cancelled,
    Completed,
    Failed,
}

/// Typed queue/result failures. Model and job errors remain recoverable: the
/// single worker continues serving the next request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerError {
    QueueFull,
    Cancelled,
    Shutdown,
    Panicked,
    Model(String),
    Job(String),
    ResultType,
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull => f.write_str("inference queue is full"),
            Self::Cancelled => f.write_str("inference job was cancelled"),
            Self::Shutdown => f.write_str("inference worker is shut down"),
            Self::Panicked => f.write_str("inference job panicked"),
            Self::Model(message) => write!(f, "model error: {message}"),
            Self::Job(message) => write!(f, "job error: {message}"),
            Self::ResultType => f.write_str("inference result type mismatch"),
        }
    }
}

impl std::error::Error for WorkerError {}

type ErasedResult = Arc<dyn Any + Send + Sync>;
type JobTask = Box<
    dyn FnOnce(&OrtModelRegistry, &CancelToken) -> Result<ErasedResult, WorkerError>
        + Send
        + 'static,
>;

struct JobStatus {
    state: JobState,
    result: Option<Result<ErasedResult, WorkerError>>,
}

struct SharedJob {
    request: JobRequest,
    cancel: CancelToken,
    status: Mutex<JobStatus>,
    changed: Condvar,
}

impl SharedJob {
    fn new(request: JobRequest) -> Self {
        Self {
            request,
            cancel: CancelToken::new(),
            status: Mutex::new(JobStatus {
                state: JobState::Queued,
                result: None,
            }),
            changed: Condvar::new(),
        }
    }

    fn set_running(&self) -> bool {
        let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
        if self.cancel.is_cancelled() {
            false
        } else {
            status.state = JobState::Running;
            self.changed.notify_all();
            true
        }
    }

    fn finish(&self, result: Result<ErasedResult, WorkerError>) {
        let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
        status.state = match &result {
            Ok(_) => JobState::Completed,
            Err(WorkerError::Cancelled) => JobState::Cancelled,
            Err(_) => JobState::Failed,
        };
        status.result = Some(result);
        self.changed.notify_all();
    }
}

struct QueuedJob {
    sequence: u64,
    shared: Arc<SharedJob>,
    task: Option<JobTask>,
}

enum WorkerMessage {
    Job(QueuedJob),
    Shutdown,
}

struct WorkerInner {
    sender: SyncSender<WorkerMessage>,
    dedupe: Mutex<HashMap<String, Weak<SharedJob>>>,
    capacity: usize,
    queued: AtomicUsize,
    sequence: AtomicU64,
    active: AtomicUsize,
    shutdown: AtomicBool,
    thread: Mutex<Option<JoinHandle<()>>>,
}

/// A bounded, single-thread heavy-inference executor. Its queue is shared by
/// indexing, transcription, and semantic-search callers; jobs carry model and
/// source identities, deduplicate while live, and yield to playback/export.
#[derive(Clone)]
pub struct OrtWorker {
    inner: Arc<WorkerInner>,
}

/// Typed view of one accepted (or deduplicated) job.
pub struct JobHandle<T> {
    shared: Arc<SharedJob>,
    marker: PhantomData<T>,
}

impl<T> Clone for JobHandle<T> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> JobHandle<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn state(&self) -> JobState {
        self.shared
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .state
    }

    pub fn cancel(&self) {
        self.shared.cancel.cancel();
    }

    pub fn wait(&self) -> Result<T, WorkerError> {
        let mut status = self.shared.status.lock().unwrap_or_else(|e| e.into_inner());
        while status.result.is_none() {
            status = self
                .shared
                .changed
                .wait(status)
                .unwrap_or_else(|e| e.into_inner());
        }
        match status.result.as_ref().expect("result checked above") {
            Ok(value) => value
                .downcast_ref::<T>()
                .cloned()
                .ok_or(WorkerError::ResultType),
            Err(error) => Err(error.clone()),
        }
    }

    pub fn wait_until_running(&self, timeout: Duration) -> Result<(), WorkerError> {
        let deadline = Instant::now() + timeout;
        let mut status = self.shared.status.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            match status.state {
                JobState::Running => return Ok(()),
                JobState::Cancelled => return Err(WorkerError::Cancelled),
                JobState::Failed | JobState::Completed => {
                    return Err(WorkerError::Job(
                        "job finished before it was observed running".into(),
                    ))
                }
                JobState::Queued => {}
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(WorkerError::Job(
                    "timed out waiting for running state".into(),
                ));
            }
            let (next, timed) = self
                .shared
                .changed
                .wait_timeout(status, deadline - now)
                .unwrap_or_else(|e| e.into_inner());
            status = next;
            if timed.timed_out() && status.state != JobState::Running {
                return Err(WorkerError::Job(
                    "timed out waiting for running state".into(),
                ));
            }
        }
    }
}

/// Single-worker model cache. A worker task can lazily install a typed model by
/// stable identity; subsequent jobs reuse the exact `Arc` without a second load.
#[derive(Default)]
pub struct OrtModelRegistry {
    models: Mutex<HashMap<String, ErasedResult>>,
}

impl OrtModelRegistry {
    pub fn get_or_try_init<T, F>(&self, key: &str, load: F) -> Result<Arc<T>, WorkerError>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Result<T, WorkerError>,
    {
        let mut models = self.models.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = models.get(key) {
            return existing
                .clone()
                .downcast::<T>()
                .map_err(|_| WorkerError::ResultType);
        }
        let model = Arc::new(load()?);
        models.insert(key.to_string(), model.clone());
        Ok(model)
    }
}

impl OrtWorker {
    /// Spawn one worker with a hard bounded admission queue.
    pub fn spawn(export_pause: ExportPause, capacity: usize) -> Self {
        let (sender, receiver) = mpsc::sync_channel(capacity.max(1));
        let inner = Arc::new(WorkerInner {
            sender,
            dedupe: Mutex::new(HashMap::new()),
            capacity: capacity.max(1),
            queued: AtomicUsize::new(0),
            sequence: AtomicU64::new(0),
            active: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            thread: Mutex::new(None),
        });
        let thread_inner = inner.clone();
        let thread = std::thread::Builder::new()
            .name("opentake-ort-worker".into())
            .spawn(move || worker_loop(thread_inner, receiver, export_pause))
            .expect("spawn bounded inference worker");
        *inner.thread.lock().unwrap_or_else(|e| e.into_inner()) = Some(thread);
        Self { inner }
    }

    /// Submit a typed job. A live duplicate key reuses the same result and does
    /// not consume queue capacity or execute a second task.
    pub fn submit<T, F>(&self, request: JobRequest, task: F) -> Result<JobHandle<T>, WorkerError>
    where
        T: Clone + Send + Sync + 'static,
        F: FnOnce(&OrtModelRegistry, &CancelToken) -> Result<T, WorkerError> + Send + 'static,
    {
        if self.inner.shutdown.load(Ordering::SeqCst) {
            return Err(WorkerError::Shutdown);
        }

        let mut dedupe = self.inner.dedupe.lock().unwrap_or_else(|e| e.into_inner());
        dedupe.retain(|_, weak| weak.strong_count() > 0);
        if let Some(shared) = dedupe.get(&request.dedupe_key).and_then(Weak::upgrade) {
            return Ok(JobHandle {
                shared,
                marker: PhantomData,
            });
        }

        if self
            .inner
            .queued
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |queued| {
                (queued < self.inner.capacity).then_some(queued + 1)
            })
            .is_err()
        {
            return Err(WorkerError::QueueFull);
        }

        let key = request.dedupe_key.clone();
        let shared = Arc::new(SharedJob::new(request));
        dedupe.insert(key.clone(), Arc::downgrade(&shared));
        let sequence = self.inner.sequence.fetch_add(1, Ordering::SeqCst);
        let erased: JobTask = Box::new(move |models, cancel| {
            task(models, cancel).map(|value| Arc::new(value) as ErasedResult)
        });
        let message = WorkerMessage::Job(QueuedJob {
            sequence,
            shared: shared.clone(),
            task: Some(erased),
        });
        match self.inner.sender.try_send(message) {
            Ok(()) => Ok(JobHandle {
                shared,
                marker: PhantomData,
            }),
            Err(TrySendError::Full(_)) => {
                self.inner.queued.fetch_sub(1, Ordering::SeqCst);
                dedupe.remove(&key);
                Err(WorkerError::QueueFull)
            }
            Err(TrySendError::Disconnected(_)) => {
                self.inner.queued.fetch_sub(1, Ordering::SeqCst);
                dedupe.remove(&key);
                Err(WorkerError::Shutdown)
            }
        }
    }

    pub fn active_jobs(&self) -> usize {
        self.inner.active.load(Ordering::SeqCst)
    }

    pub fn queued_jobs(&self) -> usize {
        self.inner.queued.load(Ordering::SeqCst)
    }

    /// Cancel queued work, wait for the cooperative running job, and join the
    /// sole worker thread. Idempotent.
    pub fn shutdown(&self) -> Result<(), WorkerError> {
        if !self.inner.shutdown.swap(true, Ordering::SeqCst) {
            // A full channel is not an error: the worker observes the atomic
            // shutdown flag at its next cancellation/dispatch boundary.
            let _ = self.inner.sender.try_send(WorkerMessage::Shutdown);
        }
        if let Some(thread) = self
            .inner
            .thread
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            thread.join().map_err(|_| WorkerError::Panicked)?;
        }
        Ok(())
    }
}

fn worker_loop(inner: Arc<WorkerInner>, receiver: Receiver<WorkerMessage>, pause: ExportPause) {
    let registry = OrtModelRegistry::default();
    let mut pending = Vec::<QueuedJob>::new();
    let mut high_streak = 0usize;

    loop {
        if pending.is_empty() {
            match receiver.recv() {
                Ok(WorkerMessage::Job(job)) => pending.push(job),
                Ok(WorkerMessage::Shutdown) | Err(_) => break,
            }
        }

        let mut shutdown = false;
        loop {
            match receiver.try_recv() {
                Ok(WorkerMessage::Job(job)) => pending.push(job),
                Ok(WorkerMessage::Shutdown) => {
                    shutdown = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    shutdown = true;
                    break;
                }
            }
        }
        if shutdown || inner.shutdown.load(Ordering::SeqCst) {
            cancel_pending(&pending);
            break;
        }

        if !pause.wait_while_active(|| inner.shutdown.load(Ordering::SeqCst)) {
            cancel_pending(&pending);
            break;
        }

        // Requests may arrive while the worker is pressure-gated. Re-drain at
        // the scheduling boundary so their priority participates immediately.
        loop {
            match receiver.try_recv() {
                Ok(WorkerMessage::Job(job)) => pending.push(job),
                Ok(WorkerMessage::Shutdown) => {
                    cancel_pending(&pending);
                    inner.active.store(0, Ordering::SeqCst);
                    return;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    cancel_pending(&pending);
                    inner.active.store(0, Ordering::SeqCst);
                    return;
                }
            }
        }

        // Four high-priority jobs is the starvation bound. Otherwise choose the
        // oldest job at the highest available priority (FIFO within priority).
        let has_background = pending
            .iter()
            .any(|job| job.shared.request.priority == JobPriority::Background);
        let force_background = has_background && high_streak >= 4;
        let selected_priority = if force_background {
            JobPriority::Background
        } else {
            pending
                .iter()
                .map(|job| job.shared.request.priority)
                .max()
                .unwrap_or(JobPriority::Background)
        };
        let index = pending
            .iter()
            .enumerate()
            .filter(|(_, job)| job.shared.request.priority == selected_priority)
            .min_by_key(|(_, job)| job.sequence)
            .map(|(index, _)| index)
            .expect("pending queue is not empty");
        let mut job = pending.swap_remove(index);
        inner.queued.fetch_sub(1, Ordering::SeqCst);
        if selected_priority == JobPriority::Interactive {
            high_streak += 1;
        } else {
            high_streak = 0;
        }

        if !job.shared.set_running() {
            remove_dedupe(&inner, &job.shared);
            job.shared.finish(Err(WorkerError::Cancelled));
            continue;
        }
        inner.active.fetch_add(1, Ordering::SeqCst);
        let task = job.task.take().expect("queued job owns one task");
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            task(&registry, &job.shared.cancel)
        }))
        .unwrap_or(Err(WorkerError::Panicked));
        inner.active.fetch_sub(1, Ordering::SeqCst);
        remove_dedupe(&inner, &job.shared);
        job.shared.finish(outcome);
    }

    inner.active.store(0, Ordering::SeqCst);
    inner.queued.store(0, Ordering::SeqCst);
}

fn cancel_pending(pending: &[QueuedJob]) {
    for job in pending {
        job.shared.cancel.cancel();
        job.shared.finish(Err(WorkerError::Cancelled));
    }
}

fn remove_dedupe(inner: &WorkerInner, shared: &Arc<SharedJob>) {
    let mut dedupe = inner.dedupe.lock().unwrap_or_else(|e| e.into_inner());
    if dedupe
        .get(&shared.request.dedupe_key)
        .and_then(Weak::upgrade)
        .is_some_and(|current| Arc::ptr_eq(&current, shared))
    {
        dedupe.remove(&shared.request.dedupe_key);
    }
}

#[cfg(feature = "ort-backend")]
mod model {
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Mutex;

    use ndarray::{ArrayD, IxDyn};
    use ort::session::Session;
    use ort::value::Tensor;

    use super::ExecutionProvider;
    use crate::error::{MediaError, Result};

    pub type OrtIoContract = (Vec<(String, String)>, Vec<(String, String)>);

    /// A loaded ONNX model + a CPU-fallback-friendly session. `Session` is not
    /// `Sync`; wrap in a `Mutex` so the worker can share it.
    pub struct OrtModel {
        session: Mutex<Session>,
    }

    impl OrtModel {
        /// Load `path` with the given EP preference, falling back to CPU.
        pub fn load(path: &Path, _ep: ExecutionProvider) -> Result<Self> {
            crate::initialize_ort_backend();
            let builder =
                Session::builder().map_err(|e| MediaError::ModelInstall(format!("ort: {e}")))?;
            let builder = builder
                .with_intra_threads(
                    std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(4),
                )
                .map_err(|e| MediaError::ModelInstall(format!("ort threads: {e}")))?;
            let session = builder
                .commit_from_file(path)
                .map_err(|e| MediaError::ModelInstall(format!("ort load: {e}")))?;
            Ok(OrtModel {
                session: Mutex::new(session),
            })
        }

        /// Run inference with named f32 inputs, returning named f32 outputs as
        /// dynamic-dim arrays.
        pub fn run_f32(
            &self,
            inputs: Vec<(String, ArrayD<f32>)>,
        ) -> Result<HashMap<String, ArrayD<f32>>> {
            let mut session = self.session.lock().unwrap();
            let mut tensors = Vec::with_capacity(inputs.len());
            for (name, arr) in inputs {
                let t = Tensor::from_array(arr)
                    .map_err(|e| MediaError::Decode(format!("ort tensor: {e}")))?;
                tensors.push((name, t));
            }
            let session_inputs: Vec<(&str, ort::session::SessionInputValue)> = tensors
                .iter()
                .map(|(n, t)| (n.as_str(), t.into()))
                .collect();
            let outputs = session
                .run(session_inputs)
                .map_err(|e| MediaError::Decode(format!("ort run: {e}")))?;

            let mut out = HashMap::new();
            for (name, value) in outputs.iter() {
                if let Ok((shape, data)) = value.try_extract_tensor::<f32>() {
                    let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
                    if let Ok(arr) = ArrayD::from_shape_vec(IxDyn(&dims), data.to_vec()) {
                        out.insert(name.to_string(), arr);
                    }
                }
            }
            Ok(out)
        }

        /// Names and debug-formatted tensor contracts declared by the model.
        /// Used to fail closed when a downloaded advanced model does not match
        /// the pinned architecture before any user media reaches inference.
        pub fn io_contract(&self) -> OrtIoContract {
            let session = self.session.lock().unwrap();
            let inputs = session
                .inputs()
                .iter()
                .map(|input| (input.name().to_owned(), format!("{:?}", input.dtype())))
                .collect();
            let outputs = session
                .outputs()
                .iter()
                .map(|output| (output.name().to_owned(), format!("{:?}", output.dtype())))
                .collect();
            (inputs, outputs)
        }
    }
}

#[cfg(feature = "ort-backend")]
pub use model::{OrtIoContract, OrtModel};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_default_is_a_known_provider() {
        let ep = ExecutionProvider::platform_default();
        assert!(matches!(
            ep,
            ExecutionProvider::CoreML | ExecutionProvider::DirectMl | ExecutionProvider::Cpu
        ));
    }

    #[test]
    fn io_spec_default_is_empty() {
        let s = IoSpec::default();
        assert!(s.inputs.is_empty() && s.outputs.is_empty());
    }

    #[test]
    fn io_tensor_carries_dynamic_dims() {
        let t = IoTensor {
            name: "pixel_values".into(),
            dtype: TensorDType::F32,
            shape: vec![-1, 3, 256, 256],
        };
        assert_eq!(t.shape[0], -1);
        assert_eq!(t.dtype, TensorDType::F32);
    }

    #[cfg(feature = "ort-backend")]
    #[test]
    fn installed_advanced_model_contract_can_be_inspected_before_inference() {
        let Some(path) = std::env::var_os("OPENTAKE_TEST_ONNX_MODEL") else {
            return;
        };
        let model = OrtModel::load(
            std::path::Path::new(&path),
            ExecutionProvider::platform_default(),
        )
        .expect("load supplied ONNX model");
        let contract = model.io_contract();
        assert!(!contract.0.is_empty());
        assert!(!contract.1.is_empty());
        eprintln!("ONNX_IO_CONTRACT={contract:?}");
    }
}
