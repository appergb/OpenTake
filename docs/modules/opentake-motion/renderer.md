# renderer — 渲染契约与实现

> 上级：[模块目录 INDEX.md](INDEX.md) · [总览 OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
> 源码：[`../../../crates/opentake-motion/src/renderer.rs`](../../../crates/opentake-motion/src/renderer.rs)

---

## 职责

定义动效渲染的**单一契约** `MotionRenderer` trait，并提供两个实现 + 一个共享的确定性时钟脚本。给定一个已校验的 `MotionRenderRequest`，产出 `RenderedClip`（磁盘上的 RGBA 帧序列）。这是其余系统唯一依赖的渲染抽象。

> 完成状态：`StubRenderer`、确定性时钟与 feature-gated 的真实 `HeadlessChromiumRenderer` 均已实现并测试；桌面 app/core 接线仍属于后续工作。

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

1. 冻结页面时钟——把 `document.timeline.currentTime`、`Date.now()`、`performance.now()` 钉死在虚拟时间（`seconds * 1000` ms），并按帧重置确定性 `Math.random()`。
2. 暴露 `window.OpenTake.seek(seconds)`：宿主每帧调用一次（`t = frameIndex / fps`），确定性推进时间而非依赖墙钟。
3. 暴露 `OpenTake.onSeek(fn)`：作者注册逐帧回调。

脚本刻意保持极小、无依赖；`__installed` 守卫避免重复安装。纯函数，可单测（测试只断言它包含 `OpenTake` / `seek` / `currentTime` / `onSeek`）。

---

## `StubRenderer`（已实现）

确定性、**无浏览器**的渲染器，给测试与离线管线用。

- 每帧是一块纯色 RGBA 填充，颜色是 `(帧号, content-hash)` 的纯函数（`frame_color`：从 hash 前几字节 XOR/加帧号派生 RGB）——保证可复现、且不同请求不同。
- 透明时 alpha 沿 clip 线性渐变 `0..=255`（单帧 clip 不透明），让测试能断言 alpha 通道存活。
- 即便是 stub 也执行沙箱**文档大小检查**（`SandboxPolicy::default().check_document_size`），且在创建内容哈希目录之前拒绝超限输入；精确拥有者与 Chromium 路径共同断言零缓存副作用。
- 流程：`req.validate()` → 大小检查 → `content_hash(req)` → `cache.ensure_dir` → 逐帧 `write_solid_rgba_png` 到 `frame_{i:05}.png` → 返回 `RenderedClip`。

### 自制 PNG 编码器（无依赖）
lib 代码里不引 `image` 依赖，而用一个微型**无依赖** RGBA PNG 编码器，使 stub 在测试外也可用：
- `encode_solid_rgba_png`：构建 PNG 容器（签名 + IHDR(8bit/RGBA) + IDAT + IEND）。
- `zlib_store`：用 stored（type 0，未压缩）deflate 块 + Adler-32 包裹原始扫描线，能被任意标准 PNG 解码器还原。
- `write_chunk` / `Crc32`（表-free，PNG/zlib 多项式）/ `adler32`：手写校验和。

输出是有效（虽未压缩）的 PNG；dev-test 用真实 `image` crate 解码回来验证尺寸、alpha、精确 RGBA，并覆盖原始扫描线超过 65,535 bytes、必须拆成多个 stored-deflate block 的边界。相同输入直接编码两次及跨两个独立缓存渲染均逐字节一致。`image` 仍只在本 crate 的 `dev-dependencies`，不进入 production 直接依赖。

---

## `HeadlessChromiumRenderer`（真实 CDP 后端）

真实后端 feature-gated 于 `chromium`；默认 build 不带 WebSocket/截图解码依赖，也不需要本机浏览器。

**纯辅助（可单测）：**
- `data_url_for_code(html_css_js)`：把内联文档百分号编码成 `data:text/html;charset=utf-8,…`（保留 alnum 与 `-_.~`，其余编码）。确定性时钟由引擎注入而非内联，作者代码无法观测/剥离。
- `frame_time_grid(req)`：返回 `[0/fps, 1/fps, …, (n-1)/fps]` 的虚拟时间戳网格，文档化并测试时间网格而不启动任何东西。
- `policy()` / `cache()` 访问器。

**`render()` 行为：**
- 总是先 `req.validate()` + 应用自己负责的沙箱文档大小检查，**即便最终走 "unavailable" 路径**——这样接线方无论浏览器在不在都能看到策略失败（有专门测试：超限文档先报 `Sandbox` 错）。
- `#[cfg(feature = "chromium")]`：显式 path → `OPENTAKE_CHROMIUM_PATH` → 平台安装位置 → `PATH` 依次定位 Chrome / Chromium / Edge；未找到时返回可操作的 `RendererUnavailable`。
- 每次 render 使用唯一空 profile，启动时关闭浏览器后台网络/扩展/同步/文件系统访问 API；页面只导航到内联 `data:` 文档。
- 在作者文档最前注入严格 CSP，同时用 CDP `Fetch.enable` 拦截每个请求：`data:` 与精确匹配白名单 origin 才放行，CSP 拒绝也从 `Log.entryAdded` 作为 `Sandbox` 失败返回。重定向会再次被拦截。
- `Page.addScriptToEvaluateOnNewDocument` 在作者脚本前安装确定性时钟；文档同步加载后暂停 Chromium 虚拟时间，逐帧 `OpenTake.seek(i/fps)` + `Page.captureScreenshot`。
- 全程共享一个 deadline 与 `MotionCancellationToken`；超时、运行中取消、tab/browser crash、CDP/PNG 错误均 fail-closed，终止子进程、删除 profile 与 partial 帧后才返回。
- 完整内容寻址缓存直接复用；partial cache 在重渲染前清理。
- `#[cfg(not(feature = "chromium"))]`（默认 / CI）：返回 `RendererUnavailable("…not compiled in…")`。

`MotionSource::Template` 必须先由调用层解析/绑定为内联 `Code`；直接交给浏览器后端会返回 `UnknownTemplate`，不会猜测模板文件或授予文件系统路径。

---

## 移植铁律落地

- **确定性**：stub 帧色是纯函数；真实后端用虚拟时间 + 注入时钟冻结墙钟 → 同请求字节一致。
- **显式失败不假装**：后端不可用返回带可操作文案的 `RendererUnavailable`，绝不静默或伪造成功帧。
- **沙箱不可绕过**：连 stub 与 "unavailable" 路径都先跑文档大小检查；live 路径用 CSP + CDP 请求拦截双层执行。
- **默认依赖面精简**：lib 代码自带 stub PNG 编码器，`image` 只在 dev-dep；`tungstenite` / `base64` 仅随 `chromium` feature 启用。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
