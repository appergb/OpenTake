# OpenTake 剪辑主体模块独立验证报告

- 审计日期：2026-08-07–08（Asia/Shanghai）
- 审计基线：`f9398a9302e57690caa3ea21ff995fdd0ce97706`
- 比较基线：`v1.0.0-beta.1` / `c73c192f6fdc01e35a1711089b6e59edc3312309`
- 安装包：`/Applications/OpenTake.app`，`1.0.0-beta.2`
- 审计组织：独立总审计 Agent 负责模块规划、证据复核和 fresh GUI；两个独立子 Agent 分别检查资源/UI、竞品官方资料。产品源码在本报告冻结前未修改。

## 1. 总结结论

播放竞态修复 **对时间线视频播放有效**：fresh 4K60 安装包实测从 frame 17 连续推进到 frame 239，且确定性 4K60 双轨慢消费者 harness 为 25 次 HTTP 200、0 次 204。合成/渲染和已声明导出格式也形成了自动化、真实产物与 GUI 闭环。

但当前版本不能判为“剪辑主体全部正常”。最严重的确认问题是：**项目素材缩略图破图、选中源素材预览黑屏、Home 工程封面为空**。缓存图已正确生成且可解码，保存退出重开后仍稳定复现；时间线合成与导出同时正常。这说明 `f9398a9` 的精确源媒体授权没有修复资源展示主链，故障已收窄到 `opentake-asset` 自定义协议的请求/响应/消费链，确切 HTTP 状态尚未取得。

资源有界性同样不通过：音频素材卡会无共享上限地并发请求 waveform；搜索缩略图、快速预览和缩略图 pending 也存在绕过/积压。UI 的总体布局和用户提出的六条视觉规则大体一致，但 Export 主按钮对比度仅 1.144:1、Library 核心操作键盘不可达，因此 UI 整体为 FAIL。

### 模块判定矩阵

| 模块 | 判定 | 证据 | 结论 |
|---|---|---:|---|
| 播放引擎 | **PASS（视频主链）/ PARTIAL（完整 A/V）** | E1+E2+E3 | 慢消费者修复有效，fresh 安装包 4K60 时间线画面与播放头推进；未用带音频真实工程验证 A/V 同步和音频设备边界。 |
| 合成引擎与渲染 | **PASS（声明范围）** | E1+E2+E3+E4 | 多轨/效果/文字/转场等 116 项渲染测试通过；fresh 时间线预览与 GUI 导出帧正确。竞品效果广度不等于本模块 bug。 |
| 导入与导出 | **PARTIAL** | E1+E2+E3+E4 | 导入持久化、工程重开和导出产物成立；导入后的素材展示失败，显式导入也缺输入上限/完整取消。导出主路径 PASS。 |
| 资源加载 | **FAIL** | E1+E2+E3 | asset 展示主链失败；waveform 无界 fan-out；搜索/预览绕过统一调度；persisted scope 生命周期不闭环。 |
| 与其他剪辑软件差异 | **PARTIAL（产品广度）** | E1+O1 | OpenTake 的 local-first、开源、跨平台、Agent-first 是定位差异；后台队列、团队协作、Fusion 等是能力差距，不作为本轮可靠性 bug。 |
| 前端 UI | **FAIL（整体）** | E1+E2+E3 | 六条视觉规则大部分为 PASS/PARTIAL；资源破图、Export 对比度和 Library 键盘不可达使整体不能通过。 |

## 2. 判定与证据等级

- **PASS**：已声明范围内有直接证据，未发现阻断缺陷。
- **PARTIAL**：主路径成立，但存在明确边界或未覆盖的高风险路径。
- **FAIL**：代码、测量或 fresh GUI 已直接证明缺陷。
- **BLOCKED**：现有工具/fixture 无法形成有效结论，不能用低等级证据冒充。
- **E1**：当前代码、配置、依赖与逐行 diff。
- **E2**：可重复自动化测试、确定性 harness 或日志。
- **E3**：当前安装包真实 GUI、真实进程与重启操作。
- **E4**：ffprobe、帧数、画面抽帧、SSIM 等真实输出解析。
- **O1**：2026-08-08 访问的厂商一手资料。

