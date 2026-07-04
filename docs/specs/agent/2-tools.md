# 31 个工具完整提取（工具名 + 关键参数 + 行为描述）

> **来源**：`ToolDefinitions.swift:45-588`（描述字段 `description:`）。**描述字符串原样保留**（ARCHITECTURE §7 `:151`「工具描述字符串承载行为契约，原样照搬」）。下表「关键参数」列出 schema 字段；`allowedKeys`（用于严格未知字段校验，§4）来自 `ToolExecutor+Clips.swift` 等各 `DecodableToolArgs`。
>
> **实现指令**：每个工具的 `description` 直接复制本节附带的 JSON（§2.2）中的字符串到 Rust 工具注册，**一字不改**（含 `\n`、`•`、emoji 等）。Rust 侧分发到 `opentake-core` 的对应 `EditCommand`（§8）。

## 2.1 工具清单（31 个，分域 + 关键参数 + 背后命令）

枚举顺序严格按 `ToolName`（`ToolDefinitions.swift:4-36`）：`get_timeline, get_media, add_clips, insert_clips, remove_clips, remove_tracks, move_clips, set_clip_properties, set_keyframes, split_clip, ripple_delete_ranges, undo, add_texts, add_captions, generate_video, generate_image, generate_audio, upscale_media, import_media, list_models, inspect_media, get_transcript, inspect_timeline, search_media, list_folders, create_folder, move_to_folder, rename_media, rename_folder, delete_media, delete_folder`。

### A. 读 / 内省（只读，7 个）

| # | 工具 | 关键参数（schema 字段；`*`=required） | required 字段 | 背后命令（opentake-core） | Context Signal 附加（§6） |
|---|---|---|---|---|---|
| 1 | `get_timeline` | `startFrame?:int`, `endFrame?:int`（窗口分页） | — | `core.timeline_snapshot(window)` → 压缩编码 | `video_classification`+`track_roles`+`editing_stage`+`stage_guidance` |
| 2 | `get_media` | （无） | — | `core.media_manifest()` | — |
| 3 | `inspect_media` | `mediaRef*:string`, `clipId?:string`, `maxFrames?:int(≤12)`, `startSeconds?:number`, `endSeconds?:number`, `wordTimestamps?:bool`, `overview?:bool` | `mediaRef` | `opentake-media`：FFmpeg 抽帧 + whisper 转写 | `clip_analysis_hint`（镜头类型/景别） |
| 4 | `get_transcript` | `startFrame?:int`, `endFrame?:int`, `clipId?:string` | — | `opentake-media`：遍历时间线音/视轨，映射 trim/speed/position | `break_analysis`（气口/句界/重复/啰嗦） |
| 5 | `inspect_timeline` | `startFrame?:int`, `endFrame?:int`, `maxFrames?:int(≤12)` | — | `opentake-render`：合成帧（transform/opacity/crop/keyframe + 文字烧入） | — |
| 6 | `search_media` | `query*:string`, `scope?:enum{visual,spoken,both}`, `mediaRef?:string`, `limit?:int(≤50)` | `query` | `opentake-media`：CLIP 视觉 + 转写口语检索 | `material_match_hint`（B-roll 匹配优先级） |
| 7 | `list_models` | `type?:enum{video,image,audio,upscale}` | — | `opentake-gen`：`ModelCatalog` | — |

### B. 时间线编辑（写，11 个）——核心剪辑能力

