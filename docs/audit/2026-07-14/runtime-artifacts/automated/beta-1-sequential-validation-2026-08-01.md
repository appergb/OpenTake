# OpenTake 1.0.0-beta.1 顺序验收记录

- 验收时间：2026-08-01 22:48 CST（Asia/Shanghai）
- 平台：Apple Silicon macOS
- 候选版本：`1.0.0-beta.1`
- 候选应用：`target/release/bundle/macos/OpenTake.app`
- 候选 DMG：`target/release/bundle/dmg/OpenTake_1.0.0-beta.1_aarch64.dmg`
- 验证工程：`~/Documents/OpenTake/未命名.opentake/OpenTake-Beta1-Sequential-QA.opentake`

## 结论

候选包按照发布清单 1 → 11 顺序完成了代码门禁、真实桌面 GUI 操作、项目保存重开、
本地 Motion 渲染、官方 Codex Agent 编辑和最终媒体导出。验收中发现并修复了一个真实
阻断问题：旧工程缺少可选 `voiceModels` 字段时，声音克隆页的 Zustand selector 每次
返回新的空数组，导致 React 无限重渲染和 WebView 空白。新增回归测试先复现失败，改为
稳定空集合后通过；重建候选包后在原旧工程上重新打开声音克隆页成功。

## 代码门禁

