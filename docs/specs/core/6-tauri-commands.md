## 6. Tauri command 表面(精确签名草案)

Tauri 只负责参数解析、边界校验、路由和序列化；时间线业务规则仍由 `AppCore::apply` 单一持有。注册表的唯一真相源是 `src-tauri/src/lib.rs` 中的 `tauri::generate_handler!`，前端生产桥接是 `web/src/lib/api.ts`、`libraryApi.ts` 和 `haptic.ts`。`api.commandContract.test.ts#frontend_command_names_match_invoke_handler` 静态比对两侧，任何前端字面量命令没有注册 handler 都会使 CI 失败。

### 6.1 命令清单

当前默认构建注册 81 个 handler：

| 领域 | 已注册命令 |
|---|---|
| 时间线/历史 | `get_timeline`, `edit_apply`, `undo`, `redo`, `can_undo`, `can_redo` |
| 工程/首页/样例 | `project_new`, `project_open`, `project_save`, `get_default_project_dir`, `check_path_exists`, `home_projects_sync`, `home_project_register`, `home_project_remove`, `home_project_trash`, `home_project_reveal`, `sample_project_materialize` |
| 交换与导出 | `export_xmeml`, `export_fcpxml`, `export_fcpxml_modern`, `export_edl`, `export_otio`, `export_subtitles`, `export_video`, `save_range_as_media`, `cancel_export` |
| 工程媒体 | `import_folder`, `import_media`, `relink_media`, `get_media`, `toggle_favorite`, `sync_project_favorites`, `save_clip_as_media`, `extract_audio`, `get_waveform`, `generate_thumbnail`, `request_timeline_sprite`, `set_timeline_sprite_interactive`, `preview_poster`, `preload_media` |
| 预览/播放 | `snap_haptic`, `composite_frame`, `cancel_composite_frame`, `capture_frame_to_media`, `playback_start`, `playback_pause`, `playback_stop`, `playback_seek`, `get_preview_endpoint` |
| 生成/字幕/搜索 | `generation_cancel`, `generation_retry`, `transcribe_model_status`, `download_transcribe_model`, `transcribe_media`, `transcript_get`, `generate_captions`, `search_model_status`, `download_search_model`, `search_index_status`, `search_index_start`, `search_query` |
| 密钥/账户/聊天 | `secret_save`, `secret_load`, `secret_delete`, `account_set_backend_url`, `account_get_backend_url`, `account_login`, `account_logout`, `account_get_status`, `chat_send`, `chat_history`, `chat_sessions`, `chat_session_set_open`, `chat_cancel` |
| 全局素材库 | `library_list`, `library_favorite`, `library_unfavorite`, `library_categorize`, `library_rename`, `library_delete`, `library_import_to_project` |

`playback_*` 与 `get_preview_endpoint` 受 `playback-engine` feature 控制；这些 handler 在默认桌面构建中启用。历史上的 `seek`、`play`、`pause`、`export_start`、`clip_copy`、`clip_paste`、`project_save_as` 和 `set_timeline_settings` 独立命令已不是契约：播放使用 `playback_*`，编辑使用 `edit_apply`，另存为 `project_save { path }`。

### 6.2 关键参数类型(对齐上游)

`edit_apply` 只接收 `{ command: EditRequest }`。`EditRequest` 是以 camelCase `type` 为 tag 的 41 分支严格联合，字段使用 camelCase，未知字段直接拒绝。它与 `web/src/lib/types.ts#EditRequest` 一一对应，并由 `commands.rs#every_edit_request_maps_to_exact_edit_command` 验证每个分支只转换为同名 `EditCommand`。

成功返回 `EditResultDto`：

```ts
type EditResult = {
  changed: boolean;
  actionName: string;
  affectedClipIds: string[];
  timelineVersion: number;
  summary: string;
};
```

`get_timeline` 返回 `{ timeline, projectEpoch, version, projectPath, compatibilityReadOnly, compatibilityBlockers }`。导出、媒体、播放等命令的请求/响应 DTO 以各自 Rust `#[derive(Deserialize, Serialize)]` 结构和同名 TypeScript wrapper 为准，不再维护一份可漂移的伪代码签名。

### 6.3 错误约定

核心编辑、undo/redo 和保存边界返回稳定结构：

```ts
type CommandError = {
  code: "validation" | "internal";
  message: string;
};
```

前端将该 DTO 投影为 `TauriCommandError`，保留 `code` 并让 `message` 可直接显示。输入校验失败必须在任何 timeline/version/event 副作用前返回 `validation`；内部失败返回 `internal`，且面向前端的消息不泄漏内部路径或凭据。`opentake-core::dto::edit_apply_handler_maps_validation_error` 同时断言 timeline、version、project identity 和事件数均不变。

### 6.4 `edit_apply` 与 agent 工具的关系(避免重复定义)

UI 通过 Tauri `edit_apply` 进入；Agent/MCP 在 Rust 进程内直接持有 `AppCore`。两者最终都只调用 `AppCore::apply`，因此共享同一个校验、原子提交、undo 和事件语义；Agent 不绕过核心实现另写一套编辑逻辑。
