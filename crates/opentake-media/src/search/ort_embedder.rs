//! SigLIP2 embedder using native ORT or fixed-shape tract on Windows. Implements
//! the [`Embedder`] trait; the default build and tests use the mock instead.
//!
//! Image input is `NCHW` f32 `(1,3,256,256)` (mean/std from
//! `crate::search::embedder`); text input is int64 `(1,context_length)`,
//! right-padded with 0. Output is a `(1, embedding_dim)` f32 vector; we assert
//! the length matches the spec, mirroring upstream `vector(from:dim:)`.
//!
//! IO defaults match the pinned ONNX Community graphs: `pixel_values` /
//! `input_ids` → `pooler_output`. Features are L2-normalized for cosine ranking.
//! Windows binds the known input shapes in memory; downloaded assets stay intact.

use std::path::Path;
use std::sync::Mutex;

#[cfg(not(target_os = "windows"))]
use ndarray::Array2;
use ndarray::Array4;
#[cfg(not(target_os = "windows"))]
use ort::{session::Session, value::Tensor};
#[cfg(target_os = "windows")]
use tract_backend::Encoder as Session;

use super::embedder::{
    l2_normalize, preprocess_image, Embedder, EmbedderSpec, SIGLIP_MEAN, SIGLIP_STD,
};
use super::tokenizer::SiglipTokenizer;
use crate::error::{MediaError, Result};
use crate::frame::RgbaFrame;

/// ONNX graph IO tensor names.
#[derive(Clone, Debug)]
pub struct IoNames {
    pub image_input: String,
    pub image_output: String,
    pub text_input: String,
    pub text_output: String,
}

impl Default for IoNames {
    fn default() -> Self {
        // ONNX Community separate encoder graphs expose pooled, unnormalized features.
        IoNames {
            image_input: "pixel_values".into(),
            image_output: "pooler_output".into(),
            text_input: "input_ids".into(),
            text_output: "pooler_output".into(),
        }
    }
}

/// ONNX-backed SigLIP2 embedder. `Session` is not `Sync`, so each is behind a
/// `Mutex` to satisfy the `Embedder: Send + Sync` bound.
pub struct OrtEmbedder {
    image: Mutex<Session>,
    text: Mutex<Session>,
    tokenizer: SiglipTokenizer,
    spec: EmbedderSpec,
    #[cfg(not(target_os = "windows"))]
    io: IoNames,
}

impl OrtEmbedder {
    /// Load image+text encoders and the tokenizer for `spec` from disk. Uses the
    /// platform's default execution provider, falling back to CPU.
    pub fn new(
        image_encoder: &Path,
        text_encoder: &Path,
        tokenizer_json: &Path,
        spec: EmbedderSpec,
    ) -> Result<Self> {
        Self::with_io(
            image_encoder,
            text_encoder,
            tokenizer_json,
            spec,
            IoNames::default(),
        )
    }

    pub fn with_io(
        image_encoder: &Path,
        text_encoder: &Path,
        tokenizer_json: &Path,
        spec: EmbedderSpec,
        io: IoNames,
    ) -> Result<Self> {
        #[cfg(not(target_os = "windows"))]
        let image = build_session(image_encoder)?;
        #[cfg(not(target_os = "windows"))]
        let text = build_session(text_encoder)?;
        #[cfg(target_os = "windows")]
        let image = Session::load(
            image_encoder,
            &io.image_input,
            &io.image_output,
            tract_onnx::prelude::DatumType::F32,
            &[1, 3, spec.image_size as usize, spec.image_size as usize],
            spec.embedding_dim,
        )?;
        #[cfg(target_os = "windows")]
        let text = Session::load(
            text_encoder,
            &io.text_input,
            &io.text_output,
            tract_onnx::prelude::DatumType::I64,
            &[1, spec.context_length],
            spec.embedding_dim,
        )?;
        let tokenizer = SiglipTokenizer::from_file(tokenizer_json, spec.context_length)?;
        Ok(OrtEmbedder {
            image: Mutex::new(image),
            text: Mutex::new(text),
            tokenizer,
            spec,
            #[cfg(not(target_os = "windows"))]
            io,
        })
    }

    fn finalize(&self, v: Vec<f32>) -> Result<Vec<f32>> {
        finalize_embedding(v, &self.spec)
    }
}

