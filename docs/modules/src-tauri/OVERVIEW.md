# src-tauri — 总览

> 上级：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)

## 一句话定位

**Tauri 2 桌面壳 + 命令边界层**：把所有 crate 装配成一个原生桌面进程，持有权威 [`opentake_core::AppCore`] 作为 Tauri managed state，对前端暴露一层薄 `#[tauri::command]` 接口，并把 core 的事件总线桥接回 WebView。它本身不含领域逻辑——每个命令都只是「取 State → 委派 core → 把边界错误转成 `Err(String)`」的转接层。

### 依赖分层（src-tauri 在最上面，装配一切）

```
opentake-domain                                值语义叶子层
   ▲
opentake-ops                                   纯引擎 + EditCommand + 撤销栈
   ▲
opentake-project / render / media / motion / agent / gen   能力层
   ▲
opentake-core                                  会话 / DI / 事件总线
   ▲
src-tauri  ★ 本模块                            Tauri 壳 + 命令注册 + 事件桥
   ▲
web                                            React/TS 前端（只持只读镜像，经 IPC 调本模块）
```

`Cargo.toml` 直接依赖 `opentake-core / opentake-ops / opentake-domain / opentake-media / opentake-render / opentake-gen / opentake-agent / opentake-project`，外加 `tauri`、`tauri-plugin-dialog`、`image`(PNG 编码)、`base64`。它是 workspace member（`members = [..., "src-tauri"]`），但**不在 `crates/` 下**。

## 职责边界

**做什么：**
- 在 `setup` 里构造唯一的 `AppCore`，连同媒体引擎、全局素材库、GPU 渲染上下文一并 `manage` 进 Tauri 状态。
- 用 `generate_handler!` 注册全部命令（读取 / 生命周期 / 唯一编辑入口 / 媒体 / 渲染 / 导出 / 密钥 / 库）。
- 订阅 `AppCore` 的 `CoreEvent` 总线，逐条转发为同名 Tauri 事件给前端。
- 窗口生命周期门控（关窗不退出、Dock 重开、标题栏样式）。
- 启动前解析 FFmpeg/ffprobe 绝对路径。
- 在 `setup` 时拉起回环 MCP server（与 UI 共享同一会话）。

**不做什么：**
- 不持有撤销栈、不做时间线变更运算（全在 `opentake-ops` / `opentake-core`）。
- 不做帧↔秒换算之外的领域计算。
- 像素↔帧换算属前端；帧↔秒换算属 Rust。
- 不直接写 `EditCommand` 的 serde（见下「IPC 序列化陷阱」）。

## 关键概念与数据流

### 单一真理 + 只读镜像

权威 `Timeline` 只在 Rust 的 `AppCore` 里。前端（Zustand）只持**只读镜像 + 版本号**，不做撤销、不持领域逻辑（`docs/architecture/ARCHITECTURE.md` §2「真相源在 Rust，前端持镜像」）。

### 编辑闭环（一次手势的完整链路）

```
前端 UI 手势
  → web/src/lib/api.ts  editApply(command)
  → Tauri invoke("edit_apply", { command })          ← IPC 边界，command 是 camelCase JSON
  → commands.rs  edit_apply(EditRequest)              ← serde 反序列化成 DTO
  → EditRequest::into_command() → EditCommand         ← DTO 映射成纯枚举
  → opentake_core::dto::handle_edit_apply → AppCore::apply()
        （快照 → 纯函数变更 → 有变更才提交 → version++，并发 TimelineChanged 事件）
  → lib.rs forward_event → app.emit("timeline_changed", …)
  → 前端监听 timeline_changed → 调 get_timeline() 刷新只读镜像
```

要点：**写**走 `edit_apply`（单向命令），**读**走 `get_timeline`（拉取镜像）。前端从不就地改镜像，而是收到事件后整体重取。详见 [commands-ipc.md](commands-ipc.md)。

### 命令注册与状态装配

