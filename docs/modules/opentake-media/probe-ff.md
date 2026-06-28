# probe-ff — ffmpeg sidecar 封装 + 媒体探测

> 上级：[INDEX.md](INDEX.md) · [OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：`ff.rs`（内部 mod）、`probe.rs`。对应上游 `MediaAsset.loadMetadata`（`MediaAsset.swift:96-162`）。

---

## 职责

媒体引擎的最底座：**发现并驱动系统 `ffmpeg`/`ffprobe` 二进制**，以及**零解码地探测媒体头信息**（时长 / 旋转校正后的分辨率 / fps / 有无音视频轨）。所有上层解码 / 编码 / 缩略图 / 波形模块都经 `ff.rs` 拿二进制路径或一次性 ffprobe JSON。

---

## 关键决策：为何 CLI sidecar 而非 libav 绑定

`ff.rs` 模块注释明确：**有意不链接 `libav*`**。本机工具链是 ffmpeg 8.1（libavcodec 62），C 绑定 crate（`ffmpeg-next` / `ffmpeg-the-third`）不支持该版本，且 `pkg-config` 缺失。改用 `ffmpeg-sidecar`：shell 出 `PATH` 上的二进制，零原生链接、跨平台干净构建。

> ⚠️ **与文档的偏差**：[SPEC.md](SPEC.md) §1.2、[ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) §1、[ROADMAP.md](../../architecture/ROADMAP.md) 都仍写 `ffmpeg-next`（libav 绑定）。**以代码为准**——实际全栈走 CLI sidecar。这影响所有解码路径：帧用裸 stdin/stdout 原始像素管道交换，而非内存中的 `AVFrame`。

---

## `ff.rs` — 二进制驱动

| 函数 | 作用 |
|---|---|
| `ffmpeg_path()` / `ffprobe_path()` | 返回二进制路径：优先环境变量 `OPENTAKE_FFMPEG` / `OPENTAKE_FFPROBE`，否则 `PATH` 上的 `ffmpeg` / `ffprobe`。打包构建可指向 bundled 二进制（与 `src-tauri/src/lib.rs` 一致）。 |
| `ffmpeg()` | 构造绑定到 `ffmpeg_path()` 的 `FfmpegCommand`。 |
| `ffmpeg_available()` / `ffprobe_available()` | `-version` 是否退出 0。集成测试用它在无 ffmpeg 的机器上 skip，保证默认测试运行绿。经 `lib.rs::ffmpeg_status` 重导出供宿主能力检查。 |
| `ffprobe_json(path)` | 运行 `ffprobe -v quiet -of json -show_streams -show_format <path>`，返回解析后的 `serde_json::Value`。**零解码**，仅读头/流参数。spawn 失败或非零退出 → `MediaError::Ffmpeg`。 |

---

## `probe.rs` — `MediaProbe`

```rust
pub struct MediaProbe {
    pub duration_secs: f64,   // 优先视频流 duration，回退容器 duration
    pub width: Option<u32>,   // 已应用旋转 side-data / display matrix
    pub height: Option<u32>,
    pub fps: Option<f64>,     // avg_frame_rate（回退 r_frame_rate）= nominalFrameRate 语义
    pub has_audio: bool,
    pub has_video: bool,
}
pub fn probe(path: &Path) -> Result<MediaProbe>;          // 文件不存在 → Io(NotFound)
pub fn parse_probe(json: &serde_json::Value) -> MediaProbe; // 纯函数，可从 fixture 单测
```

`probe()` 先存在性检查，再 `ff::ffprobe_json` → `parse_probe`。**JSON→Probe 拆成纯函数 `parse_probe`**，使旋转/时长/fps 规则不依赖 ffprobe 即可单测。

### 关键规则（逐项对齐上游）
- **旋转校正**：读取流的 `tags.rotate`（字符串）或 `side_data_list[*].rotation`（数字，常为负角），折叠为 `[0,360)` 的非负角；**90 或 270 时交换宽高**。等价 `appliesPreferredTrackTransform` / `size.applying(transform)`（`MediaAsset.swift:133-136`）。180 不交换。
- **时长回退顺序**：视频流 `duration` → 容器 `format.duration` → `0.0`（`MediaAsset.swift:141-147`）。
- **fps**：`avg_frame_rate`，为 `0/0`（未知）时回退 `r_frame_rate`；`parse_rate` 解析 `"30000/1001"` 形式，分子或分母为 0 → `None`（对齐 `nominalFrameRate`，`MediaAsset.swift:138`）。
- **`has_video`**：存在 `codec_type=="video"` 的流。
- **`has_audio`**：存在 `codec_type=="audio"` 的流，**但 `channels==0` 的占位/空音轨不计为有音频**。

### 不变量 / 边界（含一处 bug 修复）
- **零声道音轨防幻影链接**：某些导出器会加 0 声道的空音轨。若把它当成「有音频」，删除视频时会派生出一个幻影链接音频片段（用户报「明明没声音却分出一条音轨」）。故要求 `channels > 0` 才算音频；**不报告 `channels` 的流保守地仍当作音频**。
- 文件不存在 → `MediaError::Io(NotFound)`；流缺失各字段 → 对应 `None` / `0.0` / `false`。

### 测试
`parse_probe` 有约 12 条 fixture 单测：横屏不交换、tags 旋转 90 交换、side-data −90 折叠为 270 交换、180 不交换、fps 回退 `r_frame_rate`、纯音频无视频尺寸、时长回退容器、全空为 0、视频带音轨双标记、0 声道音轨不计音频、多声道计音频。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 模块文档树：[../INDEX.md](../INDEX.md) · docs 总目录：[../../INDEX.md](../../INDEX.md)
- 源码根：`../../../crates/opentake-media/src/`