fn finalize_embedding(mut v: Vec<f32>, spec: &EmbedderSpec) -> Result<Vec<f32>> {
    if v.len() != spec.embedding_dim || v.iter().any(|x| !x.is_finite()) {
        return Err(MediaError::BadModelOutput);
    }
    let norm_squared = v.iter().map(|x| x * x).sum::<f32>();
    if !norm_squared.is_finite() || norm_squared <= f32::EPSILON {
        return Err(MediaError::BadModelOutput);
    }
    if !spec.normalized {
        l2_normalize(&mut v);
    }
    Ok(v)
}

#[cfg(not(target_os = "windows"))]
fn build_session(path: &Path) -> Result<Session> {
    crate::initialize_ort_backend();
    let builder = Session::builder().map_err(|e| MediaError::ModelInstall(format!("ort: {e}")))?;
    // Default EP set; ort falls back to CPU when an accelerator is unavailable.
    let builder = builder
        .with_intra_threads(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        )
        .map_err(|e| MediaError::ModelInstall(format!("ort threads: {e}")))?;
    builder
        .commit_from_file(path)
        .map_err(|e| MediaError::ModelInstall(format!("ort load {}: {e}", path.display())))
}

#[cfg(not(target_os = "windows"))]
fn extract_vec(value: &ort::value::Value) -> Result<Vec<f32>> {
    let (_, data) = value
        .try_extract_tensor::<f32>()
        .map_err(|_| MediaError::BadModelOutput)?;
    Ok(data.to_vec())
}

impl Embedder for OrtEmbedder {
    fn spec(&self) -> &EmbedderSpec {
        &self.spec
    }

    fn encode_image(&self, frame: &RgbaFrame) -> Result<Vec<f32>> {
        let tensor: Array4<f32> =
            preprocess_image(frame, self.spec.image_size, SIGLIP_MEAN, SIGLIP_STD);
        #[cfg(not(target_os = "windows"))]
        let v = {
            let input = Tensor::from_array(tensor)
                .map_err(|e| MediaError::Decode(format!("ort tensor: {e}")))?;
            let mut session = self.image.lock().unwrap();
            let outputs = session
                .run(ort::inputs![self.io.image_input.as_str() => input])
                .map_err(|e| MediaError::Decode(format!("ort run image: {e}")))?;
            let value = outputs
                .get(self.io.image_output.as_str())
                .ok_or(MediaError::BadModelOutput)?;
            extract_vec(value)?
        };
        #[cfg(target_os = "windows")]
        let v = self
            .image
            .lock()
            .map_err(|_| MediaError::ModelInstall("image session poisoned".into()))?
            .run(
                tensor.shape(),
                tensor.as_slice().ok_or(MediaError::BadModelOutput)?,
            )?;
        self.finalize(v)
    }

    fn encode_text(&self, text: &str) -> Result<Vec<f32>> {
        let ids = self.tokenizer.tokenize(text)?;
        #[cfg(not(target_os = "windows"))]
        let v = {
            let arr = Array2::from_shape_vec((1, ids.len()), ids)
                .map_err(|e| MediaError::Decode(format!("ort text shape: {e}")))?;
            let input = Tensor::from_array(arr)
                .map_err(|e| MediaError::Decode(format!("ort tensor: {e}")))?;
            let mut session = self.text.lock().unwrap();
            let outputs = session
                .run(ort::inputs![self.io.text_input.as_str() => input])
                .map_err(|e| MediaError::Decode(format!("ort run text: {e}")))?;
            let value = outputs
                .get(self.io.text_output.as_str())
                .ok_or(MediaError::BadModelOutput)?;
            extract_vec(value)?
        };
        #[cfg(target_os = "windows")]
        let v = self
            .text
            .lock()
            .map_err(|_| MediaError::ModelInstall("text session poisoned".into()))?
            .run(&[1, ids.len()], &ids)?;
        self.finalize(v)
    }
}

// The ort-tract adapter cannot bind input dimensions and rejects ORT thread
// options. Use its same pinned tract engine directly for these fixed encoders.
#[cfg(target_os = "windows")]
mod tract_backend {
    use super::*;
    use tract_onnx::prelude::*;
    use tract_onnx::tract_hir::infer::{Factoid, ShapeFactoid};

    pub(super) struct Encoder {
        plan: TypedRunnableModel<TypedModel>,
        shape: Vec<usize>,
        datum_type: DatumType,
        embedding_dim: usize,
    }

