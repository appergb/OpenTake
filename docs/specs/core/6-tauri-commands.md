## 6. Tauri command 表面(精确签名草案)

> 全部 `#[tauri::command] async`,在 `src-tauri` 定义,**薄**(ARCHITECTURE §2:「Tauri command 边界(薄胶水:序列化 + 路由)」),body 仅:取 `State<EditorCore>` → 调 core 方法 → map 错误。**零业务逻辑**。命名采用 ARCHITECTURE §2 列出的集合。前端命名 camelCase,Rust snake_case(Tauri 自动转换;DTO 字段用 `#[serde(rename_all="camelCase")]`)。

### 6.1 命令清单

```rust
// ───────── 读 ─────────
#[tauri::command]
async fn get_timeline(core: State<'_, EditorCore>) -> Result<TimelineSnapshot, CmdError>;
// → { timeline: TimelineDTO, version: u64 }  (§4.1 规则1)

// ───────── 写(唯一编辑入口) ─────────
#[tauri::command]
async fn edit_apply(core: State<'_, EditorCore>, command: EditCommand) -> Result<EditResult, CmdError>;
// command: 见 §2.2;返回 EditResult(含 timeline_version, changed, summary)

#[tauri::command]
async fn undo(core: State<'_, EditorCore>) -> Result<EditResult, CmdError>;  // 全局撤销(Cmd+Z),§2.4
#[tauri::command]
async fn redo(core: State<'_, EditorCore>) -> Result<EditResult, CmdError>;

// ───────── 工程生命周期 ─────────
#[tauri::command]
async fn project_open(core: State<'_, EditorCore>, path: String) -> Result<TimelineSnapshot, CmdError>;
// 打开 .opentake 目录;成功后返回首个快照(§5.4)。对应 AppState.openProject(:143-154)

#[tauri::command]
async fn project_save(core: State<'_, EditorCore>, path: Option<String>) -> Result<(), CmdError>;
// path=None: 存回 project_dir(对应 autosave);path=Some: 另存为。对应 VideoProject.save(:66-73)

// ───────── 播放 / 预览(不进撤销栈) ─────────
#[tauri::command]
async fn seek(core: State<'_, EditorCore>, frame: i64, mode: SeekMode) -> Result<(), CmdError>;
// 钳制 + 透传 render 后端;帧经 preview_frame 事件回(§3.4)。对应 seekToFrame(:259)

// ───────── 媒体导入 ─────────
#[tauri::command]
async fn import_media(core: State<'_, EditorCore>, source: ImportSource) -> Result<ImportedMedia, CmdError>;
// ImportSource = Path(String) | Url(String) | Bytes{name,data}(ARCHITECTURE Phase2「本地/URL/bytes」)
// 返回 { assetId, ... };异步缩略图/波形就绪后另发事件。对应 ToolExecutor+Import.swift

// ───────── 导出(后台 job + 进度事件) ─────────
#[tauri::command]
async fn export_start(core: State<'_, EditorCore>, opts: ExportOptions) -> Result<ExportHandle, CmdError>;
// 立即返回 { jobId };进度走 export_progress 事件(§3.2)。对应 ExportService.export(:73)
```

### 6.2 关键参数类型(对齐上游)

```rust
pub enum SeekMode { Exact, InteractiveScrub }   // 对应 PreviewSeekMode(EditorViewModel.swift:259)

pub enum ImportSource {                          // ARCHITECTURE Phase 2「本地/URL/bytes,扩展名白名单」
    Path(String),
    Url(String),
    Bytes { name: String, data: Vec<u8> },
}

pub struct ExportOptions {                       // 对应 ExportService.export(format:resolution:)
    pub format: ExportFormat,                    // H264 | H265 | ProRes | Xml(对应 ExportService.swift:5 / ExportView.swift:13-16)
    pub resolution: ExportResolution,            // R720p | R1080p | R4K(对应 ExportView.swift:26 默认 1080p)
    pub output_path: String,
}
pub enum ExportFormat { H264, H265, ProRes, Xml }
pub enum ExportResolution { R720p, R1080p, R4K }

pub struct ExportHandle { pub job_id: String }   // 后续进度经事件(§3.2)
```

> **导出表面证据**:`ExportService.swift:73-78` `func export(... format: ExportFormat, resolution: ExportResolution)`;`ExportService.swift:5` `enum {h264,h265,prores,xml}`;`ExportView.swift:13-26` `VideoCodec{h264,h265}` + `ExportResolution` 默认 `.r1080p`。OpenTake 把 `xml`(FCPXML 导出,`XMLExporter.swift:40`)保留为 `ExportFormat::Xml`(纯逻辑,无需 wgpu,可早做)。**预设码率/profile** 对齐属 Phase 5(ARCHITECTURE §6、ROADMAP Phase 5),core 只定 `ExportOptions` 契约。

### 6.3 错误约定

```rust
#[derive(Serialize)]
pub struct CmdError { pub code: String, pub message: String }  // code 机读,message 人读
```

- 校验失败(如越界帧、未知 clipId)→ `code: "validation"`,`message` 带**精确路径**(ARCHITECTURE §7:`entries[3].startFrame: missing required field`,用 `serde_path_to_error`)。对应上游 `formatDecodingError`(`ToolExecutor.swift:210-229`)与各工具的 `entries[idx]: …` 报错(`ToolExecutor+Clips.swift:148-167`)。
- core 内部错(IO/解码)→ `code: "internal"`,`message` 友好化,详细上下文记日志(CLAUDE.md 错误处理:UI 友好 + 服务端详细)。
- **错误不致命**:命令失败时 timeline 不变、version 不变、无事件(§2.3 步骤 2 早返回)——前端镜像保持一致。

### 6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)

UI 直接传 `EditCommand` 给 `edit_apply`。Agent/MCP **不**经 `edit_apply` Tauri 命令(它们在 Rust 进程内直接持 `EditorCore` 句柄,§2.5),但**最终汇入同一个 `EditorCore::apply`**。即:`edit_apply` Tauri 命令 = UI 客户端的入口;`EditorCore::apply` = 三客户端的共同汇聚点。二者不重复——前者是后者的一个调用方。

---
