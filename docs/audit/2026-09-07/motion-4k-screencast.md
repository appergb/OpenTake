---
status: canonical
stage: final-verified
updated_at: 2026-09-07
---

# macOS Motion 4K 截帧超时专项

## 范围与验收

用户授权仅修改 `crates/opentake-motion/src/renderer.rs`、必要的 `tests/chromium.rs` 与本专项 audit/knowledge。实际产品修改集中在 renderer，现有 Chromium 集成测试保持原断言。模型文件、Tauri/worker/export、浏览器启动参数、180 秒预算和其他文档均不在写集。

Read Set：当前 AGENTS；Motion OVERVIEW/INDEX/renderer 文档；renderer 与 tests/chromium；主线 `output/public-beta-2026-09-06/rust-final-tests.log`、`chromium-isolated-retry.log`；Chrome 对应版本公开 page_handler.cc。

验收顺序：单 4K 原样复现 → 最小 trace 定位 → 源码解释 → 协议顺序修复 → 单 4K opaque/transparent → 协议回归单测 → 完整 Motion 单测和 Chromium 组 → 定向 fmt/clippy。全程不操作电脑 GUI；浏览器只由授权的 headless 测试进程启动。量测前通知主代理暂缓 Cargo/GUI验收，空档由其独立处理导出主线程问题；等待到 build 锁释放后才进入最终测试。

## 修复前证据

- 主线全量：4K opaque 在 `tests/chromium.rs:432` 报 `Timeout(180s)`，随后三个 Chromium 测试因共享 test gate 被 poison 连带失败。
- 主线独立 trace 重跑：opaque 10688 ms 通过，transparent 在 line456 命中 180 秒预算。最后停在 black transition guard 等待，virtual-time budget 已正常结束。
- 本任务仅加 opt-in trace 的单 4K 复现仍失败，0 passed / 1 failed，180.18 秒。trace 显示期望 transition `[1,0,165]`，实际依次收到白色、seed `[1,0,90]`、seed `[1,0,90]`；三个帧的 ACK 均成功，随后没有新帧到达。不是 PNG 解码或 ACK 本身卡住。
- 实际 Chrome 进程版本 152.0.7977.76；GPU trace 为 Apple M4 / ANGLE Metal，GPU compositing enabled。没有换浏览器或关闭 GPU。

## 根因与最小修复

对应 Chrome 版本的 screencast 窗口实际容纳三个未 ACK 帧；窗口满时新的 compositor 帧直接丢弃，ACK 只释放额度，不重放被丢弃画面。官方来源、版本和有效期统一记录在[专项知识记录](../../knowledge/2026-09-07-motion-4k-screencast.md)。

原协议在开始消费之前连续发布 seed、作者 marker、transition。启动旧画面和 seed 更新可能先占满发送窗口，使唯一的 transition 绘制被丢弃。之后页面静止，即使逐一 ACK，等待目标 guard 的循环也不会再得到对应像素。

修复在 `capture_isolated_viewport` 中增加 **seed + 作者 marker 的一次提交握手**。真正收到且 ACK seed 代际后，才发布 transition。原有 transition→desired 检查、三次作者 marker 采样/一致性、黑白透明度重建、CSP/Fetch、取消与清理路径保持。没有增加重试、加长 timeout 或放宽像素检查。

trace 仅在 `OPENTAKE_MOTION_TRACE` 开启时输出等待/ACK/guard/marker 状态；角点来自作者画布外的宿主 guard，不输出作者画面像素或 nonce。

## 回归覆盖

- 模拟 Chrome 三个未 ACK 启动帧的满窗口，严格要求全部 ACK 后才能发布 transition，并继续拒绝旧 guard。
- 透明协议 mock 增加 seed 阶段；seed 颜色正确而作者 marker 过期时仍必须拒绝。原有两种背景、每种三个作者代际、错误 marker/旧 session 检查和逐像素 alpha 重建断言保留。
- pre-start / post-start 错误仍测试 detach、stop 以及原始错误优先级；CSP late-request、帧 ACK、作者隔离、稳定读回、取消和缓存测试保留。

## 执行记录

```sh
OPENTAKE_MOTION_TRACE=1 cargo test -p opentake-motion --features chromium \
  --test chromium --jobs 2 four_k_single_frame_opaque_and_transparent_budget_smoke \
  -- --exact --nocapture --test-threads=1
```

修复前：0 passed / 1 failed，180.18 秒。修复后：**1 passed / 0 failed**，总计108.02秒；opaque **39567 ms**，transparent **64435 ms**，每段均保持原180秒预算，透明段全像素非平凡 alpha 断言通过。这是共享宿主下的 debug/trace 运行记录，不是稳定性能基准。

```sh
cargo test -p opentake-motion --features chromium --lib guarded_candidate \
  --jobs 2 -- --nocapture --test-threads=1
```

三项 guard/清理协议单测通过。

初次全模块运行有96项通过、1项透明协议 mock 失败：mock 尚按旧顺序期待 transition，而产品已先执行 seed 握手。仅补齐该 mock 的 seed/错误作者 marker 事件，不改其 alpha/CSP/三代际验算标准。

最终关闭 `OPENTAKE_MOTION_TRACE`，避免把日志输出改变时序误当成修复：

```sh
cargo test -p opentake-motion --features chromium --lib --test chromium \
  --jobs 2 -- --nocapture --test-threads=1
cargo clippy -p opentake-motion --features chromium --lib --test chromium \
  --jobs 2 -- -D warnings
rustfmt --check --edition 2021 crates/opentake-motion/src/renderer.rs \
  crates/opentake-motion/tests/chromium.rs
```

结果：

| 验证 | 实际结果 |
|---|---|
| Motion 完整单测 | **97 passed、0 failed**，5.72秒 |
| Chromium 完整组 | **7 passed、0 failed、0 ignored**，130.72秒 |
| 全组中的 4K opaque | **18870 ms**，原全像素 opaque 断言通过 |
| 全组中的 4K transparent | **30293 ms**，原全像素非平凡 alpha 断言通过 |
| 定向 clippy `-D warnings` | 通过；仅有已有依赖 block 0.1.6 的 future-incompatibility 提示 |
| 格式 / 最终 diff 空白检查 | 通过 |

完整 Chromium 组包含浏览器池失效/复用、并发错误、host/CSP/guard、预览确定性、网络/虚拟时间/超时清理/帧身份。没有只凭 opaque 通过收尾，也没有 poison 后跳过余下测试。

结论：本机可重复的 4K 截帧阻塞已在原浏览器上收敛。属于产品对 Chromium 发送背压的顺序缺陷，宿主/4K 时序是触发条件，不应简单归为主机环境故障。代价是每个隔离读回增加一次 seed 提交握手；原有每次 render 的时间预算不变。以上是 debug 构建的实测记录，不承诺稳定吞吐。

实际写集仅 `renderer.rs` 与两份专项文档；`tests/chromium.rs` 没有修改。2026-09-07 本轮定向检查结束后已通知主代理恢复 Cargo/GUI，产品代码冻结。最终发布仍应以主线整合提交及其跨平台 CI 为准；修复后的 Windows/Linux 本轮未原生重跑，不沿用修复前 Windows 4K gate 当作新提交的通过证据。

本地日志：`/tmp/opentake-motion-4k-20260906/{single-4k-red.log,single-4k-seed.log,guard-unit.log,motion-full.log,motion-full-final.log,clippy.log}`；对应版本 Chromium 源码为同目录 `page_handler.cc`。未写主线 output 目录，未提交/push/tag/release。
