# Proxy 真实回归证据

> 执行日期：2026-08-08（Asia/Shanghai）
> 范围：`crates/opentake-media/src/proxy.rs`

## 结论

`cargo test -p opentake-media proxy::tests -- --test-threads=1` 已从审计基线的
「0 tests」变为 11 个真实 FFmpeg/FFprobe 回归，全部通过。用例使用 lavfi 现场
生成视频/音频 fixture，不以 mock 代替转码与探测。

最终实现满足以下安全和生命周期不变量：

- 源文件只做一次 fail-fast retained open；Unix 使用 `O_NONBLOCK|O_NOFOLLOW`，
  Windows 使用 no-recall/reparse-point flags，并拒绝非普通文件。
- 前后 SHA-256、FFmpeg stdin 与源身份复核使用同一 retained capability；路径名
  A→B→A、unlink 或 Cloud/FIFO 入口均不能改变已授权输入或无限阻塞 lease。
- FFmpeg 从一开始只写目标同目录下权限收紧的私有 `TempDir` stage；环境可见的
  `<output>.partial` 从未创建，因此不存在「FFmpeg 退出后、retained open 前」
  的公开重绑定窗口，也没有可能删除攻击者 rebound 文件的 ambient cleanup。
- stage 的 retained identity、真实 FFprobe 结果和取消点均复核后，使用
  `TempPath::persist_noclobber` 原子 no-replace 发布；竞争目标不被覆盖。
- 输出真实 probe 断言恰好 1 条视频流、至多 1 条音频流，防止重复 `-map` 回归。

## RED 证据

### 基线没有真实 proxy unit 回归

```text
cargo test -p opentake-media proxy::tests -- --nocapture
running 0 tests
test result: ok. 0 passed; 415 filtered out
```

### Retained source 与取消/发布 race

旧实现稳定复现过以下失败：

```text
pathname_replacement_a_to_b_to_a_cannot_change_the_retained_source ... FAILED
left: (180, 320), right: (320, 180)

cancellation_at_verification_probe_and_prepublication_cleans_partial ... FAILED
phase 910 did not return Cancelled

destination_appearing_after_final_check_is_preserved_and_partial_is_cleaned ... FAILED
assertion failed: matches!(result, Err(MediaError::Ffmpeg(_)))
```

它们分别证明旧代码会在多个 pathname reopen 间接受 ABA 输入、后处理取消仍可发布，
以及 `exists` 后 `rename` 会覆盖竞争写者。

### 公开 partial 生命周期

早期补丁虽在 probe 后保留 capability，FFmpeg 仍先写公开 sibling partial；复审发现
攻击者可在 FFmpeg 退出到 retained open 之间置换有效文件。新增用例在 progress=900
观察旧实现，稳定发现公开 partial：

```text
ffmpeg_output_is_never_exposed_at_the_ambient_partial_path ... FAILED
assertion failed: !ambient_partial.exists()
```

结构性修复后，FFmpeg 从创建时即写私有 stage。progress=990 对环境 sibling 写入普通
文件或 FIFO 只是在测试外部 namespace：输出仍来自已验证 stage，发布失败也不会打开、
跟随或删除该 sibling。

### 初始 source open 阻塞

新增 Unix 回归把 source 设为无 writer 的 FIFO，并同时覆盖 symlink。旧 `File::open`
可在进入 deadline 前阻塞；修复后的 retained opener 在测试的 5 秒上限内 fail closed。

## GREEN 用例（11/11）

| 用例 | 断言 |
|---|---|
| `successful_proxy_is_bounded_probed_and_source_preserving` | 真实 640×360 H.264/AAC 转 320×180；恰好 1 video、至多 1 audio；源字节/SHA 不变 |
| `cancellation_during_transcode_kills_child_and_cleans_partial` | 返回 `Cancelled`、回收子进程、无输出或公开 partial |
| `changed_source_fails_identity_check_and_cleans_partial` | retained inode 就地改写返回 `Checksum`，不发布 |
| `pathname_replacement_a_to_b_to_a_cannot_change_the_retained_source` | 路径 ABA 后仍只能输出原 A 的 320×180 |
| `cancellation_at_verification_probe_and_prepublication_cleans_partial` | 910/950/990 三个后处理阶段均取消且不发布 |
| `destination_appearing_after_final_check_is_preserved_and_partial_is_cleaned` | 目标竞争者保持原字节，no-clobber 返回错误 |
| `verified_partial_rebound_at_prepublication_cannot_change_output_or_delete_replacement` | ambient sibling 重绑不能改变输出，也不被清理 |
| `ffmpeg_output_is_never_exposed_at_the_ambient_partial_path` | progress=900 时环境 `<output>.partial` 不存在 |
| `rebound_fifo_does_not_block_or_get_deleted_on_publication_error` | ambient FIFO 不阻塞、不删除；竞争输出保留 |
| `initial_source_fifo_and_symlink_are_rejected_without_blocking` | 初始 FIFO/symlink fail-fast 拒绝 |
| `unlinked_source_namespace_does_not_break_the_retained_source` | POSIX unlink 后 retained source 仍可完成校验和发布 |

## 验证记录

本机工具：FFmpeg/FFprobe 8.1.2（`/opt/homebrew/bin`）。

```text
cargo test -p opentake-media proxy::tests -- --nocapture --test-threads=1
running 11 tests
test result: ok. 11 passed; 0 failed; 0 ignored

cargo test -p opentake-media --test proxy -- --nocapture
running 1 test
test result: ok. 1 passed; 0 failed

cargo clippy --workspace --all-targets -- -D warnings
exit 0

cargo fmt --all -- --check
exit 0

cargo test --workspace
exit 0
# opentake-media unit: 434 passed / 1 ignored
# opentake-tauri unit: 536 passed
# 其余 workspace integration/doc-test binaries 全部通过
```

## 限制

- 回归需要可用的 FFmpeg/FFprobe；正式 CI/打包仍须供应锁定 sidecar。
- macOS 本机验证了当前实现；Windows no-recall opener 与 `persist_noclobber` 分支需要
  Windows CI/实机构建补充平台证据。
