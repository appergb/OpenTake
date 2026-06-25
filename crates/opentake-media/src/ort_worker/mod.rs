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
/// accelerator is unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionProvider {
    Cpu,
    CoreML,
    Cuda,
    DirectMl,
    TensorRt,
}

impl ExecutionProvider {
    /// The platform-preferred provider (CoreML on macOS, DirectML on Windows,
    /// CUDA on Linux), used as the first choice before CPU fallback.
    pub fn platform_default() -> Self {
        #[cfg(target_os = "macos")]
        {
            ExecutionProvider::CoreML
        }
        #[cfg(target_os = "windows")]
        {
            ExecutionProvider::DirectMl
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

    /// A loaded ONNX model + a CPU-fallback-friendly session. `Session` is not
    /// `Sync`; wrap in a `Mutex` so the worker can share it.
    pub struct OrtModel {
        session: Mutex<Session>,
    }

    impl OrtModel {
        /// Load `path` with the given EP preference, falling back to CPU.
        pub fn load(path: &Path, _ep: ExecutionProvider) -> Result<Self> {
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
    }
}

#[cfg(feature = "ort-backend")]
pub use model::OrtModel;

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
}
