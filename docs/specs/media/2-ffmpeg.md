# ffmpeg-sidecar:解码 / 编码 / 缩略图(seek 解帧)/ 抽 PCM

## 2.1 媒体探测 `MediaProbe`(替 `MediaAsset.loadMetadata` 的视频/音频分支)

上游 `MediaAsset.loadMetadata`(`MediaAsset.swift:96-162`)用 AVFoundation 读:时长(优先 video track timeRange,否则 asset duration)、`naturalSize.applying(preferredTransform)` 校正后的像素宽高、`nominalFrameRate`、是否有音轨。Rust 用 ffprobe 等价。

```rust
// probe.rs
#[derive(Clone, Debug, PartialEq)]
pub struct MediaProbe {
    pub duration_secs: f64,        // 优先视频流 duration,回退容器 duration
    pub width: Option<u32>,        // 已应用 rotate side-data / display matrix
    pub height: Option<u32>,
    pub fps: Option<f64>,          // avg_frame_rate(对齐 nominalFrameRate 语义)
    pub has_audio: bool,
    pub has_video: bool,
}

/// 打开容器,读首个视频流 + 是否存在音频流。零解码(仅读头/流参数)。
pub fn probe(path: &Path) -> Result<MediaProbe>;
```
实现要点(逐项对齐上游):
- **旋转校正**:ffmpeg `AV_PKT_DATA_DISPLAYMATRIX` side-data → 90/270° 时交换宽高,等价 `appliesPreferredTrackTransform`/`size.applying(transform)`(`MediaAsset.swift:133-136`)。
- **时长回退顺序**:video stream `duration` → 容器 `duration`(`MediaAsset.swift:141-147`)。
- **fps**:用 `avg_frame_rate`(若 0 用 `r_frame_rate`),映射 `sourceFPS`(`MediaAsset.swift:138`)。
- **has_audio**:存在 audio stream(`MediaAsset.swift:148-150`)。

填回 `opentake-domain::media::MediaAsset` 的 `duration/source_width/source_height/source_fps/has_audio` 字段(见 §9.1)。

## 2.2 解一帧 `decode_frame_at`(缩略图/采样/取帧共用底座)

上游三处用 `AVAssetImageGenerator`:`MediaVisualCache`(缩略图,`maximumSize=120×68`,tolerance 1s,`MediaVisualCache.swift:113-118`)、`FrameSampler`(采样,`maximumSize=512×512`,tolerance `max(interval/2,1)`,`FrameSampler.swift:54-60`)、`MediaAsset`(首帧缩略图 320²,`MediaAsset.swift:152-159`)。统一为一个带 tolerance 的「seek 到最近关键帧 + 解码 + 缩放」函数。

```rust
// decode/frame.rs
#[derive(Clone)]
pub struct RgbaFrame { pub width: u32, pub height: u32, pub rgba: Vec<u8> } // 紧凑 RGBA8

pub struct FrameRequest {
    pub time_secs: f64,
    pub max_size: (u32, u32),     // 等比缩放上界(对齐 maximumSize 语义:不放大,保宽高比)
    pub tolerance_secs: f64,      // 允许落到最近的解码可达帧(≈ requestedTimeTolerance)
    pub apply_rotation: bool,     // 默认 true(= appliesPreferredTrackTransform)
}

/// `-ss` 到 time-tolerance 起点附近的关键帧,解码至首个 pts ≥ (time-tolerance) 的帧;
/// 返回其实际 pts 与 RGBA。失败/越界返回 Err。
pub fn decode_frame_at(path: &Path, req: &FrameRequest) -> Result<(f64 /*actual_secs*/, RgbaFrame)>;

/// 批量(用于缩略图序列/采样):升序时间点,内部单次顺序解码尽量复用 decoder,
/// 对每个目标时间产出最近帧;跳过解不出的点。返回按实际时间升序。
pub fn decode_frames_at<'a>(path: &'a Path, times_secs: &'a [f64], opts: &'a FrameRequest)
    -> impl Iterator<Item = Result<(f64, RgbaFrame)>> + 'a;
```
要点:
- **缩放语义**:`max_size` = 等比缩放上界(上游 `maximumSize` 是「不超过此框、保宽高比」),用 swscale `SWS_BILINEAR`/`area`。**注意区分** §5 的 SigLIP 预处理用的是 squash-resize(忽略宽高比,见 §5.2),二者不同函数。
- **tolerance**:上游用 `requestedTimeToleranceBefore/After` 让解码器取最近 sync frame(避免逐帧解到精确帧)。Rust 用 `-ss` 到 `time-tolerance` 处 seek 到关键帧后顺序解到第一个 `pts ≥ time - tolerance`,即可。
- **去重**:批量路径要复刻 `t > lastTime` 去重(`FrameSampler.swift:74`)——同一关键帧被多个近邻时间点命中只产一次。

