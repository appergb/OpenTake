# 文本栅格化 — CosmicTextRasterizer（对应上游 CATextLayer）

> 上级：本模块目录 [INDEX.md](INDEX.md)

## 职责

把每个文字 clip 排版并栅格化成一张**预乘 RGBA 纹理**，让文字像任何视频/图片层一样参与合成（逐帧 opacity 走 `LayerDraw.opacity = clip.opacity_at(f)`，与视频同路径）。这取代上游的文字方案：上游预览用长期存活的 `CATextLayer` 树逐帧改 opacity、导出用一次性 `CAKeyframeAnimation`（discrete）+ `AVVideoCompositionCoreAnimationTool` 把文字烤进视频（`TextLayerController`）。OpenTake 合成器原生吃纹理，故不需要任何「烧文字」中间步骤。

纹理覆盖文字 clip 的**框**（而非整张画布）：plan 把文字层 `nat_size` 设为框的像素尺寸，于是既有的 `affine_transform` 把框纹理 1:1 贴到画布上 clip 的 transform 处（位置/旋转/翻转/opacity 全由合成器处理，与视频/图片完全同构）。

## 关键类型与算法

### trait 边界（`gpu/text_raster.rs`）
- `TextRasterRequest<'a>`：`clip_id` / `content` / `style: &TextStyle` / `box_norm`（归一化文字框 0–1）/ `canvas`（画布像素）。
- `TextRasterizer::rasterize(req) -> Option<DecodedFrame>`：栅格化，文字栈不可用（如 headless 无字体）或请求退化时返回 `None`（**永不 `todo!()`/`unimplemented!()`**）。
- `NullTextRasterizer`：占位后端，恒返回 `None`，让管线能编译、能路由文字 clip、端到端跑通而不触 panic。

### cosmic-text 后端（`gpu/text_engine.rs`）
`CosmicTextRasterizer` 持 `FontSystem`（启动扫描系统字体一次，~数十 ms，应复用）+ `SwashCache`，因 layout/raster 需可变而置于 `RefCell` 以保 trait 的 `&self`。`has_fonts()` 供调用方/测试判断有无可用字面。

`rasterize_box` 流程（对拍 `TextLayerController.applyStyle` L152 + `TextLayout`）：
1. 空内容 / 退化框（`box_pixels` 返回 `None`）→ `None`；框尺寸钳到 `MAX_BOX_SIDE=8192`。
2. **画布相对字号**（上游基准）：`font_px = font_size * font_scale * (canvas.h / 1080)`，下限 1px；行高 `1.2×`。`1080` = `CANVAS_BASIS_HEIGHT`（上游 referenceCanvasHeight）。
3. cosmic-text 排版：`Buffer` 设框尺寸 + `set_text`（`attrs_for` 从 PostScript 名如 `Helvetica-Bold` 切出 family + 推断 bold）+ 行对齐（`to_align`）+ `shape_until_scroll`。
4. **覆盖掩码**：`buffer.draw` 用白色，按 max 合并出每像素 0..255 coverage（重叠 glyph cell 取最强）。
5. **合成进预乘 RGBA**（底到顶）：背景盒（`background.enabled` 填色）→ 投影（box-blur 掩码，按画布缩放算 `radius`/`offset`，**Y 上→图像行 Y 下故 offset_y 取负**）→ 文字（掩码 × `style.color`）→ 描边（`border` 2px 周边）。`over` 做直通源 alpha-over 到预乘缓冲。
6. 返回框尺寸的预乘 `DecodedFrame`。

## 源文件
- [`crates/opentake-render/src/gpu/text_raster.rs`](../../../crates/opentake-render/src/gpu/text_raster.rs) — `TextRasterizer` trait + `TextRasterRequest` + `NullTextRasterizer`。
- [`crates/opentake-render/src/gpu/text_engine.rs`](../../../crates/opentake-render/src/gpu/text_engine.rs) — `CosmicTextRasterizer` + 排版/掩码/投影/描边/box-blur。

## 不变量
- **画布相对缩放基准 = 1080**：字号、投影 offset/blur 一律乘 `canvas.h / 1080`，与上游一致，否则文字框尺寸/位置漂移（MODULE-PORT-MAP 文字度量条目）。
- **文字层 `nat_size` = 框像素尺寸、`preferred_transform` = 单位阵**（由 plan 侧 `make_clip_plan` 文字分支保证），使框纹理经标准 affine 1:1 落位。
- **文字不做同轨去重**：每个可见文字 clip 各自一张纹理、各自一个 `LayerDraw`，且整体叠在所有视频之上（plan 的 `text_plans`，对齐上游 CoreAnimationTool 文字在视频合成之上）。
- **输出预乘**：`DecodedFrame.premultiplied = true`，合成器对其 `needs_premultiply=false`。
- **无字体不崩**：headless 无字面时仍产出框尺寸帧（背景/描边照画），仅 glyph 像素缺失。

## 关系
- 输入 `TextStyle` / `Rgba` / `TextAlignment` 来自 [opentake-domain](../opentake-domain/INDEX.md)；逐帧 opacity 由 [render-plan.md](render-plan.md) 的 `opacity_at` 提供。
- 输出预乘 RGBA 纹理给 [gpu-compositor.md](gpu-compositor.md) 合成（文字层 `TextureSource::Text`，经 `TextureResolver` 接入）。

## 计划中
- 完整样式深化（更精确的换行/字体回退/阴影 padding 12×2 余量对齐、描边宽度按缩放）属 ROADMAP Phase 8 文字渲染收口；当前已覆盖 family+weight / 画布相对字号 / 颜色 / 水平对齐 / 背景盒 / 投影（offset+box-blur）/ 描边。
- 文字静态渲染像素对拍上游（CoreText vs cosmic-text 边缘 Δ 不可避免，验收对文字区放宽到结构一致 SSIM，几何/字号/对齐需准）见 SPEC §6.2。

---

> 上级：本模块目录 [INDEX.md](INDEX.md)