## 3. 为什么之前的修复部分有效、部分无效

### 3.1 播放冻结修复有效

`beta.1 → f9398a9 前一提交` 的播放引擎目录、`RustFrameBuffer.tsx` 与 `previewEngine.ts` 无差异；问题是重负载才暴露的 latest-frame 单槽竞态，不是一次引擎回归。`f9398a9` 把 transport lookup 改为：同 epoch/version/session 且请求帧不晚于最新帧时返回最新帧，未来帧和跨 session 仍拒绝；前端非 terminal 204 再重试一次。

验证结果：

- playback Rust 单元 103 项、命令集成 1 项、transport 集成 6 项、前端 87 项通过。
- clean rebuild 后完整 playback integration 7/7；第一次 15 项失败来自审计 harness 共用 target 导致重复 crate 类型碰撞，清理 16.1 GiB 后重跑通过，不能把首轮环境污染当产品失败。
- 锁定 4K60 双轨 harness：`published=14`、`newest_frame=211`、`http_200=25`、`http_204=0`、`invalid_jpeg=0`；未来帧/跨 session 均 204。
- fresh GUI：frame 17 与 frame 239 两张截图显示播放头和图像都变化。渲染跟不上时允许掉帧，但不再冻结。

边界：real-media ignored probe 命令显示 4 项通过，但其中 2 项因环境 fixture 缺失实际打印 skip；只有生成的 ProRes/调色 fixture 是真实执行。本文不把 4 项全部记作真实媒体覆盖，也不把无音频 4K60 fixture 推断为 A/V 同步 PASS。

### 3.2 CSP 不是缩略图/预览失败的充分解释

fresh 安装包实测形成稳定反馈环：

1. native dialog 导入 4K60 视频，等待 15 秒后素材卡仍破图；
2. 同时磁盘上的 120×68 thumb 与 1920×1080 preview 为有效 PNG、内容正确；
3. 选中源素材等待 5 秒，中央 Preview 黑且时间码为 0/0；
4. 双击加入时间线后 Rust 合成预览立即正常；
5. 保存、退出、重新启动、重开工程后重复 15 秒/5 秒等待，缩略图与源 Preview 仍失败；
6. Home 最近项目封面也为空，但工程目录内 `thumbnail.jpg` 存在。

`f9398a9` 确实只给 manifest 中已物化的源文件做精确 `allow_file`；而缓存文件依赖 `$APPCACHE/**/*` 静态 scope。两种来源都失败，且缓存和源文件授权都存在，因此“只缺源文件授权”与“缓存未生成”均被排除。前端三类资源都经 `convertFileSrc(..., "opentake-asset")`，而正常的时间线画面走独立 Rust framebuffer/MJPEG，最小共同故障面是自定义 asset 协议链。

release WKWebView 无可检查 target；终端直接启动 release、统一日志和一次 dev 启动诊断没有给出 asset 错误。dev WebView 未成功被同一自动化驱动，不作为 release 佐证。故实际请求 URL、HTTP 状态、Content-Type、Range 和 DOM decode error 仍为 **BLOCKED**；本报告不猜测为 CSP、403 或特定 helper 错误。

完整 GUI 时间线见 [fresh-gui-ledger.md](editor-core-validation-assets/fresh-gui-ledger.md)。

## 4. 六个主体模块

### 4.1 播放引擎

判定：**PASS（视频主链）/ PARTIAL（完整 A/V）**。

强证据是机制、负载与安装包三层一致：transport contract 仍拒绝未来帧/跨 session；4K60 双轨慢消费者没有 204；fresh 4K60 单轨安装包播放显示并推进到末帧。暂停定位、跳帧和时间线静止帧还由既有 playback/transport 测试覆盖。

