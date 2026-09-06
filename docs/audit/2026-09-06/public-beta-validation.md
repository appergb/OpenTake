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
| Rust workspace tests | 最终整合：80组、2899通过、0失败、10个显式ignored；包含Motion握手、poster发布、导出后台/提前取消和PCM完整轨修复 |
| Rust minimal clippy | 无默认feature构建检查通过 |
| Web tests/build | 最终整合后153files/1466tests通过，生产构建通过；保留既有chunk/dynamic-import warnings |
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
| 远端CI/Release | PR #249 已建立；两轮CI发现并定位的PCM问题均已修复、本地原测试转绿；真实模型资格34043674623双平台成功；最终候选远端CI待启动，尚未公开发布 |

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

## 2026-09-07 收尾进展

- 精确候选 `64dce596d2d724f08339d469e1233601e927bace` 的 [真实模型平台资格](https://github.com/appergb/OpenTake/actions/runs/34043674623) 已成功：macOS与Windows各1项真正执行、0失败、0忽略，五条图文查询结果一致。相关日志和JSON保存本轮output目录。
- 首包原生GUI贴纸导入/预览/落轨、文本创建并保存OpenTake Beta 6、视频+音频+图片+文本组合导出已走通。输出 `Beta6-GUI-export-720p.mp4` 为1056×720 H.264/30fps/450帧 + AAC48kHz单声道，15秒，1,095,344bytes，完整解码成功。分割实际发生在第299帧；AX slider setValue未触发播放头变化，不把它记为中点定位成功。
- GUI导出耗时期间界面阻塞，sample92542明确主AppKit/WebKit IPC线程同步执行export_video→run_export_with_control→decode_frame。已改async+spawn_blocking，先claim lease和snapshot，再由worker拥有lease到真正结束；ExportControl共享Arc状态，保留更新安装互斥和取消generation。新增跨thread lease取消测试先RED借用非static，后export73项通过。当前首包尚未包含该修复，需新包验证进度和取消。
- transcribe_media存在同样的sync-worker错误假设，已沿现有有界推理worker改async调度，活动lease由任务持有；4项既有转写测试通过。文档与最终评审继续同步。

- 提前取消回归确实RED：旧路径返回取消但已有输出文件已被删除；common export preflight修复后74项export测试通过。无range PCM已恢复完整轨语义，追加最多一秒的有界padding；原full-track equality测试未修改，media facade与FFmpeg集成15项通过/1项原有ignored。此前统一裁剪方案不再是当前实现。

## 最终整合本地验收（2026-09-07）

Rust fmt、workspace clippy、minimal clippy均通过；80组workspace测试合计2899 passed/0 failed/10 ignored，全部命令exit0。Web153文件1466测试、TypeScript和生产构建通过。源码最后只读审查关闭，Motion/Poster两片无确定P1/P2。Release与Windows工作流合同通过，408篇Markdown本地链接检查无错误。新包仍须复验poster冷导入及后台导出进度/取消；旧首包的完成导出不能代替这两项。

独立首包QA已通过原生Open选择器重新打开保存工程，15秒、V2文本+V1视频/图片+A1音频四片段全部保留。未将Home最近工程tile双击无变化记为重开成功，也未把AX range setter当成成功seek。

## aa40672 新包与远端结果

新包二进制 SHA256 为 `80e9018cd85a6cef822de5a268dc7cc1633bd3f39e3417dbeec06fdf0d2f5099`，原测试进程 92542 已退出，新进程 25806；用户旧安装包进程 59956 保持运行。新建 `Beta6-Final-QA` 后冷导入 `cold-playback-2s.mp4`，卡片实际显示完整缩略图。2 秒和 10 秒 1080p H.264/AAC 导出已完成，分别为 60/300 帧、724357/3621960 bytes。随后在 1% 进行中状态立即取消，UI 显示“已取消导出”，`Beta6-final-cancel-confirmed.mp4` 不存在。前两次点击晚于导出结束，未冒充取消通过；文件已改名为 `short-completed` / `long-completed`。

桌面 UI 已实际下载安装 1.54 GB 视觉模型并进入“建立索引”阶段。新候选模型资格 [34047980719](https://github.com/appergb/OpenTake/actions/runs/34047980719) 再次在 macOS/Windows 成功。常规 CI [34047983006](https://github.com/appergb/OpenTake/actions/runs/34047983006) 仍有 Motion 跨平台阻塞：Windows 实际 4K opaque 180 秒超时；Linux 模拟透明协议用例等待 ACK 超时。其余 7 项通过，PCM 原失败场景已通过。Carver 继续同一协议切片；当前仍为候选，未创建 tag 或公开 Release。

最终安装包还完成了原生双文件导入、真实视觉索引和查询：`semantic-cats.png` / `semantic-parrots.png` 经模型实际处理，“沙发上的两只猫”在“画面”组返回 `semantic-cats`，“彩色鹦鹉”返回 `semantic-parrots`，`a photo of an airplane` 显示无匹配。中文查询不匹配英文文件名，不能将其误当作 Files 回退结果。卡片和实际画面均已查看。完成/取消导出文件已分别通过 probe、完整解码或不存在检查。测试工程已保存，测试进程 25806 已退出，用户旧安装实例不变。

## Motion 第四轮候选

第三轮 Windows trace 的三个启动帧全部是旧帧，seed 也可能被丢弃。新补丁在任何变更之前启动并消费/ACK 旧帧，然后逐项提交 seed、作者 marker、transition、desired，每个匹配检查后排空当前 session 的已排队 ACK。Linux 模拟测试的真实 deadline 为 1 秒；原 socket 未启用生产连接的 TCP_NODELAY，连续小消息的延迟累积导致失败。测试传输已对齐，deadline 未延长，错误预算标签同步为 1 秒。

作者完成 97 项 Motion 单测与 7 项实际 Chromium 集成，4K opaque/transparent 为 7.31/12.09 秒，trace 关闭，原 4K、alpha、CSP 验收文件未修改。独立只读复核通过。新增独立 Windows/Linux 资格 workflow 绑定精确 SHA，强制实际执行 97+7 和两个 4K 路径并保存日志，原 CI/release 合同未改；提交后的远端结果待回读。

第四轮 `d1a020e` 的 Motion 专项 `34052875635` 已真实双平台通过 97+7（0 failed/ignored）：Linux 4K 10.200/17.805 秒，Windows 13.821/23.327 秒。模型资格 `34052875624` 也双平台成功，完整 CI 的 Rust 及其余 7 个任务通过。Windows full-product 在最后 Tauri 单测仅普通输出清理失败（689 passed/1 failed），Motion 阻塞已经解除。

当前处理这项 Windows 清理缺口：普通输出补 DELETE 访问权，保留 deny-delete-sharing；验证复用打开的文件而非重开路径。详细依据见 [Windows 输出句柄记录](../../knowledge/2026-09-07-windows-export-handles.md)。未关闭测试、未绕过失败，未创建公开 tag。

这项修复的本地 Tauri clippy、75 项 export 单测（含 reserved 外层所有权断言）和 6 项真实 export_integration 已通过，后者覆盖完整视频、音频 mux、HDR、文字、4K 和 ProRes 4444 alpha。新增精确 SHA 的 Windows export qualification，使用锁定 sidecars，强制两个句柄回归实际通过并保存日志；最终 Windows 结果待回读。
