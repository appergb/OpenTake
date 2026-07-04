# Symphonia 波形(RMS 降采样,归一化 0..1,缓存格式)

对应 `MediaVisualCache` 波形分支(上游外包给 `DSWaveformImage.WaveformAnalyzer`,`MediaVisualCache.swift:181`)。`docs/_analysis/02` 表 L77:「Symphonia 解 PCM + 自算 RMS/peak 降采样」,成熟、低风险。

## 4.1 接口

```rust
// waveform/mod.rs
/// 归一化样本:0 = 响,1 = 静(对齐上游注释 "normalized 0=loud, 1=silence",
/// MediaVisualCache.swift:11)。长度 = sample_count(duration)。
pub fn waveform(path: &Path, duration_secs: f64) -> Result<Vec<f32>>;

/// 带磁盘缓存:命中 <cache_root>/MediaVisualCache/<key>.waveform 直接返回。
pub fn waveform_cached(cache_root: &Path, path: &Path, duration_secs: f64) -> Result<Vec<f32>>;
```

## 4.2 样本数量公式(逐字照搬)

`waveformSampleCount`(`MediaVisualCache.swift:186-190`):
```text
duration 非有限或 ≤ 0          -> 4000
duration ≥ 20000/150 (≈133.3s) -> 20000(硬上限)
否则                           -> max(4000, floor(duration * 150))
```
即每秒 150 个桶,下限 4000,上限 20000。Rust 复刻为 `pub fn waveform_sample_count(duration: f64) -> usize`,纯函数 + 单测边界(0、1s、100s、133.3s、1000s)。

## 4.3 降采样与归一化(RMS,对齐「0=响,1=静」)

DSWaveformImage 默认输出的是「振幅包络」,上游语义是 **0=loud,1=silence**(注意是反的)。复刻策略:
1. Symphonia 解码整轨为 f32 mono(多声道下混为均值)。
2. 把样本切成 `count` 个等长桶,每桶算 **RMS**(`sqrt(mean(x²))`)→ `amp ∈ [0,1]`(已是归一化幅度;若源 >1 截断)。
3. 归一化到上游语义:`out = 1 - clamp(amp_normalized, 0, 1)`,其中 `amp_normalized` 按整轨峰值或固定满刻度归一。**关键风险**:DSWaveformImage 的精确归一化方式未在上游代码内(是第三方),无法逐位复刻。

> **决策**:波形仅用于 UI 绘制(时间线音轨直观),**非帧级编辑量**,不要求与上游逐位一致(对齐 `docs/_analysis/02` 风险登记:波形列为 🟢 低)。规格采用「RMS → 满刻度归一 → `1 - x`」,在单测中断言:全静音→全 1±ε、满幅正弦→接近 0、单调性正确。缓存格式与文件名严格复刻(可互读),但样本值容许与上游有视觉等价差异。若后续要求逐位一致,再换 peak 包络并标定缩放(留 `WaveformMode { Rms, Peak }` 扩展位)。

## 4.4 缓存格式(逐字节复刻)

`.waveform` 文件 = 裸 `[f32]` little-endian(`MediaVisualCache.swift:218-227`):写 `samples.withUnsafeBytes`,读校验 `!data.isEmpty && data.count % 4 == 0` 后 `bindMemory(to: Float)`。

```rust
// waveform/store.rs
pub fn load_waveform(cache_root: &Path, key: &str) -> Option<Vec<f32>>; // 读 <key>.waveform
pub fn save_waveform(cache_root: &Path, key: &str, samples: &[f32]) -> Result<()>;
```
- 文件名:`<key>.waveform`(key = `file_identity_key(path, 32)`)。
- 格式:小端 f32 连续;`byteorder` 写、读校验 `len%4==0 && len>0`。
- ⚠️ 字节序:上游 `Data($0)` 是宿主端序(macOS arm64 = LE);跨平台固定 LE,与 arm64 mac 写出的互读一致。
