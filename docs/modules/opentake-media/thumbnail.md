# thumbnail — 缩略图 + JPEG 雪碧图缓存

> 上级：[INDEX.md](INDEX.md) · [OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：`thumbnail/mod.rs`、`thumbnail/sprite.rs`。对应上游 `Timeline/MediaVisualCache.swift`（缩略图 sprite 分支）。

---

## 职责

两类缩略图 + 磁盘缓存，是**纯生成 + 磁盘缓存**函数（内存表 / 触发重绘属上层，见 [OVERVIEW.md](OVERVIEW.md) §2）：

1. **视频缩略图序列**：按时间点公式 seek 解多帧 → 拼成一张 JPEG **雪碧图（sprite）网格** + JSON sidecar 缓存。
2. **图片单缩略图**：解码 + 等比缩放到长边 ≤ 120。

底层解帧复用 [decode.md](decode.md) 的 `decode_frames_at`。

---

## `thumbnail/mod.rs`

```rust
pub const THUMB_MAX_SIZE: (u32, u32) = (120, 68);   // 上游 maximumSize
pub const THUMB_TOLERANCE_SECS: f64 = 1.0;          // 解码容差
pub const IMAGE_THUMB_MAX_PIXEL: u32 = 120;         // 图片缩略图长边
pub const PARTIAL_STRIDE: usize = 50;               // 每 50 帧渐进回调一次

pub fn video_thumbnail_times(duration: f64) -> Vec<f64>;     // 纯函数
pub fn video_thumbnails(cache_root, path, duration_secs, on_partial: Option<PartialThumbCallback>) -> Result<Vec<VideoThumb>>;
pub fn image_thumbnail(path, max_pixel: u32) -> Result<RgbaFrame>;
```

### 时间点公式（逐字照搬上游 `videoThumbnailTimes`，`MediaVisualCache.swift:192-202`）
```text
duration 非有限或 ≤ 0  → []
interval = duration < 10.0 ? 1.0 : 2.0
times = stride(from: 0, to: duration, by: interval)   // 严格 < duration
```
即短片（<10s）1s 间隔、长片（≥10s）2s 间隔，从 0 开始。

### `video_thumbnails` 流程
1. `cache_key::file_identity_key(path, 32)` → 试 `load_sprite`，命中直接返回。
2. miss：生成时间列表 → `decode_frames_at`（`max_size=THUMB_MAX_SIZE`、`tolerance=1.0`、`apply_rotation=true`）。
3. 每 `PARTIAL_STRIDE=50` 帧触发一次 `on_partial` 回调（长视频渐进发布，对齐 `MediaVisualCache.swift:123-129`；UI 进度交上层 Tauri event）。
4. `save_sprite` 落盘。

### `image_thumbnail`
`image` crate 解码（EXIF 方向由解码器处理）→ 转 RGBA8 → `fit_within(w, h, (max_pixel, max_pixel))`（Triangle 重采样、不放大）。对齐 `makeImageThumbnail`（`MediaVisualCache.swift:152-163`）。

---

## 雪碧图缓存 `thumbnail/sprite.rs`

```rust
pub const MAX_COLUMNS: u32 = 50;   // sprite 列数上限
pub const JPEG_QUALITY: u8 = 75;   // 编码质量

pub struct VideoThumb { pub time_secs: f64, pub image: RgbaFrame }

#[serde(rename_all = "camelCase")]
pub struct ThumbnailCacheMeta { pub tile_width: u32, pub tile_height: u32, pub columns: u32, pub times: Vec<f64> }

pub fn load_sprite(cache_root, key) -> Option<Vec<VideoThumb>>;
pub fn save_sprite(cache_root, key, thumbs: &[VideoThumb]) -> Result<()>;
```

### 网格几何（纯函数）
- `grid_geometry(count)`：`columns = min(50, count).max(1)`，`rows = div_ceil(count, columns)`（对齐 `MediaVisualCache.swift:268-269`）。
- `tile_position(i, columns)`：`(col = i % columns, row = i / columns)`——**行主序、左上原点**。

> 与上游坐标系的处理差异：上游 CGContext 是左下原点需翻转 `y`；本实现用 `image` crate（左上原点），写/读自成闭环（写时行主序左上、读时同规则裁剪），只保证 `times` 顺序与 tile 顺序一致即可，无需复刻翻转。

### 落盘 / 读取
- 文件：`<cache_root>/MediaVisualCache/<key>.thumbs.jpg` + `<key>.thumbs.json`（`CACHE_SUBDIR` 与波形共用同目录）。
- `save_sprite`：`compose_sprite` 拼图（只放与首帧同尺寸的 tile，背景填黑）→ RGBA 丢 alpha → JPEG 质量 **75** 编码（对齐 `kCGImageDestinationLossyCompressionQuality:0.75`，`MediaVisualCache.swift:286`）→ 先写 JPG，**最后写 JSON sidecar（= 完整条目标记）**。
- `load_sprite`：先读 JSON（缺失/解析失败 → `None`）→ 校验 meta（尺寸 > 0、times 非空）→ 开 JPG 转 RGBA8 → **尺寸校验** `sprite ≥ (tile_w×cols_used, tile_h×rows)`，否则 `None`（对齐 `:249-250`）→ 逐 tile 裁回 `VideoThumb`。

### 兼容性目标
OpenTake 写出的 `.thumbs.jpg/.json` 与上游**同机可互读**：同 key、同 camelCase 字段名 `tileWidth/tileHeight/columns/times`。JPEG 有损，但暗/亮 tile 重建后仍可区分（测试断言）。

---

## 并发闸门
上游用 `AsyncSemaphore`（波形 gate=2、图片 gate=4）。本模块不持并发状态——调度集中在上层（[library-index.md](library-index.md) 的索引内核 / `opentake-core` 的 worker）。

## 完成状态
缩略图生成 + 雪碧图缓存能力**已实现**。导入路径接线一度缺失（`MediaItemDto.thumbnail` 写死 `None`，[PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) P1-2/P1-3）——属上层接线，不是本模块缺能力。

## 测试
时间公式（短/长片/边界/零值）、图片缩略图（长边缩放/不放大）、网格几何（50 列上限/行数/零输入）、tile 位置（行主序）、save/load 往返（时间戳/像素/尺寸保留、JPEG 有损边界、camelCase JSON）、完整性标记（缺 sidecar→None、JSON 损坏→None）、多行 sprite（60 tile → 50×2）。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 相关：[decode.md](decode.md) · [waveform.md](waveform.md)（共用 `MediaVisualCache` 缓存目录）
- 模块文档树：[../INDEX.md](../INDEX.md) · docs 总目录：[../../INDEX.md](../../INDEX.md)
- 源码根：`../../../crates/opentake-media/src/`
