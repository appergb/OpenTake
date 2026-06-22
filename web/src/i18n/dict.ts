/**
 * Translation dictionaries. Self-built, dependency-free i18n: a flat key->string
 * map per locale. `zh-CN` is the shipped default (SPEC: 全中文 i18n,默认 zh-CN);
 * `en` is the original upstream copy, kept so the Settings language switch has a
 * second locale. Keep keys grouped by surface so coverage is auditable.
 */

export type Locale = "zh-CN" | "en";

export const LOCALES: ReadonlyArray<{ id: Locale; label: string }> = [
  { id: "zh-CN", label: "简体中文" },
  { id: "en", label: "English" },
];

/** A flat translation table. Every locale must define the same key set. */
export type Dict = Record<string, string>;

const zh: Dict = {
  // App / product
  "app.name": "OpenTake",
  "app.tagline": "为 AI 打造的视频编辑器。生成与剪辑,一处搞定。",

  // TitleBar
  "title.toggleAgent": "切换 Agent 面板",
  "title.export": "导出",
  "title.exportHint": "导出 (⌘E)",
  "title.home": "主页",
  "title.settings": "设置",
  "title.backHome": "返回主页",

  // View menu
  "view.menu": "视图",
  "view.layout": "布局",
  "view.panels": "面板",
  "view.layoutDefault": "默认布局",
  "view.layoutMedia": "媒体布局",
  "view.layoutVertical": "竖屏布局",
  "view.mediaPanel": "媒体面板",
  "view.inspector": "检查器",

  // Media panel
  "media.tab.media": "媒体",
  "media.tab.captions": "字幕",
  "media.tab.music": "音乐",
  "media.import": "导入媒体",
  "media.importHint": "导入媒体 (⌘I)",
  "media.importFolder": "导入文件夹",
  "media.importFiles": "导入文件",
  "media.generate": "生成",
  "media.search": "搜索",
  "media.viewMode": "视图模式",
  "media.sort": "排序",
  "media.filter": "筛选",
  "media.library": "媒体库",
  "media.itemCount": "{count} 个项目",
  "media.empty": "拖放媒体文件到此处,或使用导入 / 生成。",
  "media.importing": "正在导入…",
  "media.importFailed": "导入失败:{error}",
  "media.dropToAdd": "松开以添加到时间线",

  // Inspector
  "inspector.title": "检查器",
  "inspector.timeline": "时间线",
  "inspector.selectedCount": "已选择 {count} 项",
  "inspector.tab.text": "文本",
  "inspector.tab.video": "视频",
  "inspector.tab.audio": "音频",
  "inspector.tab.aiEdit": "AI 编辑",
  "inspector.section.levels": "电平",
  "inspector.section.transform": "变换",
  "inspector.section.playback": "播放",
  "inspector.section.format": "格式",
  "inspector.field.volume": "音量",
  "inspector.field.scale": "缩放",
  "inspector.field.rotation": "旋转",
  "inspector.field.opacity": "不透明度",
  "inspector.field.speed": "速度",
  "inspector.field.resolution": "分辨率",
  "inspector.field.frameRate": "帧率",
  "inspector.field.aspectRatio": "宽高比",
  "inspector.field.duration": "时长",
  "inspector.keyframes": "关键帧",

  // Toolbar
  "toolbar.undo": "撤销 (⌘Z)",
  "toolbar.redo": "重做 (⇧⌘Z)",
  "toolbar.pointer": "选择工具 (V)",
  "toolbar.razor": "切割工具 (C)",
  "toolbar.split": "在播放头处分割 (⌘K)",
  "toolbar.trimStart": "将入点修剪到播放头 (Q)",
  "toolbar.trimEnd": "将出点修剪到播放头 (W)",
  "toolbar.addText": "添加文本",
  "toolbar.zoom": "时间线缩放",

  // Timeline track headers
  "timeline.mute": "静音",
  "timeline.hide": "隐藏",
  "timeline.syncLock": "同步锁定",

  // Preview
  "preview.fit": "适应",
  "preview.timelineTab": "时间线",
  "preview.jumpStart": "跳到开头",
  "preview.stepBack": "上一帧",
  "preview.playPause": "播放/暂停 (空格)",
  "preview.stepForward": "下一帧",
  "preview.jumpEnd": "跳到结尾",
  "preview.captureFrame": "截取当前帧到媒体",

  // Agent panel
  "agent.placeholder": "Agent 面板(独立功能)",

  // Home
  "home.welcome": "欢迎使用 OpenTake",
  "home.newProject": "新建项目",
  "home.openProject": "打开项目",
  "home.settings": "设置",
  "home.myProjects": "我的项目",
  "home.recentEmpty": "还没有最近的项目。新建一个开始吧。",
  "home.untitled": "未命名",
  "home.remove": "从最近中移除",
  "home.opening": "正在打开…",

  // Settings
  "settings.title": "设置",
  "settings.done": "完成",
  "settings.section.general": "通用",
  "settings.section.appearance": "外观",
  "settings.section.import": "导入",
  "settings.section.ai": "AI",
  "settings.section.about": "关于",
  "settings.language": "语言",
  "settings.languageDesc": "界面显示语言。",
  "settings.theme": "主题",
  "settings.themeDesc": "应用配色方案。",
  "settings.theme.dark": "深色",
  "settings.theme.light": "浅色",
  "settings.defaultImportFolder": "默认导入文件夹",
  "settings.defaultImportFolderDesc": "导入对话框默认打开的位置。",
  "settings.chooseFolder": "选择…",
  "settings.clear": "清除",
  "settings.notSet": "未设置",
  "settings.byokTitle": "API 密钥 (BYOK)",
  "settings.byokDesc":
    "填入你自己的模型 API 密钥以启用 AI 生成与 Agent。密钥安全存储在系统钥匙串中（macOS 钥匙串 / Windows 凭据管理器 / Linux Secret Service），绝不以明文写入应用配置。",
  "settings.byokProvider": "服务商",
  "settings.byokKey": "API 密钥",
  "settings.byokKeyPlaceholder": "粘贴你的 API 密钥",
  "settings.byokSave": "保存密钥",
  "settings.byokSaved": "密钥已安全存储在系统钥匙串中。",
  "settings.byokDelete": "删除密钥",
  "settings.byokSaveFailed": "保存失败：{error}",
  "settings.aboutVersion": "版本",
  "settings.aboutLicense": "许可",
  "settings.aboutDesc": "OpenTake 是 Palmier Pro 的开源跨平台分支。",

  // Common
  "common.cancel": "取消",
  "common.open": "打开",
};

