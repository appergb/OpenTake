## 5. 与 ops / project / render / agent 的装配关系

`opentake-core` 是**装配中枢**(ARCHITECTURE §3:「`opentake-core/` # 组装:EditorState…、command 路由、事件总线」)。依赖法则(ARCHITECTURE §3 末「依赖法则」):`domain` 零依赖叶子;`ops` 只依赖 `domain`;`command` 是唯一编辑入口;UI/Agent/MCP 是三个对等客户端。

### 5.1 依赖方向(谁依赖谁)

```
opentake-domain  ← ops ← core
                  ↑        ↑  ↑  ↑
            project ──────┘  │  │   (core 持 project 句柄做 open/save)
            render ─────────┘  │   (core 持 render 句柄做 seek/preview/export)
            media  ────────────┘   (core 持 media 句柄做 import/缩略图/波形)
            agent  →  core           (agent 依赖 core,反向:agent 是 core 的客户端)
            src-tauri → core, agent  (Tauri 装配二者)
```

- **core 依赖** `domain`(类型)、`ops`(`EditCommand`+`apply`)、`project`(读写)、`render`(seek/export)、`media`(import/物化)。
- **agent 依赖 core**(agent 是 core 的客户端,持 `EditorCore` 句柄,把工具翻译成 `EditCommand`)。**core 不依赖 agent**(单向),否则成环。
- **`src-tauri` 装配** core + agent + 前端,注册 `#[tauri::command]` 与事件桥(§3.2)。

### 5.2 `CoreDeps`:注入而非硬连(可测、解耦)

core 不直接 `use` render/media/project 的具体实现函数,而是持 trait 句柄(便于 Phase 1 单测时 mock,符合 CLAUDE.md 依赖注入/可测性):

```rust
// crates/opentake-core/src/deps.rs
pub struct CoreDeps {
    pub project: Arc<dyn ProjectStore>,   // open/save .opentake(opentake-project,Phase 2)
    pub media:   Arc<dyn MediaImporter>,  // 导入 + 物化 assets + 缩略图/波形(opentake-media,Phase 2)
    pub preview: Arc<dyn PreviewBackend>, // request_frame → emit PreviewFrame(opentake-render,Phase 4)
    pub export:  Arc<dyn ExportBackend>,  // start_export → emit ExportProgress(opentake-render,Phase 5)
    pub gen:     Option<Arc<dyn GenBackend>>, // BYOK/托管生成(opentake-gen,Phase 9;前期 None)
}
```

### 5.3 各装配点对应的上游证据

| 装配点 | core 做什么 | 上游对应 |
|---|---|---|
| **ops** | `apply` 内调 `ops::apply(&mut timeline, &cmd, &assets)`(§2.3 步骤 2) | `ToolExecutor.run` 各 case → `EditorViewModel` mutator(`ToolExecutor.swift:72-106`) |
| **project.open** | `open(dir)`:读 project.json/media.json/generation-log.json → 填 `EditorState`,调 media 物化 assets,version 归 0 | `VideoProject.read`(`:31-64`)+ `makeWindowControllers`(`:186-255`)装配顺序 + `restoreAssetsFromManifest`(`:304-339`) |
| **project.save** | `save()`:把 `timeline/manifest/generation_log` 序列化进 `.opentake` 目录 | `VideoProject.captureSaveSnapshot`(`:99-110`)+ `fileWrapper`(`:75-97`) |
| **media.import** | `import_media`:落地媒体 → 加 manifest entry → 物化 `MediaAsset` → 触发缩略图/波形(异步) → emit `MediaImported` | `ToolExecutor+Import.swift` + `VideoProject.swift:323-332`(restore 时生成缩略图/波形) |
| **render.seek/preview** | `seek` 透传 render 后端,后端 emit `PreviewFrame`(§3.4) | `EditorViewModel.seekToFrame`(`:259-267`)→ `videoEngine?.seek` |
| **render.export** | `export_start` 起后台导出 job,流式 emit `ExportProgress/Done/Failed` | `ExportService.export(format:resolution:)`(`ExportService.swift:73-159`) |
| **generation_log** | AI 生成成功后 append `generation_log`(append-only) | `EditorViewModel.generationLog`(`:31`)+ `seedGenerationLogFromAssets`(`VideoProject.swift:246`) |

### 5.4 装配顺序(open 流程,照搬上游 `makeWindowControllers`)

`VideoProject.swift:186-255` 的顺序是经实战的,必须照搬到 `EditorCore::open`:
1. 读并 decode `timeline`(`:31-42`)→ 设 `state.timeline`,`version = 0`。
2. 设 `project_dir` / 派生 `project_id`(`:192` + `EditorViewModel.swift:116-125`)。
3. decode `manifest`(`:43-50`)→ `state.manifest` → **从 manifest 物化 `assets`**(`restoreAssetsFromManifest`,`:304-339`):每个 entry 解析 URL → `MediaAsset` → 文件存在则触发波形/缩略图(异步)→ `loadMetadata`。
4. decode `generation_log`(`:51-53`);缺失则 `seed_generation_log_from_assets`(`:246`)。
5. `search_index.project_opened()`(`:248`,Phase 8 才实装)。
6. 不发 `timeline_changed`(open 是初始化,前端 open 后主动 `get_timeline`)。

> **容错**:所有 decode 用 `#[serde(default)] + Option`(ARCHITECTURE §9、§4「向后兼容容错解码」);`manifest`/`generation_log` 缺失或损坏**不**致命(上游 `loadedGenerationLog = try?`,`:52`),只 `timeline` 缺失才报错(`VideoProject.swift:32-34`)。

---
