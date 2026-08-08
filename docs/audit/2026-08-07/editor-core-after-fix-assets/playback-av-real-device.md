# 播放引擎真实设备 A/V Probe

验证日期：2026-08-08（Asia/Shanghai）。本验证直接运行 `src-tauri/tests/playback_probe.rs` 的 `PlaybackEngine` 真实设备 probe；没有用导出音频替代播放验证，也没有修改产品代码或测试代码。

## 设备与 strict 条件

`system_profiler SPAudioDataType` 确认当前默认输出与系统输出均为“MacBook Air扬声器”，2 channels、96,000 Hz。测试设置：

```sh
OPENTAKE_PROBE_VIDEO=/tmp/.../av-safe-1584x1080-30fps-aac.mp4 \
OPENTAKE_REQUIRE_AUDIO_CALLBACK=1 \
cargo test -p opentake-tauri --test playback_probe \
  probe_realtime_playback_with_audio_or_safe_fallback \
  -- --ignored --nocapture --exact
```

strict 模式要求 `build_clock_paused` 返回真实 `AudioPlayback`。该路径在返回前会建立 CPAL output stream，并要求 callback epoch 在 1 秒内连续推进至少两次；否则回退 wall clock 后 `audio_active=false`，strict 断言失败。因此日志中的 `clock=audio` 证明本次播放由真实设备 callback 的音频 master clock 驱动，而不是 wall-clock fallback。

## Fixture

fixture 只放在 `mktemp` 目录，不写产品树；使用已安装 app 的 FFmpeg sidecar 生成：

- H.264、1584×1080、30/1 fps、300 frames、10.000 s。
- 深蓝动态背景 + 随时间水平移动的红框，不含 neon green；不是静态色视频。
- AAC、48,000 Hz、stereo、470 audio frames、10.000 s、880 Hz sine。
- 文件大小 185,584 bytes；SHA-256 `00b863e8392cacd0cf7aeafefb237a6687c17f2e668d4723d67262f1bdc6091c`。

媒体拥有真实音视频两条 stream；probe 会通过生产解码/混音路径建立 CPAL 输出，通过 `AudioClock` 推进播放头，同时由真实 GPU/render sink 收集视频帧和像素质量。

## 结果

首次安全 fixture 单次：

```text
frames=85 playhead=90 clock=audio
nonblack_ratio=1.000 neon_green_ratio=0.000
1 passed; 0 failed
```

随后连续三次 exact strict probe：

| run | frames / 3 s | playhead | clock | nonblack | neon green | 结果 |
|---:|---:|---:|---|---:|---:|---|
| 1 | 85 | 91 | audio | 1.000 | 0.000 | PASS |
| 2 | 84 | 90 | audio | 1.000 | 0.000 | PASS |
| 3 | 84 | 89 | audio | 1.000 | 0.000 | PASS |

验收阈值是 frames ≥ 73、playhead ≥ 73；三次均超过阈值，且视频帧全程非黑、无绿屏哨兵命中。

日志：

- [首次 testsrc2 假阳性](playback-av-real-device-probe.log)
- [安全动态 fixture 单次 PASS](playback-av-real-device-probe-safe-fixture.log)
- [安全动态 fixture 三连 PASS](playback-av-real-device-probe-three-runs.log)

首次使用 `testsrc2` 的运行已经显示 `frames=84 playhead=89 clock=audio`，但因为 testsrc2 画面本身包含 16.2% 纯绿色，触发“防解码绿屏”质量断言而失败。该失败只说明 fixture 与绿屏哨兵冲突；换成不含 neon green 的动态红/蓝图案后，同一产品代码和同一 strict probe 稳定通过。失败日志完整保留，没有选择性删除。

## 判定边界

据此可把“播放引擎核心 A/V 设备/时钟集成”从 PARTIAL 提升为 **PASS（真实 CPAL 设备、AAC 解码/混音路径、audio master clock、视频跟随）**。

本 probe 没有对扬声器做麦克风 loopback，因此不能客观证明用户感知到的声压/响度；也只有 3 秒单轨窗口，不能替代长时多轨 A/V drift、热插拔设备、系统静音或异常设备专项。这些边界继续保持 PARTIAL，不被三次短 probe 外推。
