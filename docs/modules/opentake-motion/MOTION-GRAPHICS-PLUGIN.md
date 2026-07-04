# Motion Canvas 外部动效 / AI Video 插件规划

> 状态:设计规划已切换到 **Motion Canvas 优先**。已有 `opentake-motion` scaffold 作为后续原生 frame-cache / alpha fallback,不再作为 v1 主渲染器。
> Issue trail:#34 motion dispatch / motion graphics;已认领实现切片。
> 决策:先 fork / vendor Motion Canvas(MIT)做独立外部模块或内置插件,让它产出可导入的视频文件;OpenTake 负责一站式导入、落轨、预览和导出。

## 0. 方向修正

原计划是让 Agent 直接输出 CSS / HTML / JS,再由 OpenTake 自己实现无头 Chromium + CDP 逐帧渲染。现在改为:

1. **主线:Motion Canvas 插件**
   - Motion Canvas 是 TypeScript + TSX 的程序化动画系统,适合 AI 生成讲解动画、标题卡、数据图形、流程图和片段级视频。
   - Motion Canvas 许可证为 MIT,可 fork、修改、打包,但必须保留 copyright 和 license notice。
   - 插件输出 `mp4` 或图片序列;OpenTake 先按普通 media asset 导入并放入时间线。

2. **后续:OpenTake native motion**
   - `crates/opentake-motion` 现有 scaffold 保留,负责后续透明 overlay / RGBA frame cache / 原生 HTML/CSS fallback。
   - 不在 v1 阶段继续把 CDP renderer 当主线 blocker。

3. **不采用为默认 fork 的方案**
   - Remotion:许可证不是普通 MIT,默认不 fork 本体;未来只做 optional adapter。
   - OpenMontage:AGPL-3.0,不作为默认内嵌插件。
   - Vanta:顶层 MIT 但依赖 Remotion,必须逐项审计后再考虑。

## 1. 用户体验

新增独立 **Motion / AI Video Panel**,不是 Media Panel 的子标签。

```
Motion Panel
├─ Prompt / Template          用户或 Agent 描述要生成的动画
├─ Params                     文字、颜色、时长、比例、分辨率、风格
├─ Preview                    Motion Canvas 预览或最近一次渲染结果
├─ Generate / Render          调用插件生成工程并渲染输出
└─ Add to timeline            自动导入 media manifest 并创建 timeline clip
```

核心流程:

1. 用户输入 prompt 或选择模板。
2. OpenTake 调用 Motion Canvas 插件宿主生成一个独立 Motion Canvas project。
3. 插件渲染 `output.mp4`(v1)或 `frames/*.png`(后续 alpha / frame-cache)。
4. OpenTake probe 输出文件,创建 media asset。
5. OpenTake 用一个 undoable command 把生成结果放入时间线。
6. 预览与导出无需认识 Motion Canvas,只处理普通 video clip。

## 2. 模块边界

### `plugins/motion-canvas-studio/`(待新增)

建议作为 forked upstream 的独立目录,不要混入 Rust core:

```
plugins/motion-canvas-studio/
├── upstream/ or src/             Motion Canvas fork / wrapper
├── templates/                    OpenTake 内置动效模板
├── renderer/                     render orchestration
├── package.json
├── LICENSE                       Motion Canvas MIT license notice
├── THIRD_PARTY_NOTICES.md
└── README.md                     upstream、license、修改说明
```

职责:
- 接收 `MotionCanvasJob`。
- 生成或更新 `src/project.ts` 与 scene TSX。
- 调用 Motion Canvas image-sequence 或 FFmpeg video exporter。
- 输出 `motion-result.json`,包含文件路径、fps、尺寸、时长、模板/代码 hash、license 元数据。

### `src-tauri/src/motion_canvas.rs`(待新增)

Tauri command 层,只做边界 glue:
- `motion_canvas_render_job(job)` 启动插件进程或内置 runner。
- 限制 job 工作目录在 app cache / project cache 下。
- 捕获 stdout/stderr/progress。
- 返回输出文件路径和 metadata。

