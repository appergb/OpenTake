# 资源加载与前端 UI 静态验证

审计时间：2026-08-08（Asia/Shanghai）
工作树：`OpenTake-generation`
分支 / HEAD：`audit/remediation-2026-08-05` / `f9398a9302e57690caa3ea21ff995fdd0ce97706`
审计方式：独立子 Agent，只读产品源码、测试和既有证据；未修改产品源码、配置或测试。

## 1. 判定口径

- **PASS**：检查范围内未发现阻断缺陷，且证据能直接支持结论。
- **PARTIAL**：主路径成立，但存在明确缺口、退化路径或未完成的动态验证。
- **FAIL**：存在可由代码或测量直接证明的缺陷，已不满足检查项。
- **BLOCKED**：当前证据不足以判断真实 GUI/运行时结果，不能用单元测试替代。
- **E1**：产品代码、配置、依赖实现的静态证据。
- **E2**：本次独立运行的可重复自动化测试。
- **E3\***：既有打包 GUI 截图；只有画面与文件时间，没有本 Agent 的命令/网络/进程账本，因此仅作为观察线索，不等同于可重复 E3。
- **E4**：真实输出解析、跨实现比对或带账本的资源峰值测量。本子任务没有 E4。

本次逐行读取了 `editor-core-validation-evidence-b/README.md` 全部 123 行和 `command-ledger.md` 全部 48 行，并重新核对其关键代码路径和高信号测试；下述结论不是转述摘要。

## 2. 总结

| 范围 | 判定 | 证据 | 结论 |
|---|---|---:|---|
| 项目打开媒体授权 | PARTIAL | E1 + E2 | `f9398a9` 已在打开成功后逐个授权当前项目引用的、已物化普通文件；不扩目录。旧项目的精确文件授权随 persisted-scope 跨项目累积，最小权限生命周期未闭环。 |
| 项目素材缩略图/选中预览 | PARTIAL / BLOCKED（打包 GUI） | E1 + E2 + E3\* | URL、scope、Range、近视口调度的代码契约成立；既有截图仍出现两个导入视频缩略图空白，缺少同一构建的 asset 请求/日志，不能确认真实显示已恢复。 |
| 资源是否过度加载 | **FAIL** | E1 + E2 | 视频/图片缩略图、prewarm 和 asset 读取有界；但全部已挂载音频卡会直接并发 `getWaveform`，搜索命中与快速预览也绕过共享调度/后端取消。 |
| Home UI | PASS（静态）/ PARTIAL（动态） | E1 + E2 + E3\* | 字号、按钮尺寸、左/中对齐、一行文案、信息层级在 1224×768 截图和组件中一致；未做缩放/中文长文案/屏幕阅读器实测。 |
| Library UI | **FAIL（键盘可用性）** | E1 + E2 | 网格和筛选布局清楚，但卡片三个主操作仅在鼠标 hover 后挂载，卡片本身不可聚焦，键盘无法到达。普通收藏项也没有可显示的持久缩略图。 |
| Editor / Import UI | PARTIAL | E1 + E2 + E3\* | 主布局、面板最小宽度、网格 roving、24 px 工具按钮基本成立；搜索框缺显式可访问名称，多个弹出菜单缺完整的打开状态和焦点/方向键模式。 |
| Preview UI | PARTIAL | E1 + E2 + E3\* | 播放控制对齐、slider 语义和 24 px 命中成立；预览页签缺完整 tab/panel 语义，部分 listbox 键盘模式不完整。 |
| Export UI | **FAIL（对比度）** | E1 + E2 | 对话框、焦点圈闭、字段逐行和右对齐按钮结构良好；主导出按钮是白字配近白背景，静态计算对比度仅 **1.144:1**。 |
| UI 六条设计原则整体 | **FAIL（存在高优先级缺陷）** | E1 + E2 + E3\* | 布局统一性总体良好，但 Export 文本不可读和 Library 操作键盘不可达使整体不能判 PASS。 |

## 3. 资源加载检查

### 3.1 项目打开授权：功能成立，权限生命周期不完整