剩余边界不是当前冻结缺陷：带音频的真实 4K60 双轨项目、系统音频设备异常、长时间 A/V drift、代理切换未做 fresh E3/E4，因此完整播放仍保留 PARTIAL。

证据：[05](editor-core-validation-evidence-a/logs/05-playback-tests.log)、[06](editor-core-validation-evidence-a/logs/06-playback-transport-integration.log)、[07](editor-core-validation-evidence-a/logs/07-playback-web-tests.log)、[10b](editor-core-validation-evidence-a/logs/10b-playback-4k60-dual-track-harness-locked.log)、[11b](editor-core-validation-evidence-a/logs/11b-playback-integration-full-rebuild.log)、[GUI frame 17](editor-core-validation-assets/08-playback-frame-17.jpg)、[GUI frame 239](editor-core-validation-assets/09-playback-frame-239-terminal.jpg)。

### 4.2 合成引擎与渲染

判定：**PASS（声明范围）**。

预览和导出共享 `RenderPlan`/wgpu compositor。渲染日志覆盖 116 项：基础 compositor、复合验收、compound/nested、downscale、11 effects、smoke、文字、Y 方向、LUT、optical、14 pixel assertions、稳定、转场。fresh 重开工程后时间线合成预览正常；GUI 导出的 2 秒帧包含预期彩条、对角线和棋盘元素。

效果注册表数量、Fusion 节点合成、复杂遮罩/变速、更多转场属于竞品能力广度，不应被描述为当前像素链错误；OpenTake 对未支持组合 fail-closed 是正确性边界。

证据：[render full tests](editor-core-validation-evidence-a/logs/12-render-full-tests.log)、[reopen preview](editor-core-validation-assets/11-project-reopened-timeline-preview-ok.jpg)、[GUI export frame](editor-core-validation-assets/14-gui-export-frame-2s.png)。

### 4.3 导入与导出

判定：**PARTIAL；导出主路径 PASS**。

导入相关 38 项、递归镜像 9 项、重链 3 项与 project open 2 项通过；fresh GUI 能导入、保存、退出、重开且保留一条 240 帧 V1 clip。功能缺陷是导入后资源不可见；工程性缺陷是显式 `import_media` 没有路径数/总字节硬限制，probe 无 hard deadline，公开目录导入传 `cancel=None`。

导出证据完整：集成 5/5，H.264/H.265/ProRes probes 3/3；三个 1280×720、30 fps、60 帧、2 秒产物可解析，跨编码 SSIM 为 0.992708/0.993718；XMEML、FCPXML 1.10、EDL、OTIO、SRT、VTT 产物均已生成。fresh GUI 另导出 H.264 1280×720、60 fps、240/240 帧、4 秒、BT.709，抽帧视觉正确。

证据：[import/resource ledger](editor-core-validation-evidence-b/command-ledger.md)、[export integration](editor-core-validation-evidence-a/logs/13-export-integration.log)、[codec probes](editor-core-validation-evidence-a/logs/14-export-codec-probes.log)、[ffprobe](editor-core-validation-evidence-a/logs/15-export-artifact-ffprobe.log)、[SSIM](editor-core-validation-evidence-a/logs/16-export-codec-ssim.log)、[interchange](editor-core-validation-evidence-a/logs/17-project-interchange-tests.log)、[fresh GUI MP4](editor-core-validation-assets/gui-export-720p.mp4)。

### 4.4 资源加载

判定：**FAIL**。

局部有界性是成立的：主 prewarm 64/3 workers、低优先级 8/1；素材缩略图前端并发 4、LRU 256、同键前端合并；asset 读取 4 active + 32 pending、5 秒 deadline、body/range 限额。

确认反证：

