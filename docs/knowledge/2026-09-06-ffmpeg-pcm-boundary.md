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

最终契约：显式区间在重采样后按区间目标帧数裁剪，保留一帧舍入余量；`range=None` 保留原FFmpeg完整解码参数，不按容器时长隐式裁剪。无区间的stdout预算允许在探测时长之外最多一秒输出采样作为编解码器padding余量（16kHz/mono/f32额外64,000bytes（62.5KiB）），该余量通过checked乘加计算，超限仍拒绝并回收进程。不会静默截断过大的完整音轨。采样率/声道非零验证归一到所有时长预算调用都会经过的入口。

回归使用真实 FFmpeg，把两秒 48kHz WAV 转为 16kHz 且限于一秒预算：修复前 128,000 bytes，期望 64,000；修复后准确 64,000。13 项 PCM 单测与一项 facade 集成通过。同一轮 Windows full-product gate 也在相同 AAC 夹具复现相同字节数超限，证据为 job 101508803694。Linux/Windows 的原始失败场景须由新候选远端 CI 复验，不能用本地成功代替。

2026-09-07第二轮CI校正：`64dce59`的统一裁剪虽然修复facade，却被既有`extract_pcm_without_explicit_range_matches_full_track_decode`用例拒绝：Linux和Windows都应保留16,384采样，而统一裁剪只有16,000。该测试保留原样；代码按上述完整轨契约修正。1秒是明确的缓冲容差策略，不是对所有编解码器最大padding的测量或保证。当前本地media facade与FFmpeg集成15项通过/1项原有ignored；新候选远端复验待执行。
