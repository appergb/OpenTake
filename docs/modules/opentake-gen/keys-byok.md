# keys-byok — BYOK 密钥管理（系统钥匙串）

> 上级：[opentake-gen 目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 子系统级文档（不逐函数）。源码：[`keys.rs`](../../../crates/opentake-gen/src/keys.rs)，与 [`src-tauri/src/secret.rs`](../../../src-tauri/src/secret.rs) 配合。完整规格见 [SPEC.md](SPEC.md) §2.3。安全要点见 [`../../architecture/PORT-1TO1-GAP.md`](../../architecture/PORT-1TO1-GAP.md) P1-12。

---

## 定位

BYOK（Bring Your Own Key）密钥的存取层。提供商 API key **绝不硬编码、绝不进 JS 内存 / 设置文件 / localStorage**，只存系统钥匙串与 Rust 后端。存储抽象在 `KeyStore` trait 之后，使测试用内存实现、生产用 OS 钥匙串。

## `ProviderKey` —— 受管密钥枚举

`keys.rs` 定义 5 个受管密钥：`Fal` / `Replicate` / `OpenAI` / `ElevenLabs` / `Anthropic`。每个有三项稳定派生：

| 派生 | 用途 | 示例（Fal） |
|---|---|---|
| `account()` | 钥匙串账户串（每键一个，稳定） | `fal-api-key` |
| `env_var()` | **仅 debug 构建**的覆盖环境变量 | `FAL_KEY` |
| `prefix()` | 对应的 provider 路由前缀（与 adapter `prefix()` 一致） | `fal` |

`SERVICE = "io.opentake.app"` 是钥匙串 service 标识（上游 bundle id，OpenTake 重命名）。

> 前四个键对应[四个生成 provider](providers.md)；`Anthropic` 用于内置 Agent 聊天的 LLM key（非生成厂商）。

## `KeyStore` trait 与两个实现

- `KeyStore`：`save` / `load`（归一化，空→`None`）/ `delete`。`dyn KeyStore` 上的便捷方法 `load_key`/`save_key`/`delete_key` 按 `ProviderKey` 操作；`load_key` 在 **debug 构建**先查 env var 覆盖（复刻上游 `#if DEBUG`）。
- `KeyringStore`：生产实现，基于 `keyring` crate，**跨平台**——macOS Keychain / Windows Credential Manager / Linux Secret Service。`NoEntry` 视为「无 key」，删除不存在的 key 是 no-op。
- `MemoryKeyStore`：测试用内存实现，**从不碰真实钥匙串**；`with_key` 便捷播种。

### 归一化规则

`load` 一律 `normalize`：去首尾空白；空串视为不存在（`None`）。复刻上游 `KeychainStore.load` 的 trim 行为（值边界防御）。

## 与 `src-tauri` 钥匙串命令的配合

`src-tauri/src/secret.rs` 是 `#[tauri::command]` 薄封装，复用本 crate 的 `KeyringStore`，提供 `secret_save` / `secret_load` / `secret_delete`。安全设计：

- **明文单向**：WebView 仅在 `secret_save` 时发出明文 key；**永不回传前端**——`secret_load` 只给**掩码**表示（复刻上游 `AgentPane.mask`：≤4 字符显 32 个圆点，否则 36 圆点 + 末 4 位）。
- **provider 白名单**：`account_for` 校验 provider，未知值无法寻址任意钥匙串项。

> **现状（与 `secret.rs` 对齐）**：`secret.rs` 的 `account_for` 白名单已放开**全部 6 个账户**——`anthropic` / `fal` / `replicate` / `openai` / `elevenlabs` / `google`（账户串沿用 `<prefix>-api-key` 约定），即**聊天与生成 provider 的 key 均可由前端 `secret_save` 写入钥匙串**（3 聊天 + 3 生成）。注意：`opentake-gen::keys::ProviderKey` 目前只定义 5 个受管键（Fal/Replicate/OpenAI/ElevenLabs/Anthropic），`google` 账户仅由 `secret.rs` 单独映射到钥匙串账户串。

## 安全要点（移植铁律 + 安全）

- **不硬编码**：源码无任何明文 key；唯一注入路径是钥匙串（生产）或 debug env var（仅调试）。
- **不外泄**：key 不进 JS 内存 / 前端持久化；前端只见掩码与 `has_key` 布尔。
- **值边界防御**：读出即 trim，空白等同缺失，避免「看似有 key 实为空格」。
- 内部错误（`keyring::Error` 等）经 `From` 收敛为 `GenError`；边界层转 `Err(String)`，不泄露内部细节。

## 对应上游 Swift

- `KeyStore` / `KeyringStore` ← `KeychainStore.swift:4-52`（service=bundle id、account=稳定串、trim、空即缺失）。
- debug env 覆盖 + `Anthropic` 键 ← `AnthropicClient.swift:7-30`（`AnthropicKeychain`）。
- `secret.rs` 掩码 ← `AgentPane.swift:131-134` 的 `mask`。
- 移植定位：MODULE-PORT-MAP「Generation」段把云后端/鉴权归 `cloud-rebuild`；密钥安全存储是 PORT-1TO1-GAP **P1-12**（「BYOK 密钥安全存储」，要求 keyring/stronghold + 命令，杜绝明文）。

## 完成状态

- **已实现**：`ProviderKey` 五键 + 三派生、`KeyStore` trait、跨平台 `KeyringStore`、`MemoryKeyStore`、归一化与 debug env 覆盖（均有单测）；`src-tauri` 三命令（save/load/delete）+ 掩码 + provider 白名单（6 账户：3 聊天 + 3 生成）已接线并测试；生成 key 经钥匙串 → `GenClient::byok`（`src-tauri/src/generation.rs` 的 `configured_byok_prefixes` / `build`）装配驱动四个生成工具，已随生成桥接线交付。
- **计划中**：
  - 上游 `ModelPreferences`（本地持久化被禁用模型 id）属 `ui-rebuild`，尚未实现。

## 源码

| 文件 | 内容 |
|---|---|
| [`keys.rs`](../../../crates/opentake-gen/src/keys.rs) | `ProviderKey` / `SERVICE` / `KeyStore` trait + `load_key` 便捷 / `KeyringStore` / `MemoryKeyStore` / `normalize` |
| [`../src-tauri/src/secret.rs`](../../../src-tauri/src/secret.rs) | `secret_save`/`secret_load`/`secret_delete` 命令 + `mask` + provider 白名单（6 账户：3 聊天 + 3 生成） |

---

页脚：[opentake-gen 目录 INDEX.md](INDEX.md) · [模块文档树 ../INDEX.md](../INDEX.md) · [docs 总目录 ../../INDEX.md](../../INDEX.md)
