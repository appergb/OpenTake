---
status: canonical
stage: implementation-backed
---
# 本轮候选应用素材采集记录

日期：2026-09-06。目标应用 `/Applications/OpenTake.app`，是主代理允许操作的既有候选安装包；不是尚在构建的 beta.6。只通过 CUA 操作目标应用。

1. OpenTake 初始未运行。启动后首页 AX 正常，创建独立 `OpenTake Official Demo.opentake`；新建 SavePanel 初始 Save 也灰置，后来通过 AX 变为可用，成功创建。
2. 导入文件 → Go to 指定 `public/assets/original-motion.mp4`，文件已选中且系统缩略图可见，但 Open 一直 disabled。坐标双击返回 `noWindowsAvailable`。点击 Cancel 成功返回编辑器。这不是成功导入证据。
3. 返回首页后，由脚本在该独立工程写入合法 schema 与当前媒体绝对路径。双击首页卡片重开，AX 可见媒体库 2 项、V2/V1/A1、三个视频片段、文字和音频片段。
4. 对目标应用截图时只有全黑窗口和窗框按钮。Raise 后同样黑屏，未推断是产品渲染缺陷还是锁屏/捕获环境问题。
5. 点击“导出视频 → 导出”，原生 SavePanel 默认目录为 `marketing/official-beta-2026-09`，文件名 `OpenTake Official Demo.mp4`。等待后 Save 仍 disabled，未生成文件。
6. 已点击 SavePanel 的 Cancel，回到导出设置；再点“取消”，关闭导出设置。目标应用留在独立演示工程，不占用对话框。

主代理已收到复现消息，应在可见桌面及新构建上另行确认导入、导出路径。宣传片以历史实拍版交付，这些问题不被隐藏为成功验收。