- `cargo fmt --all -- --check`：通过。
- `git diff --check`：通过。
- `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `CARGO_INCREMENTAL=0 cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings`：通过。
- `CARGO_INCREMENTAL=0 cargo test --workspace`：通过；所有执行测试无失败，真实设备专用 probe
  按测试声明保持 ignored。
- `npm test`：119 个测试文件、882 项测试通过。
- `npm run build`：通过。
- `web/node_modules/.bin/tauri build --bundles app,dmg`：通过。

已知非阻断警告：`block v0.1.6` 的 Rust future-incompatibility 提示，以及 Vite 的大 chunk /
动态导入提示；两者均未造成编译、测试或运行失败。

## 候选包哈希与签名边界

- DMG SHA-256：`394ae88d0d47ffe1f1286b34903fceb79cb38acf0b73b70fe0c3ec86a1e1b1cf`
- 应用主程序 SHA-256：`c015e1af2a9369e6e5fb030818e8c1e32648dca400d578faddd3cfda3105721e`
- 应用架构：Mach-O arm64。
- 当前签名：ad-hoc / linker-signed，`TeamIdentifier` 为空；没有 Developer ID Application
  身份和 Apple 公证凭据，因此本 Beta 不宣称已 Developer ID 签名或公证。

## 1 → 11 顺序桌面验收

1. **Home / 工程生命周期**：保存、返回主页、完全退出应用、重新启动、从最近工程选择并
   按 Return 打开均成功；聊天、时间线、媒体、文本和历史状态恢复正确，撤销/重做栈按重开
   规则清空。
2. **素材库**：视频缩略图、音频波形素材、收藏开关、“我的”全局库、音频声部分离素材、
   文件/文件夹导入原生选择器均可见并可操作；720p 代理实际生成并持久化，重开后代理文件
   存在；离线重链接由条件 UI 与 Rust 回归测试覆盖。
3. **时间线**：链接选择同时选中视频/音频；播放头移动至 138 帧后对链接组实际分割为两组，
   撤销、重做、再次撤销恢复原状；复制/粘贴和出点裁剪实际执行后全部撤销。吸附、拖放与嵌套
   序列由相同候选包的时间线控件观察及 workspace 集成测试覆盖。
4. **预览与画布**：真实播放/暂停从 0 推进到 138 帧（00:04:18），逐帧/seek 控件可用；
   Transform、Crop、Mask、运动追踪和关键帧控件可见。保存工程中的 X/Y 位置关键帧重开后仍
   标记为“已在关键帧面板动画化”；A/V 同步由实际导出媒体 probe 共同验证。
5. **文本与字幕**：文本片段内容 `OpenTake Beta 1`、字体、字号、颜色和样式重开后正确；
   字幕转写在无语音测试素材上明确返回“未检测到语音”，翻译保持 Provider 同意/费用门禁；
   SRT / VTT 导出入口和原生保存流程实际执行。
6. **特效与调色**：视频检查器实际观察并可进入曝光、色温、Lift/Gamma/Gain、对比、饱和、
   3D LUT、HSL 二级调色、参考画面色彩匹配、绿幕、蒙版、智能擦除、效果/滤镜、防抖和运动
   追踪；相邻片段转场面板正确识别切点并显示 15 帧交叉溶解。对应 GPU / LUT / HSL / 蒙版 /
   防抖 / 补帧 / 擦除测试全部通过。
7. **音频**：声部分离已实际产出 `Vocals` 与 `Accompaniment` 两个独立 WAV 素材并落入独立
   轨道；响度目标 `-16 LUFS`、降噪预览/重置、分离控件可见；最终 H.264/H.265/ProRes 导出
   均含 48 kHz 单声道音频。
8. **Agent / MCP / Codex**：官方 `codex-cli 0.144.1` 显示“已通过官方 Codex 登录：
   ChatGPT”。真实 Agent 指令仅把唯一文本改为 `OpenTake Codex Beta Verified`，Codex 通过
   `get_timeline` 和 `set_clip_properties` 完成 MCP 编辑；撤销/重做验证后最终恢复原文并保存。
   Anthropic 未配置时明确引导设置，不伪装成功。图文成片计划 UI、同意/费用边界和对应原子性
   测试通过。
9. **Motion Canvas**：实际用本地浏览器渲染 3 秒标题卡，生成 `Motion Graphic` MP4 并在
   138 帧加入新视频轨；保存、重开后媒体和片段仍存在。再次运行 1 秒渲染正常留在编辑器并
   显示“动效已添加到时间线”，随后撤销测试用第二片段。
10. **数字人 / 声音克隆**：数字人页面必须同时选择素材并勾选本人/声音授权和付费授权；
    实际触发后因无 fal key 明确拒绝。声音克隆空白页缺陷修复后，参考音频、名称、同意、费用、
    注册、生成试听、永久撤销控件均正常显示；实际注册因无 ElevenLabs key 明确拒绝并显示重试，
    未发起付费请求。取消、试听、撤销和撤销不可逆边界由组件及 Rust workflow 测试覆盖。
11. **交付导出与最终重开**：实际导出 H.264、H.265/HEVC、ProRes 422、SRT、VTT、XMEML、
    FCPXML、OTIO 和 CMX3600 EDL；随后保存、完全退出、重启并重开工程，最终文本、Motion、
    代理、图片、媒体和时间线均恢复。

## 导出媒体 probe

| 输出 | 视频 | 音频 | 尺寸 / 帧率 | 时长 | SHA-256 |
| --- | --- | --- | --- | --- | --- |
| H.264 MP4 | `h264` | AAC, 48 kHz, mono | 1280×720 / 30 fps | 12.5 s | `e6c21d87a2de852a0cd1f73288db39defafc17edccd9e79dd428ea8f1ce282d4` |
| H.265 MP4 | `hevc` | AAC, 48 kHz, mono | 1280×720 / 30 fps | 12.5 s | `4bf347462eef62160260c6bc4173eee4078870d0161e5c813af9ac799f064156` |
| ProRes MOV | `prores` | PCM s16le, 48 kHz, mono | 1280×720 / 30 fps | 12.5 s | `57fba9c267f89cd097be8497f2f6cc6f438ca012fcc0e641d9422ef479d7cb80` |

- `xmllint --noout` 验证 XMEML 与 FCPXML：通过。
- `jq -e` 验证 OTIO JSON：通过。
- EDL 含三个视频事件和 12 帧交叉溶解记录。
- 当前时间线没有字幕 cue，因此 SRT 为 0 字节，VTT 为合法的 8 字节 `WEBVTT` 头；这是后端
  空字幕导出的既定契约，不是静默失败。

## 外部发布门槛

- 最终 Git 标签必须指向包含本记录和声音克隆回归修复的精确提交。
- GitHub Actions 必须针对该精确提交 SHA 成功完成 Windows / Web / Rust 构建测试后才可
  创建 `v1.0.0-beta.1` prerelease。
- macOS 资产可用于本地 Beta 测试，但因缺少 Developer ID 与公证，不适合向不愿绕过
  Gatekeeper 的普通终端用户宣称为正式安装包。