- 每张已挂载音频素材卡立即直接 `getWaveform`；全量 `items.map`，无共享 worker/pending/single-flight/后端取消。
- Moments/Spoken 搜索命中各最多 20，但直接生成缩略图，绕过共享并发 4。
- 快速切换 Preview 时旧 1080p poster 只被 React 忽略，后端不取消；可能与近视口 prewarm 重复冷解码。
- 缩略图 pending 数组没有硬上限，快速滚动可积累闭包；项目/Library DOM 无虚拟化。
- exact source grants 被 persisted-scope 保存，项目 A→B/退出不替换 active set，历史精确文件权限累积。
- proxy 静态实现看起来有 single-flight/cancel/atomic rename，但 `cargo test -p opentake-media proxy::tests` 实际运行 0 项，真实转码/取消/残留回归缺失。

详见独立子审计 [resource-ui-static.md](editor-core-validation-assets/resource-ui-static.md) 和 [Evidence B](editor-core-validation-evidence-b/README.md)。

### 4.5 与其他剪辑软件的主要区别

判定：**产品定位清楚，能力广度 PARTIAL；不把广度当本轮 bug**。

| 参照 | OpenTake 的结构性差异 | 属于定位还是确认缺陷 |
|---|---|---|
| Premiere Pro | OpenTake 是单机本地项目、单导出任务、项目内 Agent；Premiere 有 Media Encoder 队列、媒体缓存管理、Team Projects/Productions。 | local/Agent-first 是定位；后台队列和团队能力是产品差距。 |
| DaVinci Resolve | OpenTake 单工作台多面板、合成刻意收窄；Resolve 分 Media/Edit/Fusion 等任务页并有节点合成/多人协作。 | 轻量单工作台是定位；Fusion/协作是能力差距。 |
| Final Cut Pro | OpenTake 是传统轨道、Rust/Tauri 跨平台；FCP 是 macOS 专属 Magnetic Timeline、Roles、后台优化媒体。 | 轨道/跨平台是架构选择；Roles/后台媒体管理是能力差距。 |
| CapCut | 素材类别布局相近；OpenTake 更强调本地可审计工程交换与项目内 Agent，部分分类仍是占位。 | Agent/local 工程是定位；模板/云 Space/内容生态是能力差距。 |

更准确的定位是“本地优先、开源、跨平台、Agent-first 的轻量专业剪辑器”。团队协作、Fusion 等未声明广度不会进入本轮 bug 修复门槛；播放可靠性、资源有界性、可见素材和可访问 UI 则必须修复。

竞品事实全部来自厂商官方页，见 [competitor-official-sources.md](editor-core-validation-assets/competitor-official-sources.md)。

### 4.6 前端 UI 与用户六条规则

总体判定：**FAIL**。五区工作台的视觉信息架构清楚，按钮居中/左对齐、单行省略、功能逐行与紧凑纵深大多成立；但确认的可见性和可访问性缺陷足以使整体失败。

| 用户规则 | 判定 | 结论 |
|---|---|---|
| 设计统一原则 | PARTIAL | tokens 统一字号/面板/工具栏；Home/Editor 风格一致。asset 破图、Export 颜色和 Library 交互是显著例外。 |
| 文字大小与按钮大小匹配 | PARTIAL | 主操作 11–13 px 配 24–34 px；8/9 px micro/xxs 仍需缩放实测，Export 主按钮文字几乎不可读。 |
| 按钮居中或靠左 | PASS（主要路径） | 图标按钮 flex 居中；菜单/分类/项目行左对齐；对话框 footer 右对齐成组符合惯例。 |
| 文字描述尽量一行 | PASS（主要路径） | 项目名、素材、tab、预览文件名普遍 nowrap/ellipsis；错误清单换行是合理例外。 |
| 每功能描述单独一行、前后紧凑 | PASS（静态） | Export 字段、空状态、菜单项均逐行且纵深紧凑。 |
| 布局符合人类逻辑、易用清晰 | PARTIAL/FAIL（键盘） | 视觉层级合理，但 Library 核心操作 hover-only 且卡片不可聚焦；多个 popup/menu/tab 语义不完整。 |