判定：**PARTIAL（E1 + E2）**。

证据链：

1. `src-tauri/src/commands.rs:415-463` 和 `:512-537`：有/无 playback feature 的 `project_open` 都只在项目提交成功后调用 `grant_project_media_asset_scope`。
2. `src-tauri/src/media.rs:4421-4446`：遍历当前 manifest，经 `resolve_source_path` 后，仅对 `is_materialized_regular_file` 为真的路径调用 `allow_file`；没有 `allow_directory`。
3. `src-tauri/src/media.rs:7480-7525` 的测试覆盖“已存在外部文件允许、缺失外部文件不允许”；本次 `cargo test -p opentake-tauri asset_scope --lib -- --nocapture` 为 **3 passed / 0 failed**。
4. `web/src/lib/asset.ts:13-23`：前端只在 Tauri 环境用 `convertFileSrc(path, "opentake-asset")` 生成资源 URL。
5. `src-tauri/src/safe_asset_protocol.rs:151-169`：请求仍需当前项目 authority 或运行时 scope 允许；`tauri.conf.json:38-75` 的 CSP/asset scope 与协议一致。

缺口：

- `src-tauri/src/lib.rs:122-128` 启用了 `tauri-plugin-persisted-scope`。项目媒体只有 grant；项目 A→B 切换/关闭没有替换 active media set 或撤销 A 媒体的逻辑。现有 `forbid_file` 仅覆盖代理文件删除路径（`media.rs:4449-4455`）。因此“不扩目录”成立，但历史精确文件权限会累积。
- 测试未覆盖“被授权文件的 sibling 仍 403”、A→B 后 A 文件拒绝、重启后 active-set 不泄漏。

### 3.2 缩略图、预览和 asset 读取

判定：**PARTIAL（E1 + E2）/ BLOCKED（可重复打包 GUI）**。

有界主路径：

- `web/src/components/media/MediaPanel.tsx:87-101,221-265`：项目素材缩略图共享并发为 4，同键 in-flight Promise 合并，已解析路径用 256 项 LRU；但 pending 数组没有容量上限。
- `MediaPanel.tsx:1785-1835`：视频/图片缩略图与 video poster prewarm 由 `IntersectionObserver` 在 160 px 近视口触发；无 `IntersectionObserver` 时缩略图退化为立即排队。
- `src-tauri/src/media/prewarm.rs:25-28,112-143,212-290`：主队列 64 / 3 workers，低优先级队列 8 / 1 worker；过载 Busy，同键 reservation，项目 epoch 变更取消。
- `src-tauri/src/safe_asset_protocol.rs:38-48,77-131,387-461`：4 个并发读取、32 个 pending、5 秒期限；普通 body 32 MiB、图片 128 MiB、Range 单次约 1000 KiB，过载返回 503/504。
- `web/src/components/preview/Preview.tsx:643-668,704-724`：预览媒体经 asset URL，视频 `preload="metadata"` 且使用 Range；poster 只在视频选中时请求。

独立自动化复核：prewarm **9/9**、safe asset **12/12**、asset scope **3/3** 均通过；这些测试证明调度和协议契约，不证明真实 WebView 中图片成功绘制。

既有 GUI 观察：

- `editor-core-validation-assets/03-import-thumbnails-failed.png`（1224×768，文件时间 2026-08-07 19:02:56 CST）显示两个 4K60 视频卡片的缩略图区域为空/失败。
- 该截图没有与之绑定的 HEAD、asset URL、HTTP 状态、Tauri stderr 和缓存文件账本，不能归因于授权、CSP、解码、旧应用实例或陈旧缓存；也不能被 E2 测试“覆盖掉”。所以真实打包 GUI 结论仍为 BLOCKED，需要第 7 节复核。

### 3.3 明确的过度加载风险

#### RU-01 — HIGH / P1：音频卡波形无界 fan-out

判定：**FAIL（E1；波形算法 E2）**。

