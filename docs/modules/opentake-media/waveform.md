# waveform — 波形：ffmpeg 抽 PCM → RMS 降采样 → 归一化

> 上级：[INDEX.md](INDEX.md) · [OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：`waveform/mod.rs`、`waveform/dsp.rs`、`waveform/store.rs`。对应上游 `MediaVisualCache` 波形分支（外包 `DSWaveformImage`）。

---

## 职责

为时间线音轨生成可视波形：解整轨 PCM → 切桶算 RMS → 归一化到 `0..1` + `.waveform` 二进制磁盘缓存。波形**仅用于 UI 绘制，非帧级编辑量**，不要求与上游逐位一致（[SPEC.md](SPEC.md) §4.3，风险评级 🟢 低）。

---

## 关键根因：改用 ffmpeg 抽 PCM，而非 Symphonia

[SPEC.md](SPEC.md) §1/§4 与 [ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) 媒体映射表都写「Symphonia 解 PCM + 自算 RMS」。**实际实现改用 `decode::extract_pcm`（ffmpeg sidecar）**，与 probe / 缩略图同一条解码路径。

> **根因**（`waveform/mod.rs` 注释）：Symphonia 解不出 `.mov` 等容器里的部分非 AAC 编码、以及多种容器格式，导致这些素材波形渲染直接失效。改走 ffmpeg 后，**波形成功率与 ffmpeg 的解码覆盖一致**。这是本模块「波形用 ffmpeg `extract_pcm` 而非 symphonia」移植铁律的来源（[OVERVIEW.md](OVERVIEW.md) §6 第 2 条）。

解码 spec：**22050 Hz / 单声道 / f32**（`WAVEFORM_SAMPLE_RATE = 22_050`）——波形是视觉 affordance，低采样率即可，降成本。

---

## `waveform/mod.rs`

```rust
pub const BUCKETS_PER_SECOND: f64 = 150.0;
pub const MIN_BUCKETS: usize = 4000;
pub const MAX_BUCKETS: usize = 20000;

pub fn waveform(path, duration_secs) -> Result<Vec<f32>>;                 // 无缓存
pub fn waveform_cached(cache_root, path, duration_secs) -> Result<Vec<f32>>; // 带磁盘缓存
pub fn waveform_sample_count(duration: f64) -> usize;   // re-export 自 dsp
```

`waveform_cached` 先 `file_identity_key(path, 32)` → `load_waveform` 命中即返回，否则 `waveform()` 生成再 `save_waveform`。

---

## DSP `waveform/dsp.rs`（纯算法）

### 样本数量公式（逐字照搬 `waveformSampleCount`，`MediaVisualCache.swift:186-190`）
```text
duration ≤ 0 或非有限       → MIN_BUCKETS (4000)
duration ≥ 20000/150 ≈ 133.33s → MAX_BUCKETS (20000)   // 硬上限
否则                        → max(4000, floor(duration * 150))
```
即每秒 150 桶、下限 4000、上限 20000。

### RMS 降采样 + 归一化（`rms_downsample_normalized`）
1. 样本均匀切成 `count` 个桶，半开区间 `[lo, hi)`：`lo = bucket*n/count`，`hi = ((bucket+1)*n/count).max(lo+1).min(n)`。
2. 每桶 RMS：`sqrt(Σ(x²)/len)`（用 f64 累加保精度，输出 f32）。
3. **全局归一化 + 反演**：`peak = max(rms[])`；若 `peak ≤ f32::EPSILON` → 全 `1.0`（静音保护）；否则 `out[i] = 1.0 - (rms[i]/peak).clamp(0,1)`。

> **语义：`0 = 响(loud)，1 = 静(silence)`**——这是上游惯例（注释 "normalized 0=loud, 1=silence"，`MediaVisualCache.swift:11`）。DSWaveformImage 的精确归一化方式在上游代码外（第三方），无法逐位复刻；故采用「RMS → 满刻度归一 → `1 - x`」，断言全静音→全 1、满幅→接近 0、单调性正确即可。

### 边界 / 不变量
空样本 → 全 1.0；`count==0` → 空 `Vec`；样本少于桶数仍返回 `count` 个值；输出一律夹到 `[0,1]`。

---

## 缓存格式 `waveform/store.rs`（逐字节复刻）

```rust
pub fn load_waveform(cache_root, key) -> Option<Vec<f32>>;
pub fn save_waveform(cache_root, key, samples: &[f32]) -> Result<()>;
```

- 文件：`<cache_root>/MediaVisualCache/<key>.waveform`（`CACHE_SUBDIR = "MediaVisualCache"`，与缩略图共用目录、同机与上游可互读）。
- 格式：**裸 `[f32]` little-endian 连续**（对齐 `MediaVisualCache.swift:218-227`）。`byteorder` 写 `write_f32::<LittleEndian>`、读循环 `read_f32::<LittleEndian>`。
- 读校验：**非空 且 `len % 4 == 0`**，否则视为无效返回 `None`。
- 字节序：上游 `Data($0)` 是宿主端序（macOS arm64 = LE）；本模块固定 LE，与 arm64 mac 写出的互读一致。单测断言 `1.0f32` → `[0x00,0x00,0x80,0x3F]`。

---

## 完成状态 / 扩展位
已实现（ffmpeg PCM + RMS + 归一 + 缓存）。若后续要求与上游逐位一致，可换 peak 包络并标定缩放（[SPEC.md](SPEC.md) §4.3 预留 `WaveformMode { Rms, Peak }` 扩展位）。

## 测试
`waveform_sample_count` 边界（0/1s/100s/133.33s/1000s）、`rms_downsample_normalized`（空→全1、count=0→空、满幅正弦→接近0、前静后响→单调递减、范围夹紧）、`.waveform` 往返（LE 字节布局、长度校验）。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 相关：[decode.md](decode.md)（`extract_pcm`）· [thumbnail.md](thumbnail.md)（共用 `MediaVisualCache` 目录）
- 模块文档树：[../INDEX.md](../INDEX.md) · docs 总目录：[../../INDEX.md](../../INDEX.md)
- 源码根：`../../../crates/opentake-media/src/`
