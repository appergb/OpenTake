---
status: historical
stage: final-verified
---
# OpenTake 官方宣传片制作记录

范围：仅本目录。独立 60 秒 / 1920×1080 / 30 fps / H.264 成片、封面、完整源文件与验收。
用户已授权视觉设计、渲染、应用素材采集；不修改产品、不发布、不覆盖其他未提交素材。

意图：以真实编辑界面体现导入、剪辑、多轨、文字/效果、导出与 Agent；Public Beta 通用版本文案。
视觉：黑绿背景、薄荷绿光影、克制排版，原创几何动态样片、真实桌面界面和说明文字结合。
Read set：项目 AGENTS.md、docs/INDEX.md、docs/releases/1.0.0-beta.5.md、docs/capabilities/CAPABILITY-LEDGER.md、旧 marketing 空脚手架、audit 截图与工程 schema（只读）。

里程碑：
- [x] 核对基线 release/v1.0.0-beta.5 / 33ee8e2，盘点素材和应用状态。
- [x] 原创样片、声音与 Remotion 场景。
- [x] 目标应用采集尝试并记录限制；采用有日期的历史实拍完成成片。新包 GUI 验收不在本任务完成声明内。
- [x] 成片与封面渲染。
- [x] ffprobe、全片解码、12 时点视觉抽帧与来源归档。

验收：60 秒左右，1920×1080 H.264 yuv420p + AAC；ffprobe 保存完整 JSON；ffmpeg -xerror 全片解码；首中尾与关键功能场景抽帧人工检查。
风险：安装应用可能旧于当前源代码；既有审计素材不代表当前版本新验收。AI 需要用户配置服务，本片不宣称云端任务成功。正式版本未收到前不写具体新版本号。
回滚：本目录独立，所有生成与复用副本仅存在此处；保留旧目录不动。

最终交付见 README.md / VALIDATION.md。主代理后续明确候选版本为 1.0.0-beta.6，影片已采用该版本 + Public Beta；没有发布完成措辞。
