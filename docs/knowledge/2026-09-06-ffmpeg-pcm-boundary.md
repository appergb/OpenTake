---
status: canonical
content_stage: implementation-backed
retrieved: 2026-09-06
freshness_class: normal
valid_until: 2026-09-16
confidence: high
---

# FFmpeg PCM 尾部补齐与采样边界

GitHub CI 34041435244 的 Ubuntu FFmpeg `7:6.1.1-3ubuntu5` 在一秒、16kHz AAC 夹具上输出 16,384 个 f32 采样（65,536 bytes），超过按容器时长推得的一秒预算及单帧余量 64,004 bytes，触发媒体 facade 集成失败。macOS 当前 FFmpeg 9.0.1 没有在该夹具复现相同多余尾部。远端格式与 clippy 步骤通过，失败发生在实际媒体测试。

[FFmpeg 官方 atrim 文档](https://ffmpeg.org/ffmpeg-filters.html#atrim)说明 `end_sample` 是第一个应丢弃的采样编号，按实际采样计数而不是时间戳。应用需先转换到请求采样率，再以和 stdout 内存预算相同的目标帧数裁剪，避免对 48kHz 源直接按 16kHz 数量裁剪而截短内容。

修复沿用现有 PCM 解码参数，追加 `aresample=<目标采样率>,atrim=end_sample=<预算帧数>`。预算仍由既有请求区间或探测时长计算；stdout 上限、溢出拒绝、非零退出处理、取消与进程回收逻辑不变。采样率/声道非零验证归一到所有时长预算调用都会经过的入口。

回归使用真实 FFmpeg，把两秒 48kHz WAV 转为 16kHz 且限于一秒预算：修复前 128,000 bytes，期望 64,000；修复后准确 64,000。13 项 PCM 单测与一项 facade 集成通过。同一轮 Windows full-product gate 也在相同 AAC 夹具复现相同字节数超限，证据为 job 101508803694。Linux/Windows 的原始失败场景须由新候选远端 CI 复验，不能用本地成功代替。
