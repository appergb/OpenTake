# 合成引擎与渲染发布前独立测试

- 日期：2026-08-08（Asia/Shanghai）
- 仓库：`OpenTake-generation`
- 分支：`audit/remediation-2026-08-05`（测试开始时相对 upstream ahead 2，工作树已有其他 Agent 改动）
- 范围：`opentake-render`、暂停预览/连续播放/导出 RenderPlan 接线、GPU 合成、真实 FFmpeg 媒体探针
- 总结论：**PASS（核心逻辑、Metal GPU 像素路径、真实媒体播放与三编码最终探针均通过）**
- 非阻塞风险：固定全局临时文件名的 ignored probe 在共享/并发测试环境出现过输出文件竞态；ProRes 实时吞吐阈值一次 46/48 失败、立即复跑为 56/48 通过，属于性能抖动，不能视为稳定性能余量。

## 分层结论

| 层级 | 结论 | 证据 |
|---|---|---|
| RenderPlan 纯逻辑 | PASS | `opentake-render` 单元测试含整数帧、半开区间、多轨顺序、同轨 overlap、trim/speed/reverse、nested/compound；58/58 单元测试通过。 |
| GPU 合成/像素 | PASS | 本机 Metal 适配器可用，GPU 用例没有走自动跳过；双轨 alpha-over、方向、文本、LUT、调色、蒙版、转场、readback/PNG、pixel diff 均真实执行。 |
| preview/export RenderPlan parity | PASS | 两条路径都使用 `try_build_render_plan`、`plan.frame`、同一 `Compositor::render_to_rgba_with_interpolation`，且 OpticalFlow + Blend 参数相同；golden/parity 测试通过。 |
| 真实媒体播放 | PASS，带性能抖动警告 | 自动生成 H.264/ProRes 媒体，经 FFmpeg 解码、RenderLoop、GPU 合成；集成测试 7/7 通过。ProRes ignored probe 第一次 46 帧/2s 未过 48 门槛，复跑 56 帧/2s、非黑像素比 1.000。 |
| 真实媒体导出 | PASS，带探针并发警告 | H.264/H.265/ProRes 最终单线程 probe 3/3 通过；每个输出 1280×720、30 fps、60 帧，ffprobe 分别报告 `h264,60`、`hevc,60`、`prores,60`。 |

## 命令、exit code 与计数

1. `cargo test -p opentake-render -- --nocapture`
   - exit code：`0`
   - 计数：`116 passed; 0 failed; 0 ignored`（另 0 doctest）
   - 明细：58 unit + 58 integration。Metal GPU 实际执行，日志无 `[skip] no GPU`；因此本机自动跳过数为 0。

2. `cargo test -p opentake-tauri --test playback_integration -- --nocapture`
   - exit code：`0`
   - 计数：`7 passed; 0 failed; 0 ignored`
   - 覆盖：精确 trimmed source frame、解码失败不发布黑帧、取消、seek/推进、双轨并发合成、播放线程 sink/emitter。

3. `cargo test -p opentake-tauri --test export_integration -- --nocapture`
   - exit code：`0`
   - 计数：`5 passed; 0 failed; 0 ignored`
   - 覆盖：可播放 MP4、HDR→BT.709、4K render target、AAC mux、文字字体边界；FFmpeg/ffprobe 和 GPU 均实际可用，未跳过。

4. `cargo test -p opentake-tauri --test export_probe -- --ignored --nocapture`
   - 首轮（默认并行）exit code：`101`；`1 passed; 2 failed`。H.264 通过；H.265/ProRes 在 post-encode probe 阶段遇到输出路径消失/ffprobe exit 1。
   - `--test-threads=1` 首次复跑 exit code：`101`；`2 passed; 1 failed`。H.264/H.265 通过，ProRes 输出路径在校验时消失。
   - `probe_export_prores` 隔离复跑 exit code：`0`；`1 passed; 0 failed; 2 filtered out`，`prores,60`。
   - 全组单线程最终复跑 exit code：`0`；`3 passed; 0 failed; 0 ignored`，H.264/H.265/ProRes 均为 60 帧。
   - 判定：probe 使用固定的全局临时路径（`opentake-export-probe.*`），共享工作区/跨进程并发时可被另一个运行删除；最终串行及隔离证据证明三编码产品路径可用，但测试夹具不具备跨进程隔离性。

