# Zustand 状态结构（只读镜像 + UI-only 态）

> 拆分依据:上游 `EditorViewModel` 字段（`EditorViewModel.swift`）按「Rust 真相镜像」vs「纯前端 UI 态」分流。**镜像态只能由 `timeline_changed` event 更新,前端绝不直接改;UI 态前端自由改。**

### 10.1 镜像态（来自 Rust，只读）—— `useProjectStore`

```ts
interface ProjectMirror {
  // 由 timeline_changed{version} 驱动重取(get_timeline)
  timelineVersion: number;          // ← 上游 timelineRenderRevision (EditorViewModel.swift:76)
  timeline: Timeline;               // fps/width/height/tracks (见 §12)
  // 媒体库(运行时富对象, 来自 Rust)
  mediaAssets: MediaAsset[];        // EditorViewModel.swift:110
  folders: MediaFolder[];
  offlineMediaRefs: Set<string>;    // :111
  unprocessableMediaRefs: Set<string>; // :112
  // 工程信息
  projectUrl: string | null;
  projectId: string | null;
  isDocumentEdited: boolean;        // :185
  // 能力/账户(只读)
  canGenerate: boolean;
}
```

### 10.2 UI-only 态 —— `useEditorUiStore`（前端自管）

```ts
interface EditorUiState {
  // —— 播放/播放头 (上游 EditorViewModel.swift:55-98) ——
  currentFrame: number;             // 提交后的播放头帧
  activeFrame: number;              // = playheadState.timelineFrame(scrub 时的实时帧, :58)
  sourcePlayheadFrame: number;      // 源预览播放头(:96)
  isPlaying: boolean;               // :59
  isScrubbing: boolean;             // :77

  // —— 选择 (上游 :60-66) ——
  selectedClipIds: Set<string>;     // :61
  isMarqueeSelecting: boolean;      // :62
  selectedGap: GapSelection | null; // :63
  selectedTimelineRange: TimelineRange | null; // :64
  selectedMediaAssetIds: Set<string>; // :65
  selectedFolderIds: Set<string>;   // :66

  // —— 时间线视图 (上游 :68-77) ——
  zoomScale: number;                // = pixelsPerFrame, 初值 4.0 (:68)
  minZoomScale: number;             // 由可视宽+总帧算; 前端复刻或从镜像取
  timelineVisibleWidth: number;     // :75
  scrollLeft: number; scrollTop: number; // 滚动位置(上游隐含在 NSScrollView)
  toolMode: 'pointer' | 'razor';    // :78 (ToolMode)
  trackDisplayHeights: Record<string, number>; // 轨道高(不持久, 默认 50; 上游 Track.displayHeight)

  // —— 画布(Preview) (上游 :69-74) ——
  canvasZoom: number;               // :69 (≤1 时 offset 归零)
  canvasOffset: { width: number; height: number }; // :74
  cropEditingActive: boolean;       // :90
  cropAspectLock: CropAspectLock;   // :91

  // —— 面板 (上游 :46-47, 135-157) ——
  focusedPanel: Panel | null;       // :46
  maximizedPanel: Panel | null;     // :47
  layoutPreset: 'default'|'media'|'vertical'; // :99 (持久化 localStorage)
  agentPanelVisible: boolean;       // :135 (默认 false, 持久化)
  mediaPanelVisible: boolean;       // :141 (默认 true, 持久化)
  inspectorPanelVisible: boolean;   // :147 (默认 true, 持久化)
  keyframesPanelVisible: boolean;   // :153 (默认 false, 持久化)

  // —— Preview tabs (上游 :92-95) ——
  previewTabs: PreviewTab[];        // 初值 [timeline]
  activePreviewTabId: string;       // :93
  previewTabHistory: string[]; previewTabHistoryIndex: number; // :94-95(前进/后退)

  // —— Media 面板导航 (上游 :161-170) ——
  mediaPanelCurrentFolderId: string | null;
  mediaPanelRevealAssetId: string | null;
  mediaPanelScrollTarget: string | null;
  mediaPanelToast: string | null;

  // —— 对话框/生成 (上游 :79-89) ——
  showExportDialog: boolean;        // :79
  showGenerationPanel: boolean;     // :80 (打开时切到 Media tab)
  pendingReplacements: Set<string>; // :89 (生成中的 clip id)
  pendingSwapClipId: string | null; // :66

  // —— 剪贴板(可由 Rust 持有, 前端只读能否粘贴) ——
  canPasteClips: boolean;
}

type Panel = 'agent'|'media'|'preview'|'inspector'|'timeline';
```

### 10.3 派生选择器（前端纯函数，不进 store）

复刻上游计算属性：`totalFrames`(Timeline.swift:16-22)、clip rect / playhead x（§5.2 geometry）、`zones`(视频/音频区划分，TimelineHeaderView 用)、`validSelectedTimelineRange`、各 Inspector 的 `selectedVisualClip(s)`/`selectedAudioClip(s)`/`availableTabs`(InspectorView.swift:170-200)。**这些是纯函数,从镜像 + UI 态算,不存储**(避免冗余,符合上游"派生不存"原则)。

### 10.4 持久化

- `localStorage`：`layoutPreset` / `agentPanelVisible` / `mediaPanelVisible` / `inspectorPanelVisible` / `keyframesPanelVisible`（对应上游 `UserDefaults`，`EditorViewModel.swift:99-157`）。
- 其余 UI 态会话内存即可。