`lib.rs` 的 `run()` 用 `tauri::generate_handler![…]` 一次性注册全部命令，`setup` 闭包里 `app.manage(...)` 注入四类共享状态：`AppCore`、`MediaState`（媒体引擎包装）、`LibraryState`（全局素材库）、`RenderState`（懒加载 GPU 上下文）。详见 [setup-lib.md](setup-lib.md)。

### 事件桥

`CoreEvent` → Tauri 事件的映射（`lib.rs::forward_event`）：

| CoreEvent | Tauri 事件名 |
|---|---|
| `TimelineChanged` | `timeline_changed` |
| `ProjectOpened` | `project_opened` |
| `ProjectSaved` | `project_saved` |
| `MediaChanged` | `media_changed` |

转发是 best-effort：WebView 不在（拆卸期）时 `emit` 失败被忽略，不 panic 发事件的线程。

## IPC 序列化陷阱（高频 bug 来源，务必先懂）

`opentake_ops::EditCommand` 是**纯枚举，没有 serde derive**（它携带引擎值类型）。因此 IPC 层在 `commands.rs` 另有一个 serde DTO **`EditRequest`**：

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EditRequest { … }
```

由 `edit_apply` 经 `into_command()` 映射成 `EditCommand`。三条铁律：

1. **多词字段在前端线上必须是 camelCase**（如 `atFrame` / `trackIndex` / `clipIds` / `mediaRef`）。
2. **serde 的枚举级 `rename_all` 不会重命名结构体变体的字段**——所以**每个变体还要各自再加 `#[serde(rename_all = "camelCase")]`**。漏掉就反序列化失败（`missing field clip_ids`），表现为「删除 / 分割 / Inspector 全部静默失效」（历史真实 bug，见 `command.rs` 同名回归测试 `deserializes_camelcase_multiword_commands`）。
3. **改 IPC 字段时三边同步**：Rust DTO（`src-tauri/src/commands.rs` 的 `EditRequest`）、前端类型（`web/src/lib/types.ts` 的 `EditRequest`）、调用处（`web/src/lib/api.ts`）必须一起改。IPC 内若静默吞错，先加 `try/catch` 把错误暴露出来。

同样的 DTO 模式也用于其它带值类型的命令：`export.rs` 的 `ExportRequest`、`render.rs` 的入参等，均 `#[serde(rename_all = "camelCase")]`，且对可选字段加 `#[serde(default)]` 以兼容旧 / 部分载荷。

## 对应上游 Swift

参见 [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md)（App 层 / Export 等条目）：

- **进程入口 + 启动序列**（`main.swift`）、**应用代理**（`AppDelegate`）→ 本模块 `main.rs` + `lib.rs::run()`。
- **关窗不退、回主页 / Dock 重开** 对应 `applicationShouldHandleReopen`（无可见窗口时回主页）与 `setActivationPolicy(.regular)`。Tauri 下用 `WindowEvent::CloseRequested` 拦截隐藏 + `RunEvent::Reopen` 重显。
- **暗色透明全尺寸标题栏**（`makeWindowControllers`）→ `tauri.conf.json` 的 `titleBarStyle: "Overlay"` + `hiddenTitle` + `trafficLightPosition`。
- **NSDocument 生命周期 / undoManager 注入** → ui-rebuild：撤销栈下沉到 Rust core 的快照栈，本模块只暴露 `undo`/`redo`/`can_undo`/`can_redo` 命令。
- **导出**（AVFoundation composition/export）→ needs-replacement：用 wgpu 合成 + FFmpeg 编码重写，见 [export.md](export.md)。
- **BYOK 密钥本地存储**（Security.framework + `AgentPane.mask`）→ keyring crate，掩码规则照搬，见 [secret.md](secret.md)。
- **MCP server**（原 Agent/ToolExecutor）→ Rust 实现，端口 19789、绑 `127.0.0.1` 行为照搬，见 [mcp.md](mcp.md)。