确认 UI 缺陷：

- Export 主按钮 `#fff` / `rgb(245,239,228)`，对比度 **1.144:1**，低于 4.5:1/3:1。
- Library 导入/分类/取消收藏按钮仅 hover 时挂载，父卡不可聚焦，键盘用户无法到达。
- 素材搜索只有 placeholder，无可访问名称；Import/排序/筛选触发器缺完整 haspopup/expanded 和焦点模式。
- Media breadcrumb 未保证 24×24；Preview 页签与部分 listbox 缺完整语义/方向键/焦点恢复。
- Export error 缺 live region。

完整 WCAG 证据、组件行号和 159+135 项独立前端测试见 [resource-ui-static.md](editor-core-validation-assets/resource-ui-static.md)。

## 5. 确认缺陷与产品能力差距

### 5.1 本轮应修复的确认缺陷

| ID | 优先级 | 结论 | 修复目标 |
|---|---:|---|---|
| ASSET-01 | **P0** | FAIL（E3） | 缩略图、源素材 Preview、Home 工程封面在安装包中可正确显示；项目重开后仍成立；补实际自定义协议回归。 |
| RES-01 | **P1** | FAIL（E1） | waveform/poster/search thumbnail 统一有限 worker/pending、同键合并、epoch/latest-wins 取消；缩略图 pending 硬上限。 |
| UI-01 | **P1** | FAIL（E1） | Export 主按钮文字对比度达到 WCAG；错误 live announcement。 |
| UI-02 | **P1** | FAIL（E1） | Library 核心操作可由键盘聚焦和执行，视觉 hover 仍可保留。 |
| UI-03 | P2 | PARTIAL（E1） | 搜索、popup、breadcrumb、Preview tab/listbox 的 name/role/value、焦点和 24×24。 |
| SCOPE-01 | P2 | PARTIAL（E1+E2） | 当前项目 active media set 二次门禁；A→B 后 A 拒绝，B 可读，重启不泄漏 active set。 |
| IMPORT-01 | P2 | PARTIAL（E1） | 显式导入数量/总量/hard deadline；公开目录导入取消和进度。 |
| PROXY-01 | P2 | BLOCKED（E2 缺口） | 真实 FFmpeg 成功/取消/源变更/partial cleanup/ffprobe 回归，而不是 0 tests。 |

### 5.2 不作为本轮 bug 的能力差距

- 多人实时同编、项目锁、审阅链接、云 Space。
- Fusion 级节点式 2D/3D 合成、专业调色/音频专页。
- Premiere Media Encoder 式多任务后台队列、FCP Roles 分轨母版。
- 模板市场、内容生态、平台一键发布。
- 与专业 NLE 同数量的效果、转场、格式和母版选项。

这些项目可进入产品路线图，但不能挤占 ASSET-01、资源有界性、播放/导出正确性和基本可访问性的发布门槛。

## 6. 测试与证据总账

