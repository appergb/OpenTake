# Safe asset authority 身份绑定证据

> 执行日期：2026-08-08（Asia/Shanghai）
> 范围：`src-tauri/src/safe_asset_protocol.rs`、`src-tauri/src/safe_asset_protocol/tests.rs`

## 结论

非项目内置资源的 authority token 不再把「原始字符串路径相等」当文件身份。请求前
先用 fail-fast retained open 捕获平台最终路径和 ETag/文件身份；isolated helper 返回后，
仍在 project lease 下同时复核：

1. helper 的 final path 与最初 retained final path 按平台路径语义等价；
2. helper ETag 与最初 retained identity 一致；
3. 原始 requested path 此刻仍在 asset scope/当前 project manifest 中；
4. 对原 requested path 的 fresh retained authority 与初始 token 完全一致。

因此稳定 symlink alias 不再被误判为 403，而 ancestor A→B、同 pathname inode 替换、
项目 epoch/manifest 变化仍 fail closed。

ScopeOnly 也不再只凭原始请求路径位于 app cache/data/resource 或精确 Home 缩略图
grant 即放行：retained open 得到的 final path 必须重新通过 lexical scope，并仍属于
同类应用自有根或精确 Home 缩略图授权。这样应用缓存目录内的 ancestor alias 不能
把请求稳定导向 scope 外部文件。

## RED → GREEN

- `stable_external_alias_remains_authorized_for_the_same_opened_file`：旧 raw `PathBuf` 比较
  返回 403；修复后对同一 retained final file 返回 200。
- `external_authority_rejects_same_path_identity_replacement`：旧实现对同路径替换返回 200；
  修复后 ETag/fresh authority 不一致，返回 403。
- 既有同 epoch ancestor swap 回归继续返回 403，未因 alias 兼容而放宽。
- Windows-only `external_authority_accepts_windows_case_equivalent_final_paths` 覆盖大小写
  等价路径；此用例已写入源码并受 `cfg(target_os = "windows")` 保护。
- `response_for_request_rejects_scope_only_alias_that_resolves_outside_scope` 通过真实协议
  请求入口复现：旧代码在 helper semaphore 为 0 时返回 503，证明已越过 authority；
  修复后在尝试获取 helper slot 前直接返回 403。该用例不以 `serve_open_file` 代替
  生产响应链。

## 验证记录

```text
cargo test -p opentake-tauri safe_asset_protocol::tests -- --nocapture
test result: ok. 18 passed; 0 failed

cargo clippy --workspace --all-targets -- -D warnings
exit 0

cargo fmt --all -- --check
exit 0

cargo test --workspace
exit 0
# opentake-tauri unit: 537 passed
```

尝试了 `cargo check -p opentake-tauri --tests --target x86_64-pc-windows-msvc`，但本机没有
Windows MSVC C/C++ sysroot（依赖构建缺 `assert.h`/`lib.exe`），因此 Windows-only 用例
未在本机交叉编译或执行。它必须由 Windows CI/实机门禁补证；这不是通过声明。