### `opentake-core` / `opentake-ops`

需要一个单事务 API:

```
Render Motion Canvas -> Import Media -> Place Clip
```

v1 可先拆成两步命令实现,但最终应是一个 undoable workflow:
- 渲染失败:不改 manifest / timeline。
- 导入成功但落轨失败:清理临时 manifest entry。
- 成功:返回 `{ mediaRef, clipId, outputPath, durationFrames }`。

### `opentake-agent`

`add_motion_graphic` 的语义改为:
- 优先生成 Motion Canvas scene / project。
- 渲染成 materialized video。
- 导入并落轨。

`edit_motion_graphic` 的 v1 语义:
- 如果 clip 有 `generation_input` / motion metadata,重新生成并替换 media ref 或 relink 同一 media asset。
- 如果只是普通视频 clip,返回明确错误。

### `crates/opentake-motion`(已完成一部分,改为 fallback)

当前已完成:
- `MotionSource` / `MotionRenderRequest` / `RenderedClip` value types。
- content-hash cache。
- sandbox policy。
- `StubRenderer`。
- `MotionClipSource` bridge。
- `HeadlessChromiumRenderer` skeleton。

保留用途:
- 后续透明 RGBA overlay。
- 后续 HTML/CSS fallback。
- Motion Canvas image-sequence 输出接入 wgpu compositor 时,可复用 cache / `MotionClipSource` 概念。

非 v1 目标:
- 不先补完 CDP renderer。
- 不先新增 `ClipType::Motion` 作为主路径。
- 不让 preview/export 依赖浏览器截图。

## 3. 持久化

v1 以普通 media asset 为真相:

```
MyProject.opentake/
├── media.json
├── media/generated/motion/<job-id>/output.mp4
└── motion-jobs/<job-id>/motion-result.json
```

建议新增 metadata:

```json
{
  "engine": "motion-canvas",
  "engineVersion": "...",
  "upstream": "https://github.com/motion-canvas/motion-canvas",
  "license": "MIT",
  "prompt": "...",
  "templateId": "lower-third.tsx",
  "sourceHash": "...",
  "output": "media/generated/motion/job-1/output.mp4",
  "fps": 30,
  "width": 1920,
  "height": 1080,
  "durationFrames": 150
}
```

短期可把 metadata 放进 `generation_input` 或单独 `motion-jobs/*.json`;长期再决定是否给 `MediaManifestEntry` 增加 `motion_metadata` 字段。

## 4. 许可证与 README 要求

在引入 Motion Canvas 代码之前必须完成:

- 保留 Motion Canvas MIT `LICENSE`。
- 在 OpenTake `NOTICE` 或插件 `THIRD_PARTY_NOTICES.md` 中注明 upstream、copyright、license。
- README 记录:
  - fork 来源仓库和 commit/tag。
  - 本项目修改内容。
  - 打包方式。
  - 第三方依赖 license 清单。
- CI 增加依赖 license report,防止后续依赖引入 AGPL / proprietary license。

上游依据:
- Motion Canvas repo / license: https://github.com/motion-canvas/motion-canvas / https://github.com/motion-canvas/motion-canvas/blob/main/LICENSE。
- Motion Canvas rendering docs: https://github.com/motion-canvas/motion-canvas/blob/main/packages/docs/docs/getting-started/rendering/index.mdx。
- Motion Canvas quickstart export说明: https://motion-canvas-docs.vercel.app/docs/quickstart。

## 5. 渲染策略

### v1:MP4 materialization

优先使用 Motion Canvas FFmpeg video exporter 输出 `.mp4`。

优点:
- 直接复用现有 OpenTake import / preview / export。
- 不需要新增 `ClipType::Motion`。
- 适合完整片头、解释动画、数据动画片段。

限制:
- H.264 MP4 无 alpha;透明 lower-third 不是 v1 主目标。
- 当前 OpenTake import whitelist 支持 `mov/mp4/m4v`,不支持 `webm` 或图片序列一键导入为 clip。

### v2:图片序列 / alpha overlay

