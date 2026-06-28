# secret — BYOK 密钥的系统钥匙串存储

> 上级：[本模块目录](INDEX.md) · [总览](OVERVIEW.md) · [模块文档树](../INDEX.md)
>
> 源码：[`../../../src-tauri/src/secret.rs`](../../../src-tauri/src/secret.rs)

## 定位

安全的 BYOK（Bring Your Own Key）API 密钥存储命令。薄 `#[tauri::command]` 包 `opentake-gen` 的跨平台 `KeyringStore`（macOS Keychain / Windows Credential Manager / Linux Secret Service）。

**单向边界**：明文 key 只在 `secret_save` 方向从 WebView 进来一次；**永不回传**前端——`secret_load` 只给**掩码**表示（复刻上游 `AgentPane.mask`）。因此 key 只活在 OS 钥匙串 + Rust 后端，绝不进 JS 内存 / settings store / `localStorage`。

## 命令清单（3 个）

| 命令 | 入参 | 返回 | 说明 |
|---|---|---|---|
| `secret_save` | `provider`、`key` | `SecretStatus` | key 先 trim；空 key 被拒（不存）。返回新掩码状态，免前端往返明文 |
| `secret_load` | `provider` | `SecretStatus` | 该 provider 的掩码状态（绝非明文） |
| `secret_delete` | `provider` | `SecretStatus` | 删除；删不存在的 key 视为成功（no-op）。返回清空后状态 |

### SecretStatus（camelCase）

```rust
pub struct SecretStatus {
    has_key: bool,    // 驱动 UI
    masked: String,   // 项目符号掩码（无 key 时为空）
}
```

## Provider 白名单（账户映射）

```rust
fn account_for(provider: &str) -> Result<&'static str, String> {
    match provider {
        "anthropic" => Ok("anthropic-api-key"),
        "openai"    => Ok("openai-api-key"),
        "google"    => Ok("google-api-key"),
        other       => Err(format!("unknown provider: {other}")),
    }
}
```

遵循 `opentake_gen::keys` 的 `<prefix>-api-key` 约定。**在此校验 provider 意味着未知值绝不能寻址任意钥匙串条目**——唯一可写的账户就是 UI 提供的这三个。这是一道安全边界（防止任意账户名注入钥匙串）。

## 掩码规则（mask）

复刻上游 `AgentPane.mask`（`AgentPane.swift:131-134`）：

- key 长度 ≤ 4：显示 32 个项目符号 `•`，不泄露任何字符。
- 否则：36 个项目符号 + 末 4 个字符（用户可辨认而不可恢复）。

按 **char（码点）** 而非 byte 计数（多字节 key 也正确）。有单测覆盖短 key 全掩、长 key 仅露末 4、Unicode 计数。

---

> 相关：[setup-lib.md](setup-lib.md)（命令注册）· 跨模块 [opentake-gen](../opentake-gen/INDEX.md)（`KeyringStore` / `KeyStore` / keys 约定）· [opentake-agent](../opentake-agent/INDEX.md)（密钥变更后 Agent 重连，对应上游行为）
>
> 导航：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
