# opentake-project — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> `opentake-project` = 工程持久化层：`.opentake` 目录包读写 + 自包含归档 + XMEML(FCP7 XML) 时间线导出 + 生成日志。依赖只向下：仅依赖 `opentake-domain`，被 `opentake-core` / `src-tauri` 调用。

---

## 总览

- **[OVERVIEW.md](OVERVIEW.md)** — 定位与分层位置、职责边界、`.opentake` 包结构与三类数据流、对应上游 Swift（Project + Export 两模块的纯逻辑子集）、完成状态（已实现 vs 计划中）、移植铁律（重点：serde `#[serde(default)]` + `Option<T>` 保证读旧工程不破坏）。

## 子系统文档

- **[bundle-archive.md](bundle-archive.md)** — `bundle.rs`（`Project` 句柄 + `open`/`save` 读取容错分级 + 原子写）+ `archive.rs`（自包含归档：纯词法去重 / Foundation 扩展名语义 / remove-then-land / 附属搬运）。含 `error.rs` 错误类型说明。重点：工程文件格式与 serde 向后兼容。
- **[layout.md](layout.md)** — `layout.rs`：`.opentake` 包内文件名 / 目录契约（`project.json` / `media.json` / `generation-log.json` / `thumbnail.jpg` / `media/` / `chat-sessions/`）与路径拼接函数。`chat/` → `chat-sessions/` 的刻意差异。
- **[fcpxml-export.md](fcpxml-export.md)** — `fcpxml.rs`：时间线导出。**产物是 XMEML 4 / FCP7 XML（`.xml`）**（因 Premiere 不读 FCPXML），1:1 端口上游 `XMLExporter`。覆盖位置 / 裁剪 / 变速 / 音量 / 不透明度 / 变换 / 裁切 / 淡变 / A·V 链接；两处跨平台降级；文末含单文件 >800 行的拆分建议。
- **[gen-log.md](gen-log.md)** — `gen_log.rs`：生成式 AI 操作审计日志（`generation-log.json`）。`version` 缺省 1、美元 → credits 向上取整迁移、`total_credits`。

## 相关跨切面（架构）

- [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md) — 逐模块上游 Swift → Rust 移植地图（本 crate 对应上游 `Project` 与 `Export` 两节的纯逻辑子集；NSDocument / 窗口 / 缩略图 / ProjectRegistry / SampleProjectService 归他处或计划中）。
- [ROADMAP.md](../../architecture/ROADMAP.md) — 分阶段路线图（Phase 2 = 持久化；Phase 5 = 导出）。
- [PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) — 1:1 复刻差距逐项（P0-1 新建即落盘 / P1-1 自动保存 / P2-1 缩略图等，本 crate 周边的计划项；⚠️ 历史参考）。
- [ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) — 总体架构（§9 `.opentake` 包结构）。

## 相关模块

- [opentake-domain](../opentake-domain/INDEX.md) — 提供 `Timeline` / `MediaManifest` / `MediaManifestEntry` / `MediaSource` / `MediaResolver` 等值类型；本 crate 只加 IO 与生成日志类型。

## 源码

```
crates/opentake-project/src/
├── lib.rs        模块声明 + 公开 API re-export（Project / archive / export_xmeml / GenerationLog / domain 值类型）
├── bundle.rs     Project 句柄 + open/save/save_to + 原子写（端口 VideoProject 持久化）
├── archive.rs    archive() 自包含归档 + ArchiveReport（端口 PalmierProjectExporter）
├── fcpxml.rs     export_xmeml() XMEML 4/FCP7 XML 导出（端口 XMLExporter，约 1489 行）
├── gen_log.rs    GenerationLog / GenerationLogEntry（端口 GenerationLog，含美元迁移）
├── layout.rs     .opentake 包文件名契约 + 路径函数（端口 enum Project 常量）
└── error.rs      ProjectError（thiserror）+ Result 别名
```

源文件树根：[`../../../crates/opentake-project/src/`](../../../crates/opentake-project/src/)

---

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
