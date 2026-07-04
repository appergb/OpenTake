# gen-log — 生成式 AI 操作日志

> 上级：本模块目录 [INDEX.md](INDEX.md)

源文件：[`gen_log.rs`](../../../crates/opentake-project/src/gen_log.rs)。端口自上游 `GenerationLog` / `GenerationLogEntry`（`Editor/ViewModel/EditorViewModel+Cost.swift`），持久化为包内 `generation-log.json`。

## 职责

记录工程内每次 AI 生成（视频 / 图片 / 音频 / upscale 等）的**追加式审计日志**：用了哪个模型、花了多少 credits、何时生成。domain 层刻意零 IO 故不含此类型，由本持久化 crate 补上。它是一份纯数据 + 序列化容错，**不**调用任何生成接口（生成在 `opentake-gen` / `opentake-agent`）。

## 关键类型

- **`GenerationLog`** — 整份日志：
  - `version: i64`（`#[serde(default = "default_version")]`，缺省 1）。
  - `entries: Vec<GenerationLogEntry>`（`#[serde(default)]`，缺省空）。
  - `Default` / `new()` 给出空日志（`version = 1`）。
  - `total_credits() -> i64` — 各行 `cost_credits` 求和（`None` 当 0），对拍上游 `totalGenerationCost`。
- **`GenerationLogEntry`** — 一行（`#[serde(rename_all = "camelCase")]`）：
  - `id: String` — 稳定行 id；旧文件缺省时解码为空串。
  - `model: String` — 必填，生成所用模型标识。
  - `cost_credits: Option<i64>` — credits，未知为 `None`，序列化时 `skip_serializing_if = "Option::is_none"`。
  - `created_at: Option<f64>` — Apple 参考日秒，未知为 `None`，同样 skip-if-none。
  - 用 `i64` 匹配上游 Swift `Int` 在 arm64 的 64 位宽。

## 关键算法 / 容错（必须 1:1 复刻）

- **`version` 缺省 + 回退都是 1** — 结构体默认 1，缺 `version` 键也回退 1。**注意**与 `media.json` 不同：`MediaManifest` 默认 2、缺省回退 1，二者别混（见 [OVERVIEW](OVERVIEW.md) 移植铁律 1）。
- **美元 → credits 迁移**（手写 `Deserialize`）— 当 `costCredits` 缺失但旧字段 `cost`（美元 float）在场时：`cost_credits = ceil(cost * 100)`（Swift `(dollars*100).rounded(.up)`，**向上取整，永不截断**，如 `0.005 USD → 1`）。`costCredits` 与 `cost` 同在时，**`costCredits` 优先**（上游仅在 `costCredits` 缺失时才看旧 `cost`）。
- **缺 `id` 解码为空串** — 上游会合成 UUID；这里解码为空串，bundle 层不改它（行只追加、不被别处按 id 引用）。
- **`created_at` 是 Apple 参考日秒** — `f64`，`JSONEncoder` 默认 `Date` 编码；墙钟换算归上层。

## 序列化形状

写出时 `model` / `id` 恒在，`costCredits` / `createdAt` 为 `None` 则省略键，camelCase。源文件内含 8 个单测覆盖：缺省 version、camelCase 往返、None 字段省略、美元迁移（含向上取整）、`costCredits` 优先、缺 id、`total_credits` 求和、整份往返。

## 与其他子系统的关系

- [`bundle`](bundle-archive.md) 的 `Project::open` **宽松**读 `generation-log.json`（失败降级 `None`），`save` 在持有日志时写出。
- [`archive`](bundle-archive.md) 归档时把传入的 `GenerationLog` 原样写进目标包。
- 通过 `lib.rs` re-export 给下游（`opentake-core` / `src-tauri`），供成本统计 / 工程活动展示。

---

> 上级：本模块目录 [INDEX.md](INDEX.md)
