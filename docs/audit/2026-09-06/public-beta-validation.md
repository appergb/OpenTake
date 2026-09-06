---
status: draft
content_stage: partial-implementation
updated: 2026-09-06
---

# Public Beta 候选验证记录

本记录只报告本轮实际执行结果。候选基线为 `33ee8e2` 加本任务改动，分支 `release/v1.0.0-beta.6`，本轮首个候选快照为 `3c68cf7`，GitHub 候选 [PR #249](https://github.com/appergb/OpenTake/pull/249)，后续修复与最终发布 SHA 按执行记录更新。

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
| 远端CI/Release | PR #249 已建立，CI 34041435244 执行中；Web/Motion及三平台安全文件系统已通过；尚未公开发布 |

## 缺口与证据边界

- 语义搜索manifest/模型链路已完成并通过实际Rust推理；模型v2索引过滤、同尺寸损坏修复路径和真实下载大小正在独立复核后的收尾。Windows ort-tract真实运行发现线程配置未实现及动态shape推理错误，正在修复，尚未达发布条件。
- 既有安装包可读取AX但窗口截图全黑；系统`IOConsoleLocked=No`，Raise及窗口zoom后仍黑。尚不能区分应用显示与窗口采集问题，已向用户异步询问可见状态；不能把AX按钮存在当成视觉通过。
- 历史 GUI 记录不自动继承为当前候选的稳定性证明。
- 用户已有 audit 工程、截图删除和视频素材改动均保留；本轮使用独立输出目录。
- [执行计划](../../plans/active/2026-09-06-public-beta.md)与[候选说明](../../releases/1.0.0-beta.6.md)持续写回。

## 独立复核及新增资格工具

只读审查确认贴纸/模型恢复/有界排队修复无新增确定缺陷。`prepare_search_model_fixtures.py` 已实测成功下载、损坏缓存修复、错误hash与超长内容拒绝，并确认旧目标文件保留和临时文件清理；workflow YAML 可解析。真实模型清单由 Rust 生产常量导出，不维护另一套模型revision/字节数/hash。新流程后续必须在真实Windows/macOS runner实际成功，工具测试不能代替平台结论。

当前旧安装包窗口已能正常采集，但其中打开了其他工程；本次没有修改该工程，也没有将其素材用于宣传片。之前全黑采集现象不再持续，不能据此推定新包已验收。

## 首轮远端失败与修复

CI `34041435244` 的 Linux Rust 格式/clippy成功，workspace在实际 `facade_contract` 音频解码失败：AAC尾部输出65,536bytes超过64,004预算。已补目标采样率之后的精确采样裁剪，保留stdout保护；本地13项PCM+1项facade通过，远端待复验。详见 [FFmpeg边界记录](../../knowledge/2026-09-06-ffmpeg-pcm-boundary.md)。

首个本地release app已构建并启动独立进程，Info.plist为1.0.0-beta.6，二进制SHA256 `b59cb93b06f166ef74e7d36b52ba9dc21520edb40c5051be72eaa8ca6b7dabc2`。旧安装包另一个进程保持原工程；新包GUI已创建独立QA工程、原生导入10秒MP4并通过双击加入V1/A1链接轨，进一步验收进行中。此首包早于最终PCM修复重打包，不能作为最终发布源绑定。

- 首轮CI最终为7项成功、Linux Rust与Windows full-product两项失败，二者都在同一AAC facade测试触发相同超限，已有本地修复待新SHA复验。
- Windows模型固定输入适配完成并独立审查通过：同产品分支的宿主真实双编码器+五条查询14秒/RSS1.89GiB，4项Windows分支单测与72项macOS模块实测通过；正式Windows/MSVC仍待qualification workflow。
- 新增模型/PCM修复后的workspace clippy与minimal clippy通过。workspace最终重跑在4K Chromium捕获出现180秒超时，后续同组3项因共享锁poison未执行成功；已启动独立Chromium组诊断重跑，未放宽180秒门槛。
