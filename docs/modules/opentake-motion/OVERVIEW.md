# opentake-motion — 模块总览

> 上级：[模块目录 INDEX.md](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> ⚠️ **本模块当前是脚手架 / fallback 层**。动效 / AI Video 的 v1 主路径改为外部 **Motion Canvas 插件**（待新增 `plugins/motion-canvas-studio/`，目前仓库中**尚不存在**），由它产出可导入的 `mp4`，OpenTake 按普通视频媒体导入、落轨、预览、导出。本 crate 保留为后续**原生透明 alpha overlay / RGBA frame-sequence / HTML-CSS fallback** 的基础设施。完整设计见 [Motion Graphics 插件设计](MOTION-GRAPHICS-PLUGIN.md)（只读规格）。

---

## 1. 一句话定位与依赖分层

`opentake-motion` 是**原生 web 动态图形（HTML/CSS/JS）的 fallback 渲染原语层**：把一段自包含的 web 文档（或模板 + 参数）确定性地逐帧栅格化为**磁盘上的 RGBA PNG 帧序列**，经内容寻址缓存复用，并把结果适配成 [`opentake-render`](../opentake-render/INDEX.md) 的 clip source，使合成器能把它当作普通纹理。它**不是 Lottie 渲染器**（Lottie 在 render；见 §3），也**不是** v1 的主动效引擎。

依赖分层（只向下依赖）：

```
opentake-domain          值语义叶子层（禁 I/O）—— 本 crate 仅作为 workspace 依赖挂着
   ▲
opentake-render          定义 DecodedFrame / SourceMetrics / FrameProvider 契约
   ▲
opentake-motion          ← 本模块：实现上述契约，提供 motion 帧序列 source
   ▲
opentake-core / src-tauri / opentake-agent   未来的调用方（v1 走 Motion Canvas 插件，尚未接线）
```

- **依赖**：`opentake-render`（**实现**它定义的 `DecodedFrame` / `SourceMetrics` / `FrameProvider` 三个 clip-source 契约，让 motion 帧序列对合成器零特殊处理）、`opentake-domain`（作为 workspace 依赖挂着，当前模块内未直接消费其类型）；纯逻辑依赖 `serde` / `serde_json` / `sha2` / `hex`（缓存键）/ `thiserror`（错误）。
- **被调用**：设计上由 `opentake-core` / `src-tauri` / `opentake-agent` 调用（`add_motion_graphic` / `edit_motion_graphic`）；**但当前未接线**——v1 走外部 Motion Canvas 插件，原生 fallback 入口待后续阶段补。
- **可选后端**：真实 headless-Chromium（CDP）渲染藏在 `chromium` cargo feature 之后；**默认 build 与 CI 完全离线、不需要浏览器**，骨架仍编译但 `render()` 返回 `RendererUnavailable`。

模块没有顶层"门面 struct"，公开 API 在 `lib.rs` 扁平 re-export（值类型 + 缓存 + 渲染 trait + 沙箱 + 集成桥）。

---

## 2. 职责边界（做什么 / 不做什么）

**做：**
- 定义动效**值类型**：`MotionSource`（`Code` 内联 HTML/CSS/JS，或 `Template` 模板 id + 参数）、`MotionRenderRequest`（fps / 帧数 / 宽高 / 透明）、`RenderedClip`（磁盘帧路径 + 元数据）。纯值、可序列化、可全单测。
- **边界校验**：源（空代码 / 非法 hex 颜色）、请求范围（fps / 帧数 / 尺寸硬上限，见 `source::limits`）。
- **内容寻址缓存**：SHA-256 over（源 + 参数 + fps + 尺寸 + 透明）→ 帧目录；同输入命中复用，任意改动失效（path-independent、self-invalidating）。
- **确定性渲染契约** `MotionRenderer` trait + 两个实现：`StubRenderer`（无浏览器、纯函数纯色帧，给测试 / 离线管线）与 `HeadlessChromiumRenderer`（真实 CDP 后端**骨架**，feature-gated）。
- **沙箱策略类型** `SandboxPolicy`：网络默认全拒（仅显式 origin 白名单）、渲染超时熔断、内联文档大小上限——把安全要求建模成**类型**，渲染器无法"忘记应用"。
- **模板清单模型** `MotionPlugin`（`plugin.json`）：名称 / 参数 schema / 时长模型 / fps 策略 / 透明，含严格校验与"已绑定参数 vs schema"校验。
- **集成桥** `MotionClipSource`：把 `RenderedClip` 适配成 render 的 `SourceMetrics` + `FrameProvider`，解码器（PNG→RGBA）由调用层注入（本 crate 默认依赖面不带解码器）。

**不做（有意省略 / 不在本模块）：**
- **不渲染 Lottie**。Lottie 的 `TextureSource::Lottie` / `lottie_frame` / `lottie_frame_count` 在 [`opentake-render`](../opentake-render/INDEX.md)；上游 `LottieVideoGenerator`（`.json`/`.lottie`→ProRes4444 alpha）映射到 render/media，**不是本 crate**。`MotionClipSource` 虽实现了 render trait 里的 `lottie_frame` 方法签名，但其实现只是转发到自己的帧序列，并非真正的 Lottie 源（见 §3 澄清）。
- **不是 v1 主渲染器**。完整片头 / 解释动画 / 数据动画走外部 Motion Canvas 插件产 `mp4`，复用普通视频导入/预览/导出链路；本 crate 不阻塞该路径。
- **不做真实浏览器渲染**（默认）。`HeadlessChromiumRenderer` 只有骨架 + 步骤文档，live CDP 调用 feature-gated 且**尚未实现**（即便开了 `chromium` feature 也返回 `RendererUnavailable`，绝不假装渲染成功）。
- **不持 UI 状态 / 不做时间线落轨**。导入 + 落轨的单事务（Render → Import Media → Place Clip）属 `opentake-core`/`opentake-ops`，本 crate 只产帧 + 提供 source。
- **不定义 `ClipType::Motion`**。透明动效与新 clip 类型 / frame sequence source 是后续目标。
- **不做帧↔秒折算的真理**。本 crate 内部 `t = frame / fps` 仅用于渲染时间网格；时间线帧↔秒的真理在 domain / 调用层（移植铁律，见 §6）。

---

## 3. 关键概念与数据流

### Fallback 渲染管线（native 路径）

```text
MotionSource (Code 内联文档 | Template id + params)
  └─ MotionRenderRequest (fps, duration_frames, w, h, transparent)   [validate() 范围校验]
       └─ content_hash ──▶ MotionCache   (命中 → 复用磁盘帧)
            └─ MotionRenderer::render    (未命中 → 渲染)
                 ├─ StubRenderer             (确定性、无浏览器；测试 / 离线)
                 └─ HeadlessChromiumRenderer (CDP 虚拟时间逐帧截图；feature `chromium`，骨架未实现)
                      └─ RenderedClip (磁盘 RGBA PNG 帧序列)
                           └─ MotionClipSource: impl SourceMetrics + FrameProvider
                                └─ opentake-render 合成器（未来纹理层）
```

### 确定性 = 预览与导出像素一致 + 缓存可信
渲染器**必须可复现**：同一请求每次产出字节一致的帧。这是"预览 == 导出"的前提，也是内容寻址缓存（`cache::content_hash`）成立的基础。哈希喂入一个规范、无歧义的字节流（各字段长度前缀 / 定界，模板参数用 `BTreeMap` 固定顺序），并带 `opentake-motion/v1` 版本前缀，便于将来变更哈希内容时整体失效而非静默碰撞。`StubRenderer` 的帧色是 `(帧号, content-hash)` 的纯函数，正是为了离线确定性可测。

### 确定性时钟契约
两个渲染器共享 `deterministic_clock_script()`——注入页面的 JS：冻结 `document.timeline.currentTime`、暴露 `window.OpenTake.seek(seconds)` 与 `OpenTake.onSeek(fn)`。真实 CDP 后端用 `Page.addScriptToEvaluateOnNewDocument` 在作者脚本之前注入它，宿主每帧把虚拟时间推进到 `i / fps` 并截图，从而不依赖墙钟、逐帧确定。

### 沙箱（安全隔离）
不可信的 native fallback 代码（agent 生成或社区模板）在隔离的离屏引擎里渲染。`SandboxPolicy` 把要求建模成类型：
- **网络默认全拒**——空白名单 ⇒ 完全离线（也是测试/CI 确定性的来源）；白名单只接受 `https://`（或 loopback `http://`）显式 origin，**不支持通配**。`data:` URI 永远放行（内联、无网络）。
- **渲染时间预算**熔断 `while(true)` 之类的跑飞动画（默认 60s）。
- **内联文档大小上限**（默认 256 KiB）在文档进引擎前就拒绝超大输入。
- **无文件系统 / 工程访问**——这是引擎启动期不变量（启动标志 + 空 profile），在策略类型里**故意没有任何授予路径的字段**来体现。

网络/CSP 的执行落在真实 CDP 后端（`chromium` feature 后）；策略类型与其纯检查（`check_url` / `check_document_size`）在本 crate，无需引擎即可单测。注意：连 `StubRenderer` 也会执行文档大小检查，确保安全契约被测试覆盖到。

### 与 render 的集成桥
`opentake-render` **定义** `SourceMetrics`（`natural_size` / `needs_premultiply`）/ `FrameProvider`（`decoded_frame` / `image_pixels` / `lottie_frame`）/ `DecodedFrame`；本模块 `MotionClipSource` **实现**它们：
- `natural_size` = 渲染画布尺寸；`needs_premultiply` = 是否透明（透明帧带直 alpha，合成前需预乘，与 alpha 视频同契约）。
- 帧文件→RGBA 的解码**不硬接** PNG 库，而是接收调用层注入的 `FrameDecoder`（`Fn(&Path) -> Option<DecodedFrame>`），因为帧可能来自 stub（自制 PNG）、未来 headless-Chromium（标准 PNG）、Motion Canvas 图片序列、或未来裸 RGBA 快路径。测试注入基于 `image` dev-dep 的解码器；app 注入自己的 image/ffmpeg 栈。
- 过界帧索引钳到最后一帧（freeze-frame 定格，与 `RenderedClip::frame_path` 一致，也对齐上游 Lottie/图片的"末帧定格"行为）。

> **Lottie 方法澄清**：`MotionClipSource` 为满足 render 的 `FrameProvider` trait 实现了 `lottie_frame`，但它只是把请求转发到自身的帧序列——motion clip 始终是帧序列，不存在真正的 Lottie 内部帧概念。真正的 Lottie 渲染在 render（`TextureSource::Lottie`，见 [opentake-render](../opentake-render/INDEX.md)）。

---

## 4. 对应上游 Swift

逐模块映射见 [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md)。

**本 crate 在上游 Palmier Pro 中没有直接对应物。** 上游的"动态图形"仅指 **Lottie**：`LottieVideoGenerator`（用 Lottie 库把 `.json`/`.lottie` 逐帧渲染到 `CGContext`→`CVPixelBuffer`，写成 ProRes4444 alpha `.mov`，末帧定格）。该能力按移植地图落到 **render / media**（rlottie FFI 或前端 lottie-web 烘焙），**而非本 crate**。

`opentake-motion` 是 OpenTake 为"AI 生成的程序化 web 动效"新增的能力（Issue #34），上游没有 headless-browser / HTML-CSS 动效引擎。因此本模块**不要求与上游 1:1**；唯一从上游继承的语义是 freeze-frame 末帧定格（与 `LottieVideoGenerator` 一致），以及整数帧 / 截断换算等通用移植铁律（见 §6）。

| 本模块概念 | 上游 Swift 对应 | 说明 |
|---|---|---|
| `RenderedClip` 帧序列 + 末帧定格 | `LottieVideoGenerator` 的 freeze-frame | 仅借鉴"过末端定格"行为；实现完全不同 |
| 整个 web 动效渲染管线 | （无对应） | 上游无 headless browser 动效引擎，OpenTake 新增 |
| `MotionClipSource`（→ 合成器纹理源） | （无对应） | 对齐 render 的 source 契约，非上游移植 |

---

## 5. 完成状态（已实现 vs 计划中）

对照 [ROADMAP.md](../../architecture/ROADMAP.md) **Phase 10**、[MOTION-GRAPHICS-PLUGIN.md](MOTION-GRAPHICS-PLUGIN.md) §9 与代码现况：

**已实现（代码 + 单测齐备，纯逻辑，离线可测）：**
- 值类型：`MotionSource` / `MotionRenderRequest` / `RenderedClip` / `ParamValue`（serde 往返、范围与 hex 颜色校验、`duration_seconds`、`frame_path` 钳位定格）。
- 内容寻址缓存：`content_hash`（规范字节流 + v1 版本前缀 + 参数类型标签 + `-0.0` 归一）、`MotionCache`（`dir_for` / `ensure_dir` / `is_cached` 按帧数完整性判定，partial 视为 miss / `frame_file` 零填充命名）。
- 渲染契约 `MotionRenderer` trait + `deterministic_clock_script()`。
- `StubRenderer`：确定性纯色 RGBA 帧、自制无依赖 PNG 编码器（stored-block zlib + CRC32 + Adler-32）、透明时 alpha 线性渐变、文档大小检查。
- `HeadlessChromiumRenderer` **骨架**：步骤文档齐全、`data_url_for_code`（百分号编码）、`frame_time_grid`（虚拟时间网格）、validate + 沙箱大小检查均在"unavailable"路径前执行；live CDP 渲染**未实现**（无论是否开 `chromium` feature 都返回 `RendererUnavailable`）。
- 沙箱：`SandboxPolicy`（默认离线 / 自定义超时 / origin 白名单去重 / `check_url`（`data:` 放行、通配不支持、明文远程拒绝）/ `check_document_size`）。
- 模板清单：`MotionPlugin`（容错解码、严格 `validate`、`validate_params` 必填+类型+未知参数拒绝、`effective_fps`、`DurationMode`/`FpsPolicy`/`ParamSpec`）。
- 错误：`MotionError`（thiserror，含 `RendererUnavailable` / `Timeout` / `Sandbox` / `Io` 等可匹配变体）。
- 集成桥：`MotionClipSource` 实现 `SourceMetrics` + `FrameProvider`，解码器注入、过末端钳位、缺帧返回 `None`。

**计划中 / 待做（明确未实现）：**
- **真实 headless-Chromium 渲染**：`#[cfg(feature = "chromium")]` 下接 CDP 客户端（如 `chromiumoxide`）——定位/启动浏览器、`Fetch.enable` 请求拦截执行白名单、应用 CSP 与超时熔断、虚拟时间逐帧截图、CDP 错误映射到 `RenderFailed`/`Timeout`。**当前为 TODO(#34)**。
- **v1 Motion Canvas 插件链路（主路径）**：`plugins/motion-canvas-studio/`（fork/vendor，MIT）、Tauri `motion_canvas.rs` 命令、独立 Motion Panel、agent dispatch 从 `not yet implemented` 接到 Motion Canvas workflow、license notice / 依赖 license report——**仓库中尚不存在**（无 `plugins/` 目录）。
- **原生 fallback 接线**：把本 crate 的 `MotionClipSource` 真正接入合成器纹理层、引入 `ClipType::Motion` 或 `TextureSource::FrameSequence`、PNG 序列 source、透明 alpha overlay——属 v2/v3。
- **持久化元数据**：`motion-result.json` / `media_metadata` 字段（engine/license/prompt/sourceHash…）为规划，未落地。

---

## 6. 移植铁律（本模块必须遵守）

来自 [AGENTS.md](../../../AGENTS.md) 移植铁律、[MOTION-GRAPHICS-PLUGIN.md](MOTION-GRAPHICS-PLUGIN.md) 与代码现况：

1. **一切以整数帧为单位**：渲染时间网格 `t = i / fps`（`i ∈ 0..duration_frames`）；`RenderedClip` / 缓存 / source 全以帧索引寻址。时间线帧↔秒的真理留在 domain / 调用层（`secondsToFrame` 用截断 `Int(s*fps)`，不四舍五入）。
2. **确定性可复现（预览 == 导出）**：渲染器必须对同一 `MotionRenderRequest` 产出**字节一致**的帧。这是内容寻址缓存与"预览/导出像素一致"的硬前提；任何引入非确定性（墙钟、随机、未冻结的页面时钟）的实现都违规。
3. **末帧定格（freeze-frame hold）**：clip 被拉过自然末端时定格在最后一帧而非报错——`RenderedClip::frame_path` 与 `MotionClipSource` 的过末端钳位必须保留，对齐上游 `LottieVideoGenerator` 行为。
4. **缓存键 = 影响像素的一切，且 path-independent**：哈希覆盖 源 + 参数 + fps + 宽高 + 透明，带版本前缀；规范字节流（长度前缀 + 参数类型标签 + `BTreeMap` 顺序 + `-0.0`→`0.0` 归一 + 颜色大小写不敏感）保证无歧义、无碰撞。改哈希内容必须升版本前缀。
5. **安全建模为类型**：网络默认全拒、超时熔断、文档大小上限、无文件系统访问——这些是 `SandboxPolicy` 的不变量；渲染器不可绕过。明文远程 origin 一律拒绝，白名单不支持通配。
6. **`#[serde(default)]` + `Option<T>` 容旧**：所有落盘模型（`MotionPlugin` 清单、`MotionSource` 模板参数等）字段加默认，读旧工程/旧清单不破坏；校验作为独立显式 pass，不在反序列化里硬失败。
7. **错误用 `thiserror`（`MotionError`），可匹配 + 人类可读**：后端不可用要**显式失败并带可操作文案**（`RendererUnavailable`），绝不静默吞错或假装渲染成功；边界层再转字符串。
8. **纯逻辑优先、后端可插拔**：值类型 / 缓存键 / 沙箱检查 / 清单校验全是无副作用纯函数，可全离线单测；真实浏览器后端 feature-gated，帧解码器由调用层注入——本 crate 默认依赖面不绑浏览器、不绑解码器。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
