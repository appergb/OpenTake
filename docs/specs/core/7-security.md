## 7. 安全与并发边界(跨进程新增,必须显式)

| 关注点 | 措施 | 证据 / 依据 |
|---|---|---|
| **MCP 仅本机** | MCP server 绑 `127.0.0.1:19789`,**只 loopback** | `MCPHTTPServer.swift:25-26` `requiredLocalEndpoint = 127.0.0.1`;`MCPService.swift:9` port 19789 |
| **DNS-rebinding 防护** | Origin 校验 + Content-Type + Protocol-Version 三段校验管线(tower layer 复刻) | `MCPHTTPServer.swift:46-50` `StandardValidationPipeline([OriginValidator.localhost, ContentTypeValidator, ProtocolVersionValidator])`;ARCHITECTURE §7「只绑 loopback + Origin 校验」 |
| **MCP 属 agent crate** | 以上全在 `opentake-agent`(Phase 7),**不在 core** | core 不含网络;agent 持 `EditorCore` 句柄调 `apply` |
| **命令串行化** | 所有 `apply/undo/redo` 经 `Mutex<EditorState>` 串行;`version` 因此严格单调 | §2.3、§4.3 |
| **锁内无 IO** | 临界区只做值类型 timeline 操作;解码/导出/生成在锁外 task | §1.3 锁粒度、§5.2 deps 异步 |
| **密钥** | LLM/生成 key 存 OS keychain(`keyring`),不入工程文件 | ARCHITECTURE §8 末、§10;swift/security.md(Keychain) |

> core 自身**无外部攻击面**(不开端口、不收网络输入)。攻击面在 agent crate 的 MCP server,其安全契约见上表,实装在 Phase 7。本 crate 的安全责任 = 命令路径的输入校验(§6.3 精确路径错误)+ 并发一致性(§4.3)。

---
