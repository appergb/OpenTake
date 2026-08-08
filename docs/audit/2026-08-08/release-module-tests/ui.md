# v1.0.0-beta.2 发布前 UI 一致性 / 可访问性独立测试

日期：2026-08-08（Asia/Shanghai）
范围：`LibraryView`、`MediaPanel` / `MediaTabBar`、`Preview`、`ExportDialog` 及其共享 UI token / primitive。
方式：只读产品代码与既有证据；独立运行 Web 测试和生产构建；目视复核正式安装包 PNG。未修改产品代码、测试或配置。

## 结论

**FAIL — 当前不能把前端 UI / 可访问性发布门禁判为通过。**

- 用户最初六条视觉/布局要求在现有 1224×768 正式截图和组件结构中均未发现阻断性不一致，视觉子结论为 **PASS（限定于已覆盖画面）**。
- 键盘、focus、tab、listbox、alert 的既定修复均有 E1/E2，Library 与 Export 焦点另有 E3；主导出按钮对比度为 **16.500:1**。
- 但独立静态复核确认 2 类发布阻断：
  1. MediaCard 的“提取音频”按钮仍是 `canExtractAudio && hovered` 才挂载，键盘用户没有等价路径。
  2. Export 错误 alert 与多个正常信息型 empty state 的小字号文本对比度低于 WCAG 2.2 AA 4.5:1。
- 自动化全绿没有覆盖上述两项，因此不能据此推导整体可访问性 PASS。

证据等级：E1 = 产品代码 / DOM 契约 / token 计算；E2 = 本次新鲜自动化执行；E3 = 2026-08-07 修复后正式安装包截图及其索引账本。

## 自动化门禁

| 检查 | 本次命令 | 结果 |
|---|---|---|
| UI focused | `pnpm exec vitest run src/components/media/LibraryView.test.tsx src/components/media/MediaPanel.test.tsx src/components/media/MediaTabBar.test.tsx src/components/preview/Preview.test.tsx src/components/preview/Preview.interaction.test.tsx src/components/shell/ExportDialog.test.ts src/components/shell/ExportDialog.interaction.test.tsx` | **PASS：7 文件，102/102** |
| 全 Web | `pnpm test` | **PASS：138 文件，1147/1147** |
| Web production build | `pnpm build` | **PASS：`tsc -b` + Vite，1703 modules transformed** |

构建仅有项目既有的 ineffective dynamic import 和单个 857.46 kB 大 chunk 提示；无 TypeScript 或 Vite 构建错误。这些提示不改变本模块 UI 功能判定。

## 六条原始要求逐项核对

| 原始要求 | 判定 | 可由代码 / DOM 证明（E1/E2） | 需要目视的结论（E3） |
|---|---|---|---|
| 1. 文字与按钮尺寸匹配 | **PASS（视觉）** | 正常控件集中使用 9–14px token；点击控件主要为 24/26/28/32px。Library CardAction 26px、Media/Preview 紧凑控件至少 24px、Export footer 28px；focused 测试覆盖 breadcrumb 24×24 和 scrub 24px。 | `03`、`12`、`13`、`17`、`19` 中字号、图标和控件比例一致，未见大按钮配微小孤立文案或按钮文字溢出。对比度失败另列，不用“尺寸匹配”掩盖。 |
| 2. 按钮居中或靠左 | **PASS** | 图标按钮统一 `inline-flex + alignItems/justifyContent:center`；菜单/分类操作文字左对齐；Export 次要/主操作按桌面对话框习惯右对齐。 | Export footer、Preview transport、Library 卡片操作在截图中均有明确对齐轴。 |
| 3. 描述尽量一行 | **PASS（已覆盖画面）** | 素材名、tab 和 breadcrumb 使用 `whiteSpace:nowrap` + ellipsis/横向滚动；Export timeline size 是独立短 hint；错误文本只在必要时 `wordBreak`。 | `12`/`13` 中“时间线尺寸”单行；`03`/`17` 中 tab、素材名和属性值未出现无意义换行。 |
| 4. 每功能描述独立一行且纵深紧凑 | **PASS** | Export `Row` 把 label/control 与 hint 分成相邻行，gap 4px；Media 提取空态把主说明与 hint 分开，gap 4px；上下文计数独立一行。 | Export 三个功能行和尺寸 hint 层级清楚，弹窗纵深紧凑。 |
| 5. 布局符合人类逻辑 | **PASS（已覆盖画面）** | Library = 分类树→搜索/排序→网格；Media = 主 tab→动作/副 tab→搜索→上下文→内容；Preview = tab→画布→scrub→transport；Export = header→字段→状态→footer。 | `03`/`17` 显示媒体→预览→检查器→时间线的编辑逻辑；`12`/`13` 的 Export 流程由上到下自然；`19` 的 Library 左分类、右内容清楚。 |
| 6. 整体设计统一 | **PASS（视觉）** | 四模块共享 `tokens.css` 的背景、文字、间距、圆角、24/28px 控件和全局 focus-visible；组件普遍复用 `HoverButton`、`Dropdown`、`PanelHeaderBar`。 | 五张代表图的深色层级、边框、圆角、焦点轮廓和信息密度一致。仅证明当前正式截图的 1224×768 画面，不外推缩放和所有状态。 |