## 完成状态

| 子系统 | 状态 | 说明 |
|---|---|---|
| 命令边界 / `EditRequest` 映射 | ✅ 已实现 | 30 个命令注册；`EditRequest` 覆盖前端 v1 全部编辑变体，带回归测试 |
| 事件桥 | ✅ 已实现 | 4 类 CoreEvent 全部转发 |
| 启动 / 窗口 / FFmpeg 解析 | ✅ 已实现 | 关窗隐藏 + 关窗前 flush 存盘；`RunEvent::Reopen` **仅 macOS** |
| 单帧预览合成 `composite_frame` | ✅ 已实现 | 视频 + 图片 + 文本层；**Lottie 跳过**（resolver 返回 `None`，待 #65 后续） |
| 整片导出 `export_video` | 🟡 部分 | **仅 H.264 / .mp4** + 线性音频混音；H.265 / ProRes 类型已留位但**未接线**（`resolve_preset` 明确报错）；**无进度回调 / 取消** |
| 媒体导入 / relink / 波形 | ✅ 已实现 | 缩略图仍为占位（`thumbnail: None`） |
| 全局素材库（7 命令） | ✅ 已实现 | copy-on-favorite，跨工程 |
| 密钥（BYOK，3 命令） | ✅ 已实现 | 仅 anthropic / openai / google 三个白名单账户 |
| MCP server spawn | ✅ 已实现 | 与 UI 共享会话克隆 + workflow registry；bind 失败仅记日志不致命 |
| 跨平台窗口重显 | 🟡 计划中 | `RunEvent::Reopen` 仅 macOS；其它平台靠托盘 / OS 重现是后续项 |
| FFmpeg 随包分发 | 🟡 计划中 | 现依赖宿主机磁盘上的 ffmpeg；Tauri `externalBin` 打包是后续项 |

## 运行期

- **FFmpeg ≥ 6.0 必须在 PATH**。打包后 macOS `.app` 从 Finder/Dock 启动只继承精简的 launchd `PATH`（不含 Homebrew），故 `lib.rs::resolve_media_tools()` 在解码前把 `ffmpeg`/`ffprobe` 的**绝对路径**写入环境变量 `OPENTAKE_FFMPEG` / `OPENTAKE_FFPROBE`（`opentake-media` 的 `ff` 模块读取它们）；显式设置的覆盖值始终优先。查找顺序：现有 `PATH` 目录 → `/opt/homebrew/bin` → `/usr/local/bin` → `/opt/local/bin` → `/usr/bin`。
- **关窗不退出**：`WindowEvent::CloseRequested` 被 `prevent_close()` 拦截——先 `save_project(None)` 做最终落盘（无打开工程时静默忽略错误），再 `hide()` 窗口并 `emit("go_home")`。`Cmd+Q`（`ExitRequested`）仍正常退出。
- **`RunEvent::Reopen` 仅 macOS**：Dock 点击且无可见窗口时重显并聚焦主窗口（`#[cfg(target_os = "macos")]` 门控）。
- **窗口配置**（`tauri.conf.json`）：`titleBarStyle: "Overlay"` + `hiddenTitle: true` + `trafficLightPosition {x:18,y:24}`（自绘标题栏，红绿灯内嵌）；尺寸 1600×1000，最小 760×480；`backgroundColor: "#0A0A0A"`；`dragDropEnabled: false`。
- **权限**（`capabilities/default.json`）：`core:default` + 事件 listen/emit + `dialog`（open/save）。`security.csp: null`，`assetProtocol` 开启且 scope `**`（本地资源可被 WebView 读取）。

---

> 子系统文档：[commands-ipc.md](commands-ipc.md) · [setup-lib.md](setup-lib.md) · [export.md](export.md) · [library-media.md](library-media.md) · [render.md](render.md) · [secret.md](secret.md) · [mcp.md](mcp.md)
>
> 导航：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
