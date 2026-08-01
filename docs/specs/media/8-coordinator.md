# 与 domain / render 的接口

## 8.1 消费 `opentake-domain`(不可改)

本 crate 依赖 `opentake-domain`,消费以下类型(均已存在,见 `crates/opentake-domain/src/`):
- `media::MediaAsset`(`media.rs:283`):`id/url:PathBuf/kind:ClipType/duration:f64/source_width/source_height/source_fps/has_audio/...`。本 crate 的 `probe()` 结果**回填**这些字段(由 `opentake-core` 调用,§8.4);索引/转写调度直接读 `MediaAsset`(`kind/url/has_audio/is_generating`)。
- `media::MediaResolver`(`media.rs:226`):`expected_path(asset_id)` 把 asset id 解析为 `PathBuf`(零 IO);本 crate 所有 IO 函数收 `&Path`,由调用层先经 resolver 解析。
- `clip_type::ClipType`(`clip_type.rs:783`):`Video/Audio/Image/Text/Lottie`,`is_visual()`/`from_file_extension()`。调度按 `kind` 路由(video→缩略图+波形+视觉索引+转写;audio→波形+转写;image→图片缩略图+视觉索引)。
- `timeline::{Timeline, Track}`、`clip::Clip`:仅 §8.3 的「物化纹理」需要读 clip 属性;本 crate 不直接消费 Timeline(渲染在 render)。

**单向依赖**:`opentake-domain` ← `opentake-media`;本 crate **不**反向暴露类型给 domain(domain 零 IO 叶子)。

## 8.2 被 `opentake-render` 复用的解码/编码

Tauri 的 render/playback/export adapter（调用 `opentake-render` 的 RenderPlan +
wgpu 合成器）通过 `MediaEngine` **复用本 crate 的**:
- `MediaEngine::decode_frame` / 批量 flat decoder(预览/导出取源帧 → 上传纹理)。
- `decode::reader`(顺序解帧迭代器,导出后端逐帧喂合成器)。
- `MediaEngine::extract_pcm`(导出混音前取各 clip 音频 PCM)。
- `MediaEngine::video_encoder` + `ExportPreset`(把合成 RGBA 帧序列 + 混音编码成容器)。
- `MediaEngine::probe`(渲染尺寸/源 fps 决策)。

**职责切分**(`docs/ARCHITECTURE.md` §1/§6):
- `opentake-media` = **读取/编码 + 离线分析**(解码到 RGBA、抽 PCM、缩略图、波形、转写、语义索引/搜索、ort worker)。
- `opentake-render` = **合成 + 调度**(RenderPlan 纯函数、wgpu 逐帧合成、媒体物化为纹理、预览/导出后端、A/V 同步)。`renderSize` 偶数化、BT.709 instruction、关键帧 ramp **全在 render**。
- 二者通过 **`RgbaFrame` / `PcmBuffer`** 这两个朴素值类型交换帧/样本,无 wgpu/ffmpeg 类型泄漏到边界。

## 8.3 媒体物化(图片/Lottie → 纹理)的归属

上游用 `ImageVideoGenerator`(图片烧静止视频)、`LottieVideoGenerator`(Lottie 烧 ProRes)、`AlphaVideoNormalizer`(直 alpha 预乘)绕开 AVPlayer 限制。`docs/_analysis/02` 表 L74/L75/L81 与 `docs/ARCHITECTURE.md` §6 L130:**自建 wgpu 合成器后,这三类 hack 整类消失**——图片/Lottie 在合成前**物化为纹理**(content-hash 缓存),由 `opentake-render` 负责。
- 本 crate **提供**:图片解码 → `RgbaFrame`(§3.2 / `image` crate);(可选)Lottie 解码用 `rlottie` FFI 或 `velato`(`docs/_analysis/02` 表 L81),渲成 `RgbaFrame` 序列。**建议** Lottie 放 render 的物化层或独立 `opentake-motion`(Phase 10),本 crate 仅暴露图片解码;Lottie 列为**有意暂不归本 crate**。
- 本 crate **不提供**:静止视频烧制、ProRes 烧制、alpha 预乘转码(整类删除)。

## 8.4 facade `MediaEngine`(供 `opentake-core` 调用)

```rust
// lib.rs
pub struct MediaEngine {
    cache_root: PathBuf,          // 缩略图/波形/转写/embedding 缓存根(Tauri app_cache_dir)
    models_dir: PathBuf,          // 模型安装根(Tauri app_data_dir)
    export_pause: ExportPause,    // 与进程唯一 OrtWorker 共用的压力信号
}
impl MediaEngine {
    pub fn probe(&self, path: &Path) -> Result<MediaProbe>;
    pub fn decode_frame(&self, path: &Path, req: &FrameRequest) -> Result<(f64, RgbaFrame)>;
    pub fn extract_pcm(&self, path: &Path, spec: &PcmSpec,
                       range: Option<(f64,f64)>) -> Result<PcmBuffer>;
    pub fn video_encoder(&self, out: &Path, width: u32, height: u32,
                         fps: i32, preset: &ExportPreset) -> Result<VideoEncoder>;
    pub fn video_thumbnails(&self, path: &Path, dur: f64, cb: Option<&dyn Fn(&[VideoThumb])>) -> Result<Vec<VideoThumb>>;
    pub fn image_thumbnail(&self, path: &Path) -> Result<RgbaFrame>;
    pub fn waveform(&self, path: &Path, dur: f64) -> Result<Vec<f32>>;
    pub fn transcribe(&self, path: &Path, is_video: bool, range: Option<(f64,f64)>,
                      backend: &dyn Transcriber, cache: &TranscriptCache)
                      -> Result<TranscriptionResult>;
    pub fn search_spoken(&self, query: &str, assets: &[(String, PathBuf)], limit: usize) -> Vec<SpokenHit>;
    pub fn search_visual(&self, query_vector: &[f32], indexes: &[(String, AssetIndex)],
                         limit: usize, relative_cutoff: f32,
                         min_score: Option<f32>) -> Vec<Hit>;
    pub fn export_pause(&self) -> ExportPause;
}
```
- 错误边界:`MediaEngine` 返回 `Result<_, MediaError>`;`opentake-core` 转 Tauri `Err(String)`(AGENTS.md Rust 风格)。
- 缓存根/模型根由 core 注入(跨平台路径,替上游硬编码的 `~/Library/...`)。
- 重模型调度不作为 facade 的伪字段保存：Tauri `search_index_start` 把 owned
  manifest snapshot 和本 facade 的 `ExportPause` 提交给进程唯一的 bounded
  `OrtWorker`；worker registry 负责 model identity/lazy load。这样 facade
  保持同步值类型边界，调度生命周期仍由应用 runtime 拥有。
- 依赖拥有测试
  `facade_contract.rs#all_services_are_reachable_only_through_facade_and_dependencies_stay_acyclic`
  用真实 A/V fixture 贯通 probe/decode/PCM/transcribe/encode/reprobe，并以
  manifests 锁定 `domain <- media`、media 不反向依赖 core/render 的 DAG。