## 可访问性核对

### 已证明通过

- **键盘与可见焦点（E1/E2/E3）**：`global.css` 为 native / explicit focusable controls 提供 1.5px accent outline；正式图 `13-export-keyboard-focus.png` 和 `19-library-keyboard-action-focus.png` 的焦点轮廓可辨。
- **Library（E1/E2/E3）**：三个卡片动作常驻 DOM/Tab 顺序，focus capture 后显现；测试执行导入、分类、取消收藏；正式图 `19` 证明零 hover 路径能到达卡片操作。
- **Media popup（E1/E2）**：Import/Sort/Filter trigger 有 `aria-haspopup`、`aria-expanded`、`aria-controls`；Arrow/Home/End 移动，Escape 和 Enter/Space 选择后恢复 trigger。
- **Media tabs（E1/E2）**：主/副标签有 `tablist/tab/tabpanel`、单一 tab stop、roving；禁用主 tab 会被跳过，受控 panel 节点始终存在。
- **Preview（E1/E2）**：tabs 与 panel 关联；scrub 是具 Name/Role/Value 的 24px slider，支持逐帧、Shift+5、Home/End；BadgeMenu trigger/listbox/option 关联并管理打开焦点、方向键和 Escape 返回。
- **Export dialog（E1/E2/E3）**：`aria-modal` dialog 把焦点移入并循环 Tab/Shift+Tab，关闭后恢复；失败消息具 `role="alert" + aria-live="assertive" + aria-atomic="true"`；正式图 `13` 显示主按钮 focus。
- **主 Export CTA 对比度（E1/E2/E3）**：`#111` / `rgb(245,239,228)` 的计算结果为 **16.500:1**，现有回归测试阈值 ≥4.5；正式图 `12`/`13` 目视清晰。

### 发布阻断缺陷

#### UI-A11Y-01 — 提取音频是 mouse-hover-only（HIGH / release blocker）

- `web/src/components/media/MediaPanel.tsx:2350-2375` 只在 `canExtractAudio && hovered` 时把按钮挂入 DOM。
- 卡片 keyboard Enter/Space 只执行选择/预览；卡片 context menu 只有删除；“提取”副 tab 仍复用同一个 `MediaCard`，没有独立的键盘动作。
- 因此从零 hover 开始，Tab、方向键、Enter、Space、ContextMenu/Shift+F10 都不能执行提取，违反 WCAG 2.2 SC 2.1.1 Keyboard。
- 当前 102 个 focused 测试没有“无 hover 可提取音频”的用例，自动化全绿属于覆盖缺口，不是反证。

#### UI-A11Y-02 — 正常信息 / 错误文本对比度不足（HIGH / release blocker）

