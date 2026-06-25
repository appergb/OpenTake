## 8. 实施清单

> 与 ROADMAP 对齐:opentake-core 横跨 Phase 1(命令路由/事务/version,可纯逻辑单测)与 Phase 6/7(Tauri 边界 + 事件桥)。下列按"先纯逻辑、可对拍,再接边界"排序(ROADMAP 关键里程碑顺序)。

### 阶段 A — 纯逻辑核(随 Phase 1,无 IO,可全单测)
1. **`EditorState` 结构**(§1.2):字段 = timeline+manifest+generation_log+undo+version+assets+project 引用。去掉一切 UI-only 态。
   - 验证:构造空 state,断言 `version == 0`、`dirty == false`。
2. **`EditorCore` + `Arc<Mutex<EditorState>>`**(§1.3),`Clone` 只复制 Arc。
   - 验证:克隆两个句柄,一个 `apply` 后另一个 `get_timeline` 读到新 `version`(共享同一 state)。
3. **`EditorCore::apply` 事务**(§2.3),逐句对应 `withTimelineSwap`:before 快照 → ops::apply → `before==after` 短路 → push undo + version+1 → emit。
   - 验证(对拍):喂上游导出的 `project.json`,跑等价 `EditCommand` 序列,断言结果 timeline 帧级一致(ROADMAP Phase 1「对拍测试」)。
   - 验证(不变量):无变更命令 `changed==false` 且 version 不变、无事件;有变更命令 version 恰好 +1。
4. **`undo()/redo()`**(§2.4):基于 `UndoStack`,**撤销也 bump version + emit**。
   - 验证:apply→undo 回到 before 且 version 递增;redo 回到 after;空栈 undo 返回 `NothingToUndo`。
5. **`EventBus`**(§3.1):`tokio::broadcast`,`emit` 无订阅者不 panic。
   - 验证:subscribe 后 apply,收到 `TimelineChanged{version}`,version 与返回值一致。

### 阶段 B — 装配(随 Phase 2/4/5,接 deps)
6. **`CoreDeps` trait 句柄**(§5.2):project/media/preview/export/gen,先用 mock 实现跑通 core,再接真实 crate。
7. **`open()/save()`**(§5.3/§5.4):照搬 `VideoProject` 装配顺序(timeline→manifest→物化 assets→generation_log);容错 decode(`#[serde(default)]`)。
   - 验证:打开上游导出工程还原 timeline(ROADMAP Phase 2);往返 save/open 无损;缺 manifest 不致命。
8. **`seek()`**(§3.4):钳制 `[0,total_frames]`(照搬 `seekToFrame`),透传 preview 后端。
9. **`import_media()`**(§5.3):落地 → manifest entry → 物化 asset → 异步缩略图/波形 → `MediaImported` 事件。
10. **`export_start()`**(§6.2):起后台 job,流式 `ExportProgress/Done/Failed`。

### 阶段 C — Tauri 边界(随 Phase 6)
11. **`#[tauri::command]` 薄封装**(§6.1):8+3 命令,body 仅取 State→调 core→map `CmdError`。
12. **事件桥 task**(§3.2):src-tauri 启动时 `subscribe()`,把 `CoreEvent` map 成前端 `emit`。
13. **`preview_frame` 像素旁路**(§3.3):与 render 后端协商 Channel/asset 协议(契约在 core,传输在 src-tauri+render)。
14. **`TimelineDTO`**(§4.4):serde 复用 domain 序列化(= project.json schema)。
15. **前端同步协议**(§4):Zustand `{mirror, mirrorVersion}` + `timeline_changed` 幂等重取(前端代码,Phase 6;core 侧只保证 version 契约)。

### 阶段 D — Agent 接入点(随 Phase 7,core 仅需就绪)
16. 确认 `EditorCore` 句柄可被 `opentake-agent` 克隆持有;agent 工具层把工具 args → `EditCommand` → `core.apply`(§2.5)。core **不**改动——它对 agent 是只读 API 契约。
17. 助手专属 undo 的 `AgentUndoCursor` 在 agent 层实现,基于 core 的通用 `undo()` + version 追踪(§2.4),复刻 `ToolExecutor.undo` 拒绝语义(`ToolExecutor.swift:108-123`)。

### 横切验证(贯穿)
- **并发一致性集成测试**(§4.3):N 路并发 `apply`,断言最终 version == 命令次数(全变更时)、前端镜像内容与 core 一致。
- **覆盖率 ≥ 80%**(CLAUDE.md testing)。事务核(§2.3)与 version 不变量是重点。
- **不变量断言固化为测试**:version 单调;无变更不发事件;撤销/重做 version 行为(§1.2、§2.4)。

---

## 附录:opentake-core 不做什么(边界纪律)

- **不含编辑算法**:overlap/ripple/split/keyframe 求解全在 `opentake-ops`(§2.2)。core 只起事务、调 `ops::apply`。
- **不含 UI 态**:selection/zoom/panel/scrub 在前端 Zustand(§1.1)。
- **不含网络**:MCP server / LLM 客户端在 `opentake-agent`(§7)。core 无端口、无外部输入面。
- **不含像素/解码/编码**:全在 `opentake-render` / `opentake-media`,经 `CoreDeps` 注入(§5.2)。core 只持句柄、定事件契约。
- **不含撤销算法细节**:`UndoStack`(整树快照)在 `opentake-ops`;core 只调用。
- 一句话:**core = 装配 + 路由 + 事务 + 版本 + 事件**,其余皆为它编排的对象。
