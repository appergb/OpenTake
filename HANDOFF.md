# OpenTake 交接说明 — 分支 `agent-handoff-all-prs-20260624`

> 给接手的 AI agent。本分支把**所有开放 PR + 全部改动**汇总到一起,基于当前 `main`,**编译通过(tsc 0)、52 前端测试全过**。

## 这条分支包含什么
基于当前 `main`(已含序列化总根因修复 #143)+ 以下全部合并:

| 来源 | 内容 |
|---|---|
| #77 | 媒体面板文件夹浏览 |
| #78 | 设置 7 分页 + MCP Instructions + 主页 1:1 |
| #79 | 提取音频到本地文件 |
| #105 | 复制/剪切/粘贴片段(⌘C/⌘X/⌘V) |
| #108 | 片段右键菜单 |
| #120 | 吸附迟滞+多探针 / 链接 offset 角标 / 音量橡皮筋 |
| #121 | SwapMedia 替换片段媒体 |
| #122 | Inspector live 采样 + crop/fade/flip 字段 |
| #123 | 拖放建轨 + Option 拖拽复制 |
| #138 | snap includePlayhead(对齐上游) |
| #139 | 链接 offset 角标(**与 #120 竞争实现,见下**) |
| 我的修复 | 序列化总根因 / 删除可靠性 / 蓝色选中 / 拖入落点 / 右键菜单定位 / 预览暂停止血 / scrub 防抖 / 递归导入 / Generate 提示 / 红绿灯位置 / 白底修复 |

## ⚠️ 必读:已知问题与重写方向(issue #142)
1. **时间轴播放/预览逻辑需按上游 palmier-pro 1:1 重写**(暂停抽搐、scrub 不实时)。当前是双渲染面 hack,非上游单 `AVPlayer` 模型。已在 `web/src/components/preview/{TimelinePlaybackLayer,Preview}.tsx` 顶部加 `⚠️ REWRITE-PER-UPSTREAM #142` 标记。忠实等价物 = 单 `<canvas>` 流式引擎 #53(#63 cpal + #64 MJPEG)。**别再打补丁,按上游重写。**
2. **序列化总根因(已修,留意)**:`src-tauri/src/commands.rs` 的 `EditRequest` 每个 struct 变体都已加 `#[serde(rename_all="camelCase")]`——serde enum 级 rename_all 不改字段名,缺它则多词字段命令(delete/split/inspector/keyframes/…)IPC 反序列化失败、静默无效。**新增变体务必加这个属性**,并跑 `cargo test -p opentake-tauri edit_request_serde`。
3. **#139 vs #120 链接角标竞争实现**:二者都实现 `drawOffsetBadge`,本分支保留了 **#120 的版本**;#139 的版本在本地 ref `pr-139-head`(`git diff pr-139-head -- web/src/components/timeline/clipRenderer.ts`)。两者都有偏离上游的 bug(#120 角标位置在左上应右上;#139 锚点漏 trimStartFrame、参照系用两两±非组内 min)。**按 #142 重写时统一取舍**,参考上游 `ClipRenderer.swift:624-656` + `EditorViewModel+Linking.swift:95-113`。

## 各功能 PR 仍待修的逻辑 bug(已逐个 request-changes)
- **#79** `extract_audio` 的 `out_path` 路径穿越漏洞 + 缺测试;碰 #91 媒体重写区。
- **#121** SwapMedia 缺 `kind==media_type` 类型校验(视频可被换成音频)、自创截断逻辑、缺链接组级联。
- **#122** Inspector 可编辑 opacity/volume 用了含 fade 包络的采样值 → 会把损坏值写成静态属性。
- **#123** `duplicate.rs` 无条件清空 link_group_id,A/V 联动对复制后丢链接。
- **#120** 角标位置/与 #123 改同文件冲突 / move 错吸 playhead。
- **#138** move/trim 仍 false,上游应 true。
- **#78** 主页 1:1 多处缺;**#77/#79** 碰 #91 媒体重写区。

## 纪律(沿用)
- **忠实 1:1 复刻上游 `../palmier-pro-upstream/Sources/PalmierPro/`**,别自己发明。
- 改完跑 `cargo fmt --all` + `cargo test` + 前端 `tsc -b` + `vitest run`;原生验证用 `./web/node_modules/.bin/tauri build` 装 `/Applications`。
- 详见 `CLAUDE.md`(权威状态)+ `docs/PORT-1TO1-GAP.md`。

## 关联
issue #142(总 bug + 重写)· #53/#92/#100(播放引擎)· #48(片段编辑收尾)· #91(媒体系统重写)。
