# 实施清单与 1:1 验收方式

### 13.1 落地顺序（建议）

1. **设计令牌**:把 §1 全部写成 `web/src/styles/tokens.css`(CSS 变量)。**这是地基**,先做,后续组件只引用变量。配 `lib/theme.ts` 导出数值常量(给 canvas 绘制用,canvas 不能用 CSS 变量)。
2. **PanelShell + EditorSplit + 三 preset 布局**（§2）：先把五面板框架 + surface 卡片 + 沟槽 + 焦点环 + 三种布局的初始分割位搭好。用真实 Rust 镜像或 mock Timeline 填充。
3. **Zustand store**（§10）：建 `useProjectStore`(镜像) + `useEditorUiStore`(UI 态) + 派生选择器。接 `timeline_changed`/`get_timeline`。
4. **Toolbar**（§4）：相对简单，先做，验证令牌与按钮样式。
5. **TimelineContainer + 几何 + 刻度 + 轨道头 + clip 渲染**（§5.1-5.5）：Canvas 2D 绘制，**先把静态渲染对拍像素**（给定 timeline/zoom，clip/刻度/轨道位置与上游一致）。
6. **Timeline 手势**（§5.8 + §9）：playhead/scrub → 选择 → 拖移/落轨 → trim → 缩放/平移 → 磁吸 → razor/split → 关键帧/淡变/音量 → 右键菜单 → 拖放。逐组实现 + 逐条对拍。
7. **Inspector**（§6）：四态 + tab + ScrubbableNumberField + 关键帧面板。
8. **Preview**（§8）：tab + 画布(接 `preview_frame`) + transport + scrub + 设置 + Transform/Crop overlay。
9. **MediaPanel**（§7）：标签轨 + Media/Captions/Music tab + 网格 + 拖拽/选择/右键。
10. **键盘快捷键全表**（§9.6）+ 菜单（§2.9）。
11. **全面板截图对拍 + 行为对拍**。

### 13.2 1:1 验收清单（逐项打勾）

**A. 视觉（截图对拍，1600×1000）**
- [ ] 五面板布局(default/media/vertical)的分栏比例、沟槽 5px、surface 圆角 6px、焦点环与上游一致。
- [ ] 所有间距/字号/圆角/颜色取自令牌且与 §1 数值一致（抽查每面板 ≥5 处）。
- [ ] Toolbar 按钮图标/分隔/缩放滑块(对数)外观一致。
- [ ] 时间线:刻度间隔/次刻度/标签、轨道头(色条/标签/三图标/区分隔粗线)、clip(底色/左色条/标签/trim 手柄/波形/缩略图/音量橡皮筋/淡变/关键帧菱形)、playhead(红线+下三角)、磁吸黄虚线、marquee 白虚线、新轨黄线、razor 橙虚线 —— 逐项与上游同。
- [ ] Inspector 四态、tab 下划线、AI Edit 渐变、各字段、关键帧泳道一致。
- [ ] MediaPanel 标签轨(选中竖 capsule + hover 标签)、网格、面包屑一致。
- [ ] Preview tab(下划线色)、transport 按钮、scrub(hover 变粗)、设置 badge、overlay 一致。

**B. 行为（逐条对拍 §9）**
- [ ] §9.1 选择全 8 条。
- [ ] §9.2 拖放全 7 条（含落回原位不发命令、跨轨类型钳制、linked 跟随）。
- [ ] §9.3 trim 全 7 条（含最小 1 帧、image 自由延长、linked 传播）。
- [ ] §9.4 razor/split/关键帧/淡变/音量全 8 条。
- [ ] §9.5 playhead/scrub/缩放全 9 条（含光标锚定缩放、灵敏度常量）。
- [ ] §9.6 快捷键全表逐键。
- [ ] §9.7 hover/游标全表。
- [ ] §9.8 右键菜单项与分组逐字（§5.10）。

**C. 几何（单测）**
- [ ] geometry 纯函数（clipRect/frameAt/xForFrame/trackY/dropTargetAt/insertionLineY）对给定输入与上游公式输出一致。
- [ ] 刻度间隔/次刻度选择算法输出一致。
- [ ] 磁吸 findSnap（含黏滞 1.5×、playhead 1.5× 阈值）行为一致。

**D. 状态/契约**
- [ ] 镜像只由 `timeline_changed`→`get_timeline` 更新；前端无任何直接改 timeline 的路径。
- [ ] 每个编辑手势映射到正确 `edit_apply` 命令（§11.1）。
- [ ] 持久化键（layoutPreset/三面板可见性/keyframes）跨会话保留。

### 13.3 关键陷阱备忘（防止 1:1 偏差）

