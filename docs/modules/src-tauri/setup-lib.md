# setup-lib — 应用装配、命令注册与窗口生命周期

> 上级：[本模块目录](INDEX.md) · [总览](OVERVIEW.md) · [模块文档树](../INDEX.md)
>
> 源码：[`../../../src-tauri/src/lib.rs`](../../../src-tauri/src/lib.rs) · [`../../../src-tauri/src/main.rs`](../../../src-tauri/src/main.rs) · 配置 [`../../../src-tauri/tauri.conf.json`](../../../src-tauri/tauri.conf.json) · 权限 [`../../../src-tauri/capabilities/default.json`](../../../src-tauri/capabilities/default.json)

## 入口

`main.rs` 仅一行：`opentake_tauri_lib::run()`。`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` 在 release 下抑制 Windows 控制台窗口。真正的装配全在 `lib.rs::run()`。

对应上游 `main.swift` 的顺序敏感启动序列 + `AppDelegate.applicationDidFinishLaunching`（见 [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md) App 层）。

## 启动序列（lib.rs::run）

```
resolve_media_tools()                  // 任何解码前先钉死 ffmpeg/ffprobe 绝对路径
tauri::Builder::default()
  .plugin(tauri_plugin_dialog::init())
  .on_window_event(…)                  // 关窗拦截（见下）
  .setup(|app| {
      set_activation_policy(.regular)  // 仅 macOS：窗口隐藏时仍保留 Dock 图标
      let core = AppCore::new();        // 唯一权威会话
      core.subscribe(forward_event)     // 订阅 CoreEvent → 转发 Tauri 事件
      MediaEngine::new(cache_root, models_dir)   // cache=app_cache_dir, models=app_data_dir
      mcp::spawn(core.clone(), workflows_dir)     // 拉起回环 MCP（共享会话克隆）
      LibraryStore::new(library_root)             // <app_data_dir>/OpenTake/Library
      app.manage(core / MediaState / LibraryState / RenderState)
  })
  .invoke_handler(generate_handler![ … ])         // 注册全部命令
  .build(generate_context!())
  .run(|app, event| { /* RunEvent::Reopen 仅 macOS */ })
```

平台路径不可用时一律降级到 `std::env::temp_dir()`，保证导入 / 收藏仍可工作。

## 命令注册（generate_handler!）

`run()` 一次性注册下列 **30 个命令**（按来源分组）：

| 来源模块 | 命令 |
|---|---|
| `commands` | `get_timeline`、`edit_apply`、`undo`、`redo`、`can_undo`、`can_redo`、`project_new`、`project_open`、`project_save`、`get_default_project_dir`、`export_fcpxml`、`check_path_exists` |
| `media` | `import_folder`、`import_media`、`relink_media`、`get_media`、`extract_audio`、`get_waveform` |
| `render` | `composite_frame` |
| `export` | `export_video` |
| `secret` | `secret_save`、`secret_load`、`secret_delete` |
| `library` | `library_list`、`library_favorite`、`library_unfavorite`、`library_categorize`、`library_rename`、`library_delete`、`library_import_to_project` |

子系统详情：[commands-ipc.md](commands-ipc.md)（前 12 个）、[library-media.md](library-media.md)、[render.md](render.md)、[export.md](export.md)、[secret.md](secret.md)。

## Managed State（四类共享句柄）

| 状态 | 类型 | 内容 | 备注 |
|---|---|---|---|
| 会话 | `AppCore` | 权威 Timeline + 撤销栈 + 事件总线 | MCP 拿的是 `core.clone()`，共享同一会话 |
| 媒体 | `MediaState` | `MediaEngine` 包装 | 此处只读（probe / 波形 / 抽音频）；见 [library-media.md](library-media.md) |
| 素材库 | `LibraryState` | `Arc<LibraryStore>` | 跨工程 copy-on-favorite |
| 渲染 | `RenderState` | `Mutex<Option<GpuContext>>` | **懒加载**，首次 `composite_frame` 才建 GPU；见 [render.md](render.md) |

## 事件桥（forward_event）

`AppCore` 释放锁后在发事件的线程上回调本闭包，故此处回调 Tauri 是安全的。映射：

