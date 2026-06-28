# renderer — 渲染契约与实现

> 上级：[模块目录 INDEX.md](INDEX.md) · [总览 OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
> 源码：[`../../../crates/opentake-motion/src/renderer.rs`](../../../crates/opentake-motion/src/renderer.rs)

---

## 职责

定义动效渲染的**单一契约** `MotionRenderer` trait，并提供两个实现 + 一个共享的确定性时钟脚本。给定一个已校验的 `MotionRenderRequest`，产出 `RenderedClip`（磁盘上的 RGBA 帧序列）。这是其余系统唯一依赖的渲染抽象。

> 完成状态：`StubRenderer` 与时钟脚本**已实现并全测**；`HeadlessChromiumRenderer` 是**骨架**——live CDP 渲染未实现（见下）。

---

## `MotionRenderer` trait

```rust
pub trait MotionRenderer {
    fn render(&self, req: &MotionRenderRequest) -> MotionResult<RenderedClip>;
}
```

- **契约要求确定性**：同一 `req` 必须每次产出**字节一致**的帧——这是"预览 == 导出"与内容寻址缓存（[cache.md](cache.md)）成立的基础。
- 请求假定已由调用方 `MotionRenderRequest::validate()` 校验过（[manifest-source.md](manifest-source.md)）；实现仍**自行**再应用它负责的沙箱检查（文档大小 / 网络），确保接线方无论是否有浏览器都能看到策略失败。

---

## `deterministic_clock_script()`

两个渲染器共享的注入 JS（返回 `&'static str`，便于 CDP 后端用 `Page.addScriptToEvaluateOnNewDocument` 在作者脚本前注入）：

1. 冻结页面时钟——把 `document.timeline.currentTime` 钉死在虚拟时间（`seconds * 1000` ms），暂停 CSS/Web 动画。
2. 暴露 `window.OpenTake.seek(seconds)`：宿主每帧调用一次（`t = frameIndex / fps`），确定性推进时间而非依赖墙钟。
3. 暴露 `OpenTake.onSeek(fn)`：作者注册逐帧回调。

脚本刻意保持极小、无依赖；`__installed` 守卫避免重复安装。纯函数，可单测（测试只断言它包含 `OpenTake` / `seek` / `currentTime` / `onSeek`）。

---

## `StubRenderer`（已实现）

确定性、**无浏览器**的渲染器，给测试与离线管线用。

- 每帧是一块纯色 RGBA 填充，颜色是 `(帧号, content-hash)` 的纯函数（`frame_color`：从 hash 前几字节 XOR/加帧号派生 RGB）——保证可复现、且不同请求不同。
- 透明时 alpha 沿 clip 线性渐变 `0..=255`（单帧 clip 不透明），让测试能断言 alpha 通道存活。
- 即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），让安全契约被测试覆盖。
- 流程：`req.validate()` → 大小检查 → `content_hash(req)` → `cache.ensure_dir` → 逐帧 `write_solid_rgba_png` 到 `frame_{i:05}.png` → 返回 `RenderedClip`。

### 自制 PNG 编码器（无依赖）
lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：
- `encode_solid_rgba_png`：构建 PNG 容器（签名 + IHDR(8bit/RGBA) + IDAT + IEND）。
- `zlib_store`：用 stored（type 0，未压缩）deflate 块 + Adler-32 包裹原始扫描线，能被任意标准 PNG 解码器还原。
- `write_chunk` / `Crc32`（表-free，PNG/zlib 多项式）/ `adler32`：手写校验和。

输出是有效（虽未压缩）的 PNG；dev-test 用真实 `image` crate 解码回来验证尺寸与 alpha。

---

## `HeadlessChromiumRenderer`（骨架，未实现）

真实后端的骨架，文档化并排序确定性 CDP 流程，但 live Chromium 调用 feature-gated。

**骨架已实现的纯辅助（可单测）：**
- `data_url_for_code(html_css_js)`：把内联文档百分号编码成 `data:text/html;charset=utf-8,…`（保留 alnum 与 `-_.~`，其余编码）。确定性时钟由引擎注入而非内联，作者代码无法观测/剥离。
- `frame_time_grid(req)`：返回 `[0/fps, 1/fps, …, (n-1)/fps]` 的虚拟时间戳网格，文档化并测试时间网格而不启动任何东西。
- `policy()` / `cache()` 访问器。

**`render()` 行为：**
- 总是先 `req.validate()` + 应用自己负责的沙箱文档大小检查，**即便最终走 "unavailable" 路径**——这样接线方无论浏览器在不在都能看到策略失败（有专门测试：超限文档先报 `Sandbox` 错）。
- `#[cfg(feature = "chromium")]`：当前仍 `Err(RendererUnavailable("…enabled but not yet implemented (Issue #34 native fallback TODO)"))`——开了 feature 也**不假装**渲染。
- `#[cfg(not(feature = "chromium"))]`（默认 / CI）：`Err(RendererUnavailable("…not compiled in; build with chromium feature, or use StubRenderer…"))`。

**计划中的真实流程（骨架文档记录的步骤，待实现）：**
1. 启动离屏 Chromium：无网络、空 profile、除所服务文档外无文件系统访问（应用 `SandboxPolicy`）。
2. `Emulation.setDeviceMetricsOverride` 设到请求宽高。
3. `Page.addScriptToEvaluateOnNewDocument` 注入 `deterministic_clock_script()`。
4. `Emulation.setVirtualTimePolicy { policy: "pause" }` 停真实时间。
5. 导航到文档（`Code` 用内联 `data:` URL，模板用其 served `entry`）。
6. 逐帧：推进虚拟时间到 `i/fps` + `OpenTake.seek(i/fps)` → `Page.captureScreenshot`（透明时透明背景）→ 写 `frame_iiiii.png`。
7. 返回 `RenderedClip`。

`TODO(#34)` 明确列出：定位/启动浏览器并对缺失给清晰错误、用 `Fetch.enable` + 请求拦截执行网络白名单、应用 CSP 与超时熔断、CDP 失败映射到 `RenderFailed`/`Timeout`。

---

## 移植铁律落地

- **确定性**：stub 帧色是纯函数；真实后端用虚拟时间 + 注入时钟冻结墙钟 → 同请求字节一致。
- **显式失败不假装**：后端不可用返回带可操作文案的 `RendererUnavailable`，绝不静默或伪造成功帧。
- **沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查。
- **无依赖 lib 面**：lib 代码自带 PNG 编码器，`image` 只在 dev-dep。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
