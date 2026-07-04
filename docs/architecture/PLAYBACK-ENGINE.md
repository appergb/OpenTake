# 流式播放引擎（#53 / ROADMAP Phase 4）

> Rust 流式合成播放：把时间线**连续播放**从「前端单 rAF 时钟驱动原生 `<video>`/`<audio>`」升级为「Rust 连续解码 → wgpu 合成（与导出同一像素路径）→ MJPEG 传给 WebView，cpal 音频做主时钟」。**暂停 / scrub 仍走原 `<video>` 单帧路径**（不回退 `74c4c82` 暂停冻结 / `5fa3f6f` resume 不 force-seek）。整数帧贯穿。
>
> 全部代码在 `playback-engine` cargo feature（**默认关**）+ 前端运行期 flag（**默认关**）后；翻开前对既有行为零影响。

## 为什么

旧的 `<video>` 多轨播放：高码率卡顿、ProRes 等 WebView 放不了、A/V 精度有限、**播放时看不到合成效果**（调色/特效/多轨叠加只在暂停态 `composite_frame` 有）。流式引擎在播放态用 Rust 解码+wgpu 合成，解决以上四点。对应 #92 / #100 / #131 / #142 / #151 的播放侧。

## 架构

```
PLAY 态:
  cpal 音频专线程 ── frames_played(AtomicU64) ──┐  (主时钟)
                                                ▼
  渲染专线程(自持 wgpu device):
    每轮: target = clock.frame() → plan.frame(timeline,target)
        → StreamingResolver.sync_active(按 clip_id 管 VideoStream, try_recv drain 到目标帧, 落后复用上一帧)
        → Compositor.render_to_rgba(同导出像素路径)
        → MjpegSink: JPEG → broadcast → axum /stream(multipart/x-mixed-replace)
        → emit "playback_frame"{frame}
  前端: <img src=MJPEG> 显像素; playback_frame 事件推 activeFrame(播放头/时间码)

SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)。
```

- **主时钟 = cpal 音频**：整条时间线预混成交错立体声 buffer（设备采样率），cpal 回调 lock-free 从 `buffer[pos..]` 拷贝并 `pos.fetch_add`（唯一推进点）；视频追音频（落后丢/复用、超前缓存）。**无音频/无设备 → 回退墙钟 `InstantClock`**，视频仍播。
- **预览 = 导出**：复用 `build_render_plan` + `RenderPlan::frame` + `Compositor::render_to_rgba`；timeline→source 帧用现成 `source_frame_index`（trim/speed/整数帧）。
- **隔离**：渲染线程自持独立 wgpu device，不碰暂停态 `composite_frame` 的 `RenderState`；cpal Stream（macOS `!Send`）独占音频线程。

## 文件

| 文件 | 角色 |
|---|---|
| `crates/opentake-media/src/decode/stream.rs` | 连续视频解码原语（PR1，已在 main） |
| `crates/opentake-media/src/decode/audio_stream.rs` | 交错（立体声）PCM 解码（#160） |
| `src-tauri/src/playback/resolver.rs` | StreamingResolver：按 clip_id 管流 + drain 到目标帧（PR1） |
| `src-tauri/src/playback/engine.rs` | RenderLoop + 渲染线程 + clock/sink/emitter traits + InstantClock + loop_step（PR1） |
| `src-tauri/src/playback/audio.rs` | cpal AudioClock + 立体声预混 + 设备声道映射（#63/#160） |
| `src-tauri/src/playback/transport.rs` | axum MJPEG 服务 + Origin 守卫 + MjpegSink + TauriPlayheadEmitter（#64） |
| `src-tauri/src/playback/commands.rs` | PlaybackState + playback_start/pause/stop/seek + get_preview_endpoint |
| `web/src/components/preview/previewEngine.ts` | PLAY→Rust 切换缝（守卫分支）+ 中途 seek watcher（#162） |
| `web/src/components/preview/Preview.tsx` | MJPEG `<img>` overlay |
| `web/src/components/preview/rustEngine.ts` | `rustEngineEnabled()` 运行期 flag |

## 灰度开关 + 真机验收

1. 编译：feature `playback-engine`（CI 已加 lint+test 步 + Linux `libasound2-dev`）。
2. 运行期开关（默认关）：DevTools 控制台
   ```js
   localStorage.setItem('opentake.rustEngine', '1')   // 开
   localStorage.removeItem('opentake.rustEngine')      // 回 <video>
   ```
3. 真机：`./web/node_modules/.bin/tauri build` → `cp -R target/release/bundle/macos/OpenTake.app /Applications/` → `open -a OpenTake` → 翻 flag → 验：
   - 多轨/高码率工程播放**不卡**；
   - ProRes 等 `<video>` 放不了的格式**能预览**；
   - 播放时**可见**调色/特效/多轨叠加；
   - **A/V 同步**在片段边界 / 变速下稳定（左右声道正确）；
   - **scrub / 暂停 / resume 行为不变**（不回退 74c4c82 / 5fa3f6f）。
   全绿后把 `rustEngineEnabled()` 改默认开（独立小 PR）。

## 取舍 / Follow-up

- 音频当前**预混**（启动全量解码，已用 `spawn_blocking` 移出 IPC 线程）；分块/后台填充流式解码 → #160 另半。
- CSP 暂留 `null`（已允许回环 `<img>`，功能 OK）；加固为 → #161（高风险白屏，须真机逐项验证）。
- Lottie 合成接入 → #65（`opentake-motion` 无 Lottie 渲染器，是独立大工程）。
- WebView2（Windows）multipart 长连接可靠性未验；`FrameSink` 已 trait 化，必要时换 WS/自定义 scheme。
```