    impl Encoder {
        pub(super) fn load(
            path: &Path,
            input_name: &str,
            output_name: &str,
            datum_type: DatumType,
            shape: &[usize],
            embedding_dim: usize,
        ) -> Result<Self> {
            let result = (|| -> TractResult<Self> {
                let mut model = tract_onnx::onnx().model_for_path(path)?;
                bind_input(&mut model, input_name, datum_type, shape)?;
                let model = model.with_output_names([output_name])?.into_optimized()?;
                let output = model.output_fact(0)?;
                anyhow::ensure!(
                    output.datum_type == DatumType::F32
                        && output.shape.as_concrete() == Some(&[1, embedding_dim]),
                    "expected pooled float32 output [1, {embedding_dim}], got {output:?}"
                );
                Ok(Self {
                    plan: model.into_runnable()?,
                    shape: shape.to_vec(),
                    datum_type,
                    embedding_dim,
                })
            })();
            result.map_err(|e| {
                MediaError::ModelInstall(format!("tract load {}: {e:#}", path.display()))
            })
        }

        pub(super) fn run<T: Datum + Copy>(&self, shape: &[usize], data: &[T]) -> Result<Vec<f32>> {
            if shape != self.shape || T::datum_type() != self.datum_type {
                return Err(MediaError::BadModelOutput);
            }
            let input = tract_onnx::prelude::Tensor::from_shape(shape, data)
                .map_err(|e| MediaError::Decode(format!("tract input: {e:#}")))?;
            let output = self
                .plan
                .run(tvec!(input.into()))
                .map_err(|e| MediaError::Decode(format!("tract run: {e:#}")))?;
            if output.len() != 1 || output[0].shape() != [1, self.embedding_dim] {
                return Err(MediaError::BadModelOutput);
            }
            output[0]
                .as_slice::<f32>()
                .map(|v| v.to_vec())
                .map_err(|_| MediaError::BadModelOutput)
        }
    }

