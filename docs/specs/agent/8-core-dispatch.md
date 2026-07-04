# 与 opentake-core 的接口

> `opentake-agent` 不实现编辑算法；它把工具调用翻译成 `opentake-core` 的命令并消费结果。`opentake-core` 是「唯一编辑入口」，持有权威 `Timeline`（ARCHITECTURE §2 `:62`、§5 `:103-122`）。

## 8.1 命令枚举与结果（ARCHITECTURE `:105-116`，已定义在 `opentake-core`/`opentake-ops`）

```rust
enum EditCommand {
    AddClips{..}, InsertClips{..}/*ripple*/, MoveClips{..}, RemoveClips{..},
    SplitClip{clip_id, at_frame}, TrimClips{..}, SetClipProperties{..},
    SetKeyframes{clip_id, property, keyframes}, RippleDeleteRanges{..},
    AddTexts{..}, AddCaptions{..}, Link{..}, Unlink{..},
    RemoveTracks{..}, CreateFolder{..}, MoveToFolder{..}, Undo, Redo,
}
struct EditResult { changed: bool, action_name: String, affected_clip_ids: Vec<String>, timeline_version: u64, summary: String }
```
`command::apply` = 上游 `withTimelineSwap` 事务（`快照 → 改 → before!=after 才压 UndoStack 整树快照 → version+1 → 广播 timeline_changed`，ARCHITECTURE `:118-120`）。

## 8.2 `opentake-agent` 需要 `opentake-core` 暴露的接口（`CoreHandle`）

本 crate 通过一个 `CoreHandle`（`Arc<Mutex<EditorState>>` 或 actor mpsc）调用：

```rust
pub trait CoreHandle: Send + Sync {
    // 读
    fn timeline(&self) -> Timeline;                                   // 当前权威 Timeline（短 ID 宇宙、Context Signal 检测、execute 快照都用它）
    fn timeline_version(&self) -> u64;
    fn media_manifest(&self) -> MediaManifest;
    fn folders(&self) -> Vec<Folder>;
    fn media_assets(&self) -> Vec<MediaAsset>;
    fn current_frame(&self) -> u64;
    fn can_generate(&self) -> bool;                                   // get_timeline 的 canGenerate
    // 写（唯一入口）
    fn apply(&self, cmd: EditCommand) -> Result<EditResult, CoreError>;
    // undo 治理（§4.3）
    fn can_undo(&self) -> bool;
    fn undo_action_name(&self) -> Option<String>;
    // 媒体/转写/渲染/搜索/生成 —— 转发到对应 crate（opentake-media / opentake-render / opentake-gen）
    async fn inspect_media(&self, args: InspectMediaArgs) -> Result<ToolResult, CoreError>;
    async fn get_transcript(&self, args: GetTranscriptArgs) -> Result<ToolResult, CoreError>;
    async fn inspect_timeline(&self, args: InspectTimelineArgs) -> Result<ToolResult, CoreError>;
    async fn search_media(&self, args: SearchMediaArgs) -> Result<ToolResult, CoreError>;
    fn list_models(&self, kind: Option<ModelKind>) -> ToolResult;
    async fn submit_generation(&self, req: GenerationRequest) -> Result<ToolResult, CoreError>;
    async fn import_media(&self, src: ImportSource, name: Option<String>, folder: Option<String>) -> Result<ToolResult, CoreError>;
}
```

映射表（工具 → CoreHandle 调用）：

| 工具 | CoreHandle 调用 |
|---|---|
| `get_timeline` | `timeline()` + 压缩编码（§8.3） + `total_frames`/`current_frame`/`can_generate` |
| `get_media` | `media_manifest()` |
| `inspect_media` / `get_transcript` / `inspect_timeline` / `search_media` | 对应 async 转发（→ `opentake-media`/`opentake-render`） |
| `list_models` | `list_models(kind)`（→ `opentake-gen`） |
| add/insert/remove/move/split/trim/set_*/ripple/add_texts/add_captions/remove_tracks/create_folder/move_to_folder | `apply(EditCommand::...)` |
| `undo` | §4.3（`can_undo`/`undo_action_name`/`apply(Undo)`） |
| `generate_*`/`upscale_media` | `submit_generation(...)` |
| `import_media` | `import_media(...)` |
| `rename_media`/`rename_folder`/`delete_media`/`delete_folder` | `apply(...)` 或专用方法 |

## 8.3 get_timeline 编码（压缩规则，照搬 `ToolExecutor+Timeline.swift:17-112`）

`opentake-agent` 负责把 `Timeline` 编码成 LLM 友好 JSON（**省 token**）：
- 剥离等于默认值的字段：track 默认 `{muted:false, hidden:false, syncLocked:true}`（`:60`）；clip 默认 `{mediaType:"video", speed:1, volume:1, opacity:1, trims/fades:0, identity transform/crop, default textStyle}`；`sourceClipType` 等于 `mediaType` 时剥离（`:113-115`）；text clip 不报 trim（`:117-120`）。
- caption clip（共享 `captionGroupId`）折叠成 `captionGroups`：共享样式 hoist + 每 clip `[clipId, startFrame, durationFrames, text]` 行，**上限 200 行**（`:7-8` captionRowLimit/captionRowFormat、`compactTrack` 分组逻辑）。偏离组的 caption clip 单独列入 `clips`。
- keyframe 压成紧凑数组；浮点保留 **3 位**（`roundJSONFloatingPointNumbers(..., toPlaces: 3)`，`:46`）。
- 窗口分页：`startFrame`/`endFrame` → 只返回相交 clip；被窗口隐藏时报 `totalClips`/`totalFrames`（`:32-44`）。
- track 报**显示标签**（镜像视频编号），非存储 seed（`:38`）。

> 这层是 `opentake-agent` 的职责（不是 `opentake-core`），因为它是「面向 LLM 的表示」，与短 ID 缩短（§3.3）同属出站表示层。

## 8.4 跨进程一致性（ARCHITECTURE §2 `:62`）

OpenTake 跨进程：Rust 持有权威 `Timeline`，前端拿快照 + 单调递增 `timeline_version`；每次 `apply` 广播 `timeline_changed{version}`。**MCP/chat 工具结果里的 ID/帧位在下一次编辑前可能因前端或另一前端的编辑而失效**——上游单进程无此问题。处理：工具结果照常返回当前状态；系统提示词保留「Re-read with get_timeline after a failure that suggests your model is stale」（上游已有，`AgentInstructions.swift:25-28`）。
