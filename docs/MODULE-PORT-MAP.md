# OpenTake 模块移植地图

> 由 20 个 max-思考子 Agent 对 palmier-pro-upstream 逐模块拆解生成。verdict 含义:direct-port=Rust直写 / needs-replacement=换跨平台库 / ui-rebuild=React重建 / cloud-rebuild=自建后端。
> 注: 这是迁移地图与历史对照文档。当前 OpenTake 已将媒体 IO 定位为 `ffmpeg-sidecar`,语义搜索定位为 `ort + SigLIP2 + tokenizers`;文中出现的 `ffmpeg-sidecar` / `candle` 多数是在保留上游迁移语境。

## 总览

| 模块 | 分层 | 移植判定 | 一句话职责 |
|---|---|---|---|
| **Models** | core-domain | direct-port | PalmierPro 的领域模型层(纯数据 + 少量渲染/元数据辅助)。定义视频编辑器的核心可序列化数据结构:时间线/轨 |
| **Project** | mixed | needs-replacement | 这是 PalmierPro 的「工程文件格式 + 主屏启动器 + 示例工程下载」层。它定义了 .palmier 工程包( |
| **Editor** | mixed | needs-replacement | PalmierPro 编辑器模块,是整个 AI 视频编辑器的"编辑领域核心 + 编辑器 UI 外壳"。它围绕一个巨型 @ |
| **Timeline** | ui | ui-rebuild | PalmierPro 的时间线 UI 与交互层：基于 AppKit 自绘(NSView + CGContext)渲染多轨 |
| **Preview** | mixed | needs-replacement | Preview 是 PalmierPro 的"预览/合成"子系统:把领域模型 Timeline(轨道/片段/关键帧)实时 |
| **Export** | mixed | needs-replacement | 导出子系统，把内存中的 Timeline 落地为三种产物：(1) 渲染好的视频文件 (.mp4 H.264/H.265  |
| **Generation** | mixed | cloud-rebuild | PalmierPro 的「生成式 AI」子系统:把文/图/音/视频生成请求、AI 二次编辑(放大 Upscale、重跑  |
| **Agent** | mixed | needs-replacement | PalmierPro 的 AI 智能体子系统：把自然语言剪辑意图翻译成对编辑器领域模型(Timeline/Clip)的工 |
| **MediaPanel** | mixed | ui-rebuild | 左侧停靠面板，承载三个标签页：Media（媒体素材库浏览器：文件夹/扁平/分组三种视图、搜索、拖拽、选区、导入、AI 整 |
| **Inspector** | ui | ui-rebuild | Inspector 是 PalmierPro 右侧的属性检查器面板：根据当前选区（单/多 clip、纯文字 clip、媒 |
| **Account** | cloud-client | cloud-rebuild | 账户/订阅/计费/鉴权的云客户端层。负责 Google OAuth 登录(Clerk)、把账户信息从 Convex 后端 |
| **Search** | engine | needs-replacement | 完全本地 (on-device) 的语义化媒体搜索子系统:用 SigLIP2(CLIP 风格的图文双编码器,经 Core |
| **Settings** | ui | ui-rebuild | Palmier Pro 的"设置"窗口模块:一个独立 NSWindow,左侧侧边栏 + 右侧详情,承载 5 个分页(Ac |
| **Help** | ui | ui-rebuild | 应用内"帮助/支持"模块,纯展示型 UI。提供三块内容:(1) 键盘快捷键速查表(静态硬编码),(2) MCP serv |
| **App** | infra | needs-replacement | 这是 PalmierPro(AI 原生 macOS 视频编辑器)的"应用外壳/引导层"。它负责进程启动与依赖装配(日志、 |
| **Utilities** | infra | needs-replacement | PalmierPro 的通用基础设施工具集,集中放置不属于任何具体业务域的横切能力:有界并发信号量、磁盘缓存目录管理、图 |
| **UI** | ui | ui-rebuild | PalmierPro 的共享设计系统与通用 SwiftUI/AppKit 表现层组件目录。它集中定义全局设计令牌(颜色/ |
| **Transcription** | engine | needs-replacement | 封装"音频/视频 → 文字稿"的全链路：从媒体里抽取音频轨、调用 Apple 设备端语音识别(macOS 26 新 Sp |
| **Telemetry** | infra | needs-replacement | 对 Sentry Cocoa SDK 的一层极薄静态封装,负责崩溃/错误/异常上报、面包屑(breadcrumb)日志、 |
| **Toolbar** | ui | ui-rebuild | Toolbar 是编辑器顶部的工具栏 UI 条，提供撤销/重做、指针/剃刀工具模式切换、在播放头处分割、把入点/出点裁到 |

---

## 模块详情

- [Models (core-domain)](port-map/models.md): PalmierPro 的领域模型层(纯数据 + 少量渲染/元数据辅助)。定义视频编辑器的核心可序列化数据结构:时间线/轨
- [Project (mixed)](port-map/project.md): 这是 PalmierPro 的「工程文件格式 + 主屏启动器 + 示例工程下载」层。它定义了 .palmier 工程包(
- [Editor (mixed)](port-map/editor.md): PalmierPro 编辑器模块,是整个 AI 视频编辑器的"编辑领域核心 + 编辑器 UI 外壳"。它围绕一个巨型 @
- [Timeline (ui)](port-map/timeline.md): PalmierPro 的时间线 UI 与交互层：基于 AppKit 自绘(NSView + CGContext)渲染多轨
- [Preview (mixed)](port-map/preview.md): Preview 是 PalmierPro 的"预览/合成"子系统:把领域模型 Timeline(轨道/片段/关键帧)实时
- [Export (mixed)](port-map/export.md): 导出子系统，把内存中的 Timeline 落地为三种产物：(1) 渲染好的视频文件 (.mp4 H.264/H.265
- [Generation (mixed)](port-map/generation.md): PalmierPro 的「生成式 AI」子系统:把文/图/音/视频生成请求、AI 二次编辑(放大 Upscale、重跑
- [Agent (mixed)](port-map/agent.md): PalmierPro 的 AI 智能体子系统：把自然语言剪辑意图翻译成对编辑器领域模型(Timeline/Clip)的工
- [MediaPanel (mixed)](port-map/mediapanel.md): 左侧停靠面板，承载三个标签页：Media（媒体素材库浏览器：文件夹/扁平/分组三种视图、搜索、拖拽、选区、导入、AI 整
- [Inspector (ui)](port-map/inspector.md): Inspector 是 PalmierPro 右侧的属性检查器面板：根据当前选区（单/多 clip、纯文字 clip、媒
- [Account (cloud-client)](port-map/account.md): 账户/订阅/计费/鉴权的云客户端层。负责 Google OAuth 登录(Clerk)、把账户信息从 Convex 后端
- [Search (engine)](port-map/search.md): 完全本地 (on-device) 的语义化媒体搜索子系统:用 SigLIP2(CLIP 风格的图文双编码器,经 Core
- [Settings (ui)](port-map/settings.md): Palmier Pro 的"设置"窗口模块:一个独立 NSWindow,左侧侧边栏 + 右侧详情,承载 5 个分页(Ac
- [Help (ui)](port-map/help.md): 应用内"帮助/支持"模块,纯展示型 UI。提供三块内容:(1) 键盘快捷键速查表(静态硬编码),(2) MCP serv
- [App (infra)](port-map/app.md): 这是 PalmierPro(AI 原生 macOS 视频编辑器)的"应用外壳/引导层"。它负责进程启动与依赖装配(日志、
- [Utilities (infra)](port-map/utilities.md): PalmierPro 的通用基础设施工具集,集中放置不属于任何具体业务域的横切能力:有界并发信号量、磁盘缓存目录管理、图
- [UI (ui)](port-map/ui.md): PalmierPro 的共享设计系统与通用 SwiftUI/AppKit 表现层组件目录。它集中定义全局设计令牌(颜色/
- [Transcription (engine)](port-map/transcription.md): 封装"音频/视频 → 文字稿"的全链路：从媒体里抽取音频轨、调用 Apple 设备端语音识别(macOS 26 新 Sp
- [Telemetry (infra)](port-map/telemetry.md): 对 Sentry Cocoa SDK 的一层极薄静态封装,负责崩溃/错误/异常上报、面包屑(breadcrumb)日志、
- [Toolbar (ui)](port-map/toolbar.md): Toolbar 是编辑器顶部的工具栏 UI 条，提供撤销/重做、指针/剃刀工具模式切换、在播放头处分割、把入点/出点裁到