1. **帧↔秒用截断**（`Int(s*fps)`），非四舍五入（ARCHITECTURE §4、MODULE-PORT-MAP）。
2. **浮点 round 用「四舍五入 .5 远离 0」**（= Rust `f64::round`），如 `sourceFramesConsumed`、缩放后关键帧位（`Timeline.swift:122,287`）。
3. **clip rect 上下各留 2px**（`y+2, height-4`）—— 别漏（`TimelineGeometry.swift:64-67`）。
4. **第一轨前有 60px 顶部 drop zone**（在刻度 24 之下）（`TimelineGeometry.swift:41`）。
5. **trim 手柄 4px、磁吸 8px、拖拽阈值 3px、轨道 resize 区 6px、insert 阈值 10px** —— 逐个照搬常量。
6. **磁吸黏滞 1.5×、playhead 优先 1.5×** —— 别用单一阈值（`SnapEngine.swift:67,82-83`）。
7. **缩放对数滑块 + 光标/playhead 锚定** —— scrub 滑块行程要均匀（`ToolbarView.swift:50-53`）。
8. **selection linked 默认开，Option 为单次关闭**（`TimelineInputController.swift:86`）—— 选择/移动/trim/marquee 全受此影响。
9. **volume 关键帧画在橡皮筋上，其他关键帧画在 clip 底部**（`ClipRenderer.swift:162-169`）。
10. **clip 标签 = 名字 + 双空格 + 时长时间码，linked 名字加下划线**（`ClipRenderer.swift:598-609`）。
11. **playhead/snap/刻度是「相对可视区」叠加**（按 scrollLeft 偏移），不是画在 document 全宽（`PlayheadOverlay.swift:51`、`TimelineRuler` 绘制矩形用 scrollOffset）。
12. **轨道 displayHeight 不持久化**（开工程重置 50）—— 当 UI 态。
13. **面板点击副作用**:点 media 清 clip 选择,点 timeline 清资产选择（`EditorWindowController.swift:188-189`）。
14. **文本输入聚焦时全局快捷键失效**（`EditorWindowController.swift:39-41`）。
15. **SF Symbols 必须换等价 SVG 图标**（§3.3），颜色用 currentColor + 上游 foregroundStyle。
16. **无触觉反馈**（上游磁吸触发 `NSHapticFeedbackManager`，`SnapEngine.swift:93`）—— 前端忽略，不要因此漏掉磁吸逻辑本身。

---

## 附:本规格的源码证据索引(便于核对)

| 主题 | 上游文件 |
|---|---|
| 设计令牌 | `UI/AppTheme.swift` |
| 布局常量 | `Utilities/Constants.swift` |
| 五面板/preset/shell/焦点环 | `Editor/EditorView.swift` |
| 窗口/快捷键/聚焦/菜单动作 | `Editor/EditorWindowController.swift` |
| 主菜单/快捷键定义 | `App/MainMenu.swift` |
| 标题栏 | `Editor/TitleBarView.swift` |
| 工具条 | `Toolbar/ToolbarView.swift` |
| 时间线容器/滚动 | `Timeline/TimelineContainerView.swift` |
| 时间线几何 | `Timeline/TimelineGeometry.swift` |
| 时间线绘制/叠加/右键/拖放 | `Timeline/TimelineView.swift` |
| 时间线手势/缩放/命中 | `Timeline/TimelineInputController.swift` |
| clip 渲染 | `Timeline/ClipRenderer.swift` |
| 刻度 | `Timeline/TimelineRuler.swift` |
| playhead | `Timeline/PlayheadOverlay.swift` |
| 磁吸 | `Timeline/SnapEngine.swift` + `SnapIndicatorOverlay.swift` |
| 拖拽状态 | `Timeline/DragState.swift` |
| 轨道头 | `Timeline/TimelineHeaderView.swift` |
| Inspector | `Inspector/InspectorView.swift` + `Components/` + `Keyframes/KeyframesLane.swift` |
| 可拖拽数字字段 | `Inspector/Components/ScrubbableNumberField.swift` |
| Media 面板 | `MediaPanel/MediaPanelView.swift` + `MediaTab/` |
| Captions | `MediaPanel/CaptionsTab/CaptionTab.swift` |
| Preview | `Preview/PreviewContainerView.swift` + `PreviewTab.swift` + overlays |
| 数据模型 | `Models/Timeline.swift` / `Keyframe.swift` / `ClipType.swift` |
| 小组件 | `UI/HoverHighlight.swift` / `CapsuleButton.swift` |
| 目标架构(Tauri/React/Zustand/命令-事件) | `OpenTake/docs/ARCHITECTURE.md` |
