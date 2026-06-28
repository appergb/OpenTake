# encode — 编码：导出编码器 + 预设 + 线性混音

> 上级：[INDEX.md](INDEX.md) · [OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：`encode/mod.rs`、`encode/preset.rs`、`encode/mix.rs`。上游：`ExportService`（`AVAssetExportSession`）、`ImageVideoGenerator`（BT.709 色彩）。供 [opentake-render](../opentake-render/INDEX.md) 导出后端调用。

---

## 职责

把 render 的 wgpu 合成器**逐帧合成出的 RGBA 帧序列 + 混音 PCM**，编码成容器文件。本模块**只负责编码与混音**——逐帧合成（多轨叠加 / transform / opacity ramp）由 `opentake-render` 完成。三部分：

1. `encode/mod.rs` — `VideoEncoder`：两趟 ffmpeg（rawvideo→无声视频，再 mux 音频）。
2. `encode/preset.rs` — `ExportPreset`：codec / 分辨率 → ffmpeg token，BT.709，偶数尺寸。
3. `encode/mix.rs` — `mix_clips`：纯线性混音 + 硬限幅 + f32→s16le。

---

## `VideoEncoder`（`encode/mod.rs`）

```rust
impl VideoEncoder {
    pub fn new(out: &Path, w: u32, h: u32, fps: i32, preset: &ExportPreset) -> Result<Self>;
    pub fn push_frame(&mut self, rgba: &RgbaFrame) -> Result<()>;
    pub fn push_audio(&mut self, pcm: PcmBuffer);   // 记录待 finish 时 mux
    pub fn finish(self) -> Result<()>;
}
```

流程：
1. `new()` 起 ffmpeg 编码子进程（第一趟：RGBA stdin → 无音频视频文件）。
2. `push_frame()` 逐帧把 RGBA 写进 stdin，**预检字节长度 == `w*h*4`**。
3. `push_audio()` 暂存 `PcmBuffer`（不立即写）。
4. `finish()` 关 stdin、等第一趟完成；若有暂存音频，**第二趟 mux**。

### 第一趟编码命令（纯函数 `encode_args`）
```
ffmpeg -y -f rawvideo -pix_fmt rgba -s {w}x{h} -r {fps} -i - \
  -c:v {vcodec} -pix_fmt {pix_fmt} [BT.709 color args 仅 H.26x] <out>
```

### 第二趟 mux 命令（纯函数 `mux_args`，仅当有音频）
```
ffmpeg -y -i <encoded_video> -f s16le -ar {sr} -ac 1 -i <pcm_file> \
  -c:v copy -c:a {acodec} -shortest <out>
```
- 视频流 **`copy`**（不重编码），音频编 AAC（H.26x）或 LPCM（ProRes）。
- `-shortest` 修剪到较短流。
- **临时文件**用同目录 sibling（`out.mp4.{tag}.tmp`，纯函数 `sibling_temp`），保证原子 rename；**mux 失败时回退**到无音频版本（best-effort，对齐 [ROADMAP.md](../../architecture/ROADMAP.md) #117 描述）。

---

## 预设 `encode/preset.rs`

```rust
pub enum VideoCodec { H264, H265, ProRes422 }
pub enum ExportResolution { P720, P1080, P2160 }   // 短边
pub struct ExportPreset { pub codec: VideoCodec, pub resolution: ExportResolution }
```

ffmpeg token 映射：

| codec | `vcodec_arg` | `pix_fmt_arg` | `acodec_arg` | `color_args` |
|---|---|---|---|---|
| H264 | `libx264` | `yuv420p` | `aac` | BT.709 三件套 |
| H265 | `libx265` | `yuv420p` | `aac` | BT.709 三件套 |
| ProRes422 | `prores_ks` | `yuv422p10le`（10-bit 422） | `pcm_s16le`（LPCM） | 无 |

- **H.264/H.265 写 BT.709**：`-colorspace bt709 -color_primaries bt709 -color_trc bt709`，对齐 `ImageVideoGenerator.writeStillVideo`（`ImageVideoGenerator.swift:168-174`）与 `CompositionBuilder` 锁 BT.709。
- **`even_dimension(n) = (n - n%2).max(2)`**：向下取偶、最小 2。逐字对齐 `TimelineRenderer.even`（`TimelineRenderer.swift:85`）与 `ImageVideoGenerator.encoderDimension`（H.264 拒奇数尺寸）。

> 注：SPEC §2.4 说偶数化决策放 render（渲染尺寸决策），编码器只收已偶数化的尺寸；`even_dimension` 在此作为可复用工具同时存在。

---

## 混音 `encode/mix.rs`

```rust
pub const MIX_SAMPLE_RATE: u32 = 48_000;   // 导出音频标准率
pub struct ClipAudio { pub start_sample: usize, pub samples: Vec<f32>, pub gains: Vec<f32> }
pub fn mix_clips(clips: &[ClipAudio]) -> Result<Vec<f32>, String>;
pub fn mono_f32_to_s16le(samples: &[f32]) -> Vec<u8>;
```

- 每个 `ClipAudio` 是已在 `MIX_SAMPLE_RATE` 预解码的单声道 f32 + 起始样本索引；`gains` 空 = unity，否则逐样本增益包络（`with_static_gain` 在增益≈1 时省略数组）。
- `mix_clips`：输出长度 = 最远 clip 的 `end_sample`（无尾部静音）；逐 clip 按 `start_sample` 偏移 `out[i] += sample * gain`，**最后硬限幅到 `[-1.0, 1.0]`**。对齐 [ROADMAP.md](../../architecture/ROADMAP.md) #117「逐 clip PCM 按帧偏移 + volume_at 增益 + 叠加硬限幅」。
- `mono_f32_to_s16le`：`(clamp(s,-1,1) * 32767.0).round() as i16` → LE 字节（与 PCM 解码的 ÷32768 互为逆，round 为 .5 偶舍）。
- **不变量**：`gains` 非空时必须 `len == samples.len()`（否则 `Err`，视为编程错误）；空输入 → 空输出。

### Scope（第一切，对照 ROADMAP）
纯线性求和 + 硬限幅。**无重采样曲线 / pan / 立体声 / 动态处理**（明确标记为后续）；所有 clip 须在 mux 采样率预解码好再传入。

---

## 有意省略：`AlphaVideoNormalizer`

上游为「直 alpha 视频 → 预乘」单独转码（`AlphaVideoNormalizer.swift`）。在 OpenTake **整类消失**——wgpu 合成器内直接处理 premultiplied alpha。本模块**不**移植它；带 alpha 的源在解码层暴露 `pix_fmt` 元数据供 render 决定着色器分支即可（[SPEC.md](SPEC.md) §2.4）。

## 完成状态
H.264/.mp4 全分辨率逐帧导出 spine + 线性音频混音已落地（[ROADMAP.md](../../architecture/ROADMAP.md) #112/#117）。**H.265 / ProRes 预设的端到端导出 + 进度/取消**待补（预设 token 已就绪，集成验证未全）。

## 测试
`encode_args`（rawvideo 声明 / codec / pix_fmt / H.26x 有 bt709 而 ProRes 无）、`mux_args`（video copy / acodec / -shortest）、`sibling_temp` 路径、`even_dimension`（1920→1920/1921→1920/1→2/3→2/0→2）、`mix_clips`（空/单 clip unity/static gain/多 clip 叠加/硬限幅/逐样本包络/增益长度不匹配报错）、`mono_f32_to_s16le`（单位浮点映射/夹持）均有纯函数单测；AAC 轨 mux 有集成测试。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 相关：[decode.md](decode.md) · [opentake-render](../opentake-render/INDEX.md)
- 模块文档树：[../INDEX.md](../INDEX.md) · docs 总目录：[../../INDEX.md](../../INDEX.md)
- 源码根：`../../../crates/opentake-media/src/`