| # | 工具 | 关键参数 | required | `allowedKeys`（顶层 / Entry） | 背后命令 | Context Signal 校验 |
|---|---|---|---|---|---|---|
| 8 | `add_clips` | `entries[]`{`mediaRef*`,`trackIndex?`,`startFrame*`,`durationFrames*`,`trimStartFrame?`,`trimEndFrame?`} | `entries`；entry: `mediaRef,startFrame,durationFrames` | `{entries}` / `{mediaRef,trackIndex,startFrame,durationFrames,trimStartFrame,trimEndFrame}` | `EditCommand::AddClips`（覆写：clearRegion+placeClip；全省略 trackIndex 自动建共享轨；视频带音频自动建 linked audio；trackIndex 全有或全无） | `placement_validation`（轨道类型匹配 / A/V 拆分） |
| 9 | `insert_clips` | `trackIndex*`,`atFrame*`,`entries[]`{`mediaRef*`,`durationFrames?`,`trimStartFrame?`,`trimEndFrame?`} | `trackIndex,atFrame,entries`；entry:`mediaRef` | `{trackIndex,atFrame,entries}` / `{mediaRef,durationFrames,trimStartFrame,trimEndFrame}` | `EditCommand::InsertClips`（ripple：atFrame 及之后右移，sync-locked 轨 + linked audio 同步） | `placement_validation` |
| 10 | `remove_clips` | `clipIds*[]:string` | `clipIds` | `{clipIds}` | `EditCommand::RemoveClips`（link group 连带删；空轨 prune 并提示索引变） | 口播精剪规则（删主干 warning） |
| 11 | `remove_tracks` | `trackIndexes*[]:int` | `trackIndexes` | `{trackIndexes}` | `EditCommand::RemoveTracks`（余轨索引下移；其它轨 linked partner 不删） | — |
| 12 | `move_clips` | `moves[]`{`clipId*`,`toTrack?`,`toFrame?`}（每条至少一个 to*） | `moves`；move:`clipId` | `{moves}` / `{clipId,toTrack,toFrame}` | `EditCommand::MoveClips`（目标重叠覆写；linked partner 跟帧 delta，轨不传播） | 节奏/结构规则 |
| 13 | `set_clip_properties` | `clipIds*[]` + 任意组合 `durationFrames?`,`trimStartFrame?`,`trimEndFrame?`,`speed?`,`volume?`,`opacity?`,`transform?{centerX,centerY,width,height,flipHorizontal,flipVertical}`, 文字专用 `content?`,`fontName?`,`fontSize?`,`color?`,`alignment?{left,center,right}` | `clipIds` | `{clipIds,durationFrames,trimStartFrame,trimEndFrame,speed,volume,opacity,transform,content,fontName,fontSize,color,alignment}` | `EditCommand::SetClipProperties`（同值套全批；set volume/opacity 清该属性 keyframe；timing 传播 linked partner，文字伙伴跳过 trim/speed） | — |
| 14 | `set_keyframes` | `clipId*`,`property*:enum{volume,opacity,rotation,position,scale,crop}`,`keyframes*[][]`（`[frame,...values,interp?]`，interp∈{linear,hold,smooth}默认 smooth） | `clipId,property,keyframes` | `{clipId,property,keyframes}` | `EditCommand::SetKeyframes`（替换式，空数组清空；frame=clip 相对） | — |
| 15 | `split_clip` | `clipId*`,`atFrame*`（严格在 clip 内） | `clipId,atFrame` | （无专用结构，直接取参） | `EditCommand::SplitClip` | 口播精剪：不在词中间切 warning |
| 16 | `ripple_delete_ranges` | 二选一 `trackIndex?`/`clipId?`；`ranges*[][start,end]`；`units?:enum{seconds,frames}`默认 frames | `ranges` | `{clipId,trackIndex,ranges,units}` | `EditCommand::RippleDeleteRanges`（重叠合并；linked 同区间删；sync-locked 同步左移，放不下整体拒绝；返回 anchor 轨剪后布局） | `break_analysis` 一致性 |
| 17 | `undo` | （无） | — | — | `EditCommand::Undo` + agentUndoStack 守卫（§4.3） | — |
| 18 | `add_texts` | `entries[]`{`startFrame*`,`durationFrames*`,`content*`,`transform?{centerX,centerY,width,height}`,`fontName?`,`fontSize?`,`color?`,`alignment?`} | `entries`；entry:`startFrame,durationFrames,content` | `{entries}` / `{trackIndex,startFrame,durationFrames,content,transform,fontName,fontSize,color,alignment}` | `EditCommand::AddTexts`（overlay；同轨重叠覆写；同时显示需放不同轨；全省略 trackIndex 顶部新建一轨） | `text_placement_hint`（层级 / 安全区） |
| 19 | `add_captions` | `clipIds?[]`,`language?`,`fontName?`,`fontSize?`,`color?`,`centerX?`,`centerY?`,`textCase?:enum{auto,upper,lower}`,`censorProfanity?` | — | `{clipIds,language,fontName,fontSize,color,centerX,centerY,textCase,censorProfanity}` | `EditCommand::AddCaptions`（端侧转写 + 样式化 caption clip 到新轨；省略 clipIds 自动挑语音最多的轨） | `caption_style_hint` |

