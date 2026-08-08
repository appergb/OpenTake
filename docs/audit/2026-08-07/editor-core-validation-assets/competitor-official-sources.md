# 竞品官方资料与 OpenTake 差异核验

- 核验基线：2026-08-08
- OpenTake 基线：`f9398a9302e57690caa3ea21ff995fdd0ce97706`
- 竞品范围：Adobe Premiere Pro、DaVinci Resolve、Final Cut Pro、CapCut Desktop/Online
- 资料规则：竞品事实只采用厂商官方产品页或帮助文档；不以第三方评测、论坛帖子或营销口号推断能力。

## 判定与证据等级

| 标记 | 含义 |
|---|---|
| PASS | OpenTake 在已声明边界内有当前实现和可重复证据；不代表与竞品功能数量相同。 |
| PARTIAL | 主路径存在，但边界、产品化程度或当前实机证据不完整。 |
| FAIL | 对照能力缺失，或当前代码已确认存在反向证据。 |
| BLOCKED | 受实机、账号、付费能力或本次只读范围限制，无法形成有效结论。 |
| E1 | 当前 OpenTake 代码/架构证据。 |
| E2 | 当前可重复测试、日志或媒体产物。 |
| E3 | 当前安装包在真实设备上的端到端交互证据。 |
| O1 | 2026-08-08 访问的厂商一手资料。 |

## 结论矩阵

| 检查项 | OpenTake 当前判定 | 证据 | 结论 |
|---|---|---|---|
| 播放连续性 | PARTIAL | E1+E2；E3 缺失 | 慢消费者已可取得同会话最新帧，4K60 双轨锁定负载测试无 204；但本次没有重新操作安装包，不能把机制测试等同于完整实机 PASS。 |
| 代理与预览缓存 | PARTIAL | E1+E2+O1 | 有持久化代理、校验、单飞创建、取消和原片导出；但尚无 Premiere/FCP 式统一优化媒体与可管理播放缓存体系，当前代理实机闭环也未重跑。 |
| 合成与渲染 | PARTIAL | E1+E2+O1 | 共享 `RenderPlan`/wgpu 的预览—导出主链路在声明范围内通过；但效果、转场、遮罩和速度组合边界明显窄于四款竞品。 |
| 导入与导出 | PARTIAL | E1+E2+O1 | H.264/H.265/ProRes、音频、字幕及四种交换格式已有真实产物；无后台多任务队列、发布目的地、角色/分轨母版和更广母版格式。 |
| 资源加载不过载 | FAIL | E1+E2 | 预热、网格缩略图和协议层有界，但音频波形按卡片并发直调、搜索/预览解码缺少共享调度/后端取消；“不存在过度加载”无法成立。 |
| UI 工作流拓扑 | PARTIAL | E1+O1 | 五区工作台、三种布局、CapCut 式素材分类和项目内 Agent 是明确差异；若干顶部分类仍是禁用占位，专业工作流入口分散。 |
| 团队协作 | FAIL（竞品同类能力） | E1+O1 | 未发现多用户项目、项目锁、云同步、审阅链接/评论。若 OpenTake 定位为单机本地编辑器，这是范围选择；若目标包含专业团队/云端协作，则是实质缺口。 |
| 视觉统一细则 | BLOCKED | — | 本文件只核验竞品工作流与代码拓扑，不替代安装包视觉审计；字号、按钮尺寸、对齐和单行文案应以独立 UI 实机检查结论为准。 |

## 六模块逐项对照

### 1. 播放、代理与缓存

- **竞品可验证基线**：Premiere 可在导入时复制、转码或创建代理，代理可附加并在最终输出切回原片 [A1]；其媒体缓存包含音频峰值、转换音频和索引数据库，可定位及清理 [A2]。Resolve Proxy Generator 可监看文件夹并自动生成、关联 H.264/H.265/ProRes 代理 [D1]。Final Cut Pro 可在导入时或之后创建优化媒体/代理，后台处理并在共享前切回原始或优化媒体 [F1]，不能实时播放的区段可后台生成临时渲染文件 [F2]。CapCut Desktop 明确提供 Proxy 开关，官方帮助也说明代理只降低预览质量、不决定最终导出质量 [C1]。
- **OpenTake 现状**：播放传输只保存最新帧，但当前 lookup 接受同 project epoch、timeline version、session 且请求帧不晚于最新帧的慢消费者；前端非 terminal 204 会重试一次。锁定 4K60 双轨合成测试为 `published=14`、`http_200=25`、`http_204=0`，未来帧/跨会话仍为 204。代理使用项目内持久化记录和源文件摘要，创建时单飞、可取消、临时文件原子替换，播放按偏好选代理而导出取原片。静态资源缓存、解码队列、纹理缓存、音频窗口和销毁队列均有容量边界。
- **差异性质**：播放冻结竞态属于已修复的可靠性问题；统一的优化媒体/播放缓存管理属于能力缺口。OpenTake 选择“消费者追上最新帧并允许掉帧”，是面向实时响应的实现策略，并非与离线逐帧导出冲突。
- **判定**：播放 **PARTIAL（E1+E2，E3 缺失）**；代理/缓存 **PARTIAL（E1+E2+O1）**。