- Export alert：`web/src/components/shell/ExportDialog.tsx:597-612` 使用 10px `--accent-danger`（实际 `--status-error = rgb(229,79,79)`）叠在 `rgba(255,107,107,0.08)` / `--bg-elevated rgb(44,44,44)` 上。按实际合成背景约 `rgb(61,49,49)` 计算为 **3.307:1**，低于正常文字要求 4.5:1。
- 信息型 empty state：`LibraryView.tsx:383-396`、`MediaPanel.tsx:973-995` 和 `Preview.tsx:515-530` 使用 10–12px `--text-muted`（34% white）。在 `--bg-surface rgb(22,22,22)` 上合成后约为 `rgb(101,101,101)`，对比度 **3.115:1**。这些是 loading/empty/no-match/no-media 信息，不是禁用控件或纯装饰，不能使用 disabled-content 例外。
- 现有测试只验证主 Export CTA ≥4.5，并只验证 alert 的 live-region 语义；没有覆盖 alert 或 empty-state 的前景/背景组合。

## 正式 PNG 目视证据

本次使用原始分辨率逐一查看，并用 `file` 复核下列文件均为 **1224×768、8-bit RGB、non-interlaced 的真实 PNG**：

- `editor-core-after-fix-assets/03-source-preview-fixed.png`：MediaPanel、source Preview、Inspector、timeline 的整体层级与紧凑对齐。
- `editor-core-after-fix-assets/12-export-dialog-contrast.png`：Export 居中、字段行、hint、footer 与主按钮视觉对比。
- `editor-core-after-fix-assets/13-export-keyboard-focus.png`：Export 主按钮可见 focus。
- `editor-core-after-fix-assets/17-relaunch-editor-thumbnails-composite.png`：完整退出重开后的双素材、合成预览与整体编辑布局。
- `editor-core-after-fix-assets/19-library-keyboard-action-focus.png`：Library 分类/搜索/排序/网格结构以及卡片操作键盘焦点。

这些图能证明所见状态的视觉一致性，但不能证明 DOM、Tab 顺序、accessibility tree、VoiceOver 宣告或 200%/400% 缩放；这些项目分别依赖 E1/E2 或仍属未覆盖范围。

## Test Coverage Analysis

### Current Coverage

- 7 个相关测试文件，102 个 focused tests；全 Web 138 文件、1147 tests，均 0 failure。
- 覆盖 Library card actions、Media search/tabs/popup/breadcrumb、Preview slider/tabs/listbox、Export modal/focus/alert/主按钮对比度。
- Coverage gaps identified：hover-only 提取音频；alert/empty-state 文字对比度；真实 Chromium accessibility tree；VoiceOver；200%/400% zoom/reflow；760×480 的本轮同构建视觉复验。

### Recommended Tests

1. **MediaCard exposes extract audio without hover** — 在 `hovered=false` 的真实 DOM 中证明键盘能发现并执行提取；该测试应在当前实现上失败。
2. **All normal informational text tokens meet WCAG AA** — 对 Export alert、Library/Media/Preview empty state 的实际 alpha-composited 前景/背景做 ≥4.5 回归。
3. **Real-browser AX smoke** — 在隔离 Chromium 中读取 search/tab/slider/listbox/dialog/alert 的 Name/Role/Value，并执行完整 Tab/Arrow/Escape 路径。
4. **Zoom/reflow matrix** — 中文/英文 × 760×480/1224×768 × 100%/200%/400%，检查 one-line/ellipsis、可滚动性、焦点不被裁切。

### Priority

- Critical：无本次证据支持的数据丢失或安全类 UI 问题。
- High：UI-A11Y-01、UI-A11Y-02；在修复并新增回归前，UI 发布门禁保持 FAIL。
- Medium：真实 Chromium AX tree、VoiceOver、200%/400% 与 760×480 同构建复验。
- Low：构建 chunk / dynamic-import 提示，不阻断本次 UI 一致性判断。

## Blocker 与限制

- **Release blocker：2 类（键盘路径 1 类、对比度 1 类）。** 没有外部访问或环境阻塞；缺陷可由当前代码直接复现/计算。
- 当前会话没有 Chrome DevTools MCP，未宣称完成真实浏览器 accessibility tree、computed-style、VoiceOver 或 zoom 测量。
- 2026-08-07 正式 PNG 仅覆盖 1224×768；发布规格和旧审计对 760×480 有既有结论，但本报告不把旧结论冒充本轮同构建复验。
