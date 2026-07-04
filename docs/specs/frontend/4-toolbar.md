# Toolbar（工具条）

> 来源:`Toolbar/ToolbarView.swift`(全文)。高度 `toolbarHeight=38`（`Constants.swift:41`），位于 timeline 面板内、时间线上方（`EditorView.swift:54-60`）。

布局:水平 `HStack(spacing: md=10)`,左对齐组 + `Spacer` + 右侧缩放组,水平 padding `md=10`（`ToolbarView.swift:8,63`）。

### 4.1 左侧按钮组（从左到右，组间用竖直 Divider，高 `Spacing.xl=20`，`ToolbarView.swift:15-16`）

1. **Undo / Redo**（`ToolbarView.swift:10-13`）：图标 `arrow.uturn.backward` / `arrow.uturn.forward`。help "Undo (⌘Z)" / "Redo (⇧⌘Z)"。动作 = 走 Tauri `undo`/`redo`（上游发 `undo:`/`redo:` selector）。
2. **工具模式**（`ToolbarView.swift:19-22`）：
   - Pointer：`cursorarrow`，help "Pointer (V)"，active 态 = `toolMode==.pointer`。
   - Razor：`scissors`，help "Razor (C)"。
   - active 按钮:图标 `text-primary` + `hoverHighlight(isActive:true)`;非 active:`text-tertiary`（`ToolbarView.swift:87-98`）。
3. **Split / Trim**（`ToolbarView.swift:28-32`）：
   - Split：`square.split.2x1`，help "Split at Playhead (⌘K)"，动作 `splitAtPlayhead`。
   - Trim Start：字形按钮 `[`（等宽 16px semibold），help "Trim Start to Playhead (Q)"，动作 `trimStartToPlayhead`。
   - Trim End：字形按钮 `]`，help "Trim End to Playhead (W)"，动作 `trimEndToPlayhead`。
4. **加内容**（`ToolbarView.swift:38-40`）：Text 字形按钮 `T`（serif，17px bold，`ToolbarView.swift:103`），help "Add Text"，动作 `addTextClip`。

### 4.2 按钮样式（三种，`ToolbarView.swift:67-122`）

- **图标按钮**：`Image(systemName).font(size: md=13).foregroundStyle(text-secondary).frame(24×24).hoverHighlight()`。
- **字形按钮 bracket**：`Text("[").font(16, semibold, monospaced).foregroundStyle(text-secondary).frame(24×24).hoverHighlight()`。
- **Text 字形**：`Text("T").font(17, bold, serif)...frame(24×24).hoverHighlight()`。
- 所有按钮 `buttonStyle(.plain)` + `.help(...)`（tooltip）。

### 4.3 右侧缩放滑块（`ToolbarView.swift:45-61`）

- 结构：`HStack(spacing: xs=4)`：`minus.magnifyingglass`(text-tertiary, fontSize sm=11) → Slider → `plus.magnifyingglass`。
- Slider：**对数映射**（滑块行程对每个缩放倍率均匀），`get: log(zoomScale)`，`set: zoomScale = exp(value)`（`ToolbarView.swift:50-53`）。range = `log(minZoomScale)...log(Zoom.max=40)`（`Constants.swift:84`）。`controlSize(.mini)`，`tint = accent.primary`，`width=100`（`ToolbarView.swift:54-57`）。
- `minZoomScale`:由 `EditorViewModel` 计算(适配时间线全长到可视宽,见 §5 zoom)。前端需复刻其计算或从 Rust/镜像取。

> 前端实现:自定义 `<input type=range>` 或自绘滑块,**用对数刻度**;tint 用 `--accent-primary`;两侧放大缩小图标。