| 证据线 | 结果 | 解释边界 |
|---|---|---|
| Playback | Rust 103 unit + 1 command、transport 6、web 87、clean integration 7 通过 | 2 个 ignored real-media probes 实际 skip；无音频 fixture 不证明 A/V 同步。 |
| 4K60 双轨 harness | 25×200、0×204、0 invalid JPEG；future/foreign 204 | 确定性重负载，不替代硬件/音频长时 E3。 |
| Render | 116 项通过 | 覆盖声明的 compositor/effect/text/transition；不声称等同 Fusion 广度。 |
| Export | integration 5/5，codec probe 3/3；H.264/H.265/ProRes 产物 + SSIM | 既有产物 720p30/60 帧/2 秒；fresh GUI 另有 720p60/240 帧/4 秒。 |
| Project/interchange | 相关 crate/交换/字幕测试通过，XMEML/FCPXML/EDL/OTIO/SRT/VTT 产物存在 | 格式语法/往返证据，不代表所有第三方软件兼容矩阵。 |
| Import/resource | import 38、recursive 9、asset scope 3、prewarm 9、safe asset 12、thumbnail 27、waveform 21 | 局部契约通过；不覆盖 asset WebView 绘制和 100 音频峰值。 |
| UI | 独立前端组 159/159、邻接组 135/135 | jsdom 不证明视觉对比度、VoiceOver、真实 WebView。 |
| Fresh GUI | 导入/保存/重开/播放/导出完成；asset 故障稳定复现 | release 无 Inspector 网络状态，准确根因待失败测试与修复验证。 |

审计证据 A 中 `10-playback-4k60-dual-track-harness.log` 首次只含下载/编译，无最终结果；正式采用 `10b`。`11-playback-integration-full.log` 的失败由共享 target 类型碰撞造成；正式采用清理后的 `11b`。这些失败均保留，不做选择性删除。

## 7. 限制与下一步验收门槛

- 当前无法取得 release `opentake-asset` 的实际 Network/console；必须以可失败的协议/URL 回归测试和修复后的 fresh GUI 来闭环 P0。
- 未做 100 音频/100 视频工程的 E4 峰值采样；资源 FAIL 先由清楚的无界代码路径成立，修复后再测 FFmpeg 峰值、pending、退出后回收。
- 未做 VoiceOver、200%/400% 缩放、760×480 全页面；确认的对比度/键盘缺陷无需等待这些验证才可修。
- 未用用户原「未命名 2」项目；fresh 4K60 fixture 与双轨 harness 提供可重复替代，但完整真实双轨 A/V 项目仍是最终回归项。
- 本报告是 `f9398a9` 修复前冻结基线。后续任何修复必须另记 commit/diff、失败测试、focused/full checks、正式包 hash 和修复后 GUI 证据，不可覆盖本目录原始截图/产物。

## 8. 文件索引

- [Fresh GUI 账本](editor-core-validation-assets/fresh-gui-ledger.md)
- [资源与 UI 独立子审计](editor-core-validation-assets/resource-ui-static.md)
- [竞品官方资料独立子审计](editor-core-validation-assets/competitor-official-sources.md)
- [Evidence A：播放/渲染/导出](editor-core-validation-evidence-a/)
- [Evidence B：导入/资源](editor-core-validation-evidence-b/README.md)
- [Assets README](editor-core-validation-assets/README.md)
- [修复矩阵](editor-core-remediation-matrix.md)
- [修复后正式包 GUI / 资源证据](editor-core-after-fix-assets/README.md)
- [真实设备 A/V 播放 probe](editor-core-after-fix-assets/playback-av-real-device.md)

## 9. 修复后正式包最终复验（2026-08-08）

本节是对前述冻结基线的追加闭环，**覆盖第 4、6、7 节的最终发布状态**，但不删除原始失败证据。确认缺陷的 RED、修复、focused/full gate 与 review 记录见 [remediation matrix](editor-core-remediation-matrix.md) 和 `editor-core-remediation-assets/`；正式包证据见 [after-fix README](editor-core-after-fix-assets/README.md)。

### 9.1 代码与 review 门禁

- Rust workspace full test 通过；Tauri 最终为 537/537，media 434 passed / 1 ignored。safe-asset focused 18/18、proxy 11/11、既有 proxy integration 1/1。
- `cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all --check` 通过。
- Web full 137 files / 1143 tests、`pnpm build`（含 TypeScript build）通过；仅保留既有 dynamic-import/chunk warning。
- Rust reviewer 与综合 reviewer 对 retained asset authority、ScopeOnly final-root、import retained planning、proxy private stage/no-replace/cancel、TS scheduler/UI 修复给出 **FINAL PASS，0 CRITICAL / HIGH / MEDIUM**。

