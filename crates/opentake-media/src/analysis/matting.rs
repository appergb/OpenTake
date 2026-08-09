//! Verified local Robust Video Matting (RVM) model and frame inference.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(feature = "model-download")]
use std::sync::Arc;

use sha2::{Digest, Sha256};

#[cfg(any(feature = "ort-backend", feature = "model-download"))]
use crate::MediaCancelToken;
#[cfg(feature = "ort-backend")]
use crate::RgbaFrame;
use crate::{MediaError, Result};

pub const RVM_MODEL_ID: &str = "rvm-mobilenetv3-fp32-v1.0.0";
pub const RVM_MODEL_FILE: &str = "rvm_mobilenetv3_fp32.onnx";
pub const RVM_MODEL_SHA256: &str =
    "88d4531297118f595bf2fd60f6f566aec2e559393802d1f436c380f0cbbd2828";
pub const RVM_MODEL_BYTES: u64 = 14_975_696;
pub const RVM_MODEL_URL: &str = "https://github.com/PeterL1n/RobustVideoMatting/releases/download/v1.0.0/rvm_mobilenetv3_fp32.onnx";

#[cfg(feature = "model-download")]
pub type MattingDownloadProgress = Arc<dyn Fn(u64, u64) + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledMattingModel {
    pub id: String,
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlphaMatteFrame {
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
    /// Model-cleaned straight foreground RGB, three bytes per pixel.
    pub foreground_rgb: Vec<u8>,
}

pub fn matting_model_path(model_dir: &Path) -> PathBuf {
    model_dir.join("matting").join(RVM_MODEL_FILE)
}

pub fn verify_rvm_model(model_dir: &Path) -> Result<InstalledMattingModel> {
    let path = matting_model_path(model_dir);
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        MediaError::ModelInstall(format!("matting_model_not_installed:{error}"))
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(MediaError::ModelInstall(
            "matting_model_must_be_a_regular_file".to_string(),
        ));
    }
    if metadata.len() != RVM_MODEL_BYTES {
        return Err(MediaError::Checksum(format!(
            "matting_model_size_mismatch: expected {RVM_MODEL_BYTES}, got {}",
            metadata.len()
        )));
    }
    let mut file = File::open(&path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let actual = format!("{:x}", digest.finalize());
    if actual != RVM_MODEL_SHA256 {
        return Err(MediaError::Checksum(format!(
            "matting_model_integrity_failed: expected {RVM_MODEL_SHA256}, got {actual}"
        )));
    }
    Ok(InstalledMattingModel {
        id: RVM_MODEL_ID.to_string(),
        path,
        sha256: actual,
        bytes: metadata.len(),
    })
}

#[cfg(feature = "model-download")]
pub async fn download_rvm_model(
    model_dir: &Path,
    cancel: &MediaCancelToken,
    progress: Option<MattingDownloadProgress>,
) -> Result<InstalledMattingModel> {
    use std::io::Write;

    use futures_util::StreamExt;

    if cancel.checkpoint() {
        return Err(MediaError::Cancelled);
    }
    let destination = matting_model_path(model_dir);
    if destination.exists() {
        return verify_rvm_model(model_dir);
    }
    let parent = destination
        .parent()
        .ok_or_else(|| MediaError::ModelInstall("matting_model_path_invalid".to_string()))?;
    std::fs::create_dir_all(parent)?;
    let parent_metadata = std::fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir() || parent_metadata.file_type().is_symlink() {
        return Err(MediaError::ModelInstall(
            "matting_model_directory_must_be_regular".to_string(),
        ));
    }
    let install = async {
        let response = reqwest::Client::new()
            .get(RVM_MODEL_URL)
            .send()
            .await
            .map_err(|error| MediaError::ModelInstall(format!("matting_model_download:{error}")))?
            .error_for_status()
            .map_err(|error| MediaError::ModelInstall(format!("matting_model_download:{error}")))?;
        if response
            .content_length()
            .is_some_and(|bytes| bytes != RVM_MODEL_BYTES)
        {
            return Err(MediaError::Checksum(
                "matting_model_content_length_mismatch".to_string(),
            ));
        }
        let mut partial = tempfile::Builder::new()
            .prefix(".rvm-model-")
            .suffix(".partial")
            .tempfile_in(parent)
            .map_err(|error| {
                MediaError::ModelInstall(format!("matting_model_partial_create:{error}"))
            })?;
        let mut digest = Sha256::new();
        let mut downloaded = 0_u64;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if cancel.checkpoint() {
                return Err(MediaError::Cancelled);
            }
            let chunk = chunk.map_err(|error| {
                MediaError::ModelInstall(format!("matting_model_download:{error}"))
            })?;
            downloaded = downloaded
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| MediaError::ModelInstall("matting_model_too_large".to_string()))?;
            if downloaded > RVM_MODEL_BYTES {
                return Err(MediaError::Checksum(
                    "matting_model_download_exceeds_manifest".to_string(),
                ));
            }
            digest.update(&chunk);
            partial.as_file_mut().write_all(&chunk)?;
            if let Some(progress) = &progress {
                progress(downloaded, RVM_MODEL_BYTES);
            }
        }
        partial.as_file().sync_all()?;
        if downloaded != RVM_MODEL_BYTES {
            return Err(MediaError::Checksum(format!(
                "matting_model_size_mismatch: expected {RVM_MODEL_BYTES}, got {downloaded}"
            )));
        }
        let actual = format!("{:x}", digest.finalize());
        if actual != RVM_MODEL_SHA256 {
            return Err(MediaError::Checksum(format!(
                "matting_model_integrity_failed: expected {RVM_MODEL_SHA256}, got {actual}"
            )));
        }
        partial.persist_noclobber(&destination).map_err(|error| {
            MediaError::ModelInstall(format!("matting_model_publish:{}", error.error))
        })?;
        Ok(())
    }
    .await;
    install?;
    verify_rvm_model(model_dir)
}