| `CoreEvent` | Tauri 事件名 |
|---|---|
| `TimelineChanged` | `timeline_changed` |
| `ProjectOpened` | `project_opened` |
| `ProjectSaved` | `project_saved` |
| `MediaChanged` | `media_changed` |

payload 即事件本身（带 `kind` tag 形状）。`emit` best-effort：WebView 缺失（拆卸期）失败被忽略，不 panic。

## 窗口生命周期（关窗不退 + Dock 重开）

镜像上游「app 常驻；关窗回主页」（`AppDelegate`）。Tauri 默认「最后一个窗口关闭即退出」被覆盖：

- **关窗**（`WindowEvent::CloseRequested`）：`api.prevent_close()` → 先 `core.save_project(None)` 做最终落盘（autosave 是 debounce 的，这是兜底写盘；无打开工程时返回的错误被**有意忽略**）→ `window.hide()` → `emit("go_home")`。
- **退出**：`Cmd+Q` 触发 `ExitRequested`（未被拦截），正常退出。
- **Dock 重开**（`RunEvent::Reopen`，**仅 macOS** `#[cfg(target_os = "macos")]`）：无可见窗口时 `show()` + `set_focus()` 主窗口。其它平台靠托盘 / OS 重现窗口，是跨平台后续项。

## FFmpeg 路径解析（resolve_media_tools）

为何需要：macOS `.app` 从 Finder/Dock 启动只继承精简的 launchd `PATH`（`/usr/bin:/bin:/usr/sbin:/sbin`），不含 Homebrew（`/opt/homebrew/bin`）和 `/usr/local/bin`。纯 PATH 查 `ffmpeg` 会失败 → 每帧解码返回空 → 预览全黑（即便代码正确）。

做法：解码前把 `ffmpeg`/`ffprobe` 的**绝对路径**写入环境变量 `OPENTAKE_FFMPEG` / `OPENTAKE_FFPROBE`（`opentake-media` 的 `ff` 模块读取）。

- 已有显式 override（环境变量已设）→ 跳过，**override 始终优先**。
- 查找顺序：现有 `PATH` 各目录 → `/opt/homebrew/bin` → `/usr/local/bin` → `/opt/local/bin` → `/usr/bin`，取第一个存在的文件。
- 随包分发 ffmpeg（Tauri `externalBin`）是跨机器后续项；当前要求宿主机磁盘上有 ffmpeg ≥ 6.0。

## 窗口与权限配置

**`tauri.conf.json`**（`app.windows[0]`，label `main`）：

- `titleBarStyle: "Overlay"` + `hiddenTitle: true` + `trafficLightPosition {x:18, y:24}`——自绘标题栏，红绿灯内嵌（对应上游暗色透明全尺寸标题栏 + 圆角安全区配件）。
- 尺寸 1600×1000，最小 760×480；`backgroundColor: "#0A0A0A"`；`dragDropEnabled: false`。
- `build.frontendDist = ../web/dist`，`devUrl = http://localhost:1420`；`beforeDevCommand` / `beforeBuildCommand` 跑 `pnpm -C web dev|build`。
- `security.csp: null`；`assetProtocol.enable: true` 且 `scope: ["**"]`（WebView 可读本地资源——预览 / 素材所需）。`identifier: com.opentake.desktop`。

**`capabilities/default.json`**（仅授 `main` 窗口）：`core:default`、`core:event:default` + `allow-listen` + `allow-emit`、`dialog:default` + `allow-open` + `allow-save`。

## 构建产物

`Cargo.toml`：`[lib] name = "opentake_tauri_lib"`，`crate-type = ["staticlib","cdylib","rlib"]`；`[[bin]] name = "opentake"`。`build.rs` 仅 `tauri_build::build()`。`protocol-asset` feature 开启（配合 assetProtocol）。

---

> 相关：[commands-ipc.md](commands-ipc.md) · [mcp.md](mcp.md)（`mcp::spawn` 细节）· [render.md](render.md)（`RenderState` 懒加载）· 跨模块 [opentake-core](../opentake-core/INDEX.md)（`AppCore` / `CoreEvent`）· [opentake-agent](../opentake-agent/INDEX.md)（MCP server）· 本模块自带 [README](../../../src-tauri/README.md)
>
> 导航：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