    fn bind_input(
        model: &mut InferenceModel,
        input_name: &str,
        datum_type: DatumType,
        shape: &[usize],
    ) -> TractResult<()> {
        let inputs = model.input_outlets()?;
        anyhow::ensure!(
            inputs.len() == 1 && model.node(inputs[0].node).name == input_name,
            "expected a single input named {input_name}"
        );
        let input = model.input_fact(0)?;
        anyhow::ensure!(
            input.datum_type.concretize() == Some(datum_type),
            "unexpected input type"
        );
        anyhow::ensure!(
            input.shape.rank().concretize() == Some(shape.len() as i64),
            "unexpected input rank"
        );
        for (dimension, expected) in input.shape.dims().zip(shape) {
            if let Some(dimension) = dimension.concretize().and_then(|d| d.to_i64().ok()) {
                anyhow::ensure!(
                    dimension == *expected as i64,
                    "input dimension conflicts with model spec"
                );
            }
        }
        // The exporter includes symbolic expressions in value_info. Tract 0.22
        // parses their division precedence incorrectly. Re-infer only dynamic
        // dimensions from the fixed input; retain dtype, rank and static checks.
        for node in model.nodes_mut() {
            for output in &mut node.outputs {
                let dims = output
                    .fact
                    .shape
                    .dims()
                    .map(|dimension| {
                        if dimension.concretize().is_some_and(|d| d.to_i64().is_ok()) {
                            dimension.clone()
                        } else {
                            Default::default()
                        }
                    })
                    .collect();
                output.fact.shape = if output.fact.shape.is_open() {
                    ShapeFactoid::open(dims)
                } else {
                    ShapeFactoid::closed(dims)
                };
            }
        }
        model.set_input_fact(0, TypedFact::dt_shape(datum_type, shape).into())?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn image_model() -> InferenceModel {
            let mut model = InferenceModel::default();
            let batch = model.symbols.sym("batch_size");
            let shape: [TDim; 4] = [batch.into(), 3.into(), 256.into(), 256.into()];
            model
                .add_source("pixel_values", f32::fact(shape).into())
                .unwrap();
            model
        }

        #[test]
        fn binds_dynamic_input_without_relaxing_static_dimensions() {
            let mut model = image_model();
            bind_input(
                &mut model,
                "pixel_values",
                DatumType::F32,
                &[1, 3, 256, 256],
            )
            .unwrap();
            assert_eq!(
                model
                    .input_fact(0)
                    .unwrap()
                    .shape
                    .as_concrete_finite()
                    .unwrap()
                    .unwrap()
                    .as_slice(),
                &[1, 3, 256, 256]
            );
            assert!(bind_input(
                &mut image_model(),
                "pixel_values",
                DatumType::F32,
                &[1, 3, 128, 128]
            )
            .is_err());
        }

        #[test]
        fn rejects_wrong_input_name_type_and_rank() {
            assert!(bind_input(
                &mut image_model(),
                "wrong",
                DatumType::F32,
                &[1, 3, 256, 256]
            )
            .is_err());
            assert!(bind_input(
                &mut image_model(),
                "pixel_values",
                DatumType::I64,
                &[1, 3, 256, 256]
            )
            .is_err());
            assert!(bind_input(
                &mut image_model(),
                "pixel_values",
                DatumType::F32,
                &[1, 768]
            )
            .is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_io_names_match_pinned_onnx() {
        let io = IoNames::default();
        assert_eq!(io.image_input, "pixel_values");
        assert_eq!(io.text_input, "input_ids");
        assert_eq!(io.image_output, "pooler_output");
    }
    #[test]
    fn pooled_features_are_normalized_and_invalid_outputs_rejected() {
        let mut spec = crate::search::config::embedder_spec();
        spec.embedding_dim = 2;
        let v = finalize_embedding(vec![3.0, 4.0], &spec).unwrap();
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
        for invalid in [vec![f32::NAN, 1.0], vec![0.0, 0.0], vec![1.0]] {
            assert!(finalize_embedding(invalid, &spec).is_err());
        }
    }

    /// Opt-in real-asset check; never downloads as part of the default suite.
    /// The directory contains the three pinned repository-relative files plus
    /// cats.png and parrots.png from the audit's public fixtures.
    #[cfg(feature = "model-download")]
    #[test]
    #[ignore = "requires OPENTAKE_SEARCH_MODEL_TEST_DIR with verified public model assets and two images"]
    fn real_model_offline_install_embeddings_and_ranking() {
        use crate::search::{config, embed_store, model_download, ranker};
        let source = std::path::PathBuf::from(
            std::env::var("OPENTAKE_SEARCH_MODEL_TEST_DIR").expect("model test directory"),
        );
        let models = tempfile::tempdir().unwrap();
        let manifest = config::manifest();
        let installed =
            model_download::install_from_directory(models.path(), &manifest, &source).unwrap();
        let verified = model_download::verify_installed(models.path(), &manifest).unwrap();
        assert_eq!(installed, verified);
        let embedder = OrtEmbedder::new(
            &installed.image_encoder,
            &installed.text_encoder,
            &installed.tokenizer_folder.join("tokenizer.json"),
            installed.spec,
        )
        .unwrap();
        let check_vector = |v: &[f32]| {
            assert_eq!(v.len(), 768);
            assert!(v.iter().all(|x| x.is_finite()));
            assert!((v.iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 1e-4);
        };
        let mut indexes = Vec::new();
        for name in ["cats", "parrots"] {
            let image = image::open(source.join(format!("{name}.png")))
                .unwrap()
                .to_rgba8();
            let frame = RgbaFrame::new(image.width(), image.height(), image.into_raw());
            let vector = embedder.encode_image(&frame).unwrap();
            check_vector(&vector);
            let header = embed_store::Header {
                model: manifest.model.clone(),
                model_version: manifest.version,
                sampler_version: 1,
                dim: 768,
                count: 1,
            };
            let rows = [embed_store::Row {
                time: 0.0,
                shot_start: 0.0,
                shot_end: 0.0,
            }];
            // Exercise the same f16 representation used by persisted search indexes.
            let index = embed_store::decode(&embed_store::encode(&header, &rows, &vector).unwrap())
                .unwrap();
            indexes.push((name.to_owned(), index));
        }
        for (query, expected) in [
            ("a photo of two cats on a couch", Some("cats")),
            ("a photo of colorful parrots", Some("parrots")),
            ("沙发上的两只猫", Some("cats")),
            ("彩色鹦鹉", Some("parrots")),
            ("a photo of an airplane", None),
        ] {
            let vector = embedder.encode_text(query).unwrap();
            check_vector(&vector);
            let hits = ranker::search(
                &vector,
                &indexes,
                config::SEARCH_LIMIT,
                config::RELATIVE_CUTOFF,
                Some(config::VISUAL_MATCH_COSINE_FLOOR),
            );
            println!("query={query:?} hits={hits:?}");
            assert_eq!(hits.first().map(|h| h.asset_id.as_str()), expected);
        }
    }
}
