---
status: canonical
stage: implementation-backed
---
# 素材、声音与事实来源

制作日期：2026-09-06，Asia/Shanghai。来源均在本机，不下载外部媒体，不调用付费生成服务。

## 成片实际使用的素材

路径均相对仓库根目录；映射副本保存在本工程 `public/assets/`，逐项映射见 `review/source-map.json`。

| 镜头 / 资产 | 原始来源 | 处理与边界 |
|---|---|---|
| 品牌图标 | `assets/opentake-logo.png` | 复制品牌资产，未重绘 |
| 导入 / 素材预览 | `docs/audit/2026-08-07/editor-core-after-fix-assets/03-source-preview-fixed.png` | 真实历史安装包，2026-08-07；整体缩放与镜头轻推 |
| 时间线 | 同目录 `06-single-track-seek-midpoint.png` | 整图与底部时间线局部裁切放大；未增加任何虚构控件或操作 |
| 多轨 | 同目录 `08-dual-track-composite-stable.png` | 双视频轨实拍；文案没有将它称为新包音视频多轨验证 |
| 文字 / 效果 | `docs/audit/2026-07-14/runtime-artifacts/automated/generic-effects-packaged-ui-2026-07-31.png` | 图中含文字片段与效果 Inspector；仅说明实现界面 |
| 调色效果 | 同目录 `lgg-packaged-ui-2026-07-31.jpg` | 2026-07-31 历史实拍 |
| Agent | `docs/audit/2026-08-13/screenshots/beta5-packaged-agent.png` | 仅裁切 Agent 面板；保留历史真实工具调用；不是此次新调用 |
| 导出界面 | `docs/audit/2026-08-07/editor-core-after-fix-assets/12-export-dialog-contrast.png` | 2026-08-07 历史实拍 |
| 实际导出视频 | 同目录 `gui-export-after-fix-720p.mp4` | 只读复用已有未提交资产的副本；1280×720 H.264，60 fps，4 秒；片内标明“历史实机导出样片” |
| 背景、标题、镜头运动 | 本工程 `src/` | 本次原创 HTML/CSS/React 图形，使用 Remotion 按帧渲染 |
| 配乐 | `scripts/generate_audio.py` → `public/assets/original-score.wav` → `score.m4a` | 本次原创程序合成，E minor / Cmaj7 / G / D，96 BPM，60 秒，固定随机种子；无现成曲目、无第三方采样、无歌声 |
| 字体 | 本机 PingFang SC / Heiti SC / Arial | 系统字体；不随工程重新分发字体文件。跨平台复现需提供合法中文字体并调整 fallback |

## 额外交付、未作为当前包成功证据的素材

- `src/scenes/Footage.tsx` 与 `public/assets/original-motion.mp4`：本次原创几何动效样片，1920×1080 / 30 fps / 8 秒。用于准备 OpenTake 演示工程；未声称由 OpenTake 生成或导出。
- `scripts/prepare_demo.py` 与 `OpenTake Official Demo.opentake/`：根据项目 schema 预置的独立演示工程，含分段 V1、文字 V2、音频 A1。创建/重开经 AX 观察，未获得可用屏幕像素证明。
- `review/unusable-current-window.png`：本轮目标应用黑色窗口记录，不用于宣传片。
- `review/save-panel-disabled.png`：本轮原生导出面板 Save 灰置截图，不用于宣传片。此图包含文件选择器内本机路径，仅作为本地诊断。

## 宣传事实边界

- 候选版本 `1.0.0-beta.6` 由主代理在会话中明确给定；影片仅用 `Public Beta`，没有声称该版本已发布、稳定或已完成全部上游功能。
- 产品能力信息以读取时的 `docs/releases/1.0.0-beta.5.md`、`docs/capabilities/CAPABILITY-LEDGER.md` 为来源；仅覆盖存在实现与历史证据的功能。
- 成片使用历史界面静帧、后期镜头运动与历史真实导出片段，不是一次连续操作录屏。
- AI 仅展示已有 Agent / MCP 协作路径，明确需要模型服务与凭据；未声称此次执行付费模型任务、生成视频或新模型测试成功。
- 既有 `marketing/official-beta-video/`、`output/` 和 `docs/audit/` 仅只读检查/复用，未覆盖其未提交素材。
- 成片中的“已完成”如出现在 Agent 历史截图内，是原始工具记录，不是本次候选发布或全项目验收声明。
