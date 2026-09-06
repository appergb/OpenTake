---
status: canonical
stage: final-verified
---
# 宣传片文件验收

验收日期：2026-09-06，Asia/Shanghai。仅验收本次视频交付，不代表 OpenTake 产品发布门禁通过。

## 最终交付规格

- `deliverables/OpenTake-beta6-official-1080p.mp4`
- 大小：10,704,025 bytes（约 10.7 MB / 10.2 MiB）。
- MP4 / H.264 High / avc1 / 1920×1080 / 16:9 / 30 fps / 1800 帧。
- 视频、音频、容器时长均为 **60.000000 秒**。
- 像素格式：`yuv420p`，8 bit；limited-range，颜色矩阵 `bt709`。
- AAC LC / stereo / 48,000 Hz；原创合成配乐，无旁白。
- 封面：`deliverables/OpenTake-beta6-cover.png`，1920×1080 RGB PNG。

## 已执行的检查

| 检查 | 命令 / 证据 | 结果 |
|---|---|---|
| TypeScript | `npm run typecheck` → `review/typecheck.log` | exit 0 |
| Remotion 全帧渲染 | `npm run render` 渲染步骤 → `review/final-render.log` | 1800/1800 rendered、1800/1800 encoded |
| 交付标准化编码 | `sh scripts/master.sh` → `review/mastering.log` | exit 0；完整 1800 帧 |
| 完整元数据 | `ffprobe -v error -show_format -show_streams -of json` | 完整 JSON：`review/ffprobe-full.json` |
| 全片解码 | `ffmpeg -hide_banner -v error -xerror -i … -f null -` | exit 0，`review/full-decode.log` 空（0 bytes） |
| 元数据断言及快速起播 | `python3 scripts/check_metadata.py` | exit 0；moov 在 mdat 前，见 `review/metadata-check.log` |
| 音量测量 | FFmpeg `volumedetect` | mean -22.0 dBFS，peak -8.8 dBFS；没有数字削波；非主观听音验收 |
| 关键帧视觉检查 | 0、3、9、16、23、30、34、40、47、50、56、59 秒 | 已检查联系表；3/30/59 秒另以大图检查；无缺字、裁断或意外遮挡 |
| 本地项目预览 | Remotion Studio `http://localhost:3418/OpenTakeOfficial` | 组合加载，名称、Public Beta 文案可见 |

最终抽帧均来自标准化后的最终 MP4；`review/contact-sheet.jpg` 是 12 张验收图汇总。第 0 帧为品牌圆环开场，文字随后淡入；末帧保留完整 CTA，无强行截黑。

首次 Remotion 产物使用 full-range JPEG 色彩和 60.053333 秒 AAC 尾长；已通过 `scripts/master.sh` 转为上述正式交付规格。`review/remotion-render.mp4` 为中间件，正式交付以 `deliverables/` 文件为准。

## 真实限制

- 本轮既有 app 版本读得 `1.0.0-beta.5`。导入 Open 与导出 Save 灰置，且目标窗口捕获黑屏；已可取消、已退出对话框。未获得当前包的新成功录屏或新导出。
- 成片采用标明日期的历史安装包实拍静帧、后期镜头运动及历史实际导出视频；不等同于 beta.6 连续操作录屏或 GUI 验收。
- 原素材多为测试色条与面板，真实素材质量约束仍在；预置原创动态 demo 与音轨一并交付，供桌面恢复后换拍。
- AI 段是历史真实工具调用，片中注明需配置服务与凭据；没有执行或宣称新的云生成成功。
- 字体依赖 macOS 系统中文字体。源工程跨机重渲染需要合法字体环境；正常重渲染使用已打包资产，不依赖原 audit 目录。
- 全部文件只保存在本目录，未修改产品代码、未推送、未发布。候选文案 `1.0.0-beta.6 · Public Beta` 由主代理指定。