- `MediaPanel.tsx:893-997` 的列表/网格都直接 `items.map`，没有分页或虚拟化。
- `MediaPanel.tsx:1974-1989` 为每张音频卡挂载 `AudioWaveform`。
- `MediaPanel.tsx:2328-2350` 在组件挂载时立即 `getWaveform(mediaRef)`；cleanup 只阻止 `setState`，不取消后端工作。
- `src-tauri/src/media.rs:3601-3628` 普通 `get_waveform` 没有全局 worker/pending 上限或同键 single-flight。多个冷缓存卡片可各自启动 FFmpeg PCM 解码。
- `TimelineContainer.tsx:1044-1098` 的时间线波形另有 prewarm admission 与 in-flight Set；问题集中在素材卡直接路径。
- 本次 waveform 算法测试 **21/21** 通过，只证明缓存/采样算法，不证明跨卡片并发有界。

#### RU-02 — MEDIUM / P2：搜索和快速预览绕过共享调度

判定：**PARTIAL / 风险成立（E1 + 局部 E2）**。

- `web/src/components/media/MediaSearch.tsx:754-774`：每个搜索命中直接 `generateThumbnail`，卸载只忽略前端结果；Moments/Spoken 各最多 20，突发数量有限但绕过缩略图并发 4。
- `Preview.tsx:648-668`：快速切换视频时旧 `previewPoster` 仅被 React 忽略，后端 1920×1080 解码没有 latest-wins 取消；近视口 prewarm 可能同时请求同一冷目标。
- 相关 jsdom 测试通过，但没有断言 FFmpeg 峰值、旧工作退出或同键后端 single-flight。

#### RU-03 — MEDIUM / P2：网格 DOM 与 pending 队列随目录规模增长

判定：**PARTIAL（E1）**。

- 项目素材和 Library 都直接对全部条目 `map`，没有虚拟化/分页；大目录 DOM 数量线性增长。
- 项目视频/图片解码由近视口 + 并发 4 限制，Library 媒体挂载由 200 px `IntersectionObserver` 限制，所以“全量 DOM”不等同于“全量解码”。
- 项目缩略图 pending 数组本身无硬上限；快速滚动可积累大量尚未开始的闭包，离开视口只会阻止组件写状态，不会从队列删除。

### 3.4 全局 Library 资源可见性

判定：**PARTIAL（E1 + E2）**。

- `web/src/components/media/LibraryView.tsx:424-457`：卡片在 200 px 近视口才设置 visible。
- `LibraryView.tsx:558-561`：`libraryEntryPreviewSource` 只接受 `data:` / `blob:` thumb，拒绝 source/storedPath 充当预览 authority。
- `src-tauri/src/library.rs:90-127`：DTO 明确把 `stored_path` 置为 `None`，避免 ambient library path 暴露给 WebView。
- `src-tauri/src/media.rs:2554-2567`：从项目收藏到全局库时 `thumb: None`。普通收藏项因此只显示类型图标，安全边界清楚，但“库缩略图/快速识别”产品能力没有完成。
- `LibraryView.test.tsx` 覆盖 data/blob 接受和普通 source/storedPath 拒绝；这验证了安全契约，不验证用户期望的持久缩略图。

## 4. UI 六条规则逐项检查

| 用户规则 | 判定 | 证据 | 静态结论 |
|---|---|---:|---|
| 设计统一原则 | PARTIAL | E1 + E2 + E3\* | `tokens.css:79-91,151-159` 统一字号、工具栏/面板尺寸；Home/Editor 截图风格一致。Export 主按钮对比度和 Library 交互例外破坏整体一致性。 |
| 文字大小与按钮大小匹配 | PARTIAL | E1 + E3\* | 主操作为 11–13 px 字配 24–34 px 高按钮，匹配紧凑桌面编辑器；但 8/9 px micro/xxs 用于状态/徽章，可读性需缩放实测，Export 白字不可读。 |
| 按钮居中或靠左 | PASS（静态） | E1 + E3\* | 图标按钮均 flex 居中；菜单/分类/项目行左对齐；Home 主 CTA 居中或随 hero 左对齐；Export footer 明确右对齐成组，符合对话框惯例而非随机漂移。 |
| 文字描述尽量一行 | PASS（主要路径） | E1 + E3\* | Home 项目名、Library 名称、媒体 tab/卡片和预览文件名普遍 `nowrap + ellipsis`；错误、缺失清单允许换行，属于必要例外。 |
| 每功能描述独占一行、前后紧凑 | PASS（静态） | E1 + E3\* | Export 的 label/control/hint 为逐行结构；空状态主次信息分行；菜单每项一行，高度 28 px，纵深紧凑。 |
| 布局符合人类逻辑、易用清晰 | PARTIAL / FAIL（键盘路径） | E1 + E2 + E3\* | 视觉信息架构清楚，Editor 具面板最小宽度和 760 px 响应折叠；但 Library 核心操作键盘不可达、popup 焦点模式不完整，不能给整体 PASS。 |

