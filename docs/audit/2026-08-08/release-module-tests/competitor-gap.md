# 1.0.0-beta.2 竞品能力差异发布前独立核验

- 日期：2026-08-08（Asia/Shanghai）
- 范围：Adobe Premiere Pro、DaVinci Resolve、Final Cut Pro、CapCut 的协作、合成/VFX、时间线/角色、代理/团队能力
- 审计 HEAD：`f9398a9302e57690caa3ea21ff995fdd0ce97706`；工作树另有其他 Agent 的未提交改动
- OpenTake 证据：当前工作树、`README.md`、`docs/architecture/ROADMAP.md`、`docs/audit/2026-08-07/editor-core-validation.md` 及既有竞品资料登记
- 外部证据：仅使用 Adobe、Blackmagic Design、Apple、CapCut 官方产品页或帮助文档；所有链接于本日重新打开
- 约束：只读核验；未安装或运行四款竞品，未修改产品代码/CI，未提交或推送

## 最终裁决

**PASS（竞品事实、链接有效性与缺陷/能力差距分类）**。

既有 `competitor-official-sources.md` 登记的 **19/19** 个官方链接当前均可访问，核心结论均由页面正文支持；没有发现把竞品能力缺失误报为 OpenTake 当前剪辑核心 bug 的发布阻断项。另补充 5 个官方页面，用于把 Premiere 传统轨道、Final Cut Pro Roles/共享存储、CapCut 特效与模板生态的边界说得更精确。

本报告的 PASS 只表示“差异描述准确、分类合规”，**不表示 OpenTake 与四款竞品功能等量**。团队协作、Fusion 节点合成、Final Cut Pro Roles/媒体分轨、CapCut 模板生态、后台编码队列和更完整的代理/缓存管理都是产品能力差距；它们不是对 OpenTake 已声明编辑核心的失败测试。

## 严格分类

### 确认缺陷

本子模块 **未确认新的 OpenTake 产品缺陷**。

“确认缺陷”必须来自 OpenTake 当前代码、可重复测试、真实 GUI 或产物反证；不能仅因竞品有某功能就成立。编辑核心总报告冻结基线曾确认的 asset、资源调度与可访问性缺陷，已在其第 9 节完成 RED→GREEN、全量门禁、正式包与 GUI 复验。本次竞品对照不重新打开这些已闭环 finding，也不以竞品宣传页替代产品测试。

### 产品能力差距

| 维度 | OpenTake 当前边界 | 竞品官方资料支持的更宽能力 | 分类 |
|---|---|---|---|
| 团队协作 | 本地单用户 `.opentake` 工程；当前产品代码中未发现多人项目、项目/时间线锁、云同步、审阅链接或评论工作流。 | Premiere Team Projects/Productions；Resolve 同项目多人、锁、比较、聊天与云媒体；CapCut Space 的权限、单编辑者交接和审阅评论；FCP 可把 library/外部媒体放在共享存储，但官方页不等同证明原生同时同编。 | **能力差距，不是剪辑核心 bug** |
| 合成/VFX | 预览与导出共享 `RenderPlan`/wgpu；声明范围已验证。当前通用效果注册为 grayscale/sepia/invert，转场为 cross-dissolve，每 clip 最多 4 个 mask；不支持组合 fail closed。 | Resolve Fusion 是节点式 2D/3D 合成，含 200+ tools、跟踪、抠像、遮罩/roto、粒子和模板；Premiere/FCP/CapCut 也有更宽效果、遮罩、跟踪、转场或模板面。 | **能力广度差距，不是当前像素链 bug** |
| 时间线/角色 | 整数帧、传统视频/音频轨和 Rust 权威 Timeline。OpenTake 的 `TrackRole` 是发给 Agent 的 Context Signal（MainCamera/B_Roll/BGM 等），不是交付元数据系统。 | Premiere 使用传统 V/A tracks 与 track targeting；Resolve 按 Media/Edit/Fusion 等任务页组织；FCP 使用 Magnetic Timeline、connected clips、Roles、audio lanes 和 role-based media stems；CapCut 使用选中时间线 clip 后的右侧属性/关键帧面板。 | **架构选择 + Roles 交付差距，不是轨道算法 bug** |
| 代理/缓存 | 每素材手动生成项目内 720p H.264 proxy，single-flight、可取消/移除；播放可优先代理，导出只读原片。 | Premiere 有 ingest/attach/switch proxy 与可清理 media cache；Resolve Proxy Generator 可监看目录并自动生成/关联；FCP 有后台优化媒体、代理、render files 的生成/切换/删除/重建；CapCut PC 有 Proxy 预览优化。 | **产品化与生命周期管理差距，不是已验证 proxy 正确性 bug** |
| 后台交付/生态 | 单导出任务；本地 Motion Canvas 固定竖切与有限效果/模板面。 | Premiere Media Encoder 多任务队列；FCP 后台 share 与 Roles stems；CapCut 模板/效果/Space；Resolve 专业母版、DCP 与云审阅。 | **工作流/生态差距，不是导出主链 bug** |