5. `cargo test -p opentake-tauri --test playback_probe probe_prores_playback -- --ignored --nocapture --test-threads=1`
   - 首轮 exit code：`101`；`0 passed; 1 failed; 3 filtered out`；46 帧/2s，低于门槛 48。
   - 复跑 exit code：`0`；`1 passed; 0 failed; 3 filtered out`；56 帧/2s，`nonblack_ratio=1.000`。
   - 判定：真实 ProRes 解码/合成像素可用；吞吐有 46–56 帧/2s 抖动，门槛余量不稳定。

6. `cargo test -p opentake-tauri --test playback_probe probe_color_grade_visible_in_playback -- --ignored --nocapture --test-threads=1`
   - exit code：`0`
   - 计数：`1 passed; 0 failed; 3 filtered out`
   - 探针值：43 帧/1.5s（门槛 30），灰度调色 `max_channel_dev=0`。

## 逻辑与接线证据

### 多轨与整数帧

- `crates/opentake-render/src/plan/types.rs:149-165`：`RenderPlan` 的 `fps`/`total_frames` 为 `i32`；`ClipPlan.start_frame/end_frame` 也是整数且 end 为半开端点；单帧入口为 `frame(..., f: i32)`。
- `crates/opentake-render/src/plan/build.rs:240-246`：视频轨按 `blend_path` 逆序排序，先画高轨索引、后画 track 0，保证 track 0 在顶层。
- `crates/opentake-render/src/plan/build.rs:510-547`：source frame 只在边界处以明确 half-away-from-zero 规则映射到 `i64`，并对 trim/speed/reverse 做窗口钳制。
- 直接测试：`multitrack_blend_order_track0_above_track1`、`two_tracks_top_layer_wins_when_opaque`、`half_opacity_two_track_blend_matches_hand_computed`、`render_loop_composites_two_tracks_concurrently`、全部 source-frame integer tests 均通过。

### preview/export 共用 RenderPlan 与 compositor

- 暂停预览：`src-tauri/src/render.rs:882-947` 构建 `try_build_render_plan`，取 `plan.frame`，调用 `render_to_rgba_with_interpolation`；插值为 OpticalFlow，fallback 为 Blend。
- 导出：`src-tauri/src/export.rs:1410-1515` 构建同一 `try_build_render_plan`，逐整数帧取 `plan.frame`，调用同一 `render_to_rgba_with_interpolation`；参数同为 OpticalFlow + Blend。
- 连续播放：`src-tauri/src/playback/engine.rs:271-383` 在会话内持有 `RenderPlan`，每个目标整数帧调用 `plan.frame` 和相同 `Compositor::render_to_rgba`；连续播放采用 compositor 的 passthrough/nearest 默认而非暂停预览/导出的 OpticalFlow 配置，这是明确的性能策略差异，不是 preview/export drift。
- 直接 parity 测试：compound、nested、transition、stabilization、optical-flow、LUT、effect/mask golden 等 preview/export 相等断言全部通过。

### GPU 自动跳过边界

- `gpu_smoke.rs`、`gpu_effects.rs`、`gpu_text.rs`、`gpu_y_orientation.rs`、`pixel_diff.rs` 等均通过 `RenderDevice::try_new()` 获取设备；无适配器时打印 `[skip]` 并 early return，不把平台缺 GPU 误报为逻辑失败。
- 本机为 macOS/Metal，所有上述 GPU 用例实际执行；因此本次只能证明 Metal 后端，不能替代 Windows/DX12 或 Linux/Vulkan/软件适配器验证。

## Blocker 与未覆盖边界

- **发布阻断项：无（限本模块正确性与本机 Metal/FFmpeg 路径）。**
- 警告：ignored export probe 固定 `/tmp` 文件名，跨进程并发不安全；发布验收应使用 `--test-threads=1` 且避免另一个进程同时运行同一 probe，或后续把夹具改为唯一 tempdir。
- 警告：ProRes 实时吞吐出现一次 46/48 失败；像素路径通过，但若 48 帧/2s 是硬发布性能门槛，需在空闲机器/候选包上重复采样，而不能凭一次复跑关闭性能风险。
- 未运行 `probe_realtime_playback_with_audio_or_safe_fallback` 与 Main10 probe：环境未提供 `OPENTAKE_PROBE_VIDEO` / `OPENTAKE_MAIN10_FIXTURE`；因此未证明真实音频 callback、A/V 同步或 4K HEVC Main10。
- 未做 GUI/候选 app 包目视验收，也未做 Windows/Linux GPU 后端；发布说明已明确代码测试不能替代这些边界。