### 4.1 Home

判定：**PASS（静态）/ PARTIAL（动态，E1 + E2 + E3\*）**。

- `HomeView.tsx:189-236,495-591`：204 px 导航侧栏、32 px 行高、12 px 字、左对齐。
- `HomeView.tsx:593-728`：空状态标题 28 px、描述 12 px、34 px 主按钮居中；有项目时 `:730-905` 转为左对齐 hero 与规则网格。
- `HomeView.tsx:1005-1170`：项目卡是语义 button，名称单行省略；more/remove 为 26×26，菜单项最小 28 px。
- `01-home-current-build.png`（1224×768）可见层级清楚、文案短、CTA 与内容关系明确。
- 限制：未验证 200%/400% 缩放、中文最长翻译、VoiceOver 导航顺序。

### 4.2 Library

判定：**FAIL（核心操作键盘不可达，E1 + E2）**。

- `LibraryView.tsx:121-370`：返回按钮 26×26，分类行 32 px，搜索/排序 28 px，文本左对齐且省略，视觉结构通过。
- `LibraryView.tsx:424-482`：卡片外层是不可聚焦 `<div>`，只监听 `onMouseEnter/onMouseLeave`。
- `LibraryView.tsx:515-540`：导入、分类、取消收藏三个按钮仅在 `hovered` 为真时挂载。键盘用户无法先使容器 hover，也无法 Tab 到尚未存在的按钮。
- 按钮一旦挂载有可访问名称和 26×26 命中，但这不能修复入口不可达。
- Library 全量 `entries.map`，视觉资源懒加载但 DOM 不虚拟化；大库需要 E4 性能复核。

### 4.3 Editor / Import

判定：**PARTIAL（E1 + E2 + E3\*）**。

- `EditorSplit.tsx:18-55,89-193,202-329`：面板最小宽度、默认/Media/Vertical 布局、窄宽 Agent 折叠与焦点交接有明确逻辑；`02-editor-empty.png` 在 1224×768 显示 Media/Preview/Inspector/Timeline 层级清楚。
- `MediaTabBar.tsx:43-113,135-185`：tablist/tab/aria-selected、24 px 最小高度、单行文字成立；缺 `aria-controls`/tabpanel 和方向键 roving 行为。
- `MediaPanel.tsx:497-513`：素材搜索 input 只有 placeholder，没有 `<label>`、`aria-label` 或 `type="search"`，Name/Role/Value 不完整。
- `MediaPanel.tsx:768-858` 的 ImportMenu：菜单项左对齐、28 px、`role=menuitem`；触发器缺 `aria-haspopup/aria-expanded`，打开不移动焦点，未实现方向键/Escape/焦点恢复。
- `MediaPanel.tsx:999-1088` 的排序/筛选菜单有 `menuitemradio/aria-checked`，但触发器状态和 popup 焦点问题相同。
- `MediaPanel.tsx:660-715`：breadcrumb 行高 24 px，但 crumb button 只有 `padding: 0 2px`，没有 24 px 最小高度/宽度；短标签命中目标可能小于 WCAG 2.2 的 24×24。

### 4.4 Preview

判定：**PARTIAL（E1 + E2 + E3\*）**。

