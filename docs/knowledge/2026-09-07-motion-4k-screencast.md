---
status: canonical
stage: implementation-backed
retrieved_at: 2026-09-07
retrieval_timezone: Asia/Shanghai
freshness: normal
valid_until: 2026-09-17
confidence: high
---

# Chrome 152 screencast 背压与 Motion 4K 提交顺序

项目实测浏览器是 `/Applications/Google Chrome.app`，实际进程版本 **152.0.7977.76**，macOS ARM64 / Apple M4。本文记录对应版本行为，不把 CDP tip-of-tree 文档当作项目固定浏览器版本。

## 官方来源

- [Chrome 152.0.7977.76 的 page_handler.cc](https://chromium.googlesource.com/chromium/src/+/refs/tags/152.0.7977.76/content/browser/devtools/protocol/page_handler.cc)：获取源码并核对 `kMaxScreencastFramesInFlight`、`ShouldCaptureNextScreencastFrame`、`OnFrameFromVideoConsumer`、`ScreencastFrameAck`。
- [CDP Page.startScreencast / screencastFrameAck](https://chromedevtools.github.io/devtools-protocol/tot/Page/#method-startScreencast)：screencast 与按其会话 ID 确认帧的接口。
- [CDP Emulation.setVirtualTimePolicy](https://chromedevtools.github.io/devtools-protocol/tot/Emulation/#method-setVirtualTimePolicy)：虚拟时间策略及预算；预算耗尽事件不能替代已收到目标像素帧的确认。

## 对应版本的关键事实

`page_handler.cc` 在第 100 行把最大 in-flight 常量定义为 2，但发送条件用 `<=`，因此实际可以有 **三个**尚未 ACK 的帧。`OnFrameFromVideoConsumer` 在窗口满时直接返回，不编码/发送该帧。`ScreencastFrameAck` 只在会话匹配时减少计数，没有重发被丢弃静态画面的逻辑。PNG 编码异步执行，启动帧和多个绘制更新可能在客户端开始消费前占满窗口。

项目修复前先 startScreencast，再更新作者 marker、transition 色，最后才开始消费/ACK 帧。4K 复现收到三个旧/seed 帧并逐一正常 ACK，但 transition 帧没有到达，最终命中原有 180 秒 deadline。这个观测与上述官方实现相互印证。不是颜色 guard 的舍入误判，也不是 ACK 命令卡住。

修复给已存在的 seed 增加提交握手：startScreencast + 作者 marker 更新后，先等待符合 seed 外部 guard **和当前作者 marker** 的帧并 ACK，再发布 transition。保留后续 transition、desired、三次作者像素一致性与黑白 alpha 恢复；没有延长 timeout 或加入重试。

## 边界与新鲜度

这是产品对 Chromium 背压处理的顺序缺陷，4K 和宿主时序使其暴露；不能简单归类为“当前 Mac 环境坏了”。握手是协议层顺序保证，真实 Windows/Linux 运行仍按项目平台门槛验收。旧 Windows 4K gate 成功不代表原顺序没有竞争窗口。

来源代码与首次 trace 保存在 `/tmp/opentake-motion-4k-20260906/`，完整复现、修复和测试结果见[专项审计](../audit/2026-09-07/motion-4k-screencast.md)。若浏览器升级，应重新核对发送窗口/ACK 行为及 opaque+transparent 实测，不沿用过期可达性或性能结论。
