# export — 时间线视频导出

> 状态：draft · 阶段：implementation-backed · 同步日期：2026-09-06
> [模块总览](OVERVIEW.md) · [模块目录](INDEX.md) · [源码 export.rs](../../../src-tauri/src/export.rs)

`export_video` / `run_export` 将时间线采样成 RenderPlan，通过合成器生成 RGBA，再由 media 编码层写入目标容器。格式和扩展名在 `resolve_preset` 等入口校验。

| 路径 | 当前源码行为 |
|---|---|
| H.264 / H.265 | `.mp4`，带时间线音频混音 |
| ProRes 422 | `.mov`，不承载透明输出 |
| ProRes 4444 | `.mov`，承载 alpha；输入直 alpha 与合成预乘边界显式处理 |
| 文本与 Lottie | 经渲染/源解析器进入帧合成，不再是早期跳过 Lottie 的切片 |
| 进度与取消 | `ExportControl` 绑定导出代次；`cancel_export` 请求取消，逐帧与收尾继续校验 |
| 失败与输出身份 | 失败/取消清理不完整结果，拒绝不安全或冲突的输出身份；不会静默交付失败的无音轨输出 |

`ExportRequest`、`ExportCodec`、`ExportQuality` 与前端[导出面板](../../../web/src/components/shell/ExportDialog.tsx)共同定义选项。参数字段以类型定义为准，文档不复制易过时的完整 DTO。

导出尺寸采用目标导出配置，区别于低分辨率预览。时间重映射、音频窗口、透明输出和缺失素材均有专门代码路径；这些路径的存在不代表所有组合和平台已经实机验收。

`aae0ae6..33ee8e2` 中的候选新增包括 ProRes 4444、失败清理、取消传播和输出身份检查。[本次同步报告](../../documentation-sync-2026-09-06.md)记录源码依据，[当前验证](../../audit/2026-09-06/public-beta-validation.md)记录实际运行结果。早期 H.264-only、无进度/取消的描述已被当前实现替代。

关联：[媒体编码](../opentake-media/encode.md) · [RenderPlan](../opentake-render/render-plan.md) · [单帧渲染](render.md)。

## 2026-09-07 调度修复

`export_video` 通过async命令调用专用blocking task。调用时先获取单导出lease和项目snapshot，worker持有共享ExportControl及owned ExportGuard，直到真正结束；取消、进度事件和WebView主循环可以并行响应。GUI首包已暴露原同步命令占住主线程的问题，本次调度改动的原生进度/取消验收见[当前验证记录](../../audit/2026-09-06/public-beta-validation.md)。