最终修复没有放宽 fail-closed 边界：ProjectMedia 稳定 alias 可读；同 epoch A→B、同路径 inode 替换、ancestor swap、ScopeOnly final-scope/root 逃逸均拒绝。Proxy 从一开始写 output 同目录的私有 stage，初始 source open 非阻塞/no-follow/no-recall；取消、ABA、public partial、发布 clobber 的回归都由真 FFmpeg fixture 覆盖。

### 9.2 正式构建、签名与安装身份

最终 Tauri 命令使用 `--bundles app,dmg` 和 `macOS.signingIdentity="-"`。未完整 seal 的首个 plain bundle strict 验证失败且从未安装；ad-hoc 重建后：

- build app 与 DMG fresh readonly mount 内的 app 均通过 `codesign --verify --deep --strict`；两者 `diff -rq` 无输出。
- installed `/Applications/OpenTake.app` 与 build app `diff -rq` 无输出；main、ffmpeg、ffprobe hash 逐项相同。
- main SHA-256：`27a55587612dc70b7f2d60740b0c80c75065d53208385bb4cc38d3da5619c327`；DMG SHA-256：`bf9eb8ff32eb202fb4150fa7e6682222e11e8d05a3e9170ef8636f4ab2953aab`。
- `com.opentake.desktop` / `1.0.0-beta.2` / arm64 / minimum macOS 11.0。
- 原安装包没有删除，移动到 `/Applications/OpenTake.app.backup-20260808-131143`。安装过程和完整 hash 见 [installation ledger](editor-core-after-fix-assets/installation-ledger.md)。

签名是 **ad-hoc，未 notarize，TeamIdentifier 未设置**。因此 strict/deep 是制品完整性证据，不能表述为 Apple 公证发行；`spctl` 拒绝是该分发形态的预期限制。

### 9.3 Fresh installed GUI 结果

所有 UI 操作只驱动 `/Applications/OpenTake.app`；没有 dev/Vite 或旧 bundle 实例。验证中没有出现新的文件/媒体/应用权限弹窗，也没有修改 macOS 安全设置。

| 模块 | 最终判定 | 正式包证据与边界 |
|---|---|---|
| 播放引擎 | **PASS（视频主链 + 核心真实设备 A/V）/ PARTIAL（感知与长时多轨）** | 单轨 4K60 到 frame 220；fresh 双轨 0→239、再次 0→48；seek/pause 均稳定。另以 H.264 1584×1080@30 + AAC 48 kHz stereo 的 10 秒动态 fixture，在默认 MacBook Air 扬声器上 strict CPAL probe 三连 PASS：85/91、84/90、84/89 frames/playhead，均 `clock=audio`、nonblack 1.000、neon green 0。没有麦克风 loopback，且 3 秒单轨窗口不外推长时多轨 drift。 |
| 合成与渲染 | **PASS（声明范围）** | 双独立 H.264 4K60 源在 frames 0–239 全重叠；V2 缩放/偏移/0.8 opacity 的嵌套画面在预览、播放和 GUI 导出 2 秒帧一致可辨。 |
| 导入 | **PASS（核心安全与项目重开）/ PARTIAL（产品进度/取消 UI）** | 上限/deadline/retained identity 修复和 tests/review 通过；完整退出再启动、重开工程后两个外部源都可见。公开目录导入的独立进度/主动取消仍是能力增量。 |
| 导出 | **PASS** | GUI H.264 720p 产物 489,960 bytes；ffprobe 为 1280×720、60/1、240 frames、4.000 s、yuv420p/BT.709、单视频流；2 秒真 PNG 抽帧已目视复核。 |
| 资源加载 | **FIXED（确认无界路径）/ PARTIAL（100 项 E4）** | typed shared scheduler、active 4/pending 64、epoch/latest-wins 与 reviewer findings 均关。双轨播放进程族各进程 RSS 峰值之和约 541 MB（约 528 MiB），24 次采样无 FFmpeg 子进程；导出观察到 1 encoder + 至多 1 短 helper。未做 100 项冷缓存峰值，不冒充全面性能认证。 |
| 前端 UI | **PASS（本轮六条与键盘确认缺陷）/ PARTIAL（专项无障碍）** | Export 主按钮代码对比度 16.50:1，GUI Tab 焦点可见；Library 从零 hover 开始可依次 Tab 到每卡导入/分类/取消收藏。搜索名称、popup/tab/listbox semantics 测试与 review 通过。VoiceOver、200%/400% 缩放仍留专项。 |
| 竞品差异 | **定位不变，非 bug** | 团队协作、Fusion/节点合成、模板市场、后台队列等继续作为路线能力差距，不进入本次确认缺陷门槛。 |