- `Preview.tsx:399-590`：时间码左、播放控制居中、设置右，工具按钮 24 px，符合操作逻辑。
- `Preview.tsx:825-925`：scrub bar 有 `role=slider`、可访问名称、min/max/now、Home/End/方向键和 24 px 高命中，静态 PASS。
- `Preview.tsx:729-771`：预览页签视觉上 24 px 且文件名单行省略，但缺 tablist/tab/tabpanel 语义。
- `Preview.tsx:962-1075`：BadgeMenu 有 haspopup/listbox/expanded/option/selected 和 Escape，但未实现 listbox 的方向键选择、打开后焦点移动和关闭后焦点恢复完整模式。

### 4.5 Export

判定：**FAIL（视觉对比度，E1 + E2）**。

- `ExportDialog.tsx:169-210`：打开后移动焦点、Tab 圈闭、Escape、关闭恢复焦点完整。
- `ExportDialog.tsx:409-477`：`role=dialog`、`aria-modal`、可访问名称；关闭按钮 24×24。
- `ExportDialog.tsx:479-524,613-665`：功能字段逐行，label 左/control 右，hint 独立一行；footer 操作成组右对齐，按钮 28 px。
- `Dropdown.tsx:90-220`：28 px trigger、listbox/option 状态、方向键/Escape/焦点恢复有自动化契约。
- **缺陷**：`ExportDialog.tsx:642-657` 主按钮 `background: var(--accent-primary)`、`color: #fff`；`tokens.css:39` 的 accent 是 `rgb(245,239,228)`。按 WCAG 相对亮度公式，白字与该背景对比度为 **1.144:1**，远低于普通文字 4.5:1，也低于大字 3:1。
- `ExportDialog.tsx:597-609` 的导出错误没有 `role=alert`/live region；视觉可见但屏幕阅读器不保证及时播报。

## 5. WCAG 2.2 静态检查

| 检查 | 判定 | 证据 | 说明 |
|---|---|---:|---|
| SC 2.5.8 Target Size（Minimum）24×24 | PARTIAL | E1 | `HoverButton` 默认 24×24；Home/Library/TitleBar 26+；tab 24；Export 24/28。Media breadcrumb crumb 没有 24 px 最小尺寸，是明确缺口。 |
| SC 2.1.1 Keyboard | **FAIL** | E1 | Library 卡片操作只在 hover 时挂载且父容器不可聚焦；键盘无法执行导入/分类/取消收藏。 |
| SC 4.1.2 Name, Role, Value | PARTIAL / FAIL | E1 + E2 | 大部分 icon button 有 aria-label，slider/dialog/progress/dropdown 较完整；素材搜索缺可访问名称，多个 popup trigger 缺 expanded/haspopup，预览 tab 语义不完整。 |
| SC 2.4.3 Focus Order / 2.4.7 Focus Visible | PARTIAL | E1 + E2 | `global.css:127-132` 有全局 `:focus-visible`；Export modal 完整。Import/Toolbar/TitleBar popup 未把焦点移入、未实现完整方向键/恢复焦点。 |
| SC 1.4.3 Contrast（Minimum） | **FAIL** | E1 | Export 主按钮对比度 1.144:1。其他文字/背景未做完整像素或计算审计，不据此宣称 PASS。 |
| SC 1.4.10 Reflow / 缩放 | BLOCKED | 无 E3/E4 | 只有 1224×768 截图和 Editor 760 px 静态折叠逻辑；未做 200%/400% GUI 检查。 |

## 6. 本次独立测试账本

所有命令在 `OpenTake-generation` 运行，Exit 均为 0：