本地证据：[transport.rs](../../../../src-tauri/src/playback/transport.rs)、[rustFrameBuffer.ts](../../../../web/src/components/preview/rustFrameBuffer.ts)、[4K60 双轨锁定负载日志](../editor-core-validation-evidence-a/logs/10b-playback-4k60-dual-track-harness-locked.log)、[播放传输集成日志](../editor-core-validation-evidence-a/logs/06-playback-transport-integration.log)、[proxy.rs](../../../../crates/opentake-media/src/proxy.rs)。

### 2. 合成引擎与渲染

- **竞品可验证基线**：Premiere 的 Mercury Playback Engine 将播放、渲染、图像处理、颜色转换和缩放交给 GPU 加速，并可在渲染/导出时使用多 GPU [A3]。Resolve Fusion 是节点式 2D/3D 合成环境，公开能力包括跟踪、抠像、遮罩及大型节点树 [D4]。Final Cut Pro 的官方界面说明以 Magnetic Timeline、连接片段和 Roles 组织复合编辑 [F5]。CapCut Desktop 官方帮助确认位置、缩放、旋转和不透明度等关键帧位于右侧编辑面板 [C6]。
- **OpenTake 现状**：预览和全量导出共享 `RenderPlan` 与 wgpu compositor；已覆盖多视频轨、变换/裁剪/透明度/关键帧、文字、调色/LUT、色键、稳定、最多 4 个遮罩和 cross-dissolve。通用效果注册表当前只有 grayscale/sepia/invert，转场只有 cross-dissolve；Lottie、超过 4 个遮罩、复合场景中的 reverse/非 1 倍速会 fail-closed，而非静默错误预览。
- **差异性质**：共享渲染语义和 fail-closed 是正确性设计；效果数量、节点合成、复杂转场、复合变速等是明确能力缺口。OpenTake 不应以“核心像素链路可用”表述成与 Fusion/Premiere/FCP 的合成广度相当。
- **判定**：声明范围内像素链路 **PASS（E1+E2）**；竞品能力广度 **PARTIAL（E1+E2+O1）**。

本地证据：[playbackRoute.ts](../../../../web/src/components/preview/playbackRoute.ts)、[grade.rs](../../../../crates/opentake-domain/src/grade.rs)、[transition.rs](../../../../crates/opentake-domain/src/transition.rs)、[渲染全量测试](../editor-core-validation-evidence-a/logs/12-render-full-tests.log)。

### 3. 导入与导出

- **竞品可验证基线**：Premiere 既可直接导出，也可把多个任务交给 Media Encoder 队列并在编码时继续编辑；官方页还列出 AAF、项目整理、平台/设备预设和图像序列 [A4]。Resolve Media 页覆盖导入、元数据、同步、智能素材箱，并列出社交平台交付、广播母版和 DCP [D3]。Final Cut Pro 可导出 QuickTime/MXF、批量共享、把 Roles 输出为独立轨道，并在后台转码时继续编辑 [F3]；其官方格式表覆盖 ProRes、H.264/HEVC、AVC-Intra、MXF、无压缩和 XDCAM [F4]。CapCut Desktop 官方帮助允许配置 2K/4K、帧率和码率，最高帧率取决于源媒体和平台 [C3]。
- **OpenTake 现状**：通用导入白名单覆盖常见视频、音频和图片；文件夹导入有数量/深度/探测时限，但公开多文件导入命令仍缺少总数/总量限制和取消。全量导出支持 H.264、H.265、ProRes 422，720p/1080p/4K，MP4 AAC 或 ProRes PCM，单任务进度/取消；产物已由 ffprobe 和跨编码 SSIM 核验。工程交换支持 XMEML、FCPXML 1.10、CMX3600 EDL、OTIO JSON，字幕支持 SRT/VTT。
- **差异性质**：当前是“小而闭环”的本地导出器；后台队列、多目标发布、角色/分轨母版、图像序列和更广专业格式属于能力缺口，不是核心剪辑正确性的失败。
- **判定**：已声明格式闭环 **PASS（E1+E2）**；专业交付广度 **PARTIAL（E1+E2+O1）**。