P0 asset 反馈环已反转并跨重启保持：Home 封面正常；工程重开稳定等待 15 秒后素材卡缩略图与时间线合成预览正常；选中源稳定等待 5 秒后源 Preview 正常；退出进程、重新启动并重复上述步骤仍正常。该 E3 结果与 safe-asset 18/18、双 reviewer FINAL PASS 一致，故 ASSET-01 从基线 FAIL 更新为 **FINAL PASS**。

### 9.4 Fresh fixture、资源与截图可信度

fresh `gui-validation-dual.opentake` 是 3840×2160、60 fps、240 frames / 4 秒；V1 与 V2 都覆盖 0–239，且引用两个不同源。V2 hflip 源 SHA-256 为 `fca06a45eec96ac5b4c3618aa8bb0fca409e49eb2374f3268237c85a4c478fc8`。用户历史「未命名 2」的源位于已离线 `/Volumes/mac/...`，因此标为 **fixture BLOCKED**，不把离线媒体误报为产品回归。

音频边界由独立真实设备 probe 补齐：默认输出为 MacBook Air 扬声器 2ch/96 kHz；mktemp fixture 为 10 秒 H.264 动态红/蓝画面 + AAC 48 kHz stereo sine。`OPENTAKE_REQUIRE_AUDIO_CALLBACK=1` 下，真实 CPAL callback/audio master clock 三连通过。首次 testsrc2 运行仅因素材本身含 16.2% 纯绿色而误撞绿屏哨兵；安全动态 fixture 后 nonblack 1.000、neon green 0。完整证据和三份原始日志见 [真实设备 A/V probe](editor-core-after-fix-assets/playback-av-real-device.md)。

Computer Use 返回的截图字节实际为 JPEG。正式证据保留原始 JPEG 到 `editor-core-after-fix-assets/diagnostics/original-computer-use-jpeg/`，并用 `sips` 真正重新编码为 PNG；所有 01–19 `.png` 都经 `file` 确认为 PNG，且逐张 `view_image` 检查。两个不能证明预期行为的截图被移入 diagnostics：terminal reset 后的伪暂停，以及被忽略的 AX slider setter；正式 seek/pause 采用有 AX frame 值和稳定等待的 06、07、11。

### 9.5 最终结论

本轮确认的 P0/P1 与安全正确性缺陷均已完成 RED→GREEN、full gates、强制 review、正式 build/install 和 fresh GUI 闭环；正式包复验未发现新的产品 bug。补充的 strict real-device probe 使播放引擎核心 A/V 设备/时钟集成也达到 PASS。仍保留的事项是明确的能力/覆盖边界：扬声器主观响度/麦克风回采、带音频的长时多轨 A/V drift、100 项冷缓存性能、公开目录导入进度/主动取消、VoiceOver/高倍缩放，以及 ad-hoc 未公证分发。它们不推翻播放视频与核心 A/V、声明范围合成、GUI 导出、asset 加载和本轮 UI 确认缺陷的通过结论。