#[cfg(feature = "ort-backend")]
pub struct RvmMattingSession {
    model: crate::ort_worker::OrtModel,
    recurrent: [ndarray::ArrayD<f32>; 4],
    pub installed: InstalledMattingModel,
}

#[cfg(feature = "ort-backend")]
impl RvmMattingSession {
    pub fn load(model_dir: &Path) -> Result<Self> {
        use ndarray::{ArrayD, IxDyn};

        let installed = verify_rvm_model(model_dir)?;
        let model = crate::ort_worker::OrtModel::load(
            &installed.path,
            crate::ort_worker::ExecutionProvider::platform_default(),
        )?;
        let (inputs, outputs) = model.io_contract();
        let input_names = inputs
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        let output_names = outputs
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        if input_names != ["src", "r1i", "r2i", "r3i", "r4i", "downsample_ratio"]
            || output_names != ["fgr", "pha", "r1o", "r2o", "r3o", "r4o"]
        {
            return Err(MediaError::ModelInstall(
                "matting_model_io_contract_mismatch".to_string(),
            ));
        }
        let empty = || ArrayD::zeros(IxDyn(&[1, 1, 1, 1]));
        Ok(Self {
            model,
            recurrent: [empty(), empty(), empty(), empty()],
            installed,
        })
    }

    pub fn reset_temporal_state(&mut self) {
        use ndarray::{ArrayD, IxDyn};
        self.recurrent = std::array::from_fn(|_| ArrayD::zeros(IxDyn(&[1, 1, 1, 1])));
    }

