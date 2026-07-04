# decode — 解码：seek 解帧 + 抽 PCM

> 上级：[INDEX.md](INDEX.md) · [OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：`decode/mod.rs`、`decode/frame.rs`、`decode/pcm.rs`、顶层 `frame.rs`（`RgbaFrame`）。上游：`AVAssetImageGenerator`（多处）、`Transcription.extractAudioTrack`（`Transcription.swift:203-280`）。

---

## 职责

解码层的两个原语，都经 [ff.rs](probe-ff.md) 驱动 ffmpeg CLI、用裸 stdin/stdout 管道交换原始字节：

1. **解帧**（`decode/frame.rs`）：seek 到目标时间附近、解一帧、等比缩放，产出 `RgbaFrame`。缩略图、视觉抽帧、暂停态预览取源帧共用此底座。
2. **抽 PCM**（`decode/pcm.rs`）：解首条音轨为**单声道 f32** `PcmBuffer`。转写（16k）、波形（22050）、导出混音（48k）共用。

`decode/mod.rs` 仅 re-export。

---

## `RgbaFrame`（顶层 `frame.rs`）

跨 media/render 边界的纯像素值类型——**不泄漏 wgpu / ffmpeg 类型**（[SPEC.md](SPEC.md) §8.2）。紧凑 RGBA8、行主序、左上原点，`rgba.len() == width * height * 4`。

- `new(w,h,rgba)`：`debug_assert` 长度匹配。
- `black(w,h)`：全黑不透明帧（用作 SigLIP squash-resize 黑底，见 [semantic-search.md](semantic-search.md)）。
- `pixel_count()`；`Debug` 只打印形状不 dump 像素。

---

## 解帧 `decode/frame.rs`

```rust
pub struct FrameRequest {
    pub time_secs: f64,
    pub max_size: (u32, u32),   // 等比缩放上界；(0,0) 禁用缩放
    pub tolerance_secs: f64,    // 控制 -ss 回溯范围（默认 1.0）
    pub apply_rotation: bool,   // 默认 true（= appliesPreferredTrackTransform）
}
pub fn decode_frame_at(path, req) -> Result<(f64 /*actual_secs*/, RgbaFrame)>;
pub fn decode_frames_at(path, times_secs: &[f64], base: &FrameRequest) -> Vec<Result<(f64, RgbaFrame)>>;
pub fn fit_within(w: u32, h: u32, max: (u32, u32)) -> (u32, u32);  // 纯函数
```

### ffmpeg 命令（纯函数 `frame_args` 合成，可单测）
```
ffmpeg -ss {time_secs - tolerance_secs} -i <path> -frames:v 1 \
  [-vf "scale=w={mw}:h={mh}:force_original_aspect_ratio=decrease"] \
  -pix_fmt rgba -f rawvideo -
```
- **`-ss` 前置**到 `time - tolerance` 处：快速 seek 到最近关键帧（等价 `requestedTimeToleranceBefore/After`）。
- 单帧 → RGBA 原始视频到 stdout，直接读字节。
- 缩放滤镜仅在 `max_size` 有非零分量时加；旋转由 ffmpeg autorotate 默认处理。
- 实际帧时间取 `req.time_secs.max(输出帧 timestamp)`。

### `fit_within` 等比缩放（对齐上游 `maximumSize`）
「不超过此框、保宽高比、**永不放大**」：取 `min(mw/w, mh/h)`（0 维度忽略），`scale >= 1` 则返回原尺寸，否则 `round(w*scale).max(1)`（防除零）。**注意**这与语义搜索的 squash-resize（拉伸、忽略宽高比）是不同函数（见 [semantic-search.md](semantic-search.md)）。

### 批量去重
`decode_frames_at` 对升序时间点逐个解，**仅当 `actual > last_time` 才保留**（同一关键帧被多个近邻时间点命中只产一次），对齐上游 `t > lastTime`（`FrameSampler.swift:74`）。

---

## 抽 PCM `decode/pcm.rs`

```rust
pub enum PcmFormat { S16Le, F32 }   // ffmpeg_fmt: "s16le"/"f32le"；bytes: 2/4
pub struct PcmSpec  { pub sample_rate: u32, pub channels: u16, pub format: PcmFormat }
pub struct PcmBuffer { pub spec: PcmSpec, pub samples_f32: Vec<f32> } // 始终单声道 f32
pub fn extract_pcm(path, spec: &PcmSpec, range: Option<(f64,f64)>) -> Result<PcmBuffer>;
```

### ffmpeg 命令（纯函数 `pcm_args`）
```
ffmpeg [-ss {lo} -to {hi}] -i <path> -vn -ac {channels} -ar {sample_rate} -f {fmt} -
```
- `-vn` 丢视频；`-ac`/`-ar` 重采样到目标声道/采样率；输出到 stdout 直读裸字节（不走 event parser）。
- **`range` 语义**：`-ss lo -to hi`（绝对秒），对齐 `reader.timeRange`（`Transcription.swift:226-231`）；下游转写对截取结果 `offsetting(by: lo)` 把时间码移回源时间（见 [transcribe.md](transcribe.md)）。
- 无音轨 → `MediaError::NoTrack("audio", …)`。

### 下混为单声道 f32（纯函数 `raw_to_mono_f32`）
- `PcmBuffer.samples_f32` **始终是单声道 f32**：每帧把各声道**取平均**合成 mono。
- S16Le：i16 ÷ **32768.0**（`i16::MIN=-32768 → -1.0`）；F32 直接读 LE。尾部不完整帧丢弃。
- `duration_secs()` = `samples.len() / sample_rate`。

### 三处调用的不同 spec
| 调用方 | spec |
|---|---|
| 转写（[transcribe.md](transcribe.md)） | 16000 / 1 / F32（whisper 吃 16k mono f32） |
| 波形（[waveform.md](waveform.md)） | 22050 / 1 / F32（UI 视觉，低采样率降成本） |
| 导出混音（[encode.md](encode.md)） | 48000 / 1 / F32（mux 标准率） |

---

## 与 SPEC 的偏差
SPEC §1.1 列了 `decode/reader.rs`（顺序解帧迭代器）供 render 预览/导出复用——**当前未单独存在**；导出逐帧由 render 侧驱动，预览取帧复用 `decode_frame_at`。波形 PCM 在 SPEC 原计划走 Symphonia，实际改走本模块 `extract_pcm`（根因见 [waveform.md](waveform.md)）。

## 测试
`fit_within`（不放大/单边界/最小 1 像素/零输入）、`frame_args`（seek 夹持/RGBA 声明/缩放滤镜条件）、`pcm_args`（range/-ss/-to/-ac/-ar/-f）、`raw_to_mono_f32`（S16Le→单位浮点/立体声 f32 平均/尾帧截断）均有纯函数单测；实际解码集成测试在 ffmpeg 可用时运行。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 相关：[probe-ff.md](probe-ff.md) · [encode.md](encode.md) · [waveform.md](waveform.md) · [transcribe.md](transcribe.md)
- 模块文档树：[../INDEX.md](../INDEX.md) · docs 总目录：[../../INDEX.md](../../INDEX.md)
- 源码根：`../../../crates/opentake-media/src/`
