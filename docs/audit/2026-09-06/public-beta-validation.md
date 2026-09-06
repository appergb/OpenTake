---
status: draft
content_stage: partial-implementation
updated: 2026-09-06
---

# Public Beta 候选验证记录

本记录只报告本轮实际执行结果。候选基线为 `33ee8e2` 加本任务改动，分支 `release/v1.0.0-beta.6`，最终 commit 尚未产生。

## 环境

macOS ARM64；Node 26.7.0、pnpm 10.33.4。默认 `/usr/bin/python3` 缺少 `tomllib`，脚本改用 Codex bundled Python。初始 `web/node_modules` 缺失，已按锁文件安装。Rust工具链与构建日志保存在 `output/public-beta-2026-09-06/`。

## 当前结果

| Gate | 本轮结果 |
|---|---|
| Rust fmt | 通过 |
| Rust workspace clippy | 修复 constant chunks/manual contains 后通过，exit 0；第三方 `block 0.1.6` 有 future-incompat notice |
| Rust workspace tests | 本轮首个完整结果：80组、2885通过、0失败、10个显式ignored；之后的搜索恢复及有界排队修复18项定向测试通过，最终汇总待更新 |
| Rust minimal clippy | 无默认feature构建检查通过 |
| Web tests/build | 最终整合后153files/1462tests通过，生产构建通过；保留既有chunk/dynamic-import warnings |
| 发布/Windows workflow结构 | Beta 6 identity 更新后通过；只修改version/WiX/notes入口及对应现有合同摘要，历史Beta4恢复链不变 |
| 脚本单测 | Beta6更新后209/209通过 |
| sidecar provisioner 单测 | 7/7 通过 |
| C1B/打包sidecar | CI/evidence Ruby合同及macOS内置FFmpeg实际执行通过 |
| Motion Canvas | 2/2 测试、123项许可证记录、构建通过；runner.html与锁定输出一致 |
| Motion dependency audit | xmldom升级后0项漏洞 |
| Web dependency audit | nanoid升级后0项已知漏洞 |
| 真实媒体集成 | FFmpeg导出、Motion命令/合成、播放传输全通过；严格真实音频探针 frames=83、playhead=90、clock=audio、非黑比例1.000、neon-green=0.000 |
| 真实Keychain/MCP | 2项通过：鉴权、Host/Origin、重启/撤销、端口冲突、禁用/退出清理；敏感token未进入日志 |
| 新安装包GUI | macOS release app构建中，原生GUI待执行 |
| 宣传视频 | 独立GPT-6交付60s/1080p/30fps/1800帧H.264+AAC，主代理ffprobe、封面/联系表及全片解码通过；当前为注明日期的历史实拍剪辑，不算新包GUI证据 |
| 远端CI/Release | 未执行、未发布 |

## 缺口与证据边界

- 语义搜索manifest/模型链路已完成并通过实际Rust推理；模型v2索引过滤、同尺寸损坏修复路径和真实下载大小正在独立复核后的收尾。Windows ort-tract真实运行发现线程配置未实现及动态shape推理错误，正在修复，尚未达发布条件。
- 既有安装包可读取AX但窗口截图全黑；系统`IOConsoleLocked=No`，Raise及窗口zoom后仍黑。尚不能区分应用显示与窗口采集问题，已向用户异步询问可见状态；不能把AX按钮存在当成视觉通过。
- 历史 GUI 记录不自动继承为当前候选的稳定性证明。
- 用户已有 audit 工程、截图删除和视频素材改动均保留；本轮使用独立输出目录。
- [执行计划](../../plans/active/2026-09-06-public-beta.md)与[候选说明](../../releases/1.0.0-beta.6.md)持续写回。