    pub fn infer(
        &mut self,
        frame: &RgbaFrame,
        cancel: &MediaCancelToken,
    ) -> Result<AlphaMatteFrame> {
        use ndarray::{ArrayD, IxDyn};

        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        let width = frame.width as usize;
        let height = frame.height as usize;
        let mut src = vec![0.0_f32; 3 * width * height];
        for (index, pixel) in frame.rgba.chunks_exact(4).enumerate() {
            src[index] = pixel[0] as f32 / 255.0;
            src[width * height + index] = pixel[1] as f32 / 255.0;
            src[2 * width * height + index] = pixel[2] as f32 / 255.0;
        }
        let src = ArrayD::from_shape_vec(IxDyn(&[1, 3, height, width]), src)
            .map_err(|error| MediaError::Decode(format!("matting_input_shape:{error}")))?;
        let mut outputs = self.model.run_f32(vec![
            ("src".to_string(), src),
            ("r1i".to_string(), self.recurrent[0].clone()),
            ("r2i".to_string(), self.recurrent[1].clone()),
            ("r3i".to_string(), self.recurrent[2].clone()),
            ("r4i".to_string(), self.recurrent[3].clone()),
            (
                "downsample_ratio".to_string(),
                ArrayD::from_shape_vec(IxDyn(&[1]), vec![0.25]).expect("fixed ratio shape"),
            ),
        ])?;
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        for (index, name) in ["r1o", "r2o", "r3o", "r4o"].into_iter().enumerate() {
            self.recurrent[index] = outputs.remove(name).ok_or_else(|| {
                MediaError::Decode(format!("matting_model_missing_output:{name}"))
            })?;
        }
        let foreground = outputs
            .remove("fgr")
            .ok_or_else(|| MediaError::Decode("matting_model_missing_output:fgr".to_string()))?;
        if foreground.shape() != [1, 3, height, width] {
            return Err(MediaError::Decode(format!(
                "matting_foreground_shape_mismatch:{:?}",
                foreground.shape()
            )));
        }
        let foreground = foreground
            .as_slice()
            .ok_or_else(|| MediaError::Decode("matting_foreground_not_contiguous".to_string()))?;
        let alpha = outputs
            .remove("pha")
            .ok_or_else(|| MediaError::Decode("matting_model_missing_output:pha".to_string()))?;
        if alpha.shape() != [1, 1, height, width] {
            return Err(MediaError::Decode(format!(
                "matting_alpha_shape_mismatch:{:?}",
                alpha.shape()
            )));
        }
        let alpha = alpha
            .iter()
            .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect();
        let plane = width * height;
        let foreground_rgb = (0..plane)
            .flat_map(|index| {
                [
                    foreground[index],
                    foreground[plane + index],
                    foreground[2 * plane + index],
                ]
                .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
            })
            .collect();
        Ok(AlphaMatteFrame {
            width: frame.width,
            height: frame.height,
            alpha,
            foreground_rgb,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_is_a_typed_install_error() {
        let root = tempfile::tempdir().unwrap();
        let error = verify_rvm_model(root.path()).expect_err("model must be absent");
        assert!(matches!(error, MediaError::ModelInstall(_)));
        assert!(error.to_string().contains("matting_model_not_installed"));
    }

    #[cfg(feature = "model-download")]
    #[test]
    fn pre_cancelled_download_never_creates_a_partial_model() {
        use futures_util::FutureExt;

        let root = tempfile::tempdir().unwrap();
        let cancel = MediaCancelToken::new();
        cancel.cancel();
        let error = download_rvm_model(root.path(), &cancel, None)
            .now_or_never()
            .expect("pre-cancelled install completes without polling the network")
            .expect_err("pre-cancelled install must fail before network access");
        assert!(matches!(error, MediaError::Cancelled));
        assert!(!matting_model_path(root.path()).exists());
    }

    #[cfg(feature = "ort-backend")]
    #[test]
    fn official_rvm_model_returns_frame_aligned_alpha() {
        let Some(source) = std::env::var_os("OPENTAKE_TEST_RVM_MODEL") else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let destination = matting_model_path(root.path());
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(source, destination).unwrap();
        let mut session = RvmMattingSession::load(root.path()).expect("load verified RVM");
        let mut frame = RgbaFrame::black(64, 64);
        for y in 8..56 {
            for x in 16..48 {
                let index = ((y * 64 + x) * 4) as usize;
                frame.rgba[index..index + 4].copy_from_slice(&[210, 160, 130, 255]);
            }
        }
        let matte = session
            .infer(&frame, &MediaCancelToken::new())
            .expect("infer alpha");
        assert_eq!((matte.width, matte.height), (64, 64));
        assert_eq!(matte.alpha.len(), 64 * 64);
        assert_eq!(matte.foreground_rgb.len(), 64 * 64 * 3);
    }
}
