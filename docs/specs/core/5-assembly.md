## 5. 与 project / media / render / playback / agent 的装配关系

本节描述当前可执行代码，而不是未来接口草图。`opentake-core` 只直接依赖
`opentake-domain`、`opentake-ops` 和 `opentake-project`；桌面壳
`src-tauri` 才同时装配 media、render、playback 与 agent。UI、Agent、MCP
都是同一个 `AppCore` 会话的客户端，不能各自持有或替换编辑状态。

### 5.1 真实依赖方向

```text
opentake-domain <- opentake-ops
       ^                 ^
       |                 |
opentake-project         |
       ^                 |
       +------ opentake-core <------ opentake-agent
                    ^                    ^
                    |                    |
                    +------ src-tauri ---+
                              |
                              +-- opentake-media
                              +-- opentake-render
                              +-- playback
```

- `opentake-core` 拥有 `EditorSession`、统一 `EditCommand` 入口、项目
  open/save、版本、epoch 和 `CoreEvent`。
- `opentake-project` 拥有 `.opentake` bundle 的能力根、兼容解码和原子写入。
- `src-tauri` 在应用边界协调 project transition、playback、prewarm、render、
  media 与 Agent；这些具体 crate 不反向进入 core。
- `opentake-agent::mcp::CoreHandle` 是 `agent -> core` 的窄客户端接口。
  `AppCoreHandle` 委托同一个 `AppCore`，因此 Agent 读取到的 timeline、
  manifest 和 project directory 与 UI 一致。

这个方向是编译期约束：core 不能依赖 Agent，也不能在 open 中构造 Tauri、
GPU、provider 或 UI 类型，否则会产生反向依赖或循环。

### 5.2 `CoreDeps` 的当前边界

`crates/opentake-core/src/deps.rs::CoreDeps` 当前只有 `preview`、`export`、
`media`、`gen` 四个历史能力 seam。默认实现会返回明确的
`CoreError::Unsupported`，不会 panic，但生产装配没有通过这些字段执行
project open、render、playback 或 Agent 工作。

尤其需要区分：

- 源码中不存在 `ProjectStore` 字段。
- `AppCore::prepare_project_open` 不调用 `CoreDeps`。
- render、playback 和 media 的生产入口位于 `src-tauri`，从一个
  `ProjectRuntimeSnapshot` 或 `AppCore` clone 读取状态。
- Agent 通过 `CoreHandle` 访问 core；它不是 core 的 injected dependency。

因此，测试不能通过调用一组 fake `CoreDeps` 来声称已经覆盖 production
project-open 路径。该 seam 的 unsupported 结果由 `deps` 自身单测负责；
真实 open 的依赖协调必须在 Tauri owning boundary 测试。

### 5.3 当前装配点

| 装配点 | 当前实现符号 | 契约 |
|---|---|---|
| bundle 读取 | `ProjectRoot::open` → `Project::open_from_root` | 在一个保留的 bundle capability 下解码 timeline、manifest、generation log |
| session 构造 | `EditorSession::open_project` | 只有全部拥有的组件处理完成后才构造 version 0、空 history 的候选 session |
| 原子发布 | `AppCore::prepare_project_open` / `commit_project_open` | prepare 不触碰 live session；commit 在锁内一次替换并递增 project epoch |
| 桌面 open | `commands::project_open_with_playback_and_prewarm` | prepare 后取得 playback/prewarm transition，随后 commit，再激活新 epoch |
| media 读取 | `MediaListDto::from_core`；Agent `AppCoreHandle::media_path` | manifest 与 project directory 通过一次 `AppCore::runtime_snapshot` 组合，项目切换不能混读 |
| render | `src-tauri::render::composite_frame` | 用户请求时从一次 `runtime_snapshot` 构建 `RenderPlan`；不是 open preflight |
| playback | `playback_start` / `PlaybackState` | 用户播放时解析媒体、准备 engine；project open 只负责 epoch transition |
| prewarm/cache | `PrewarmScheduler` | open 时取消旧 epoch 工作并切换身份；新缩略图/波形是后续有界异步任务 |
| Agent/MCP | `AppCoreHandle` / `CoreHandle` | 读取和编辑都委托 live `AppCore`，不直接修改前端或私有 Agent 状态 |
| save | `EditorSession::save_project` → `Project` | 从同一 session 快照写 timeline、manifest、generation log，并保留 bundle media |

RenderPlan、GPU、解码器、MCP listener 和模型 provider 都不是项目文件的一部分。
把这些懒加载或应用级资源当成同步 open 的必备依赖，会让一个可读项目因为
暂时不可用的运行时能力而无法打开，也不符合当前 production 调用链。

凡是需要组合 timeline、manifest 与 project directory 的消费者（media DTO、
caption/transcribe/search、preview/export、MCP media bridge、Agent media path），
都必须先取得一次 `AppCore::runtime_snapshot`；分别调用多个单字段 getter 会在
并发项目切换时混合两个 project epoch。

