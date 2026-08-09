# Fresh 安装包 GUI 验证账本

- 执行时间：2026-08-08 09:48–10:00（Asia/Shanghai）
- 安装包：`/Applications/OpenTake.app`
- Bundle：`com.opentake.desktop` / `1.0.0-beta.2`
- 源码 HEAD：`f9398a9302e57690caa3ea21ff995fdd0ce97706`
- 可执行文件 SHA-256：`139f4749e1e6bc35b13f980c4b36353dd211be4b1a10f784435d875ce3d4d78a`
- 一致性：安装包可执行文件与 `target/release/opentake` SHA-256 完全相同。
- 操作方式：macOS Computer Use，仅使用真实安装包和 native 打开/保存对话框；开始前确认无 OpenTake、FFmpeg、safe-asset-helper 旧进程。

## 操作序列

| 时间/阶段 | 操作与稳定等待 | 实际结果 | 证据 |
|---|---|---|---|
| 首次导入 | 新建并保存 `gui-validation.opentake`，native dialog 导入 `audit-4k60-h264.mp4`，等待 15 秒 | 素材卡出现破图图标；不是“还在生成” | `04-fresh-import-after-15s-broken-thumbnail.jpg` |
| 缓存复核 | 检查 `~/Library/Caches/com.opentake.desktop/media-cache/MediaVisualCache/` | 120×68 thumbnail 与 1920×1080 preview 均已生成、可解码、内容正确 | `05-generated-thumb-valid.png`、`06-generated-preview-valid.png` |
| 选中源媒体 | 单击素材卡后等待 5 秒 | 中央源素材 Preview 为黑；控制区仍为 `00:00:00 / 00:00:00`，Inspector 同时正确显示 3840×2160、0:04 | `07-selected-media-preview-after-5s-black.jpg` |
| 加入时间线 | 双击素材卡 | 工程自动设为 3840×2160、60 fps、240 帧；Rust 合成预览立即显示正确彩条 | 后续播放截图 |
| 时间线播放 | 点击播放并两次读取 GUI/AX 状态 | 播放头从 frame 17 推进到 frame 239，画面同步改变；没有停在播放前一帧 | `08-playback-frame-17.jpg`、`09-playback-frame-239-terminal.jpg` |
| 保存/退出/重启 | `Cmd+S`、`Cmd+Q`，确认进程退出，再从 Home 最近项目重开 | Home 项目封面为空；工程重开成功，时间线合成预览正常 | `10-relaunch-home-cover-blank.jpg`、`11-project-reopened-timeline-preview-ok.jpg` |
| 重开后等待 | 重开后等待 15 秒，再选中源媒体等待 5 秒 | 卡片仍为破图，源 Preview 仍为黑；故障稳定复现，不是首次授权/生成时序 | `12-reopen-after-15s-thumbnail-broken.jpg`、`13-reopen-selected-preview-after-5s-black.jpg` |
| GUI 导出 | 时间线切回 720p H.264，通过 native save dialog 导出 | 导出完成；产物 1280×720、60 fps、240/240 帧、4.000 秒、H.264 High、yuv420p、BT.709；2 秒帧视觉正确 | `gui-export-720p.mp4`、`14-gui-export-frame-2s.png` |

## 资源链诊断边界

1. `f9398a9` 已对项目引用的外部源文件执行精确 `allow_file`；实际 persisted scope 中也能看到该源路径。
2. `tauri.conf.json` 的静态 asset scope 已包含 `$APPCACHE/**/*`；缓存 thumbnail/preview 文件本身存在且可解码。
3. 前端的缩略图、poster 和 source video 都经 `convertFileSrc(path, "opentake-asset")`；时间线画面走 Rust framebuffer/MJPEG，不依赖同一资源绘制通道。
4. 因此当前证据排除了“没有生成缓存”“源媒体不存在”“时间线合成失败”和“只缺项目重开时源文件授权”。故障已收窄到 `opentake-asset` 自定义协议的请求/响应/消费链。
5. release WKWebView 无可检查 target；终端直接启动 release 复现时 stdout/stderr 与 macOS unified log 均无 asset 错误。另行启动 dev 只取得 `window.set_position not allowed` 的既有 console 错误，未成功驱动同一 dev WebView 完成资源复现，不能替代 release 证据。因此实际 asset URL、HTTP 状态、Content-Type 和 DOM decode error 仍为 **BLOCKED**，不得臆测为 403、CSP 或某个确定解码错误。

## 产物校验

```text
gui-export-720p.mp4
  codec=h264, profile=High, pix_fmt=yuv420p
  width=1280, height=720
  r_frame_rate=60/1, avg_frame_rate=60/1
  nb_frames=240, nb_read_frames=240
  duration=4.000000, size=489960
  color_space/color_transfer/color_primaries=bt709
  sha256=6ac341e9e8f89be91f65063fa21e65d296d8aed861d3f3b04939cc0a9c63824f

14-gui-export-frame-2s.png
  PNG 1280x720 RGB
  sha256=cc6bf4a4df410123bf1e7cb3f5bd43add507b6d914ccdcae1ca2b5c2b931a199
```

## 证据卫生

- `01-home-current-build.png`、`02-editor-empty.png`、`03-import-thumbnails-failed.png` 扩展名为 `.png`，但 `file` 识别为 JFIF JPEG。报告只把它们当早期 GUI 观察，不当无损 PNG。
- `04`–`13` 使用与真实内容一致的 `.jpg`；`05`、`06`、`14` 为真实 PNG。
- 验证结束后已停止 release、debug、Vite、FFmpeg 与 safe-asset-helper；端口 1420 无监听。