### C. 媒体生成 / 导入（写，5 个）——媒体管线接入点（依赖 `opentake-gen`）

| # | 工具 | 关键参数 | required | 背后命令 |
|---|---|---|---|---|
| 20 | `generate_video` | `prompt*`,`name?`,`model?`,`duration?`,`aspectRatio?`,`resolution?`,`startFrameMediaRef?`,`endFrameMediaRef?`,`sourceVideoMediaRef?`,`sourceClipId?`,`referenceImageMediaRefs?[]`,`referenceVideoMediaRefs?[]`,`referenceAudioMediaRefs?[]`,`folderId?` | `prompt` | `opentake-gen`：异步提交，立即返回 placeholder asset ID；**花钱、不可撤销** |
| 21 | `generate_image` | `prompt*`,`name?`,`model?`,`aspectRatio?`,`resolution?`,`quality?`,`referenceMediaRefs?[]`,`folderId?` | `prompt` | `opentake-gen`：异步提交 placeholder |
| 22 | `generate_audio` | `prompt?`,`name?`,`model?`,`voice?`,`lyrics?`,`styleInstructions?`,`instrumental?`,`duration?`,`videoSourceStartFrame?`,`videoSourceEndFrame?`,`videoSourceMediaRef?`,`folderId?` | （无） | `opentake-gen`：TTS / 文生乐 / 视频配乐；时间线区间结果自动落轨 |
| 23 | `upscale_media` | `mediaRef*`,`model?`,`sourceClipId?` | `mediaRef` | `opentake-gen`：升分辨率 placeholder |
| 24 | `import_media` | `source*`{三选一 `url?`(HTTPS≤1GB)/`path?`(本地，可目录递归)/`bytes?`(base64≤~15MB)，`mimeType?`},`name?`,`folderId?` | `source` | `opentake-core`/`opentake-project`：url 后台下载、path/bytes 同步；扩展名白名单 |

### D. 媒体库组织（写，7 个）——均可撤销，均支持「单条参数 或 `entries[]` 批量」二选一

| # | 工具 | 关键参数 | required | 背后命令 |
|---|---|---|---|---|
| 25 | `list_folders` | （无） | — | `core.folders()` |
| 26 | `create_folder` | `name?`+`parentFolderId?` **或** `entries[]`{`name*`,`parentFolderId?`} | （二选一） | `EditCommand::CreateFolder` |
| 27 | `move_to_folder` | `assetIds?[]`+`folderId?` **或** `entries[]`{`assetIds*`,`folderId?`} | （二选一） | `EditCommand::MoveToFolder` |
| 28 | `rename_media` | `mediaRef?`+`name?` **或** `entries[]`{`mediaRef*`,`name*`} | （二选一） | `core.rename_media` |
| 29 | `rename_folder` | `folderId?`+`name?` **或** `entries[]`{`folderId*`,`name*`} | （二选一） | `core.rename_folder` |
| 30 | `delete_media` | `assetIds*[]` | `assetIds` | `core.delete_media`（连带删引用 clip，同撤销步） |
| 31 | `delete_folder` | `folderIds*[]` | `folderIds` | `core.delete_folder`（连带删子文件夹/资源/clip） |

## 2.2 工具描述完整 JSON（供实现直接复制 `description` 字符串）

