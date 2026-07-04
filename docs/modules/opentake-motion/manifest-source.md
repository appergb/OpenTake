# manifest + source — 值类型与模板清单

> 上级：[模块目录 INDEX.md](INDEX.md) · [总览 OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
> 源码：[`../../../crates/opentake-motion/src/source.rs`](../../../crates/opentake-motion/src/source.rs) · [`../../../crates/opentake-motion/src/manifest.rs`](../../../crates/opentake-motion/src/manifest.rs) · [`../../../crates/opentake-motion/src/error.rs`](../../../crates/opentake-motion/src/error.rs)

---

## 职责

本文档覆盖动效的**纯数据层**：渲染什么（`MotionSource`）、怎么渲染（`MotionRenderRequest`）、产物句柄（`RenderedClip`），以及模板包的清单模型（`MotionPlugin`，`plugin.json`）与错误类型（`MotionError`）。全部纯值、可序列化、可全单测，无渲染器、无 I/O。

> 完成状态：**全部已实现并全测**（值类型 serde 往返 + 范围/颜色校验、清单容错解码 + 严格校验）。这些类型在 Motion Canvas v1 路径里不直接参与（v1 导入普通视频媒体），保留给后续 native fallback / frame-sequence 路径。

---

## source.rs — 值类型

### `limits`（硬上限常量）
请求范围在边界就拒绝，避免熔毁离屏引擎：
- `MAX_FRAMES = 3600`（如 60fps×60s）——动效是 overlay/标题，长时长应在时间线上循环而非单次巨渲。
- `MAX_DIMENSION = 4096`（覆盖 4K 任意朝向）/ `MIN_DIMENSION = 2`（合成器偶数化下限）/ `MAX_FPS = 240`。

### `ParamValue`
模板参数值，刻意小：`String` / `Number(f64)` / `Bool` / `Color(#RRGGBB|#RRGGBBAA)`。外部 serde 标签 `{type, value}`。
- `matches_declared(declared)`：值是否满足声明类型字符串（`"string"|"number"|"bool"|"boolean"|"color"`）；**未知声明类型一律接受**（前向兼容，旧 host 不被新 manifest 硬挂）。

### `MotionSource`
native fallback 动效的来源，两个 arm（外部标签 serde，紧凑往返）：
- `Code { html_css_js }`——自包含内联 web 文档（即兴模式）。确定性时钟由渲染器注入，作者对 `OpenTake.seek(seconds)` / `document.timeline.currentTime` 动画。
- `Template { id, params }`——按 id 实例化注册模板 + 绑定参数；`params` 用 `BTreeMap` 固定顺序（缓存键依赖它）。
- 构造器 `code()` / `template()`；`validate()`：拒空代码、拒空模板 id、拒非法 hex 颜色参数（**不**做模板 schema 交叉校验，那是 `MotionPlugin::validate_params` 的活）。
- `is_hex_color`：仅 `#RRGGBB` / `#RRGGBBAA`（大小写不敏感），3 位/无 `#`/非 hex 一律拒。

### `MotionRenderRequest`
确定性渲染请求 = 源 + 帧网格 + 画布。`#[serde(rename_all = "camelCase")]`（注意：IPC 线上多词字段 camelCase，对齐项目 serde 约定）。字段：`source` / `fps` / `duration_frames` / `width` / `height` / `transparent`。每个字段都参与缓存键。
- `new(...)`：`transparent` 默认 `true`（overlay 是动效主场景）；`with_transparent(bool)` builder。
- `validate()`：校验源 + fps（`1..=MAX_FPS`）+ 帧数（`1..=MAX_FRAMES`）+ 宽高（`MIN..=MAX_DIMENSION`）。纯函数，交给渲染器前调用。
- `duration_seconds()` = `duration_frames / fps`。

### `RenderedClip`
成功渲染的产物：磁盘 RGBA 帧文件序列 + 合成器所需元数据。`camelCase` serde。
- 帧**落磁盘**（不在内存）——一个 motion clip 可能几千张 4K RGBA 帧，合成器经 [integration](integration.md) 懒加载。
- 字段：`content_hash`（缓存目录名）/ `frames`（按播放序的绝对路径，`frames[i]` 对应 `t = i/fps`，PNG）/ `fps` / `width` / `height` / `transparent`。
- `frame_count()` / `duration_seconds()`。
- `frame_path(frame)`：0 基索引取帧路径，**过末端钳到最后一帧**（freeze-frame 定格，对齐上游 Lottie/图片的末帧定格）；空帧返回 `None`。

---

## manifest.rs — 模板清单 `MotionPlugin`

模板包 = web bundle + `plugin.json`。风格对齐 `opentake-agent` 的工作流插件清单：纯 JSON、`snake_case`、每字段 `#[serde(default)]` 容错（部分/旧清单仍可解码），校验是独立显式 pass。

### 子类型
- `DurationMode`：`Fixed`（内在固定时长，用 `default_seconds`）/ `Driven`（宿主驱动，调用方挑 `duration_frames` 让模板填满，如进度条/可保持卡片）。
- `DurationSpec`：`mode` + `default_seconds`（默认 5.0；`fixed` 直接用，`driven` 作建议）。
- `FpsPolicy`：`Inherit`（跟项目 fps，序列化 `"inherit"`）/ `Fixed(u32)`（模板固定 fps，序列化 `{"fixed": 30}`）。
- `ParamSpec`：`kind`（声明类型，serde 名 `type`）/ `required`（默认 false）/ `label`。
- `MotionPluginAuthor`：`name` / `url`。

### `MotionPlugin` 字段
`schema_version` / `id` / `name` / `description` / `entry`（默认 `"index.html"`）/ `author` / `license` / `params`（`BTreeMap<String, ParamSpec>` 稳定顺序）/ `duration` / `fps` / `transparent`（默认 `true`）。

> `Default` 手写（非 derive），以**精确匹配 serde 字段默认**（`entry = "index.html"`、`transparent = true`）；derive 的 `Default` 会用 `String::default()`/`bool::default()` 与解码 `{}` 的结果发散。有测试断言 `from_str("{}") == MotionPlugin::default()`。

### 方法
- `validate()`：严格校验自身字段（独立于任何实例）——非空 id/name/entry、`default_seconds` 正有限、`Fixed` fps 在范围、参数名非空、参数类型若非空须是已知类型。返回首个问题。
- `validate_params(bound)`：校验"已绑定参数 vs schema"——每个 `required` 参数须存在、present 参数须类型匹配、**未知参数拒绝**（让 typo 暴露）。
- `effective_fps(project_fps)`：`Inherit` → 项目 fps；`Fixed(f)` → `f`。

---

## error.rs — `MotionError`

库风格 `thiserror`，调用方可按失败类型 match，消息人类可读（可面向 agent/UI）。`MotionResult<T> = Result<T, MotionError>`。

变体：`InvalidSource` / `InvalidRequest` / `UnknownTemplate` / `Manifest` / `RendererUnavailable`（带可操作文案，让 agent 解释能力缺口而非不透明失败）/ `Timeout(Duration)` / `Sandbox` / `RenderFailed` / `Io(#[from] std::io::Error)`。配套 `invalid_source()` 等构造器。

---

## 移植铁律落地

- **`#[serde(default)]` + 容旧**：`MotionPlugin` 全字段默认，读旧清单不破坏；校验独立 pass，不在反序列化硬挂；未知参数类型前向兼容接受。
- **整数帧 + 末帧定格**：请求/产物以帧为单位，`frame_path` 过末端钳位。
- **缓存键确定性**：`params` 用 `BTreeMap` 固定顺序，请求每字段参与哈希（见 [cache.md](cache.md)）。
- **camelCase 边界**：`MotionRenderRequest` / `RenderedClip` 用 camelCase serde，对齐项目 IPC 多词字段约定。
- **错误显式可匹配**：`RendererUnavailable` 等带可操作文案。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