本地证据：[session.rs](../../../../crates/opentake-core/src/session.rs)、[export.rs](../../../../src-tauri/src/export.rs)、[导出集成日志](../editor-core-validation-evidence-a/logs/13-export-integration.log)、[ffprobe 产物证据](../editor-core-validation-evidence-a/logs/15-export-artifact-ffprobe.log)、[跨编码 SSIM](../editor-core-validation-evidence-a/logs/16-export-codec-ssim.log)、[工程交换测试](../editor-core-validation-evidence-a/logs/17-project-interchange-tests.log)。

### 4. 资源加载

- **竞品可验证基线**：Premiere 提供可定位、清除并重新生成的媒体缓存 [A2]；Resolve 的代理工作流支持监看文件夹、自动关联及在共享/云场景保留本地媒体副本 [D1]；Final Cut Pro 允许生成、切换、删除并重新生成优化媒体、代理和临时渲染文件 [F1][F2]。这些资料证明竞品把派生媒体生命周期作为显式用户能力，但不能仅据官方页面比较 CPU/RSS 或声称其绝不过载。
- **OpenTake 现状**：预热主队列 64/3 workers、低优先级队列 8/1 worker；网格缩略图并发 4、LRU 256；asset 协议并发 4、待处理 32，并有 5 秒和响应体限制。反向证据是音频波形卡片随全部项目直接调用 `get_waveform`，没有共享并发上限、pending key 合并或后端取消；搜索 Moments/Spoken、预览 poster 也有独立解码路径。资产 scope 精确授权当前项目文件，但持久 scope 未随切换/退出撤销，旧授权会累积。
- **差异性质**：这是当前最明确的工程缺陷，不应被“若干局部队列有界”掩盖。首先需要统一派生媒体调度器与生命周期，再讨论与竞品缓存 UI 的体验差距。
- **判定**：**FAIL（E1+E2）**。

本地证据：[资源加载审计 README](../editor-core-validation-evidence-b/README.md)、[命令台账](../editor-core-validation-evidence-b/command-ledger.md)、[prewarm.rs](../../../../src-tauri/src/media/prewarm.rs)、[storage.rs](../../../../src-tauri/src/storage.rs)。

### 5. 产品定位与主要差异

| 参照产品 | 官方资料体现的主工作流 | OpenTake 的主要区别 | 定性 |
|---|---|---|---|
| Premiere Pro | 传统专业 NLE；代理/媒体缓存、可定制界面、Media Encoder 队列、Team Projects/Productions 项目锁和共享素材 [A1][A2][A4][A5]。 | 本地项目、单一内置导出任务、轻量媒体管理；项目内 Agent 是一级工作区。 | Agent-first/local-first 是定位差异；后台队列、共享媒体与项目锁是能力缺口。 |
| DaVinci Resolve | Media/Edit/Fusion 等任务页；节点合成；Proxy Generator；多人同项目、时间线比较、锁、聊天和云媒体 [D1][D2][D3][D4]。 | 单工作台多面板，合成能力刻意收窄，不拆成调色/VFX/音频专门页面。 | 一体化轻量工作台是定位差异；高级 VFX/调色/音频和多人协作是能力缺口。 |
| Final Cut Pro | Libraries/Browser/Viewer/Magnetic Timeline，连接片段和 Roles；后台优化媒体、渲染与共享 [F1][F2][F5]。 | 传统轨道时间线、跨平台 Rust/Tauri，而非 macOS 专属的无轨磁性时间线。 | 轨道模型与跨平台是架构/定位差异；Roles 分轨交付及同等级后台媒体管理是能力缺口。 |
| CapCut | 素材类别 + 画布/时间线 + 右侧属性；代理、4K 导出、关键帧；Space 提供成员权限、单编辑者交接及审阅评论 [C1][C3][C4][C5][C6]。 | 顶部素材分类接近 CapCut，但项目格式、交换格式和本地 Agent 更偏“可审计专业工具”；部分素材分类仍为占位。 | 类别式素材入口和 AI 辅助是相近定位；模板/内容生态、云 Space 和审阅链路是能力缺口。 |

OpenTake 不是上述任一产品的等量复刻：更准确的定位是“本地优先、开源、跨平台、Agent-first 的轻量专业剪辑器”。这个定位可以合理排除云账号、模板市场等服务，但不能把播放可靠性、资源调度、导出正确性或工程交换降为可选项。

