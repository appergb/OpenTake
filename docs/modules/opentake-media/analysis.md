# analysis — 离线分析：节拍 / 静音 / 自动裁剪

> 上级：[INDEX.md](INDEX.md) · [OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：`analysis/{mod,beat,silence,autocrop}.rs`。对应 MCP 工具 `detect_beats`/`auto_cut_to_beats`、`tighten_silences`、`smart_reframe`。

---

## 职责

三个**纯算法**离线分析器，供编辑自动化（[architecture/editing-automation](../../architecture/editing-automation/README.md)）与 Agent 使用：

| 模块 | MCP 工具 | 算法 | 依赖 ffmpeg？ |
|---|---|---|---|
| `beat.rs` | `detect_beats` / `auto_cut_to_beats` | 能量包络 onset | 否（PCM 由调用层抽） |
| `silence.rs` | `tighten_silences` | RMS 阈值分割 | 否 |
| `autocrop.rs` | `smart_reframe` | 黑边/透明区扫描 | 否（帧由调用层抽） |

**关键边界**：三者都不直接调 ffmpeg——PCM / 帧由调用层用 [decode.md](decode.md) 的 `extract_pcm` / `decode_frame_at` 抽好后传入，本模块只做数值运算（符合 domain/算法分层）。`mod.rs` 仅 re-export。

> 注：beat / silence / autocrop 在上游无直接 Swift 源对应（OpenTake 为编辑自动化新增），与上游 1:1 复刻无关。

---

## 节拍检测 `beat.rs`

```rust
pub struct BeatDetectionConfig { pub sample_rate: u32, pub fps: f64,
    pub window_size_samples: usize, pub hop_size_samples: usize,
    pub min_onset_strength: f32 /*0.08*/, pub min_gap_frames: u64 /*2*/ }
pub struct BeatOnset { pub frame: u64, pub strength: f32 }
pub fn detect_beats(samples: &[f32], config) -> Vec<BeatOnset>;
```

算法（能量包络 onset 检测）：
1. 滑动窗口（hop 重叠）逐帧算 RMS 能量。
2. 算相邻帧能量上升 delta，按最大 peak_delta 归一化为 `strength`。
3. 过滤 `strength < min_onset_strength(0.08)`。
4. 帧间最小间隔 `min_gap_frames(2)` 防重复检测。
- `frame` = onset 时间换算的时间线帧（用 `fps`）。
- 边界：`sample_rate==0` 或 `fps<=0` 返回空；能量序列 <2 返回空。

---

## 静音检测 `silence.rs`

```rust
pub struct SilenceDetectionConfig { pub sample_rate: u32, pub fps: f64,
    pub window_size_samples: usize, pub hop_size_samples: usize,
    pub rms_threshold: f32 /*0.01*/, pub min_silence_frames: u64 /*1*/ }
pub struct SilenceRange { pub start_frame: u64, pub end_frame: u64 }
pub fn detect_silences(samples: &[f32], config) -> Vec<SilenceRange>;
```

算法（RMS 阈值分割）：
1. 同滑动窗口算 RMS（`sqrt(sum(x^2)/count)`，f64 累加转 f32）。
2. `RMS <= rms_threshold(0.01)` 判静音，连续静音段用状态机合并。
3. 过滤范围 `< min_silence_frames(1)` 的段。
- 不变量：`end_frame <= start_frame` 时强制为 `start_frame+1`（防零宽）。
- 与 [transcribe.md](transcribe.md) 的词级转写配套实现 ADVANCED-FEATURES「智能剪口播」（转写 + 静音 → Rust 内算 ripple 区间）。

---

## 自动裁剪 `autocrop.rs`

```rust
pub enum PixelFormat { Rgb, Rgba }
pub struct FrameBuffer<'a> { pub width, height: u32, pub data: &'a [u8], pub pixel_format: PixelFormat }
pub struct CropRect { pub x, y, width, height: u32 }
pub struct CropTransform { pub scale_x, scale_y, translate_x, translate_y: f32 }  // NDC (-1..1)
pub struct AutocropPlan { pub crop: CropRect, pub transform: CropTransform }
pub struct AutocropConfig { pub black_threshold: u8 /*16*/, pub min_alpha: u8 /*16*/,
    pub sample_step: u32 /*1*/, pub target_aspect_ratio: Option<f32> }
pub fn detect_autocrop(frame: &FrameBuffer, config) -> Option<AutocropPlan>;
```

算法（黑边/透明区扫描 → 内容边界框）：
1. 逐像素采样（`sample_step` 默认 1 = 全采）。
2. 非黑判据：`max(R,G,B) > black_threshold(16)` 且（RGB 或 `alpha >= min_alpha(16)`）。
3. 累计 min/max x/y 得内容边界框；可选按 `target_aspect_ratio` 扩展。
4. 算变换：`scale = frame / crop`（缩放使裁剪区充满帧），`translate = (frame_center - crop_center) / frame_size * 2.0`（NDC 单位居中）。
- 边界：宽或高为 0、或 `data.len() < width*height*channels` → `None`。

> ⚠️ **完成状态 / 与 SPEC 偏差**：当前实现**仅做黑边/透明区裁剪**，**未集成人脸/显著性检测**。MCP `smart_reframe` 的完整语义（主体感知重构图）属计划中。无深度学习/检脸依赖。

---

## 测试
各一条单测：beat（人工脉冲验证 frame + strength）、silence（非零/零/非零三段找中间静音）、autocrop（8x6 RGB 内嵌 4x4 白区，验证裁剪矩形 + 变换系数）。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 相关：[decode.md](decode.md)（PCM/帧来源）· [transcribe.md](transcribe.md)（剪口播配套）· [editing-automation](../../architecture/editing-automation/README.md)
- 模块文档树：[../INDEX.md](../INDEX.md) · docs 总目录：[../../INDEX.md](../../INDEX.md)
- 源码根：`../../../crates/opentake-media/src/`
