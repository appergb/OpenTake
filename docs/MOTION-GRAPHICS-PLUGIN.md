# 提案:Web 动效模块 / 插件(Agent 用 HTML/CSS/JS 编写动画)

> 状态:**设计提案(后期实现)**。
> 动机:让 Agent 直接用 Web 技术(HTML/CSS/JS,类似 Emotion 的 CSS-in-JS 写法)创作动态图形/动画,
> 由 OpenTake 确定性渲染成带透明通道的片段落到时间线。
> 这恰好补上上游 Palmier Pro 自述「尚无:图形(Graphics)」的空白,且是 AI 原生编辑器独有的杀手锏。

## 1. 为什么用 Web 技术写动画

- **LLM 最擅长写 HTML/CSS/JS**:动效用声明式 Web 代码表达,Agent 生成/迭代成本极低,远比让它操作二进制特效参数自然。
- **生态成熟**:可直接用 GSAP / Web Animations API / anime.js / Lottie-web / CSS keyframes / SVG SMIL,无需自研动画 DSL。
- **可读可改、可版本化**:动效的「源」就是一段文本代码,存进工程、content-hash 缓存,改了再重渲,天然适合 Agent 的可撤销/可编辑工作流。
- **先例**:Remotion(用 React 做视频)已验证「Web 技术 + 无头浏览器确定性渲染」这条路可行。本提案是其「Agent 主导 + 作为编辑器内一个图层源」的形态。

## 2. 在架构中的位置

新增 crate **`opentake-motion`**(渲染器 + 插件宿主),与既有系统对接:
- **作为一个片段源**接入 `opentake-render`:与图片/文字/Lottie 的「物化成纹理」走同一条路(见 ARCHITECTURE §6 媒体物化策略)。动效渲成 RGBA(带 alpha)帧序列 → content-hash 缓存 → 喂 wgpu 合成器当一层。**对合成器而言它只是又一个纹理来源,零特殊处理。**
- **暴露 MCP 工具**(`opentake-agent`,遵守「单一能力层、多前端」):`add_motion_graphic` / `edit_motion_graphic`。
- **domain**:`MediaSource` 增加 `Motion { code_ref, params }` 形态(或作为一种带源代码的生成媒体),`#[serde(default)]` 兼容。

```
Agent 写 HTML/CSS/JS ──▶ opentake-motion ──▶ 确定性逐帧渲染(无头 Web 引擎)
                                              └▶ RGBA 帧序列(带 alpha)+ content-hash 缓存
                                                    └▶ opentake-render(wgpu 合成器)当一层叠加 ──▶ 预览/导出
```

## 3. 确定性渲染(核心难点)

视频要求**每一帧可复现、预览=导出**,而 Web 动画默认依赖真实时钟。解决:
- **无头 Chromium + CDP**:用 `Emulation.setVirtualTimePolicy`(虚拟时间)把时间冻结,按 `t = frame / fps` 逐帧推进,`Page.captureScreenshot` 抓每帧(带透明背景)。这是 Remotion 等采用的成熟做法。
- **渲染合约**:动效文档需可被「按时间 seek」。约定一个全局 `OpenTake.seek(seconds)` 或基于 `document.timeline.currentTime`/CSS `animation-delay` 暂停帧步进;OpenTake 注入确定性时钟。
- **透明通道**:截图启用 alpha(`Page.captureScreenshot{format:png, captureBeyondViewport, ...}` + 透明 body 背景),得到可叠加的 overlay。
- 渲染引擎候选:无头 Chromium(`chromiumoxide`/CDP,确定性最佳)为主;轻量场景可探索 servo/纯 Rust Web 引擎,但当前成熟度以 Chromium 为准。Tauri 自带的 wry webview 用于**交互预览**可以,但**确定性导出走无头 Chromium**。

## 4. 插件形态(社区可扩展)

两种使用层级:
1. **即兴模式**:Agent 直接写一段自包含 HTML/CSS/JS 动效(一次性图形,如片头标题、数据动画、下三分之一)。
2. **模板/插件模式**:把动效封装成「动效模板插件」=`Web 包 + manifest`(声明 name、参数 schema、时长模型、fps)。Agent 用参数实例化已注册模板;社区可贡献模板库。

插件 manifest 草案:
```jsonc
{
  "id": "lower-third.glass",
  "name": "玻璃拟态下三分之一",
  "entry": "index.html",
  "params": { "title": {"type":"string"}, "subtitle": {"type":"string"}, "accent": {"type":"color"} },
  "duration": { "mode": "fixed|driven", "default": 5.0 },
  "fps": "inherit",
  "transparent": true
}
```

## 5. 安全沙箱(必须)

动效代码(尤其社区插件/Agent 生成)在隔离环境渲染:
- 离屏无头实例,**默认禁网**(或显式 allowlist,SRI),CSP 限制。
- 资源/时长上限,渲染超时熔断。
- 不接触用户文件系统/工程数据,除非经声明的参数注入。
- 与 web/security 规则(nonce CSP、禁 `unsafe-inline`、第三方脚本审计)一致。

## 6. MCP 工具草案(OpenTake 增强能力)

```
add_motion_graphic {
  source: { code: "<html/css/js>" } | { template_id, params },
  start_frame, duration_frames,
  transparent: bool, track_index?
}  // 渲染 → 物化 → 落轨,单步可撤销;返回 clip id

edit_motion_graphic { clip_id, code? , params? }  // 改源/参数 → 重渲(content-hash 失效则重算)
```

## 7. 与进阶能力的关系

- 属于 [ADVANCED-FEATURES.md](ADVANCED-FEATURES.md) 之外的**独立模块**(A 层是合成器内的着色器特效,本提案是「外部 Web 引擎产出纹理源」)。
- 与「图文成片」(E 层)协同:成片流程里 Agent 可调 `add_motion_graphic` 生成片头/字幕条/转场图形,而非只拼现成素材。
- 引入「插件系统」概念后,特效模板、转场模板、LUT 包等也可走同一套插件分发机制(后续统一)。

## 8. 落地时机

属**后期**能力,前置条件:wgpu 合成器(Phase 3)与媒体物化链路就绪后接入最自然。建议作为 **Phase 10(新)· Web 动效模块与插件系统** 单列,不阻塞核心编辑/媒体管线。
