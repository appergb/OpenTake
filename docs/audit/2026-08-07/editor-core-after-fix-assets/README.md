# 正式包修复后 GUI 证据

验证日期：2026-08-08（Asia/Shanghai）。所有 UI 操作只针对 `/Applications/OpenTake.app`；未启动 Vite、Tauri dev 或旧备份实例。

## 截图索引

| 文件 | 证明内容 |
|---|---|
| `01-home-cover-fixed.png` | 单轨工程 Home 封面不再为空。 |
| `02-reopen-thumbnail-timeline-fixed.png` | 单轨工程重开后素材缩略图与时间线预览可见。 |
| `03-source-preview-fixed.png` | 选中源素材并稳定等待 5 秒后，源预览可见。 |
| `04-single-track-playback-progress-frame-220.png` | 4K60 单轨播放推进到 frame 220。 |
| `06-single-track-seek-midpoint.png` | 坐标 seek 到 frame 115，画面与播放头同步变化。 |
| `07-single-track-pause-stable.png` | 单轨暂停在 frame 187；2.2 秒后仍为 187。 |
| `08-dual-track-composite-stable.png` | fresh 双轨工程稳定 15 秒后，两素材缩略图与合成预览正常。 |
| `09-dual-track-playback-frame-239.png` | 双轨播放从 0 连续推进到 terminal frame 239。 |
| `10-dual-track-playback-frame-48.png` | 双轨第二轮从 0 推进到 frame 48，复合画面变化。 |
| `11-dual-track-seek-pause-frame-115.png` | 双轨 seek 到 frame 115；暂停后立即与 2 秒后均为 115。 |
| `12-export-dialog-contrast.png` | Export 对话框与深色主按钮文字；此图为 4K 初始选项。 |
| `13-export-keyboard-focus.png` | 720p 已选，Tab 焦点明确落在主“导出”按钮。 |
| `14-gui-export-completed.png` | GUI 导出结束后返回双轨编辑器。 |
| `15-gui-export-frame-2s.png` | GUI 产物 2 秒真实抽帧，显示双轨复合结果。 |
| `16-relaunch-home-cover-dual.png` | 完整退出再启动后，双轨与单轨 Home 封面均可见。 |
| `17-relaunch-editor-thumbnails-composite.png` | 重启后重开双轨工程、稳定 15 秒，两缩略图与合成预览仍正常。 |
| `18-relaunch-source-preview.png` | 重启重开后选中源、等待 5 秒，源预览仍正常。 |
| `19-library-keyboard-action-focus.png` | 零 hover 开始的 Tab 路径可到达 Library 三个卡片操作；截图为第二卡“取消收藏”焦点。 |

Computer Use 服务原始捕获实际是 JPEG 字节，即使最初文件名后缀为 `.png`。原始字节已移动到 `diagnostics/original-computer-use-jpeg/`，再由 macOS `sips -s format png` 无损解码并重新编码为正式 PNG。最终 01–19 的每个 `.png` 都通过 `file` 验收并由 `view_image` 逐一目视复核。

`diagnostics/05-terminal-reset-after-pause-race.jpg` 是在 terminal reset 竞态后截到的 frame 0，不能证明暂停；`diagnostics/06-ax-set-value-ignored.jpg` 证明 AX slider setter 被前端忽略，不能证明 seek。二者因此不进入正式编号序列；有效 seek 与暂停分别采用 06、07 和 11。

## 双轨 fixture

- 工程：`gui-validation-dual.opentake/`
- 画布：3840×2160、60 fps、240 frames / 4.000 s。
- V1：`audit-4k60-h264.mp4`，H.264、3840×2160、60/1、4.000 s，frames 0–239。
- V2：`audit-4k60-h264-hflip.mp4`，不同的 hflip 源，H.264、3840×2160、60/1、4.000 s，frames 0–239；SHA-256 `fca06a45eec96ac5b4c3618aa8bb0fca409e49eb2374f3268237c85a4c478fc8`。
- V2 使用 0.8 opacity、0.42×0.42、center (0.75, 0.72)，因此合成结果可由右下嵌套色条明确辨认。
- 历史 `/Users/lvbaiqing/Documents/OpenTake/未命名 2.opentake` 的两个源均指向已离线 `/Volumes/mac/...`，故只记为 fixture BLOCKED，不用于判断正式包播放回归。

## GUI 导出与资源采样

- 产物：`gui-export-after-fix-720p.mp4`，SHA-256 `6ac341e9e8f89be91f65063fa21e65d296d8aed861d3f3b04939cc0a9c63824f`。
- ffprobe：H.264、yuv420p、BT.709、1280×720、60/1 fps、240 frames、4.000000 s、单视频流、489,960 bytes。
- `15-gui-export-frame-2s.png`：1280×720 真 PNG；SHA-256 `8b4b67914ff2f73f5fea1a53d60604eaab211961b4803e09f69fc1d7eb07d400`。
- 双轨播放 24 次采样：OpenTake + 当次 WebKit 进程族各进程 RSS 峰值之和约 541 MB（约 528 MiB）；FFmpeg/ffprobe 子进程始终 0。`ps` 瞬时 CPU 栏均为 0.0，不能当作精细 CPU profile。
- GUI 导出采样捕获 1 个持续 encoder 与逐帧短 helper/ffprobe；观察窗口内同时可见 encoder + 1 helper，未出现无界并发。峰值示例为 OpenTake 2.2% CPU、WebContent 0.4% CPU。
- 原始采样：`dual-playback-active-process-sample.txt`、`gui-export-resource-sample.txt`；解析结果不外推到 100 项冷缓存压力工程。

## 真实设备 A/V 播放

- 当前默认输出为 MacBook Air 扬声器，2 channels / 96 kHz。
- 使用 mktemp 中的 10 秒真实 A/V fixture：H.264 1584×1080@30 动态红/蓝画面 + AAC 48 kHz stereo 880 Hz sine。
- `OPENTAKE_REQUIRE_AUDIO_CALLBACK=1` 禁止 wall-clock fallback；安全 fixture 单次 PASS 后再连续三次 PASS：frames/playhead 分别为 85/91、84/90、84/89，全部 `clock=audio`，nonblack 1.000、neon green 0.000。
- 首个 testsrc2 fixture 已经得到 `clock=audio`，但它自带 16.2% 纯绿色而误撞绿屏哨兵；失败日志保留，换不含 neon green 的动态 fixture 后稳定通过。
- 完整方法、fixture hash、strict callback 语义和三份日志见 [playback-av-real-device.md](playback-av-real-device.md)。该证据把核心 A/V 设备/时钟集成提升为 PASS，但不外推主观可听响度或长时多轨 drift。

## 权限与限制

- 首次打开、退出重启、工程重开、源预览和导出均未出现新的文件/媒体/应用权限弹窗，也未更改任何 macOS 安全设置。
- 本包为 ad-hoc 签名且未 notarize；`codesign --verify --deep --strict` 通过，Gatekeeper `spctl` 拒绝是该分发形态的预期限制。
- 未覆盖扬声器麦克风 loopback、长时音频双轨 A/V drift、VoiceOver、200%/400% 缩放、100 项冷缓存峰值；这些边界不改写本次视频主链、真实设备核心 A/V 与确认缺陷的通过结论。