| 命令 | 结果 | 能证明 / 不能证明 |
|---|---:|---|
| `cargo test -p opentake-tauri asset_scope --lib -- --nocapture` | 3 passed | exact grant 的局部契约；不证明项目切换撤权和真实图片绘制。 |
| `cargo test -p opentake-tauri media::prewarm::tests --lib -- --nocapture` | 9 passed | queue、Busy、epoch/cancel；不证明 100 项素材峰值。 |
| `cargo test -p opentake-tauri safe_asset_protocol::tests --lib -- --nocapture` | 12 passed | scope/range/body/helper 等协议契约；不证明 WebView URL 成功。 |
| `cargo test -p opentake-media waveform:: --lib -- --nocapture` | 21 passed | 波形算法/缓存；不证明跨卡片并发有界。 |
| 16 个 Home/Library/Media/Preview/Editor/TitleBar/Export/Dropdown/accessibility/asset Vitest 文件 | **159 passed / 0 failed** | jsdom 交互与静态契约；有 MediaPanel React `act(...)` warning；不证明像素、VoiceOver、真实 WebView。 |
| MediaSearch/Timeline/library/media/project/action/asset 邻接集 8 个 Vitest 文件 | **135 passed / 0 failed** | 邻接 store/IPC 契约；有 MediaSearch React `act(...)` warning；不证明 FFmpeg 进程上限。 |

对 Evidence B 命令账本的复核说明：其前端 8 文件命令包含 `MediaPanel.test.tsx`，记录为 136 项；本次 135 项是另一个邻接集合，不构成结果矛盾。Evidence B 也明确把 `proxy::tests` 的 0 tests 视为缺口，本报告未把它升级为 PASS。

## 7. 主 Agent 必须执行的 GUI / 资源复核步骤

### 7.1 项目重开、授权、缩略图与预览

1. 确认运行包确为 `f9398a9`，记录 bundle version、可执行文件 hash、启动时间和完整 stderr；关闭所有旧实例。
2. 用本地、已物化、路径不在工程目录内的 4K60 视频、1080p 视频、图片、音频各 1 个，通过 native dialog 导入新工程并保存。
3. 在 Web Inspector Network 中记录每个 `opentake-asset` 请求的完整 URL、状态码、Content-Type、Range/Content-Range；同时记录缩略图缓存文件是否存在及尺寸。
4. 选中每个素材，确认卡片缩略图、Preview poster/视频/音频都显示；截取含工程名、素材名和 Preview 的截图。
5. 关闭应用，重新启动，经 native dialog 打开刚保存的工程；重复第 3–4 步。此步骤才验证“项目打开时自动授权”。
6. 在媒体文件同目录放一个未被工程引用的 sibling，通过 Inspector 请求其 asset URL；预期 403。若能读取则记录 FAIL。
7. 打开项目 B 后再次请求项目 A 的外部媒体；按最小权限预期应 403。当前实现预计仍可读，用此步骤量化 persisted-scope 累积。
8. 若缩略图仍空白，分别记录：生成命令返回路径、磁盘文件、scope、asset HTTP、DOM `img.src`、图片 decode error；不要只截图后猜 CSP。

### 7.2 音频波形过载

1. 准备包含 100 个不同、未缓存、1–2 分钟音频的工程；仅清除这些文件对应的 derived waveform cache，保留原媒体。
2. 打开 Media→Audio/Import，30 秒内每秒采样主进程/FFmpeg/safe-asset-helper 的数量、CPU、RSS、打开文件数和队列/Busy 日志。
3. 快速滚到末尾后立刻切 Home；记录 FFmpeg 峰值以及离开视图后 2/5/10 秒仍存活的工作数。
4. 当前静态实现没有共享上限；若 FFmpeg 峰值随可见/挂载音频数增长，即 E3/E4 复现 RU-01。修复后的验收应明确 worker、pending、同键合并和 cancel 上限，而非仅“看起来不卡”。

### 7.3 视频/图片缩略图、搜索和快速预览

1. 冷缓存工程放入至少 100 个视频/图片，慢速与快速滚动各一次；记录前端 active thumbnail 最大值（应 ≤4）、pending 峰值、prewarm 主 worker（应 ≤3）和 FFmpeg 子进程峰值。
2. 输入能产生 Moments 20 + Spoken 20 的搜索词，记录瞬时解码数；现有 `HitThumbnail` 绕过共享并发 4。
3. 每 100 ms 快速切换 10 个冷视频，最后停在第 10 个；确认旧 poster 任务被取消、最终图不被旧结果覆盖、后台在合理时间归零。当前代码只能保证旧 React 结果不写入，不能保证后端取消。

