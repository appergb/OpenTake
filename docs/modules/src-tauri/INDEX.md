# src-tauri — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> **Tauri 2 桌面壳 + 命令边界层**：装配所有 crate 成原生进程，持有权威 `AppCore`，对前端暴露薄 `#[tauri::command]` 接口，并把 core 事件桥回 WebView。它是 workspace member，但不在 `crates/` 下。

## 文档

- **[总览 OVERVIEW.md](OVERVIEW.md)** — 定位 / 依赖分层 / 编辑闭环数据流 / **IPC 序列化陷阱** / 上游对应 / 完成状态 / 运行期。

## 子系统

- **[commands-ipc.md](commands-ipc.md)** — 命令边界层与 IPC 序列化。前端唯一读写入口；`EditRequest` serde DTO 映射成 `EditCommand`；**camelCase 陷阱**（每个变体都要 `rename_all`，三边同步）；`export_fcpxml` 实为 XMEML。
- **[setup-lib.md](setup-lib.md)** — 应用装配与窗口生命周期。`generate_handler!` 注册 30 命令、`setup` 装配四类 managed state、事件桥、关窗不退 + `RunEvent::Reopen`（仅 macOS）、`titleBarStyle: Overlay`、FFmpeg 绝对路径解析。
- **[export.md](export.md)** — 整条时间线视频导出（`export_video`）。逐帧 GPU 合成 → ffmpeg 编码，**仅 H.264/.mp4** + 线性音频混音；H.265/ProRes 留位未接线；无进度 / 取消。
- **[render.md](render.md)** — 单帧预览合成（`composite_frame`）。Timeline→RenderPlan→ffmpeg 解码→wgpu 合成→base64 PNG data URL；GPU 上下文懒加载（`RenderState`）；Lottie 跳过。
- **[library-media.md](library-media.md)** — 媒体导入命令（6 个：import/relink/抽音频/波形…）+ 全局跨工程素材库命令（7 个 `library_*`）。
- **[secret.md](secret.md)** — BYOK 密钥存系统钥匙串（3 命令）。明文单向入、只回掩码；provider 白名单（anthropic/openai/google）。
- **[mcp.md](mcp.md)** — `setup` 时 spawn 回环 MCP server（`127.0.0.1:19789`），共享 `AppCore` 克隆 + 工作流 registry；bind 失败不致命。

## 相关

- 本模块自带说明：[`../../../src-tauri/README.md`](../../../src-tauri/README.md)
- 架构与端口映射：[`../../architecture/ARCHITECTURE.md`](../../architecture/ARCHITECTURE.md)（§2 真相源在 Rust）· [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md)（App 层 / Export 等上游 Swift 对应）
- 交叉链（直接邻居）：
  - [`../opentake-core/INDEX.md`](../opentake-core/INDEX.md) — 本模块持有的 `AppCore`、`CoreEvent`、`dto::handle_*`、`apply` 事务的所在。
  - [`../opentake-agent/INDEX.md`](../opentake-agent/INDEX.md) — `mcp::spawn` 拉起的 MCP server / 工具 / 工作流插件的实现。
- 其它被装配的能力层：[`../opentake-ops/INDEX.md`](../opentake-ops/INDEX.md)（`EditCommand`）· [`../opentake-render/INDEX.md`](../opentake-render/INDEX.md)（合成器 / RenderPlan）· [`../opentake-media/INDEX.md`](../opentake-media/INDEX.md)（编解码 / 波形 / 库存储）· [`../opentake-gen/INDEX.md`](../opentake-gen/INDEX.md)（`KeyringStore`）· [`../opentake-project/INDEX.md`](../opentake-project/INDEX.md)（XMEML 导出）· [`../opentake-domain/INDEX.md`](../opentake-domain/INDEX.md)
- 前端对端：[`../web/INDEX.md`](../web/INDEX.md) · 具体见 [`../web/ipc-api.md`](../web/ipc-api.md)（`api.ts` / `types.ts` 三边同步的另两边）

## 源码树

```
src-tauri/
├── Cargo.toml                  # 清单（依赖全部 crate + tauri/dialog/image/base64）
├── build.rs                    # tauri_build::build()
├── tauri.conf.json             # 窗口（Overlay 标题栏 1600×1000）/ 构建 / 安全
├── capabilities/default.json   # 权限：core + event + dialog
├── icons/                      # 应用图标
└── src/
    ├── main.rs                 # 入口 → opentake_tauri_lib::run()          → setup-lib.md
    ├── lib.rs                  # builder / 状态 / 事件桥 / 窗口 / FFmpeg     → setup-lib.md
    ├── commands.rs             # #[tauri::command] shims + EditRequest DTO   → commands-ipc.md
    ├── export.rs               # export_video（整片导出）                    → export.md
    ├── render.rs               # composite_frame（单帧预览）                 → render.md
    ├── media.rs                # 媒体导入 / relink / 波形 / 抽音频           → library-media.md
    ├── library.rs              # 全局素材库 library_* 命令                   → library-media.md
    ├── secret.rs               # BYOK 密钥钥匙串                             → secret.md
    └── mcp.rs                  # 回环 MCP server spawn                       → mcp.md
```

---

> 导航：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