### 6. 前端 UI 工作流

- **OpenTake 已有结构**：Media、Preview、Inspector、Timeline 与可选 Agent 五区；Default/Media/Vertical 三种布局；支持面板最大化和窄窗口折叠 Agent。素材栏有 9 个 CapCut 式分类，但 Text/Sticker/Effect 为禁用占位，文字与效果入口又分散在工具栏/Inspector，造成信息架构不一致。
- **竞品对照**：Premiere 强调可定制传统工作区 [A4][A5]；Resolve 用 Media/Edit/Fusion 等专门任务页面控制复杂度 [D2][D3][D4]；Final Cut Pro 用 Libraries、Browser、Viewer、Magnetic Timeline 和 Roles 建立稳定术语 [F5]；CapCut 把常用变换/关键帧放在选中片段后的右侧面板，并以素材类别驱动发现 [C6]。
- **建议边界**：保留 OpenTake 的五区和 Agent 差异，但应统一“素材分类—创建入口—Inspector 参数”的可发现路径。字号/按钮大小、居中或靠左、单行文案、紧凑纵深与人类逻辑布局必须由独立安装包视觉审计给出 PASS/FAIL，本次代码与官方资料对照不作伪实机结论。
- **判定**：工作流拓扑 **PARTIAL（E1+O1）**；视觉统一 **BLOCKED**。

本地证据：[EditorSplit.tsx](../../../../web/src/components/shell/EditorSplit.tsx)、[MediaTabBar.tsx](../../../../web/src/components/media/MediaTabBar.tsx)、[ViewMenu.tsx](../../../../web/src/components/shell/ViewMenu.tsx)、[AgentPanel.tsx](../../../../web/src/components/agent/AgentPanel.tsx)。

## 优先级明确的能力缺口

1. **P0：统一派生资源调度**。把 waveform、search thumbnail、preview poster、sprite/prewarm 纳入共享并发、key 去重、项目 epoch 取消、容量/时间/磁盘预算；这是当前唯一有代码反证的 FAIL。
2. **P1：完成播放与代理的当前实机闭环**。用安装包重开 4K60 双轨项目，记录播放推进、暂停定位、代理切换、CPU/RSS 与诊断帧计数；未完成前保留 PARTIAL。
3. **P1：导出产品化**。增加可继续编辑的后台队列、任务恢复/失败诊断、图像序列或角色/音轨分轨母版；优先级取决于目标用户是否为长片/团队。
4. **P2：明确协作策略**。若坚持 local-first，文档中明确“不做实时多人”；若要对标专业团队，最小闭环应是媒体可用性检查、项目锁/交接、审阅链接与评论，而不是直接承诺实时同编。
5. **P2：扩充合成能力要受产品定位约束**。优先补真实项目高频的转场、变速组合和效果，再评估节点合成；不宜以竞品功能总量驱动无边界追赶。

## 官方来源登记

所有页面均于 **2026-08-08** 访问；下表“支持事实”只记录页面明确说明的功能。

### Adobe Premiere Pro