### 7.4 UI 六规则与 WCAG

1. 在 1600×1000、1224×768、760×480 分别检查 Home、Library、Editor、Import、Preview、Export；中文和英文各一次，记录溢出/换行/遮挡。
2. 执行 200% 和 400% 缩放；检查一行文案是否合理省略、功能描述是否逐行、按钮是否仍与字号匹配、编辑器是否可滚动而非内容丢失。
3. 仅用键盘从 Home 到 Editor→Library：Library 卡片必须能聚焦并执行导入、分类、取消收藏；当前预计失败。记录 Tab 顺序和焦点截图。
4. Import、排序、筛选、TitleBar、Preview Badge 菜单逐个用 Enter/Space 打开，方向键移动，Escape 关闭；核对焦点进入和返回触发器。
5. 在 DevTools 对所有紧凑目标执行 `getBoundingClientRect()`；特别检查 Media breadcrumb，每个可点击目标均应至少 24×24 CSS px，或满足 SC 2.5.8 的 spacing 例外并记录计算。
6. 打开 Export，测量主按钮 computed foreground/background；白字近白背景应直接判 FAIL。检查 error 出现时 VoiceOver 是否播报。
7. 用 macOS Accessibility Inspector / VoiceOver 记录素材搜索、tab、slider、菜单、对话框的 Name / Role / Value；任何仅有 placeholder/title 而无可访问名称的控件判 FAIL。
8. 对 `prefers-reduced-motion`、全键盘焦点可见、浅/深背景文字对比做截图/计算；不要用 jsdom 测试代替视觉测量。

## 8. 最终缺陷清单

| ID | 严重度 | 结论 | 缺陷 |
|---|---|---|---|
| RU-01 | HIGH / P1 | FAIL（E1） | 音频卡波形无共享并发、pending、同键合并或后端取消；全量挂载可触发无界解码。 |
| RU-02 | MEDIUM / P2 | PARTIAL（E1） | 搜索缩略图与快速预览绕过 bounded scheduler，卸载只忽略前端结果。 |
| RU-03 | MEDIUM / P2 | PARTIAL（E1） | 项目/Library 全量 DOM；缩略图 pending 无硬上限和队列删除。 |
| RU-04 | MEDIUM / P2 | PARTIAL（E1+E2） | 项目媒体 exact grants 随 persisted-scope 跨项目累积。 |
| RU-05 | HIGH / P1（验证缺口） | BLOCKED（E3\*） | 既有打包截图仍显示导入缩略图空白；缺少同构建网络/日志，根因未证实。 |
| UI-01 | HIGH / P1 | FAIL（E1） | Export 主按钮白字/近白背景，对比度 1.144:1。 |
| UI-02 | HIGH / P1 | FAIL（E1） | Library 主操作 hover-only 且父卡不可聚焦，键盘不可达。 |
| UI-03 | MEDIUM / P2 | PARTIAL（E1） | 素材搜索无可访问名称；多个 popup trigger/focus/方向键模式不完整。 |
| UI-04 | MEDIUM / P2 | PARTIAL（E1） | Media breadcrumb 目标未保证 24×24；部分 tab/listbox 语义不完整。 |
| UI-05 | MEDIUM / P2 | PARTIAL（E1+E2） | 全局 Library 有意不暴露路径，但普通收藏不持久化 thumb，实际只能显示类型图标。 |

## 9. 限制

- 本 Agent 未启动、操作或重启 `/Applications/OpenTake.app`，未运行 Web Inspector、VoiceOver、axe、像素对比或 CPU/RSS/FFmpeg 子进程采样。
- 三张既有截图均为 1224×768，只能证明画面曾出现；没有足够 provenance 将其当作当前 HEAD 的可重复 GUI 证据。
- 本次没有真实大库、大音频工程、A→B 权限切换、proxy 转码或跨重启 E4 验证。
- 自动化测试全部通过不等于资源无过载，也不等于 UI 满足 WCAG；本报告已将这些边界分别标为 PARTIAL、FAIL 或 BLOCKED。