## 2.3 抽 PCM `extract_pcm`(替 `Transcription.extractAudioTrack`)

上游 `extractAudioTrack`(`Transcription.swift:203-280`)用 `AVAssetReaderTrackOutput` 解码音轨为 **16kHz / 单声道 / s16le / interleaved**,可选 `CMTimeRange` 截取,落 `.caf`。whisper 需要 **16k mono f32**。

```rust
// decode/pcm.rs
pub struct PcmSpec { pub sample_rate: u32, pub channels: u16, pub format: PcmFormat }
pub enum PcmFormat { S16Le, F32 }

/// 解码 `path` 的首条音轨为指定 PCM;`range` = 绝对秒区间(等价 CMTimeRange 端到端)。
/// 无音轨 → Err(NoTrack("audio", …))。
pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Result<PcmBuffer>;

pub struct PcmBuffer { pub spec: PcmSpec, pub samples_f32: Vec<f32> } // 始终内部转 f32 mono 供下游
```
要点:
- **重采样到 16k mono**:swresample。whisper-rs 吃 `&[f32]`(16k mono),所以 `extract_pcm(path, &PcmSpec{16000,1,F32}, range)` 直接喂 whisper。
- **range 语义**:`-ss lower -to upper`(绝对秒),对齐 `reader.timeRange`(`Transcription.swift:226-231`)。下游转写对截取结果做 `offsetting(by: lower)` 把时间码移回源时间(见 §6.1)。
- **波形复用**:波形(§4)默认走 Symphonia(纯 Rust,无 ffmpeg 链接成本);但若已 ffmpeg 解码,也可复用本函数取整段 PCM。两条路径都要产出**相同的归一化样本**(测试断言一致),实施时以 Symphonia 为准。

## 2.4 编码 / 导出预设(供 `opentake-render` 导出后端调用)

上游导出在 `Export/ExportService.swift`(`AVAssetExportSession`,preset 名按 H.264/H.265/ProRes × 720/1080/4K 映射,`docs/_analysis/02` §1.3)。本 crate 提供**编码器后端 + 预设表**;**逐帧合成由 `opentake-render` 的 wgpu 完成**,本 crate 只负责把合成出的 RGBA 帧序列 + 混音 PCM 编码成容器。

```rust
// encode/preset.rs
pub enum VideoCodec { H264, H265, ProRes422 }
pub enum ExportResolution { P720, P1080, P2160 } // 短边

pub struct ExportPreset {
    pub codec: VideoCodec,
    pub resolution: ExportResolution,
    // 实施时逐项调参逼近上游 preset 的码率/profile/色彩(BT.709)。详见 §10 验收。
}
```
```rust
// encode/mod.rs
pub struct VideoEncoder { /* ffmpeg encoder ctx */ }
impl VideoEncoder {
    pub fn new(out: &Path, w: u32, h: u32, fps: i32, preset: &ExportPreset) -> Result<Self>;
    pub fn push_frame(&mut self, rgba: &RgbaFrame, pts_frame: i64) -> Result<()>;
    pub fn push_audio(&mut self, pcm: &PcmBuffer) -> Result<()>;
    pub fn finish(self) -> Result<()>;
}
```
要点:
- **renderSize 取偶数**:`even(value) = max(2, round/2*2)`,逐字照搬 `TimelineRenderer.even`(`TimelineRenderer.swift:85`)与 `ImageVideoGenerator.encoderDimension`(`ImageVideoGenerator.swift:68-72`,H.264 拒绝奇数尺寸/`max(2, pixels - pixels%2)`)。该函数放 `opentake-render`(渲染尺寸决策),本 crate 编码器只接收已偶数化的尺寸。
- **色彩管线**:H.264/H.265 写 BT.709 primaries/transfer/matrix,对齐 `ImageVideoGenerator.writeStillVideo`(`ImageVideoGenerator.swift:168-174`)与 `CompositionBuilder` 锁 BT.709(`docs/_analysis/02` L31)。
- **ProRes422 + LPCM**:对齐上游 ProRes preset(`docs/_analysis/02` §1.3 L45)。
- **alpha 预乘**(`AlphaVideoNormalizer.swift`):上游为「直 alpha 视频 → 预乘」做单独转码。在 OpenTake **整类消失**——wgpu 合成器内直接处理 premultiplied alpha(`docs/_analysis/02` 表 L75)。本 crate**不**移植 `AlphaVideoNormalizer`;仅在解码层暴露「该帧是否带 alpha / 是否直 alpha」元数据(读 `pix_fmt`),供 render 决定着色器分支。记录于此以示**有意省略**。
