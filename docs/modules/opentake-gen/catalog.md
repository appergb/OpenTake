# catalog — 模型目录与能力/计价矩阵

> 上级：[opentake-gen 目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 子系统级文档（不逐函数）。源码：[`crates/opentake-gen/src/catalog/`](../../../crates/opentake-gen/src/catalog/mod.rs)。完整规格见 [SPEC.md](SPEC.md) §4 / §2.4。

---

## 定位

模型目录是**数据驱动 UI/Agent 的单一真相源**：每个模型一条 `CatalogEntry`，携带能力矩阵（时长/分辨率/宽高比/参考槽位上限/互斥规则…）与计价字段。托管与 BYOK 两种模式共用**同一结构**，从而 UI 与 Agent 行为一致（设计公理 A5）。

数据来源二选一：

- **托管模式**：`GET /v1/models` 从 proxy 拉取。
- **BYOK 模式**：编译进二进制的内置静态目录 `builtin_catalog.json`（无后端、无网络）。

## `CatalogEntry` 与四类能力矩阵

`entry.rs` 是上游 `CatalogEntry` + 四个 `*Caps` 结构的 1:1 端口。一条 entry 含：

- `id`（`prefix:vendorModel`）、`kind`（video/image/audio/upscale）、`display_name`、`allowed_endpoints`、`response_shape`（video/images/audio/upscaledImage）。
- `ui_capabilities`：按 `kind` 四选一的能力矩阵枚举。
- 计价字段：`credits_per_second` / `audio_discount_rate` / `credits_per_image` / `qualities` / `audio_pricing` / `credits_per_second_upscale`。

四类 `*Caps`：

| 矩阵 | 关键字段（节选） |
|---|---|
| `VideoCaps` | `durations` / `resolutions` / `aspect_ratios` / `supports_first_frame` / `supports_last_frame` / `max_reference_{images,videos,audios}` / `frames_and_references_exclusive` / `requires_source_video` / `requires_reference_image` / `reference_tag_noun` |
| `ImageCaps` | `resolutions` / `aspect_ratios` / `qualities` / `supports_image_reference` / `max_images` |
| `AudioCaps` | `category`（tts/music/sfx）/ `voices` / `default_voice` / `supports_{lyrics,instrumental,style_instructions}` / `durations` / `min_prompt_length` / `inputs`（text/video）/ `min_seconds` / `max_seconds` |
| `UpscaleCaps` | `speed`（Fast/Medium/Slow）/ `p75_duration_seconds` / `supported_types`（video/image） |

### 按 `kind` 分发的自定义 `Deserialize`

`uiCapabilities` 的具体形状取决于 `kind`，serde 无法直接表达，故 `CatalogEntry` 手写 `Deserialize`（复刻上游 `CatalogEntry.init`）：先把 `uiCapabilities` 收成原始 `serde_json::Value`，等 `kind` 解析后再决定解成哪个 `*Caps`。`#[serde(other)]` 兜底字段使未知顶层键被忽略，兼容后端新增字段（移植铁律：读旧/新工程不破坏）。

`AudioPricing` 按 `mode` 内部标签分三种：`perThousandChars{rate}` / `perSecond{rate}` / `flat{price}`。

## `Catalog` 包装与查询

`mod.rs` 的 `Catalog` 是 `Vec<CatalogEntry>` 的薄包装：

- `Catalog::builtin()` —— 加载内置静态目录。
- `entries()` / `into_entries()` / `by_id(id)` / `of_kind(kind)` —— 查询入口；`of_kind` 对应上游 agent `list_models` 的 `?type=` 过滤。

## 内置静态目录 `builtin.rs` + `builtin_catalog.json`

- JSON 在编译期 `include_str!` 进二进制；`builtin_catalog()` 解析返回 `Vec<CatalogEntry>`，解析失败 `panic`（编译期资产，属程序员错误）。
- 约定（有单测守护）：所有 id 都是 `prefix:vendorModel` 且唯一；覆盖 image/video/audio/upscale 四种 kind；覆盖 fal/replicate/openai/elevenlabs 四个 provider。
- BYOK 目录**省略计价**（BYOK 不计费），但能力矩阵填满，确保 UI 数据驱动一致。

## `list_models` 的来源

- **BYOK**：[`GenClient::list_models`](client-transport.md) 直接返回 `catalog.entries()`，零网络。
- **托管**：`GET /v1/models` + Bearer。
- **Agent 接线（已落地）**：`opentake-agent` 的 `mcp/gen_catalog.rs` 调 `opentake_gen::builtin_catalog()` + `filter_by_kind`，投影成 MCP `list_models` 的 `{ models, loaded }` 载荷（纯本地、同步、有测试）。这是 agent 与 gen 之间**第一座真实桥梁**（对应 ROADMAP #111）。

## 客户端成本预估 `cost.rs`

纯函数、仅展示用——真实扣费由 proxy 在任务完成时结算（托管），BYOK 下完全没有计费。1:1 端口上游 `CostEstimator`：

| 函数 | 规则（均 `ceil` 向上取整，≤0 记 0） |
|---|---|
| `video_cost` | `rate = creditsPerSecond[res] ?? [""]`；不生成音频时乘 `audioDiscountRate[res] ?? [""]`；`ceil(rate × duration)` |
| `image_cost` | 先查二维 `"<res>\|<quality>"`，再 quality-only，再按 res（或 `""`）；乘 `max(1, num_images)` |
| `audio_cost` | perThousandChars → `ceil(rate × chars/1000)`；perSecond → `ceil(rate × secs)`；flat → `ceil(price)` |
| `upscale_cost` | `ceil(creditsPerSecondUpscale × max(1, duration))` |
| `cost_for_input` | 按 `entry.kind` 分发上述；音频是否计时长由 caps 决定（`durations` 非空，或 `inputs` 含 `video`） |
| `format_credits` | 展示文案：`None→"—"`、`≤0→"0 credits"`、`1→"1 credit"`、其余 `"N credits"` |

「`dict[key] ?? dict[""]`」的空串默认档语义在 `resolved_rate` / `audio_discount` 中统一实现。

## 对应上游 Swift

- `CatalogEntry` / 四 `*Caps` ← `ModelCatalog.swift:112-241`。
- `Catalog` 的 `?type=` 过滤 ← 上游 agent `ToolExecutor+Generate.swift:374-387`。
- `cost.rs` ← `CostEstimator.swift:3-108`（部分规则散落于 `VideoModelConfig.swift` 的 `audioDiscount`）。
- 上游 `ModelCatalog` 从 Convex `models:list` 动态订阅；OpenTake 改为「内置静态目录（BYOK）/ proxy 下发（托管）」（MODULE-PORT-MAP「Generation」段，verdict `cloud-rebuild`）。

## 完成状态

- **已实现**：`CatalogEntry` 自定义反序列化、四类 caps、`Catalog` 查询、内置静态目录与守护测试、全套 `cost.rs` 计价（均有单测）；`list_models` 在 BYOK/托管两侧均通，且已被 agent 的 `list_models` 工具接线。
- **计划中 / 未接线**：`cost.rs` 仅为纯函数库，尚无前端成本显示 UI（上游 `GenerationView` 成本展示属 `ui-rebuild`，见 ROADMAP Phase 9 / PORT-1TO1-GAP）；托管 proxy（含 `/v1/models` 真实下发）属 Phase 9 自建后端，尚未实现。

## 源码

| 文件 | 内容 |
|---|---|
| [`catalog/mod.rs`](../../../crates/opentake-gen/src/catalog/mod.rs) | `Catalog` 包装 + `by_id` / `of_kind` 查询 |
| [`catalog/entry.rs`](../../../crates/opentake-gen/src/catalog/entry.rs) | `CatalogEntry` 自定义 `Deserialize` / `ModelKind` / `ResponseShape` / 四 `*Caps` / `AudioPricing` |
| [`catalog/builtin.rs`](../../../crates/opentake-gen/src/catalog/builtin.rs) | `builtin_catalog()`（编译期 `include_str!`）+ 守护测试 |
| [`catalog/builtin_catalog.json`](../../../crates/opentake-gen/src/catalog/builtin_catalog.json) | 内置静态目录数据（四 kind × 四 provider，省略计价） |
| [`catalog/cost.rs`](../../../crates/opentake-gen/src/catalog/cost.rs) | 客户端成本预估纯函数（展示用） |

---

页脚：[opentake-gen 目录 INDEX.md](INDEX.md) · [模块文档树 ../INDEX.md](../INDEX.md) · [docs 总目录 ../../INDEX.md](../../INDEX.md)