| ID | 页面标题与 URL | 支持事实 |
|---|---|---|
| A1 | [Ingest and Proxy workflow](https://helpx.adobe.com/premiere/desktop/organize-media/ingest-proxy-workflow/ingest-and-proxy-workflow.html) | 导入时复制/转码/创建代理；可附加既有代理；在原片与代理间切换。 |
| A2 | [Manage media cache](https://helpx.adobe.com/premiere/desktop/troubleshooting/media-issues/manage-media-cache.html) | 媒体缓存包含峰值、转换音频和数据库；可删除并重建，也可选择缓存位置。 |
| A3 | [Mercury Playback Engine GPU acceleration in Premiere](https://helpx.adobe.com/premiere/desktop/get-started/download-and-install/mercury-playback-engine-gpu-accelerated-in-premiere.html) | GPU 加速播放、渲染、图像处理、颜色转换和缩放；渲染/导出可利用多 GPU。 |
| A4 | [Export options in Premiere](https://helpx.adobe.com/premiere/desktop/render-and-export/export-files/export-options-in-premiere.html) | 直接导出或送入 Media Encoder 队列；队列编码时可继续编辑；支持多任务、AAF、项目整理、预设和图像序列。 |
| A5 | [When to use Team Projects and when to use Productions](https://helpx.adobe.com/premiere/desktop/collaborate-with-others/collaborate-using-team-projects/when-to-use-team-projects-and-when-to-use-productions.html) | Team Projects 支持云端同一项目多人协作；Productions 面向大型多项目、共享媒体与项目锁。 |

### DaVinci Resolve

| ID | 页面标题与 URL | 支持事实 |
|---|---|---|
| D1 | [DaVinci Resolve – Collaboration](https://www.blackmagicdesign.com/products/davinciresolve/collaboration) | 同项目多人、更新接受/比较、项目锁、聊天和云媒体；Proxy Generator 监看文件夹并生成/关联代理。 |
| D2 | [DaVinci Resolve – Edit](https://www.blackmagicdesign.com/products/davinciresolve/edit) | 独立 Edit 工作页、多机位界面、音频波形/时码同步和可定制快捷键。 |
| D3 | [DaVinci Resolve – Media](https://www.blackmagicdesign.com/products/davinciresolve/media) | Media 页导入、元数据、同步、bins/smart bins；交付覆盖社交平台、广播母版和 DCP。 |
| D4 | [DaVinci Resolve – Fusion](https://www.blackmagicdesign.com/products/davinciresolve/fusion) | 内置节点式 2D/3D 合成、跟踪、抠像、遮罩和可扩展节点树。 |

### Final Cut Pro

| ID | 页面标题与 URL | 支持事实 |
|---|---|---|
| F1 | [Create optimized and proxy files in Final Cut Pro for Mac](https://support.apple.com/en-euro/guide/final-cut-pro/verb8e5f6fd/mac) | 导入时或之后创建优化媒体/代理；后台处理；保留原片；可删除、重建和切换。 |
| F2 | [Background rendering in Final Cut Pro for Mac](https://support.apple.com/en-euro/guide/final-cut-pro/ver717f3ca3/mac) | 为不能实时播放的效果、转场和标题生成临时渲染文件；可配置启动时机并删除文件。 |
| F3 | [Export final mastering files in Final Cut Pro for Mac](https://support.apple.com/guide/final-cut-pro/export-final-mastering-files-ver0192a47b8/mac) | QuickTime/MXF、范围与批量共享、Roles 独立轨道；后台转码期间继续编辑。 |
| F4 | [Export formats supported in Final Cut Pro for Mac](https://support.apple.com/en-ke/guide/final-cut-pro/vere9bbb1cc4/mac) | ProRes、AVC-Intra、H.264/HEVC、MXF、无压缩和 XDCAM 等导出格式。 |
| F5 | [Final Cut Pro for Mac interface](https://support.apple.com/guide/final-cut-pro/final-cut-pro-interface-ver92bd100a/mac) | Libraries/Browser/Viewer/Magnetic Timeline；连接片段、自动闭合空隙和 Roles/音频 lanes。 |

### CapCut

| ID | 页面标题与 URL | 支持事实 |
|---|---|---|
| C1 | [Why Does the Video Quality Change After Exporting?](https://www.capcut.com/help/video-quality-change-after-exporting) | Desktop 设置存在 Proxy；代理降低编辑预览质量，但最终导出由导出设置决定。 |
| C3 | [How Do I Export 2K/4K Videos in CapCut?](https://www.capcut.com/help/export-videos-in-capcut) | Desktop 可选 2K/4K、帧率和码率；可用分辨率/帧率受源媒体、设备和平台影响。 |
| C4 | [Changes to the Team Collaboration Feature](https://www.capcut.com/help/team-collaboration-feature) | 团队身份、邀请和权限在云端集中管理；Desktop 可打开和编辑团队项目。 |
| C5 | [Collaborate on CapCut: Edit, Share and store content with your team](https://www.capcut.com/resource/collaborate-on-capcut-online) | Space 成员/权限、单编辑者交接、其他成员只读，以及审阅链接和评论。 |
| C6 | [Why Can't I See Keyframes in CapCut on PC?](https://www.capcut.com/help/keyframes-in-capcut-pc) | 片段选中后在右侧面板使用位置、缩放、旋转和不透明度关键帧。 |

## 限制

- 未安装或运行四款竞品，官方页面只能证明功能存在，不能形成速度、稳定性、CPU/RSS 或画质优劣结论。
- CapCut 能力随 Desktop/Web/Mobile、区域和账号变化；仅采用上述页面明确标注的平台事实，不外推到所有客户端。
- OpenTake 当前 E2 来自仓库内日志/产物；本次只读子任务未重跑测试，也未取得安装包 E3，因此播放、代理和视觉 UI 不提升为完整 PASS。
- “无实时多人协作”既可能是 local-first 的定位边界，也可能是团队市场缺口；是否进入发布门槛必须由产品目标决定，不能仅凭竞品功能表决定。