const en: Dict = {
  "app.name": "OpenTake",
  "app.tagline": "A video editor built for AI. Generate and edit, all in one place.",

  "title.toggleAgent": "Toggle Agent Panel",
  "title.export": "Export",
  "title.exportHint": "Export (⌘E)",
  "title.home": "Home",
  "title.settings": "Settings",
  "title.backHome": "Back to Home",

  "view.menu": "View",
  "view.layout": "Layout",
  "view.panels": "Panels",
  "view.layoutDefault": "Default Layout",
  "view.layoutMedia": "Media Layout",
  "view.layoutVertical": "Vertical Layout",
  "view.mediaPanel": "Media Panel",
  "view.inspector": "Inspector",

  "media.tab.media": "Media",
  "media.tab.captions": "Captions",
  "media.tab.music": "Music",
  "media.import": "Import Media",
  "media.importHint": "Import Media (⌘I)",
  "media.importFolder": "Import Folder",
  "media.importFiles": "Import Files",
  "media.generate": "Generate",
  "media.search": "Search",
  "media.viewMode": "View mode",
  "media.sort": "Sort",
  "media.filter": "Filter",
  "media.library": "Library",
  "media.itemCount": "{count} items",
  "media.empty": "Drop media files here, or use Import / Generate.",
  "media.importing": "Importing…",
  "media.importFailed": "Import failed: {error}",
  "media.dropToAdd": "Release to add to the timeline",

  "inspector.title": "Inspector",
  "inspector.timeline": "Timeline",
  "inspector.selectedCount": "{count} selected",
  "inspector.tab.text": "Text",
  "inspector.tab.video": "Video",
  "inspector.tab.audio": "Audio",
  "inspector.tab.aiEdit": "AI Edit",
  "inspector.section.levels": "Levels",
  "inspector.section.transform": "Transform",
  "inspector.section.playback": "Playback",
  "inspector.section.format": "Format",
  "inspector.field.volume": "Volume",
  "inspector.field.scale": "Scale",
  "inspector.field.rotation": "Rotation",
  "inspector.field.opacity": "Opacity",
  "inspector.field.speed": "Speed",
  "inspector.field.resolution": "Resolution",
  "inspector.field.frameRate": "Frame Rate",
  "inspector.field.aspectRatio": "Aspect Ratio",
  "inspector.field.duration": "Duration",
  "inspector.keyframes": "Keyframes",

  "toolbar.undo": "Undo (⌘Z)",
  "toolbar.redo": "Redo (⇧⌘Z)",
  "toolbar.pointer": "Pointer (V)",
  "toolbar.razor": "Razor (C)",
  "toolbar.split": "Split at Playhead (⌘K)",
  "toolbar.trimStart": "Trim Start to Playhead (Q)",
  "toolbar.trimEnd": "Trim End to Playhead (W)",
  "toolbar.addText": "Add Text",
  "toolbar.zoom": "Timeline zoom",

  "timeline.mute": "Mute",
  "timeline.hide": "Hide",
  "timeline.syncLock": "Sync lock",

  "preview.fit": "Fit",
  "preview.timelineTab": "Timeline",
  "preview.jumpStart": "Jump to Start",
  "preview.stepBack": "Step Back",
  "preview.playPause": "Play/Pause (Space)",
  "preview.stepForward": "Step Forward",
  "preview.jumpEnd": "Jump to End",
  "preview.captureFrame": "Capture Frame to Media",

  "agent.placeholder": "Agent panel (separate Issue)",

  "home.welcome": "Welcome to OpenTake",
  "home.newProject": "New Project",
  "home.openProject": "Open Project",
  "home.settings": "Settings",
  "home.myProjects": "My Projects",
  "home.recentEmpty": "No recent projects yet. Create one to get started.",
  "home.untitled": "Untitled",
  "home.remove": "Remove from Recents",
  "home.opening": "Opening…",

  "settings.title": "Settings",
  "settings.done": "Done",
  "settings.section.general": "General",
  "settings.section.appearance": "Appearance",
  "settings.section.import": "Import",
  "settings.section.ai": "AI",
  "settings.section.about": "About",
  "settings.language": "Language",
  "settings.languageDesc": "Interface display language.",
  "settings.theme": "Theme",
  "settings.themeDesc": "Application color scheme.",
  "settings.theme.dark": "Dark",
  "settings.theme.light": "Light",
  "settings.defaultImportFolder": "Default Import Folder",
  "settings.defaultImportFolderDesc": "Location the import dialog opens to by default.",
  "settings.chooseFolder": "Choose…",
  "settings.clear": "Clear",
  "settings.notSet": "Not set",
  "settings.byokTitle": "API Key (BYOK)",
  "settings.byokDesc":
    "Add your own model API key to enable AI generation and the Agent. The key is stored securely in your system keychain (macOS Keychain / Windows Credential Manager / Linux Secret Service) — never in plaintext.",
  "settings.byokProvider": "Provider",
  "settings.byokKey": "API Key",
  "settings.byokKeyPlaceholder": "Paste your API key",
  "settings.byokSave": "Save Key",
  "settings.byokSaved": "Key stored securely in your system keychain.",
  "settings.byokDelete": "Remove key",
  "settings.byokSaveFailed": "Save failed: {error}",
  "settings.aboutVersion": "Version",
  "settings.aboutLicense": "License",
  "settings.aboutDesc": "OpenTake is the open-source, cross-platform fork of Palmier Pro.",

  "common.cancel": "Cancel",
  "common.open": "Open",
};

export const DICTS: Record<Locale, Dict> = {
  "zh-CN": zh,
  en,
};
