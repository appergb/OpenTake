# P0 Asset / P2 Scope and Explicit Import Remediation Evidence

日期：2026-08-08（Asia/Shanghai）
基线：`f9398a9302e57690caa3ea21ff995fdd0ce97706`
说明：本文记录修复阶段的新鲜诊断与回归；不覆盖基线审计报告和原始失败截图。

## 1. P0 `opentake-asset` 504 根因与修复

### RED / 诊断

- fresh custom-protocol release 诊断包中，实际缓存缩略图请求返回 `504 Gateway Timeout`；源视频和 Home 封面同样失败。
- helper 已收到子进程，但阻塞在 `stdin.read_to_end()`；父进程同时等待 helper stdout，5 s I/O deadline 后返回 504。
- Unix/macOS 上 Tokio `ChildStdin::poll_shutdown` 不会关闭管道写端，原实现的 `AsyncWriteExt::shutdown()` 因此没有向 helper 送达 EOF。
- 新回归 `helper_request_pipe_reaches_eof_before_waiting_for_the_response` 在旧路径上 500 ms 超时，Cargo exit 101。

### GREEN / 最小修复

- 写入 JSON request 后显式 `drop(stdin)`，再等待 helper response；未改变 helper token/PID/同可执行文件校验、进程并发与 quarantine 上限、I/O deadline、MIME/body/range 边界。
- 无诊断日志源码上 `safe_asset_protocol::tests` 为 **15 passed / 0 failed**（包含 EOF 回归与下述 active-project scope 竞态回归）。
- 诊断 release/custom-protocol 包 fresh 复测：Home 封面 `200`，工程 thumb cache `200`，源视频 Range `206`，preview cache `200`，timeline sprite `200`；视觉上封面、卡片和选中素材预览恢复。
- 此次只是修复诊断包，不冒充完整正式安装包验收；完整变更合并后仍须重建、安装和 fresh GUI 重测。

## 2. P2 active-project media authority

### RED

- Tauri persisted scope 保留历史 dialog/file/directory grants，旧项目 A 的外部媒体在打开 B 后仍能通过原始 scope 判定。
- 新回归首先因 active-project authority 函数不存在而编译失败（Cargo exit 101）。
- 首轮修复仅用 `project_epoch` 作为 `ProjectMedia` token；同一工程同时引用 A/B 两路径时，祖先 symlink 从 A 重绑到 B 后，新回归稳定 RED：`expected == rebound == ProjectMedia { project_epoch: 0 }`。

### GREEN / fail-closed 边界

- 不强行删改 persisted scope；协议在 helper 启动前和 retained final path 发布前各做一次二级 authority gate。
- 静态 `$APPCACHE/**/*`、`$APPDATA/OpenTake/Library/**/*`、`$RESOURCE/**/*` 和 Home exact `thumbnail.jpg` 保持可用，但其他外部媒体必须同时存在于当前 `AppCore` manifest。
- `ScopeOnly` / `ProjectMedia` authority 都携带 normalized exact requested path，`ProjectMedia` 另携带 `project_epoch`；发布字节前持有 project identity workflow lease 并重比较 final path/token，避免项目切换或同 epoch 路径重绑与发布之间的 TOCTOU。
- 回归覆盖：即使 persisted scope 是递归目录 grant，当前工程未引用 sibling 仍拒绝；A→B 后 A 拒绝、B 允许；同 epoch 祖先 A→B 重绑的 helper response 返回 `403`；app cache 和 Home exact thumbnail 仍允许。

## 3. P2 explicit import bounds/deadline/identity

### RED

- `import_media` 接受任意长的路径列表，且使用无硬期限的 path-based `ffprobe`。
- 新限额回归首先因 `ExplicitImportLimits` / admission 函数不存在而编译失败（Cargo exit 101）。
- Rust review 又确认首轮实现仍在 deadline 之前调用可阻塞的 ambient `File::open`，且总字节数来自路径 metadata，计划与 commit 未绑定同一文件身份。

### GREEN

- 明确文件导入在任何 manifest mutation 前限制为 5,000 个路径、100 GiB 总字节，溢出也 fail closed。总字节数改由 retained handle metadata 累加，不再依赖可被替换的前置 pathname metadata。
- 打开边界复用 asset 协议的 no-follow/no-recall 策略：Unix 使用 `O_NONBLOCK | O_NOFOLLOW`，Windows 使用 `FILE_FLAG_OPEN_NO_RECALL`；FIFO 回归证明不会等待 writer。
- 同一 retained handle 同时提供总字节 metadata 和 cancellable ffprobe（15 s deadline）；manifest 记录 retained final path。commit 临界区内重新以 no-recall 方式打开用户选择路径，并通过 `same_file` 核对身份、计划时长度与重算 aggregate cap；路径替换或同 inode 原地增长都在 manifest/history/persist 发生前原子拒绝。
- reviewer 复修后又为同 inode 原地增长补测：只比 identity 的中间实现稳定 RED，错误提交了增长后的文件；加入 admitted length/aggregate 重验后转 GREEN。
- focused 回归 **5 passed / 0 failed**：限额拒绝且内存/磁盘 manifest 不变、路径替换原子拒绝、原地增长原子拒绝、FIFO 快速拒绝、正常持久化导入。
- 此修复解决确认的输入上限和 probe deadline；用户可见的进度面板/主动取消按钮仍是独立产品能力，本文不将其伪装为已完成。

## 4. 共享工作树合并门禁

```text
cargo test --workspace
exit 0

cargo clippy --workspace --all-targets -- -D warnings
exit 0

cargo fmt --all -- --check
exit 0

git diff --check
exit 0
```

上述是 asset/import/proxy 修复合并后的共享工作树结果，不是某个子 Agent 的隔离分支结果。此时尚未创建正式安装包；仍需 Rust reviewer 复审通过后才能进入 release/fresh GUI 验收。