长时间运行后还要写回编辑状态的工作流必须额外保留 snapshot 的
`ProjectRevision`，并通过 `AppCore::apply_at_revision` 在同一把 session 锁下
校验 epoch/version 后提交。这样转写和排版基于 project A 时，即使用户已经
打开或编辑 project B，也只会得到显式冲突而不会把旧结果写入新项目。只读
批处理（例如 MCP 多源转写）则在整个批次中复用同一个 snapshot。

### 5.4 Project open 的提交顺序

真实顺序如下：

1. playback build 向 `PlaybackState` 查询 transition admission；重叠 transition
   立即返回结构化 `busy`，尚未读取或替换 core。
2. `AppCore::prepare_project_open` 打开 bundle capability，并通过
   `EditorSession::open_project` 完整处理 timeline、manifest、generation log
   和 compatibility 状态。该步骤只产生一个私有 `PreparedProjectOpen`。
3. Tauri 取得 playback transition，再取得 prewarm transition。prewarm 拒绝时
   会取消刚取得的 playback transition，live core 仍未 commit。
4. `AppCore::commit_project_open` 在 project-identity 写锁和 session mutex 下
   一次替换 editor，递增 `project_epoch`，然后在锁外发出唯一
   `CoreEvent::ProjectOpened`。新 session 的 document `version` 为 0。
5. playback 与 prewarm 激活同一个新 epoch。activate 是无失败提交步骤。
6. UI、Agent、render、playback 和异步 prewarm 在后续请求中读取新 snapshot。

这个顺序建立了明确的线性化点：`commit_project_open` 之前，旧 session 和旧
运行时资源仍然有效；之后，所有客户端以新 epoch 拒绝旧工作。open 不发
`TimelineChanged`，因为 `ProjectOpened` 后客户端会主动取得首个 snapshot。

### 5.5 文件容错与恢复矩阵

| 组件 | 缺失 | 损坏或无法理解 | 保存行为 |
|---|---|---|---|
| `project.json` | typed `MissingTimeline`，open 失败 | typed JSON/IO error，open 失败 | 不发布 session，不写任何 bundle 文件 |
| `media.json` | 使用显式空 `MediaManifest` | typed JSON/IO error，open 失败 | 不发布 session，不用空 manifest 覆盖原文件 |
| `generation-log.json` | 从 manifest 中的 generation provenance 确定性补种 | 以补种状态打开，并记录 `generation-log.json:invalid-or-unreadable` compatibility blocker | recovery session 只读，拒绝覆盖损坏日志 |
| 未知的持久化字段 | 已知字段正常读取 | 记录 file-qualified compatibility blocker | session 可读但保存被拒绝，防止静默丢字段 |

缺失的 optional 组件与损坏的组件不是同一种状态。`#[serde(default)]` 只负责
有契约的旧字段默认值，不能把严格组件的 parse error 吞成空值。

### 5.6 可执行验收证据

- `opentake-project/tests/upstream_compat.rs::exhaustive_legacy_default_matrix`
  覆盖 current/legacy/default/未知字段兼容以及 save/reopen。
- `opentake-core/tests/project_open.rs::missing_generation_log_seeds_manifest_provenance_once`
  覆盖缺失/损坏 generation log、确定性补种、只读保护和重开。
- `opentake-core/tests/project_open.rs::project_open_composite_acceptance`
  覆盖 AppCore prepare/commit、typed malformed-media failure、旧 session/epoch/event
  与 bundle byte receipt 保持、optional 组件、只读 recovery、save/reopen。
- `opentake-agent::mcp::core_handle::tests::app_core_media_path_stress_never_mixes_project_snapshots`
  在并发切换项目的压力场景下持续检查 production CoreHandle 只返回成对路径；
  原子性本身由 `AppCoreHandle::media_path` 的单次 `runtime_snapshot` 实现保证。
- `opentake-core::core::tests::deferred_apply_rejects_version_and_project_drift_without_mutation`
  与 `captions::tests::caption_commit_rejects_stale_project_revision` 覆盖长任务的
  stale epoch/version 写回拒绝。
- `mcp::tests::transcript_batch_resolution_uses_one_snapshot_and_authoritative_types`
  覆盖 MCP 批处理对多个 source 复用同一 snapshot，并以该 snapshot 的媒体类型
  覆盖调用方可能过期的 type hint。
- `commands::project_prewarm_lifecycle_tests::project_open_mapped_boundaries_composite_acceptance`
  用同一个已提交 fixture 贯通 UI media lookup、render plan、playback media
  projection、Agent CoreHandle、save 和 reopen。
- `playback::commands::tests::prewarm_rejection_restores_active_playback_without_project_publish`
  证明 prewarm 拒绝会恢复既有 publication/控制路径，且不会发布候选 project；
  prepared 值由失败返回路径按所有权释放，同模块的 prepare/overlap 测试覆盖其余
  admission failure。
- playback、render 与 Agent 的具体运行能力继续由各自 owning crate 的测试负责；
  它们不通过 core fake 或一次 project-open 测试冒充覆盖。

以上测试共同形成复合验收。任何新增同步 open 依赖都必须遵守同一规则：
在 core commit 前完成可失败的 admission/prepare；失败时无 `ProjectOpened`、
无 session/epoch 替换、无 bundle 写入，并清理自己取得的 transition/resource。
