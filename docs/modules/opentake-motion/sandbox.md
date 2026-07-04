# sandbox — 渲染安全沙箱策略

> 上级：[模块目录 INDEX.md](INDEX.md) · [总览 OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
> 源码：[`../../../crates/opentake-motion/src/sandbox.rs`](../../../crates/opentake-motion/src/sandbox.rs)

---

## 职责

为渲染**不可信** native fallback 动效代码（agent 生成或社区模板）提供安全策略。核心思路：把安全要求建模成**类型**（`SandboxPolicy`），使渲染器在类型层面无法"忘记应用"某条约束。策略类型与其纯检查住在本 crate、无需引擎即可单测；网络/CSP 的真正执行落在 `chromium` feature 后的真实 CDP 后端。

> 完成状态：策略类型与纯检查（`check_url` / `check_document_size` / origin 解析）**已实现并全测**；其在真实浏览器中的执行（请求拦截 / CSP / 超时熔断）随 `HeadlessChromiumRenderer` 待实现（见 [renderer.md](renderer.md)）。

---

## 四条要求（建模为类型）

1. **网络默认全拒**。仅显式 origin 白名单可达；默认 `SandboxPolicy` 空白名单 ⇒ 完全离线（也保证测试/CI 确定性）。
2. **渲染时间预算**（`timeout`）熔断跑飞动画 / `while(true)` 脚本，超时以 `MotionError::Timeout` 中止。
3. **无文件系统 / 工程访问**。渲染器须以"除所声明模板参数外无用户文件访问"的方式启动引擎——这是引擎启动期不变量（标志 + profile），在策略类型里**故意通过"没有任何授予路径的字段"来体现**（没有可设的东西）。
4. **内容大小上限**（`max_document_bytes`）在内联文档进引擎前就界定其字节长度。

---

## 常量

- `DEFAULT_TIMEOUT = 60s`——够几百帧复杂动画，又能熔断挂死。
- `DEFAULT_MAX_DOCUMENT_BYTES = 256 KiB`——动效是标记 + 少量脚本；更大者应作为带审计资产的模板包发布。

---

## `AllowedOrigin`

允许的网络 origin（scheme + host[:port]），如 `https://cdn.jsdelivr.net`。

- `parse(origin)`：规范化为小写、去尾斜杠；只接受 `https://`，或 loopback 的 `http://localhost` / `http://127.0.0.1` / `http://[::1]`（本地 dev 服务器）。**明文远程 origin 一律拒绝**（返回 `None`）。
- **不支持通配**：每个 origin 必须显式命名（对齐 web/security.md：不 cargo-cult 宽泛 `connect-src`）。

---

## `SandboxPolicy`

```rust
pub struct SandboxPolicy {
    pub allowed_origins: Vec<AllowedOrigin>,  // 空 ⇒ 网络全拒
    pub timeout: Duration,
    pub max_document_bytes: usize,
}
```

- `Default`：**无网络** + 默认超时 + 默认大小上限——agent 的内联 `Code` 动效默认就跑在这之下，除非可信模板显式放宽。
- `offline_with_timeout(timeout)`：离线 + 自定义超时（常用测试/CI 旋钮）。
- `allow_origin(origin)`：链式加白名单，忽略不可解析/明文远程输入，自动去重。
- `is_offline()`：白名单为空时 `true`。

### `check_url(url)`（纯）
- `data:` URI **永远放行**（内联、无网络）。
- 否则当且仅当 URL（小写）以某个白名单 origin 为前缀时放行；空白名单 ⇒ 所有远程 URL 拒绝，返回 `MotionError::Sandbox`。

### `check_document_size(document)`（纯）
- 文档字节长度超 `max_document_bytes` 即 `Err(MotionError::Sandbox)`。

---

## 谁在调用

- `StubRenderer` 与 `HeadlessChromiumRenderer` 都在 `render()` 里对 `MotionSource::Code` 调 `check_document_size`——连 stub 与 "renderer unavailable" 路径都不放过（见 [renderer.md](renderer.md)）。
- `check_url` 的执行点是计划中的真实 CDP 后端（`Fetch.enable` + 请求拦截），当前仅纯检查 + 单测。

---

## 移植铁律落地

- **安全建模为类型**：约束是 `SandboxPolicy` 不变量，渲染器不可绕过；"无文件系统访问"用"无授予字段"体现。
- **默认安全 + 确定性**：默认离线既是安全默认，也是测试/CI 可复现的来源。
- **白名单显式、无通配、拒绝明文远程**：每个 origin 显式命名，明文远程一律拒。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
