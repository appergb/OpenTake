---
status: canonical
stage: final-verified
---
# OpenTake Public Beta 官方宣传片

本目录是独立营销交付，不修改产品代码、不推送、不发布。

成片：`deliverables/OpenTake-beta6-official-1080p.mp4`。封面：`deliverables/OpenTake-beta6-cover.png`。
可移植源资产包：`deliverables/OpenTake-beta6-source.tar.gz`（不含 node_modules）。
源工程：`src/`、`package.json`、`package-lock.json`、`remotion.config.ts`。

## 播放与复现

```sh
cd '/Users/trip/TRUE 开发/PRIMARY-CN/OpenTake-generation/marketing/official-beta-2026-09'
npm ci --no-audit --no-fund
npm run typecheck
npm run render
node scripts/render.mjs cover
sh scripts/verify.sh
npm run studio
```

Studio：`http://localhost:3418/OpenTakeOfficial`。仅本地预览，不是部署。成片渲染使用 Remotion 4.0.521、React 19.2.3、3 个并发。
最终片为 60 秒、1800 帧、约 10.7 MB；完整探测、全片解码和 12 个时点抽帧检查通过。

当前 Node v26.7.0 与 FFmpeg 9.0.1；Chromium 采用 `chrome-for-testing` 启动模式，避免新版 Chromium 不再支持旧 headless 模式的问题。可用 `REMOTION_BROWSER=/绝对路径/Chromium` 指定浏览器；默认尝试现有本机 Chromium/Chrome，无可用浏览器时由 Remotion 使用对应下载流程。

运行机器需有中文字体；当前使用 macOS 自带 PingFang SC，字体不打包分发。原素材副本已经在 `public/assets`，正常重渲染不依赖原仓库 audit 路径。

重新生成原创配乐（常规渲染直接使用已交付音频，不必重新生成）：

```sh
'/Users/trip/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3' scripts/generate_audio.py
ffmpeg -y -i public/assets/original-score.wav -af loudnorm=I=-18:TP=-2:LRA=9 -c:a aac -b:a 256k public/assets/score.m4a
```

可移植 Python 3 + NumPy 同样可运行 `scripts/generate_audio.py`。原始音频 48 kHz / stereo PCM，编码副本供成片使用。

可选演示工程：`node scripts/render.mjs motion` 重渲染原创 8 秒样片；在没有被 OpenTake 打开的情况下运行 `python3 scripts/prepare_demo.py` 可重建本目录演示工程并修正当前绝对素材路径。该脚本会覆盖本目录独立 demo，不能对用户实际项目使用。

## 分镜

| 时间 | 内容 |
|---|---|
| 00:00–00:06 | 品牌与“把想法，剪成作品。” |
| 00:05.5–00:13 | 导入 / 素材预览 |
| 00:12.5–00:20 | 时间线与剪辑入口 |
| 00:19.5–00:27 | 双轨叠加 |
| 00:26.5–00:35 | 文字 / 效果 |
| 00:34.5–00:44 | Agent 与 MCP，配置条件 |
| 00:43.5–00:52 | 导出界面与历史真实导出视频 |
| 00:51.5–01:00 | Public Beta / 开源 / 项目入口 |

转场重叠 15 帧，总长精确 1800 帧。配乐贯穿，片尾自然衰减；无旁白。

## 事实与验收

- [素材和音频来源](SOURCES.md)
- [本次应用采集限制](CAPTURE-LIMITATIONS.md)
- [执行记录](PLAN.md)
- 验收文件在 `review/`，最终结果见 `VALIDATION.md`。

宣传版本为候选 `1.0.0-beta.6 · Public Beta`；未称已发布或稳定版。界面镜头有历史日期，不代表本次 beta.6 的 GUI 验收通过。影片是历史实拍静帧动效 + 真实历史输出片段的宣传剪辑，不是最新版本连续操作录屏。
