# integration — 与渲染管线的集成桥

> 上级：[模块目录 INDEX.md](INDEX.md) · [总览 OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
> 源码：[`../../../crates/opentake-motion/src/integration.rs`](../../../crates/opentake-motion/src/integration.rs)

---

## 职责

把一个已渲染的动效 clip（`RenderedClip`）暴露成 [`opentake-render`](../opentake-render/INDEX.md) 认识的**普通 clip source**，使 wgpu 合成器能把未来的 native frame-sequence/alpha 动效 clip 当作任意其他纹理处理（零特殊分支）。

关键分工：**`opentake-render` 定义** `SourceMetrics` / `FrameProvider` 两个 trait 与 `DecodedFrame` 类型；本模块**实现**它们于 `RenderedClip` 之上。合成器问 clip 的自然尺寸（渲染画布），并按需拉取解码后的 RGBA 帧。

> 完成状态：`MotionClipSource` 适配器**已实现并全测**；但合成器**尚未真正接入** motion 帧序列（v1 走 Motion Canvas 视频导入，native frame-sequence source 属后续，见 [OVERVIEW.md](OVERVIEW.md) §5）。

---

## 解码器注入（为什么不硬接 PNG 库）

帧文件→RGBA 的解码**刻意不**在本 crate 硬接某个 PNG 库。帧可能来自：
- `StubRenderer`（自制 stored-block PNG），
- 未来 native headless-Chromium fallback（标准 PNG），
- Motion Canvas 图片序列输出，
- 未来裸 RGBA 快路径。

所以 `MotionClipSource` 接收一个 `FrameDecoder`——`Fn(&Path) -> Option<DecodedFrame>`——由集成层提供（它本就持有 image/codec 栈）。测试注入 stub 自己的解码器（基于 `image` dev-dep）；app 注入 `image`/ffmpeg。这样本 crate 的**默认依赖面不带解码器**，又能全测。

```rust
pub type FrameDecoder<'a> = dyn Fn(&Path) -> Option<DecodedFrame> + 'a;
```

解码器对缺失/损坏文件返回 `None`——合成器把该帧当"缺帧"处理（与视频解码失败同语义）。

---

## `MotionClipSource<'a>`

`RenderedClip` 适配到 render 的 clip-source trait。**单 clip 设计**：每个 motion clip 由合成器构建一个。

- `new(clip, decode)`：包一个 clip + 解码器。
- `clip()`：取被包的 clip。
- `frame(frame: i64)`：解码 0 基索引帧；**负索引→0**，过末端钳到最后一帧（freeze-frame 定格，与 `RenderedClip::frame_path` 一致）。

### `media_ref` 语义
每个方法都**忽略** `media_ref` 参数——本适配器只包一个 clip。在更大系统里，motion clip 的 ref 经调用方 resolver 解析到**这个** source 实例（镜像 image/video ref 解析到各自解码器的方式）。

### `impl SourceMetrics`
- `natural_size(_)` = clip 的 `(width, height)`（渲染画布）。
- `needs_premultiply(_)` = `clip.transparent`——透明动效帧带**直 alpha**，合成器混合前须预乘（与 alpha 视频同契约）。

### `impl FrameProvider`
- `decoded_frame(_, source_frame)` → `frame(source_frame)`：motion clip 是帧序列，`source_frame` 直接索引渲染帧（时间线帧→源帧的映射在上游 plan builder；1:1 overlay 时二者重合）。
- `image_pixels(_)` → `frame(0)`：不是图片源，但若调用方误当图片，返回首帧而非空。
- `lottie_frame(_, frame)` → `frame(frame)`：**不是 Lottie 源**——仅为满足 trait 签名而转发到自身帧序列。真正的 Lottie 渲染在 render（`TextureSource::Lottie`）。

---

## 数据流（本桥所处位置）

```text
RenderedClip (磁盘 PNG 帧)
  └─ MotionClipSource::new(clip, decode)
       ├─ SourceMetrics  → natural_size / needs_premultiply
       └─ FrameProvider  → decoded_frame(source_frame) → (decode)(frame_path)
            └─ opentake-render 合成器纹理层（未来接入）
```

测试覆盖：`natural_size` = 渲染画布、`needs_premultiply` 跟透明、`decoded_frame` 返回正确形状 RGBA、过末端钳位仍解码、解码器失败返回 `None`、负 `source_frame` 映射到首帧。

---

## 移植铁律落地

- **末帧定格 / 负索引归零**：`frame()` 钳位，对齐 `RenderedClip` 与上游 Lottie/图片定格。
- **透明 = 预乘契约**：`needs_premultiply` 跟随 `transparent`，与 alpha 视频一致。
- **零特殊处理**：实现 render 既有 source 契约，使 motion clip 对合成器是普通纹理。
- **默认依赖面不绑解码器**：解码器注入，本 crate 不引 PNG/ffmpeg 运行时依赖（仅 dev-dep 测试）。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 相关模块：[opentake-render](../opentake-render/INDEX.md)（定义 `SourceMetrics`/`FrameProvider`/`DecodedFrame`，含真正的 Lottie 源）
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