> 下方是 31 个工具的 `name` + `description`（**原样**，含换行/项目符号/emoji）+ `inputSchema`（已从 Swift 字典转 JSON）。实现时把每条 `description` 一字不差地放进 rmcp 工具注册的描述位；`inputSchema` 可直接作为 JSON Schema，或用 `schemars` 从对应 Rust 入参结构体派生后**人工核对字段名/描述一致**。
>
> 由于篇幅，完整 31 条以「工具名 → 上游行号」精确锚定（每条描述均在该范围内逐字可取）：

| 工具 | 描述行范围（`ToolDefinitions.swift`） | inputSchema 行范围 |
|---|---|---|
| `get_timeline` | `:48`（单条长字符串） | `:49-54` |
| `get_media` | `:58` | `:59` |
| `inspect_media` | `:63` | `:64-75` |
| `get_transcript` | `:79` | `:80-87` |
| `inspect_timeline` | `:90` | `:91-98` |
| `search_media` | `:101` | `:102-111` |
| `add_clips` | `:114` | `:115-136` |
| `insert_clips` | `:139` | `:140-161` |
| `remove_clips` | `:164` | `:165-175` |
| `remove_tracks` | `:178` | `:179-189` |
| `move_clips` | `:192` | `:193-211` |
| `set_clip_properties` | `:214` | `:215-248` |
| `set_keyframes` | `:251` | `:252-268` |
| `split_clip` | `:271` | `:272-279` |
| `ripple_delete_ranges` | `:282` | `:283-296` |
| `undo` | `:299` | `:300` |
| `add_texts` | `:304` | `:305-338` |
| `add_captions` | `:341` | `:342-355` |
| `generate_video` | `:358` | `:359-378` |
| `generate_image` | `:381` | `:382-395` |
| `generate_audio` | `:398` | `:399-416` |
| `upscale_media` | `:419` | `:420-428` |
| `import_media` | `:431` | `:432-449` |
| `list_models` | `:581` | `:582-587` |
| `list_folders` | `:452` | `:453` |
| `create_folder` | `:457` | `:458-476` |
| `move_to_folder` | `:479` | `:480-506` |
| `rename_media` | `:509` | `:510-528` |
| `rename_folder` | `:531` | `:532-550` |
| `delete_media` | `:553` | `:554-564` |
| `delete_folder` | `:567` | `:568-578` |

> **实现要点**：把这 31 条描述抽到 `crates/opentake-agent/src/tools/descriptions.rs`（`const`/`include_str!`），与 schema 一一对应。改写时**仅**把 `palmier`/`Palmier`/`palmier-pro` 字样替换为 `opentake`/`OpenTake`（例如 `get_timeline` 的「tell the user to sign in to Palmier and subscribe」、`canGenerate`）；产品名以外的行为契约文本**一字不改**。`palmier://models/*` 资源 URI → `opentake://models/*`。

## 2.3 工具入参 Schema 策略（Rust）

两条可选路线，**推荐前者**：

1. **手抄 JSON Schema（与上游 1:1）**：直接用 §2.2 的 `inputSchema` JSON 作为静态 schema。优点：与上游行为契约完全对齐，描述字段（schema 内嵌的 per-field `description`）也照搬，对 LLM 自纠正最有利。
2. **`schemars` 派生 + 人工核对**：为每个工具定义 `#[derive(Deserialize, JsonSchema)]` 入参结构体（`Option<T>` 表 optional，`Vec` 表 array），用 `#[schemars(description="...")]` 填字段描述。优点：与 `serde_path_to_error`（§4）天然配合。**风险**：派生出的 schema 字段顺序/描述需逐条核对与上游一致，否则削弱描述的契约性。

无论哪条，顶层 `additionalProperties` 行为由 §4 的「严格未知字段校验」补足（上游 `DecodableToolArgs.allowedKeys` 连嵌套 entry 也查，JSON Schema 的 `additionalProperties:false` 不足以覆盖 entry 级，必须用 §4 的运行时校验）。