## 四款产品逐项复核

### Adobe Premiere Pro

- **协作/团队**：官方 Team Projects 页面明确同一共享项目、云端更新和 timeline collaboration；Productions 面向大型项目，提供共享媒体引用、bin locking、可见性与 timeline handoff。既有结论成立。
- **合成/VFX**：Mercury Playback Engine 页面支持 GPU 加速图像处理、颜色转换、缩放、播放、渲染和导出；帮助目录同时明确效果、keying、mask tracking、blend/composite 与 After Effects Dynamic Link。OpenTake 不应宣称与 Premiere/After Effects 工作流广度相当。
- **时间线**：当前官方 track targeting 页面明确 V1/A1 等轨道可分别或同时成为编辑目标。OpenTake 同样是传统轨道模型，但没有 Premiere 的团队层和同等效果/交付广度。
- **代理**：官方正文明确 ingest 可复制/转码，proxy 用轻量文件编辑，并可在最终导出前切回原片；media cache 页面明确 `.pek`、`.cfa` 和数据库的清理、重建与位置管理。
- **分类**：Team Projects/Productions、Media Encoder 队列与更完整缓存是能力差距；不是 OpenTake split/ripple/render/export 正确性缺陷。

### DaVinci Resolve

- **协作/团队**：当前官方页面已是 DaVinci Resolve 21，仍明确支持多人同项目、更新接受/比较、bin/timeline locking、内置聊天、共享标记、云媒体和 Presentations 审阅。
- **合成/VFX**：Fusion 页面明确 node tree、2D/3D merge、200+ tools、mask/roto、2D/planar/3D tracking、Delta Keyer、粒子、模板和脚本。它是专门的 VFX/动效工作区，不是 OpenTake 当前 compositor 的验收基线。
- **时间线/工作台**：Resolve 以 Media/Edit/Cut/Color/Fusion/Fairlight 等专页拆分复杂工作流；OpenTake 的单工作台是定位选择。
- **代理**：Proxy Generator 可监看一个或多个目录，自动生成 H.264/H.265/ProRes proxies 并与原片关联；与 Cloud media sync 组合后形成团队代理工作流。
- **分类**：Fusion、专业调色/音频专页和多人协作均为能力差距；不能据此把 OpenTake 声明范围内已通过的多轨合成写成 FAIL。

### Final Cut Pro

- **协作/团队**：Apple 官方资料支持把 libraries 放在 SAN/NFS/SMB 共享存储，也支持多用户访问 shared storage 上的 external media；这些页面没有证明 Resolve/Premiere 式原生同时同编、merge、lock 或 review。发布对比必须保留这一层级差异。
- **合成/VFX**：官方用户指南列出 keying、complex masks、object tracking、compositing/blend modes、效果与 Motion 扩展。OpenTake 当前的通用效果/转场数量明显更少，但这属于广度。
- **时间线/Roles**：Magnetic Timeline 会在移动片段后自动闭合空隙；Roles 是 clip 元数据，可组织 audio lanes 并导出独立或多轨 media stems。OpenTake 的传统轨道和 Agent `TrackRole` 不等价于 FCP Roles。
- **代理**：FCP 可在导入时或之后创建 ProRes Proxy/H.264 proxy 与 optimized media，在后台执行，允许选择、删除、重建；官方明确共享前应切回 optimized/original media。
- **分类**：磁性时间线是架构差异；Roles/stems 与完整后台媒体管理是能力差距；不能写成 OpenTake 当前轨道编辑 bug。FCP 的共享存储能力也不应夸大成实时多人协作。

