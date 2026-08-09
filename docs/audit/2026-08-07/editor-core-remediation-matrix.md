# Editor Core Remediation Matrix

基线报告：[editor-core-validation.md](editor-core-validation.md)。本矩阵只安排报告中有 E1/E2/E3 反证的确认缺陷；团队协作、Fusion、模板市场等竞品能力差距不进入本轮修复。

| 优先级 | 缺陷 | 修复状态（2026-08-08） | 验收条件 |
|---|---|---|---|
| P0 | `opentake-asset` 缩略图、源 Preview、Home 封面失败 | **FINAL PASS（E1+E2+E3）**：helper EOF、retained authority 与 ScopeOnly final-root 修复经 Rust/综合 reviewer FINAL PASS；safe-asset 18/18。正式安装包首次打开与完整退出重启后，Home 封面、双素材缩略图、源 Preview 均稳定显示 | 已满足；[GUI 01–03、16–18](editor-core-after-fix-assets/README.md) |
| P1 | waveform/search/preview 派生资源过载 | **FIXED / E4 PARTIAL（E1+E2+E3）**：共享 active 4 / pending 64、typed-kind single-flight、project epoch/latest-wins、A→B→A physical reuse、interactive 抢占；TS reviewer FINAL PASS、全 Web 1143/1143。双轨播放/导出未出现永久占位或无界 FFmpeg 并发 | 代码缺陷已关；真实 100 项冷缓存 CPU/RSS 仍保留后续 E4 量化 |
| P1 | Export 对比度、Library 键盘不可达 | **FINAL PASS（E1+E2+E3）**：Export 16.50:1 + assertive alert；Library 三操作常驻 DOM。正式包 Tab 可到 Export 主按钮，Library 零 hover 可依次到达每卡导入/分类/取消收藏 | 已满足；[GUI 12、13、19](editor-core-after-fix-assets/README.md) |
| P2 | asset scope 项目生命周期 | **FINAL PASS（E1+E2+E3）**：二级 authority gate、manifest+epoch、requested path 与 retained final identity、ScopeOnly final-root 均经双 reviewer FINAL PASS；重启重开 source/cache 仍可见 | 稳定 alias 200；A→B、inode/ancestor swap、ScopeOnly escape 403；正式重启复验通过 |
| P2 | 导入上限/timeout/取消 | **FIXED CORE / PRODUCT PARTIAL（E1+E2）**：5,000 文件/100 GiB、no-recall/nonblocking retained open、同 handle 规划、commit 前 identity/length/aggregate 重验通过并经 reviewer FINAL PASS | 公开文件夹导入的独立进度 UI 与主动取消仍是产品增量，不伪装完成 |
| P2 | proxy 真回归 | **FINAL PASS（E1+E2+E4）**：真 ffmpeg/ffprobe 11/11 + integration 1/1；私有 stage、retained source、后处理 cancel、atomic no-replace；Rust/综合 reviewer FINAL PASS | FIFO/symlink、取消、ABA、unlink、冲突与无 public partial 均有真实 fixture |
| P2 | 其余 UI semantics | **FINAL PASS（E1+E2+E3）**：8 个 RED + reviewer findings 收口；UI focused 42/42、Web 1143/1143、build 通过；TS/综合 reviewer FINAL PASS。正式包 AX 暴露 search name、popup、tabs，并验证 Library/Export Tab 焦点 | 代码与声明 GUI 路径已满足；VoiceOver/200%–400% 缩放保留专项验证 |

## 确认缺陷与产品能力差距边界

- 本轮修复的是可由代码、失败测试或 fresh GUI 直接重现的问题：asset 504、历史 scope 泄漏、派生资源调度、Export/Library/UI semantics、显式导入上限/deadline、proxy partial cleanup/真回归。
- 团队协作、Fusion/节点合成、模板市场、AI 广度等竞品差异是产品路线差距，不作为本轮 bug。
- 全量 DOM 未虚拟化、已进入 Rust/FFmpeg 任务无物理 kill API、公开导入无独立进度/取消 UI、普通 Library 条目无持久 thumb 都保留为可识别的 P2 能力/风险；没有 E3/E4 过载数据的项目不冒充已确认性能故障。

## 合并纪律

- 所有 Agent 共享工作树，只修改获配文件，不回滚他人变更；发现重叠先消息协调。
- 每个缺陷先提交失败测试/可重复失败证据，再改实现；报告原始证据不覆盖。
- focused checks 通过后再跑前端全量、Rust 全量、clippy/fmt/build；正式 release bundle 与 `/Applications` 安装包必须 hash 一致。
- 安装包 fresh 验收使用新工程、退出重开和真实资源；常规文件/媒体/应用权限提示按用户授权自动接受，安全/凭据/系统高风险权限不扩张。
