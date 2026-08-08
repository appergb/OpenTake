# ProjectMedia 外部素材 scope 加固

- 日期：2026-08-08（Asia/Shanghai）
- 仓库 / 分支：`OpenTake-generation` / `audit/remediation-2026-08-05`
- 范围：项目打开后的外部素材授权、safe asset retained-file 响应链、祖先 symlink 与 alias rebinding
- 结论：**PASS**

## 根因与威胁模型

`.opentake` manifest 中的 `MediaSource::External.absolute_path` 是项目数据，不是文件系统授权。旧链路存在两处组合风险：

1. 项目打开后，`commands.rs` 调用 `media.rs`，把 manifest 引用的所有 materialized 路径直接加入 Tauri asset scope。恶意或被篡改的项目可借此铸造未经过原生文件对话框批准的 exact grant。
2. `safe_asset_protocol.rs` 的 `ProjectMedia` 分支只检查 requested lexical path 是否在 scope 且仍被当前 manifest 引用；retained open 得到的 final path 没有重新落回 scope。已批准目录内的祖先 symlink 因而可解析到目录外文件。

此外，尝试在 project open 时“仅恢复已有 exact alias”仍不安全：Tauri `allow_file(alias)` 会把当时的 canonical target 一并加入 scope。若 alias 在重启前已从 A 改绑 B，任何重新发出 `allow_file(alias)` 的逻辑都会把 B 变成新授权，形成恢复期 TOCTOU。

## RED 证据

1. 真实 `response_for_request`、真实 `AppCore` manifest、真实 Tauri scope：批准目录内 alias 指向目录外媒体；以零 helper permit 验证授权是否在 helper 前拒绝。
   - 命令：`cargo test -p opentake-tauri response_for_request_rejects_project_media_ancestor_symlink_escape -- --nocapture`
   - 结果：**FAIL（预期 RED）**；实际 HTTP `503`，期望 `403`。证明请求已越过授权边界进入 helper 容量分支。
2. project-open scope helper 对 manifest 中未批准的 materialized path 调用 `allow_file`。
   - 结果：**FAIL（预期 RED）**；未批准路径在 helper 调用后变为 allowed。
3. exact alias 先批准 A，再改绑 B，然后执行 scope restore。
   - 结果：**FAIL（预期 RED）**；restore 后 B 的 retained final path 变为 allowed。

## 最小 fail-closed 修复

- `src-tauri/src/commands.rs`
  - 两条 feature 配置下的 `project_open` 均不再在 commit 后依据 manifest 修改 asset scope。
- `src-tauri/src/media.rs`
  - 删除 `grant_project_media_asset_scope` / `grant_media_entries_asset_scope`。原生 dialog 与 persisted-scope 是 external path grant 的唯一来源。
- `src-tauri/src/safe_asset_protocol.rs`
  - `ProjectMedia` 在 retained open 后必须再次以 `initial_final_path` 命中当前 scope，否则在 helper spawn 前返回 403。
  - requested path、initial final path、ETag/file identity、project epoch 与最终 project identity lease 的既有绑定保持不变。
- `src-tauri/src/safe_asset_protocol/tests.rs`
  - 新增真实响应链的祖先 symlink escape 与请求前 exact alias A→B 回归。
  - stable exact alias 仍返回 200；请求中的 A→B、同路径 inode 替换、跨 project epoch 均继续拒绝。

## 验证证据

| 命令 | Exit | 结果 |
|---|---:|---|
| `cargo test -p opentake-tauri --lib project_media -- --nocapture` | 0 | 11 passed，0 failed |
| `cargo test -p opentake-tauri --lib safe_asset_protocol::tests:: -- --nocapture --test-threads=1` | 0 | 20 passed，0 failed |
| `cargo test -p opentake-tauri --lib -- --test-threads=1` | 0 | 538 passed，0 failed，0 ignored |
| `cargo fmt --all -- --check` | 0 | PASS |
| `cargo clippy -p opentake-tauri --lib -- -D warnings` | 0 | PASS；仅依赖 `block 0.1.6` future-incompat 提示，不是 clippy warning |

Windows-only 分支不会在本机 macOS 运行。

## 安全不变量

1. manifest 只能缩小到当前项目引用集合，不能扩大 Tauri scope。
2. external requested lexical path 与 retained final path 必须同时得到 scope 授权。
3. exact grant 的稳定 alias 可继续使用，因为原始 dialog grant 已包含其 canonical target；项目打开不得重新解析并扩展该 grant。
4. 项目切换 lease、epoch、initial final path、ETag/identity 任一不匹配都必须在发布 bytes 前 fail closed。
5. 项目 bundle 内媒体仍由 retained `ProjectAssetAuthority` 管理，不依赖 external scope grant。

## 限制

- 本机为 macOS；Windows case-equivalence/reparse 分支由 cfg 测试与 Windows 原生 CI 覆盖，本报告不声明 Windows 实机执行。
- 用户若清除 persisted scope，旧项目的 external media 会显示离线并要求重新选择；这是预期 fail-closed 行为，不再由 manifest 静默恢复权限。