### CapCut

- **协作/团队**：官方帮助页明确 team identity/permissions 在云端管理，Desktop 能打开和编辑 team workspace 项目；协作正文同时明确同一视频一次只有一名编辑者，其他成员只读，可交接编辑权并通过 review link/comment 反馈。它不是多人同时改同一时间线。
- **合成/VFX/生态**：官方页面支持 Effects、transitions、chroma key、masks、keyframes、stabilization 与模板库。与 Fusion 的节点合成不是同一层级，但其模板/内容发现和社交发布生态明显宽于 OpenTake。
- **时间线**：PC 帮助页明确选中 timeline 中的视频、图片或文字 clip 后，在右侧 Video/Transform/Adjust 面板对 position、scale、opacity 加关键帧。
- **代理**：PC 设置有 Proxy；官方明确它改善播放性能、可能降低预览质量，但最终导出由 export 设置决定。
- **分类**：Space、审阅、模板/内容生态是产品能力差距；CapCut 的单编辑者交接也不能被描述成“实时多人同编”。

## 既有 19 个链接有效性与支持度

| ID | 当前结果 | 支持度复核 |
|---|---|---|
| A1 | **200 / PASS** | ingest copy/transcode、proxy edit、最终切回原片均见正文。 |
| A2 | **200 / PASS** | `.pek`/`.cfa`、cache database、删除重建和位置管理见正文。 |
| A3 | **200 / PASS** | GPU 图像处理/颜色转换/缩放、单 GPU 播放和多 GPU render/export 见正文。 |
| A4 | **200 / PASS** | direct/Media Encoder、继续编辑、多任务、AAF、项目精简、图像序列和 presets 见正文。 |
| A5 | **200 / PASS** | Team Projects 多用户云项目；Productions 共享媒体、bin lock 与 handoff 见正文。 |
| D1 | **200 / PASS** | Resolve 21 同项目多人、锁、聊天、比较、云媒体及 Proxy Generator 见正文。 |
| D2 | **200 / PASS** | Edit 专页和专业编辑工作流存在；与 D1/D3/D4 共同支持多专页拓扑。 |
| D3 | **200 / PASS** | Media 专页、metadata、sync、smart bins、社交/广播/DCP 见正文。 |
| D4 | **200 / PASS** | Fusion node tree、2D/3D、tracking、keying、mask/roto 与大型工具面见正文。 |
| F1 | **200 / PASS** | optimized/proxy 生成、后台任务、切换、删除/重建、保留原片见正文。 |
| F2 | **200 / PASS** | 不能实时播放的区段生成临时 render files，后台启动与删除管理见正文。 |
| F3 | **200 / PASS** | QuickTime/MXF、Roles tracks、后台 transcode/继续编辑见正文。 |
| F4 | **200 / PASS** | ProRes、H.264/HEVC、MXF、AVC-Intra、XDCAM 等导出格式见正文。 |
| F5 | **200/locale redirect / PASS** | Libraries/Browser/Viewer、Magnetic Timeline、connected clips、Roles/audio lanes 见正文。 |
| C1 | **200 / PASS** | CapCut PC Proxy 的预览性能/质量边界见正文。 |
| C3 | **200 / PASS** | Desktop 2K/4K、最高 60 fps、bitrate 与设备/源限制见正文。 |
| C4 | **200 / PASS** | 云端 team identity/permissions、Desktop team project 能力和在线治理边界见正文。 |
| C5 | **200 / PASS** | Space、权限交接、单编辑者/其他人只读、review/comment 见正文。 |
| C6 | **200 / PASS** | timeline clip 选择、右侧 panel、position/scale/opacity keyframes 见正文。 |

> 注：这里的 `200` 表示本次浏览工具成功取得官方页面正文，不是长期可用性保证；外部文档仍可能在发版后移动。

