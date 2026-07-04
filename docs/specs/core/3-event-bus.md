## 3. 事件总线

### 3.1 `EventBus` 与事件类型

上游靠 SwiftUI `@Observable` 自动传播(`EditorViewModel.swift:21-22` 的 `@Observable`),无显式事件总线。OpenTake 跨进程,需要显式总线把 core 的状态变更推到 Tauri 边界,再由 Tauri 转成前端 `emit`。

```rust
// crates/opentake-core/src/events.rs
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoreEvent {
    TimelineChanged { version: u64 },                          // 对应 §2.3(5) / notifyTimelineChanged
    PreviewFrame { frame: i64, width: u32, height: u32 },      // 预览帧就绪(像素经 §3.3 旁路)
    ExportProgress { job_id: String, progress: f64,            // 0.0..1.0
                     phase: ExportPhase, eta_secs: Option<f64> },
    ExportDone { job_id: String, output_path: String },
    ExportFailed { job_id: String, message: String },
    GenerationProgress { job_id: String, status: GenStatus },  // 见 ARCHITECTURE §8 job 状态机
    MediaImported { asset_id: String },                        // 对应 mediaPanelRevealAssetId 流(AppState.swift:90-101)
}

pub struct EventBus { tx: tokio::sync::broadcast::Sender<CoreEvent> }
impl EventBus {
    pub fn emit(&self, ev: CoreEvent) { let _ = self.tx.send(ev); }   // 无订阅者不 panic
    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> { self.tx.subscribe() }
}
```

> 用 `tokio::broadcast`:Tauri 桥接 task 订阅一份转发给前端;未来其他订阅者(如自动保存、遥测)可独立订阅,不互相阻塞。`emit` 永不阻塞命令路径(§2.3 在 `drop(st)` 后才 emit)。

### 3.2 Tauri 桥接(src-tauri,薄)

`src-tauri` 启动时 `subscribe()` 一次,起一个 task 把 `CoreEvent` 映射成 Tauri 前端事件名:

| CoreEvent | Tauri event name | payload |
|---|---|---|
| `TimelineChanged{version}` | `"timeline_changed"` | `{ version: number }` |
| `PreviewFrame{..}` | `"preview_frame"` | `{ frame, width, height }`(像素见 §3.3) |
| `ExportProgress{..}` | `"export_progress"` | `{ jobId, progress, phase, etaSecs }` |
| `ExportDone/Failed` | `"export_progress"` | 同上,`phase: "done"\|"failed"` |
| `GenerationProgress` | `"generation_progress"` | `{ jobId, status }` |
| `MediaImported` | `"media_imported"` | `{ assetId }` |

> ARCHITECTURE §2 只点名了三个核心 event(`timeline_changed{version}` / `preview_frame` / `export_progress`)。本规格把它们定全签名,并补 `generation_progress` / `media_imported`(从上游 `GenerationService`、`mediaPanelRevealAssetId` 流推得,属同类"core→前端单向通知")。`generation_progress` 在 Phase 9 才实装,Phase 6/7 可只发前三。

### 3.3 `preview_frame` 的像素旁路(关键工程决策)

`preview_frame` **事件本身只带元数据(frame/width/height),不带像素**。原始 RGBA 帧不走 Tauri 事件(JSON 序列化大帧会卡)。三选一(按 ARCHITECTURE §2「Preview(canvas 显示 Rust 合成帧)」):

1. **共享内存 / 自定义 URI scheme**:core 把帧写入 Tauri `asset://` 或自定义协议端点,前端 `<canvas>` 经 `<img>`/`createImageBitmap` 拉。
2. **IPC 二进制通道**:Tauri 2 的 `Channel<&[u8]>`(比 event 高效,适合连续帧)。
3. **WebGL 直接上屏**(ARCHITECTURE §2 提到的 WebGL):core 出纹理句柄,前端 GL 直采。

> 本 crate 只定义**契约**:`PreviewFrame` 事件 = "第 N 帧已就绪,去拉",像素传输属 `opentake-render` 播放后端(Phase 4)与 src-tauri 协商的实现细节,不在 core 逻辑内。core 的 `seek` 命令(§3.4)触发 render 后端解码合成,render 后端就绪后经 `events.emit(PreviewFrame{..})` 通知。

### 3.4 谁触发 `preview_frame`

`seek` / 播放不属于 `EditCommand`(它们不改 timeline、不进撤销栈)。core 暴露独立的 `seek(frame)` API:

```rust
impl EditorCore {
    pub fn seek(&self, frame: i64, mode: SeekMode) {
        // 对应 EditorViewModel.seekToFrame(:259-267):钳制到 [0, totalFrames]
        let clamped = { let st = self.state.lock().unwrap();
                        frame.clamp(0, st.timeline.total_frames()) };
        // 交给 render 播放后端(§5 deps);后端合成完单帧后 emit PreviewFrame
        self.deps.preview.request_frame(clamped, mode);
    }
}
```

> `total_frames()` 是 domain 派生函数(`Timeline.swift:16`)。钳制语义照搬 `seekToFrame`(`EditorViewModel.swift:260`:`min(max(0, frame), max(0, totalFrames))`)。`SeekMode`(exact / interactiveScrub)对应上游 `PreviewSeekMode`,scrub 节流(30Hz)在 render 后端做(Phase 4),core 只透传。

---
