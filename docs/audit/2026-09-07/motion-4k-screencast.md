---
status: canonical
stage: implementation-backed
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

## 第三轮 CI 后的残余问题（候选 aa40672 / run 34047983006）

上一轮仅证明 macOS 本机通过，不能视为跨平台收敛。主线保留的 `output/public-beta-2026-09-06/ci-third-failures.log` 提供了两个新的确定触发点：

- Windows 第一个 opaque capture 在 **seed 阶段**收到三张旧白色帧，外部 guard 与作者 marker 均为 false，ACK 全正常；此后 seed 永不到达，180秒超时。不是后续 capture 的旧 session 泄漏才能触发，第一轮启动已足以复现。
- Linux 的透明协议 mock 在 capture-4 提前清理。mock 构造的是 `Instant::now() + 1s`，但 `SandboxPolicy::default()` 报错标签为60秒；`check_abort_state` 按传入 deadline 判断、按传入 timeout 展示，因此 `Timeout(60s)` 实际来自1秒测试 deadline，并非真实运行60秒。每次分开发送的响应/帧出现约41ms延迟，累计到第五次 capture 跨过1秒。

### 本轮最小改动

实际源码写集仍只有 `renderer.rs`，`tests/chromium.rs` 与全部实际像素/alpha/CSP验收保持原样。

1. capture 启动时不再先改 seed。先 startScreencast，消费/ACK 第一张启动 PNG，并 ACK 已经排队的旧启动帧；这些只用于释放传输窗口，绝不作为输出图像。
2. 然后单独发布 seed、等待外部 guard；再单独更新作者 marker，等待同一 seed 与新 marker 同时成立；最后沿用 transition→desired 与三代际/双背景像素校验。每次通过一个 fence 后 ACK 已排队的同代或旧代帧，再发布下一次修改。
3. 删除 `capture_stable_background` 在流启动前的冗余颜色写入，避免它额外占用未确认的启动窗口。最终背景仍由各 capture 的 desired 阶段设定并验证。
4. 不重写 attach/detach 或 start/stop 总生命周期。第三轮首个 capture 已失败，现有证据不足以把反复创建 PageHandler 本身定为根因；独立 session 和晚到事件隔离保持。
5. Linux mock 的 TCP 两端启用 `TCP_NODELAY`。生产 `tungstenite 0.29.0::connect_with_config` 在 `client.rs:68` 本就启用 NoDelay，raw-socket mock 原先没有。测试仍用原1秒 deadline，仅把 policy 标签也改为1秒，没有增加任何测试/产品预算。

新回归把启动窗口设为“三张全旧”，严格要求在第一个 seed 修改前全部 ACK。修复前0 passed/1 failed，错误为原代码先发送 Runtime.evaluate、mock期待Page.startScreencast；修复后该测试与两项原始错误/清理测试全部通过。透明 mock 继续覆盖旧 session、错误 guard、错误作者 marker、三代际与全像素 alpha；没有删除断言。

### 本轮本机结果

| 检查 | 结果 |
|---|---|
| Linux同构 raw-socket mock 修正 NoDelay 后 | 1 passed / 0 failed，0.03秒；原1秒 deadline不变 |
| 全部 Motion 单测 | 97 passed / 0 failed，2.99秒 |
| trace开启的单4K | 1 passed / 0 failed，20.16秒；opaque7521ms、transparent11892ms |
| trace关闭的完整 Chromium 组 | 7 passed / 0 failed，51.62秒；opaque7308ms、transparent12093ms |
| 定向 clippy `-D warnings`、rustfmt、diff检查 | 通过 |

日志位于 `/tmp/opentake-motion-ci-round4/`：`linux-mock-nodelay.log`、`startup-red.log`、`startup-green.log`、`unit-final.log`、`real-4k.log`、`chromium-final.log`、`clippy.log`。两项平台原因与生产传输配置来源补充在同名 knowledge。

**当前状态是本机候选已冻结，仍待真实 Windows/Linux 对这份新补丁的资格结果。** 已通知主线提交/push，未自行 push，未沿用 aa40672 的失败 run 或上一轮本机成功来宣布跨平台通过。

### 独立临时资格 workflow 提案

可审查 YAML 已准备在 `/tmp/opentake-motion-ci-round4/motion-readback-qualification.yml`，尚未写入 `.github/workflows/`，由主线决定纳入新候选。YAML 已解析检查，使用现有仓库已固定的 checkout/cache/upload-artifact action revision。

提案仅新增 `.github/workflows/motion-readback-qualification.yml`：Windows-2022 与 Ubuntu-24.04 两个 job，精确40位提交SHA校验、只读 contents 权限、jobs=2、trace日志归档。运行完整 `cargo test --locked -p opentake-motion --features chromium --lib --test chromium --jobs 2 -- --nocapture --test-threads=1`，要求确实出现97项单测通过、7项Chromium通过且opaque/transparent两段计时存在。没有额外重试、没有改180秒测试预算，不替换浏览器，不修改已有ci.yml/release.yml合同。

触发方式为 release 分支相关路径 push（可对新增workflow首次执行），或手动dispatch指定不可变SHA。主线提交并触发后，须读取两个真实job日志确认结果；如仍失败，以新trace继续诊断，不把本机绿灯当成最终结果。