## 补充官方来源

| ID | 官方页面 | 用途 |
|---|---|---|
| A6 | [Work with clips on the timeline using Track Targeting](https://helpx.adobe.com/premiere/desktop/edit-projects/intro-to-editing/work-with-clips-on-the-timeline-using-track-targeting.html) | 证明 Premiere 的 V/A 传统轨道与 track targeting。 |
| F6 | [Organize clips by roles in Final Cut Pro for Mac](https://support.apple.com/guide/final-cut-pro/organize-clips-by-roles-verdbd59f7/mac) | 证明 Roles 是元数据，可组织 timeline 并导出 media stems。 |
| F7 | [Use shared storage with Final Cut Pro](https://support.apple.com/en-la/101919) | 证明 SAN/NFS/SMB shared storage；不外推为同时同编。 |
| C7 | [CapCut Effects Download](https://www.capcut.com/resource/capcut-effects-download) | 证明 Desktop effects、chroma key、masks、keyframes、stabilization 等创作面。 |
| C8 | [How to Use Templates in CapCut](https://www.capcut.com/help/use-template) | 证明 Web/PC/Mobile 的模板入口与平台差异。 |

既有 A1–C6 的完整 URL 登记见 `docs/audit/2026-08-07/editor-core-validation-assets/competitor-official-sources.md`。

## Release notes 必须披露的边界

1. **定位**：OpenTake 是本地优先、开源、跨平台、Agent-first 的轻量专业剪辑器；不是 Premiere、Resolve、Final Cut Pro 或 CapCut 的等量替代。README 已采用这一口径，应保持。
2. **单用户**：Beta 2 不提供多人实时同编、项目/时间线锁、云 Space、审阅链接或评论。不要笼统写“团队协作”；若提及 FCP，只能说共享存储/多编辑者访问媒体，不宣称原生同时同编。
3. **CapCut 协作精度**：CapCut 官方当前口径是单编辑者权限交接、其他人只读与审阅评论，不是多人同时修改同一视频。
4. **合成边界**：可声明预览与导出共享 RenderPlan、已验证的多轨/变换/裁剪/透明度/文字/调色/LUT/色键/稳定/最多 4 masks/cross-dissolve；不得写成 Fusion 级节点式 VFX 或与竞品效果数量相当。未支持组合 fail closed。
5. **时间线/Roles**：OpenTake 使用传统整数帧轨道。Agent Context Signal 的 `TrackRole` 只指导 Agent，不是 Final Cut Pro Roles、audio lanes 或 role-based media stems；发布文案不得混用。
6. **代理**：可声明手动项目内 720p H.264 proxy、创建/取消/移除、播放优先代理且导出使用原片；不得宣称 watch-folder 自动代理、统一 optimized media/render cache 管理或 Premiere/FCP 同等级后台媒体体系。
7. **交付与生态**：当前为单导出任务和有限本地模板/效果面；没有 Media Encoder 式多任务后台队列、CapCut 模板市场/Space、Resolve 云审阅或 FCP Roles 分轨母版。
8. **缺陷分类**：上述能力差距可进入路线图，但不得进入 Beta 2 当前剪辑核心 bug 清单；实际可靠性缺陷仍须由 OpenTake 自身 E1–E4 证据确认。

## 限制

- 未安装或运行四款竞品，因此不能比较稳定性、吞吐、CPU/RSS、画质、易用性或崩溃率。
- 官方产品页能证明功能存在，不能证明所有计划、账号、区域、客户端和版本都可用；CapCut 尤其依赖 Web/Desktop/Mobile、区域与订阅。
- “仓库未发现协作实现”是基于当前产品源码和文档的只读检索，不是对未来路线图的否定。
- 当前共享工作树存在其他 Agent 的未提交改动；本报告只做事实核验，不作为 exact-SHA 产品测试或 Windows/GUI 发布凭证。

## Blocker

- **本模块无发布 blocker**。
- 仅要求 release notes 按上节披露边界，避免把 Agent `TrackRole` 写成 FCP Roles、把 CapCut 写成实时多人同编，或把 OpenTake 声明范围内 compositor 写成 Fusion 对等能力。