使用 Motion Canvas image-sequence exporter 输出 PNG 序列,再接入:
- `opentake-motion` content-hash cache。
- `TextureSource::Motion` 或等价 `FrameSequence` source。
- preview/export 共享 resolver。

### v3:原生 HTML/CSS fallback

再回到现有 CDP skeleton:
- `HeadlessChromiumRenderer`。
- deterministic clock。
- sandbox allowlist。
- transparent screenshot。

## 6. 实施顺序

1. **文档与 issue claim**
   - #34 已认领。
   - 更新本文件、ROADMAP、ARCHITECTURE、README。

2. **License spike**
   - 固定 Motion Canvas upstream tag/commit。
   - 生成第三方依赖 license 清单。

3. **插件 skeleton**
   - 新增 `plugins/motion-canvas-studio/`。
   - 能跑一个固定 template,输出 mp4。

4. **Tauri command**
   - 新增 `render_motion_canvas` 或 `add_motion_canvas_clip`。
   - 调插件生成输出。

5. **导入 + 落轨**
   - probe 输出 mp4。
   - 创建 media asset。
   - `EditCommand::AddClips` 放入 timeline。

6. **Motion Panel**
   - 新增独立面板入口。
   - prompt/template/params/render/status。

7. **Agent tool wiring**
   - `add_motion_graphic` 接 Motion Canvas v1。
   - `edit_motion_graphic` 接可识别的 motion metadata。

8. **后续 alpha**
   - 图片序列 source。
   - `ClipType::Motion` 或 `TextureSource::FrameSequence`。
   - native `opentake-motion` renderer 再进入主流程。

## 7. 验证标准

v1:
- Motion Canvas sample template 生成 `output.mp4`。
- OpenTake 自动导入该 mp4。
- 自动创建 timeline clip。
- `composite_frame` 能看到该 clip。
- `export_video` 结果包含该 clip。
- 渲染失败不会污染 manifest / timeline。
- README / NOTICE 中包含 Motion Canvas license 和 fork 修改说明。

v2:
- PNG 序列可作为一个时间线 source 播放。
- preview/export 对同一 frame sequence 输出一致。
- alpha 正确合成。

## 8. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| Motion Canvas headless render 支持不完整 | 自动化渲染可能需要浏览器/Playwright | 先 fork 插件层,必要时自动打开本地 editor 并驱动 render;把 headless runner 作为 fork 修改点 |
| MP4 无 alpha | 不能做透明 lower-third | v1 定位完整视频片段;v2 用 PNG sequence / ProRes / frame cache |
| Node/FFmpeg 打包体积 | 桌面包变大 | 插件独立可选安装;先 dev dependency,后续 externalBin |
| AI 生成 TSX 代码不稳定 | render 失败 | 模板优先,自由代码放实验入口;错误返回给 Agent 自修 |
| 依赖 license 漂移 | 合规风险 | lockfile + license CI + THIRD_PARTY_NOTICES |

## 9. 当前已完成 / 待做

已完成:
- #34 已认领: https://github.com/appergb/OpenTake/issues/34#issuecomment-4799284093。
- 本规划文档已从旧 HTML/CSS/CDP 主线改为 Motion Canvas 优先方案。
- README、README.zh-CN、ROADMAP、ARCHITECTURE、WORKFLOW-PLUGIN-SYSTEM、media-SPEC 已同步 Motion Canvas / fallback 边界。
- `crates/opentake-motion` scaffold。
- `add_motion_graphic` / `edit_motion_graphic` tool name、args、schema 描述已改成 Motion Canvas TS/TSX scene/template 语义。
- preview/export 已能处理普通 video media,所以 Motion Canvas mp4 v1 能复用现有路径。

待做:
- 新增 Motion Canvas plugin 目录。
- 新增 Tauri motion command。
- 新增 Motion Panel。
- 将 agent dispatch 从 `not yet implemented` 接到 Motion Canvas workflow。
- 增加 license notice / dependency license report。
- 后续再接透明 alpha / frame sequence / native CDP fallback。
