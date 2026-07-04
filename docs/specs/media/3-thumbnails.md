# 缩略图(seek 解帧)+ sprite 网格缓存(照搬 `MediaVisualCache`)

对应 `Timeline/MediaVisualCache.swift`。上游有三类:视频缩略图序列(sprite 网格缓存)、图片单缩略图、波形(§4)。

## 3.1 视频缩略图序列 + 时间点公式

```rust
// thumbnail/mod.rs
pub struct VideoThumb { pub time_secs: f64, pub image: RgbaFrame }

/// 生成视频缩略图序列(命中缓存直接返回)。`on_partial` 用于长视频渐进回调
/// (对齐上游每 50 帧 publish 一次,MediaVisualCache.swift:123)。
pub fn video_thumbnails(path: &Path, duration_secs: f64,
    on_partial: Option<&dyn Fn(&[VideoThumb])>) -> Result<Vec<VideoThumb>>;
```
- **时间点**:`videoThumbnailTimes`(`MediaVisualCache.swift:192-202`)——`interval = duration < 10 ? 1.0 : 2.0`,`stride(from:0, to:duration, by:interval)`。逐字照搬。
- **缩放上界**:`max_size = (120, 68)`(`MediaVisualCache.swift:114`),tolerance 1.0s(`:116-117`),apply_rotation=true(`:115`)。
- **去重**:批量解帧的 `t > lastTime`(本质同 FrameSampler)。
- **渐进发布**:每 50 帧回调一次(`MediaVisualCache.swift:123-129`)。Rust 用回调闭包(UI 进度交给上层 Tauri event)。

## 3.2 图片单缩略图

```rust
pub fn image_thumbnail(path: &Path, max_pixel: u32) -> Result<RgbaFrame>; // max_pixel 默认 120
```
对齐 `makeImageThumbnail`(`MediaVisualCache.swift:152-163`,`kCGImageSourceThumbnailMaxPixelSize:120`、应用 EXIF transform)。Rust 用 `image` crate 解码 + `kamadak-exif`/`image` 的方向处理 + 等比缩放到长边 ≤ 120。

## 3.3 sprite 网格磁盘缓存(逐字节复刻)

上游把缩略图序列拼成**一张 JPEG sprite + JSON sidecar**;sidecar 最后写、作为「完整条目」标记(`MediaVisualCache.swift:236-293`)。

```rust
// thumbnail/sprite.rs
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ThumbnailCacheMeta {
    pub tile_width: u32, pub tile_height: u32, pub columns: u32, pub times: Vec<f64>,
}

/// 缓存目录:<cache_root>/MediaVisualCache/<key>.thumbs.jpg + <key>.thumbs.json
pub fn load_sprite(cache_root: &Path, key: &str) -> Option<Vec<VideoThumb>>;
pub fn save_sprite(cache_root: &Path, key: &str, thumbs: &[VideoThumb]) -> Result<()>;
```
逐项对齐(`MediaVisualCache.swift`):
- **布局**:`columns = min(50, count)`,`rows = ceil(count/columns)`(`:268-269`)。tile 尺寸 = 首帧像素宽高(`:266-267`)。
- **坐标系**:CGContext 原点左下,行 0 在顶部 → `y = (rows-1-row)*tileH`(`:277-279`)。Rust 用 `image` crate(原点左上),则**直接** `y = row*tileH`(无需翻转,因为 `image` 已是左上原点);但裁剪/写入要与读取一致——**自成闭环即可**(写时按行优先左上,读时按同样规则裁剪),不必复刻 CG 的翻转;只需保证 `times` 顺序与 tile 顺序一致。
- **JPEG 质量**:0.75(`:286`,`kCGImageDestinationLossyCompressionQuality:0.75`)。
- **读校验**:`sprite.width ≥ tileW*min(columns,count)` 且 `sprite.height ≥ tileH*rows`,否则视为无效返回 None(`:249-250`)。逐 tile `cropping`(`:253-260`)。
- **原子写**:sidecar JSON 最后写;读时以「JSON 可解码 + sprite 可解码 + 尺寸校验通过」为完整标记(`:238-247`)。

> 兼容性目标:OpenTake 写出的 `.thumbs.jpg/.json` 与上游可互读(同 key、同 meta 字段名 `tileWidth/tileHeight/columns/times`)。`ThumbnailCacheMeta` 用 `#[serde(rename_all="camelCase")]`。

## 3.4 缩略图并发闸门

上游用 `AsyncSemaphore`:波形 gate=2、图片 gate=4(`MediaVisualCache.swift:16/27`),视频缩略图无显式 gate 但 `Task.detached(.userInitiated)`。Rust 用 `tokio::sync::Semaphore`,值照搬;调度集中在 §7.7 / §3.5 的服务层。

## 3.5 缩略图/波形服务(替 `MediaVisualCache` 的 @MainActor 内存表)

上游 `MediaVisualCache` 持三张内存表(`waveformSamples`/`videoThumbnails`/`imageThumbnails`)+ in-flight 去重 + 触发重绘。在 OpenTake,**内存缓存与「触发重绘」属于上层**(Rust core / 前端);本 crate 只提供**纯生成 + 磁盘缓存**函数(上面 §3.1–3.4 + §4)。内存表 + in-flight 去重 + 进度回调放 `opentake-core`(或 render 的预览侧),用 Tauri event 推前端。理由:`opentake-media` 保持「无 UI 状态」的可测纯度;`needsDisplay` 是 AppKit 概念,跨平台由前端订阅事件实现。
