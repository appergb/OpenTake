# OpenTake 1.0.0-beta.2 正式构建与安装账本

日期：2026-08-08（Asia/Shanghai）。工作树保持未提交；本账本不代表发布到公证/商店渠道。

## 构建

最终命令：

```sh
./web/node_modules/.bin/tauri build --bundles app,dmg \
  --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'
```

plain Tauri build 生成的首个 bundle 没有完整 bundle seal，`codesign --verify --deep --strict` 失败，因此从未安装；它保留在 `/tmp/OpenTake-unsigned-bundle-20260808-130918` 供诊断。用 Tauri 的 ad-hoc signing 配置重建后，main、ffmpeg、ffprobe 与 app bundle 都由打包流程签名。

- App：`target/release/bundle/macos/OpenTake.app`
- DMG：`target/release/bundle/dmg/OpenTake_1.0.0-beta.2_aarch64.dmg`
- DMG SHA-256：`bf9eb8ff32eb202fb4150fa7e6682222e11e8d05a3e9170ef8636f4ab2953aab`
- bundle 相对内容树 digest：`247d874c37b772ba6a5b86875e9e57880fc0c0becf3aff4728073a504f316815`
- bundle ID / version / minimum macOS：`com.opentake.desktop` / `1.0.0-beta.2` / `11.0`
- 架构：Mach-O arm64。

锁文件验证的签名前 sidecar：FFmpeg/ffprobe 7.0，GPL、无 nonfree；SHA-256 分别为 `326895b16940f238d76e902fc71150f10c388c281985756f9850ff800a2f1499` 与 `307e09bc01bd72bde5f441a1a6df68769da3b2b6e431accfbfc9cf3893ad00c4`。Tauri 签名会改变 sidecar 文件字节；签名后 hash 如下：

- `opentake`: `27a55587612dc70b7f2d60740b0c80c75065d53208385bb4cc38d3da5619c327`
- `ffmpeg`: `a83e9395c338b9e759cfba5b797e4206e60d46572c05b7467d2a33e30f53fcc3`
- `ffprobe`: `8a5ff4c6b60ce86cc6e4c7b0bd026aa7a6e91ebfb7748dbbcae68b6ef1d04739`

build app 的 `codesign --verify --deep --strict --verbose=4` 通过。DMG 以 readonly fresh mount 验证，内含 app 同样 strict/deep 通过，且 `diff -rq` 与 build app 无输出；验证后正常卸载。

签名属性为 `Signature=adhoc`、`TeamIdentifier=not set`、sealed resources v2。没有 Apple credentials，因此 Tauri 明确跳过 notarization；`spctl` 不接受该包是预期事实，不能描述为已公证发行。

## 可恢复安装

1. 先把 build app 复制到 `/Applications/.OpenTake.app.install-20260808-131143`，在 staging 上完成 strict/deep 和内容 hash 验证。
2. 原 `/Applications/OpenTake.app` 移动到 `/Applications/OpenTake.app.backup-20260808-131143`，没有删除。
3. staging 原子改名为 `/Applications/OpenTake.app`。
4. installed app 再次 strict/deep 通过；`diff -rq target/release/bundle/macos/OpenTake.app /Applications/OpenTake.app` 无输出。
5. installed main/ffmpeg/ffprobe hash 与 build app 上述三个 hash逐项相同。

旧包 main hash 为 `139f4749e1e6bc35b13f980c4b36353dd211be4b1a10f784435d875ce3d4d78a`；完整备份仍存在。第一次正式启动 PID 24234；完成 GUI 导出后 Cmd+Q 确认进程退出，再次只从 `/Applications/OpenTake.app` 启动，复验时 PID 52325。

## Fresh GUI 结论

- Home 封面、工程重开素材缩略图、源预览、时间线复合预览：PASS。
- 4K60 单轨播放/seek/pause：PASS。
- 4K60 fresh 双轨播放 0→239、再次 0→48、seek/pause：PASS。
- GUI H.264 720p 导出 + ffprobe + 2 秒视觉帧：PASS。
- Export 主按钮对比度/键盘焦点、Library 三个操作的零 hover Tab 路径：PASS。
- 完整退出后重启与工程重开授权：PASS；无新增权限弹窗。
- 未启动 dev/Vite 或额外 OpenTake bundle；验证结束时只有 installed 正式实例。

## 2026-08-08 19:36 安全收口后的最终重构建

安全终审关闭 ProjectMedia manifest 扩权与 Windows native-import 空解析问题后，旧候选包不再作为最终制品。主线程从已清理的 Cargo target 使用以下命令重新构建：

```sh
./web/node_modules/.bin/tauri build --ci \
  --target aarch64-apple-darwin --bundles app,dmg \
  --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'
```

- App：`target/aarch64-apple-darwin/release/bundle/macos/OpenTake.app`
- DMG：`target/aarch64-apple-darwin/release/bundle/dmg/OpenTake_1.0.0-beta.2_aarch64.dmg`
- DMG SHA-256：`01ee3c5a468253fc449083ab3cf7ec62dc110f6c85ea8055185a573a0ae3ab45`
- bundle 相对内容树 digest：`68e7da55ce9140cb93f995dab3fe20de8c53cd574fe678f5d23f0ec51d0ee110`
- `opentake`：`aa43de8e3a6782e546b301dc9a1fee2b7739fda8ef1ef1af8c5ff52af03db8be`
- `ffmpeg`：`a83e9395c338b9e759cfba5b797e4206e60d46572c05b7467d2a33e30f53fcc3`
- `ffprobe`：`8a5ff4c6b60ce86cc6e4c7b0bd026aa7a6e91ebfb7748dbbcae68b6ef1d04739`

App、main、两个 sidecar 均为 arm64，deep/strict codesign 通过；DMG CRC 验证通过。readonly 挂载后的 App 再次 deep/strict 通过，且与 build App 的 `diff -rq` 无输出、三个二进制 hash 完全相同。签名仍为 ad-hoc，未使用 Developer ID、未公证。

安装时先正常退出旧实例，将旧 `/Applications/OpenTake.app` 可恢复地移动到 `/Applications/OpenTake.app.backup-20260808-193612`；新 App 先复制到独立 staging、验证签名和内容，再原子改名为 `/Applications/OpenTake.app`。安装版与 build App 的 `diff -rq` 无输出，内容树 digest 相同。

最终安装版 PID 4730，二进制路径为 `/Applications/OpenTake.app/Contents/MacOS/opentake`，loopback listener 为 `127.0.0.1:58102`。Fresh GUI 快速复测：Home 正常；`gui-validation-dual` 双 4K60 素材缩略图、时间线复合预览正常；播放头从 0 连续推进到 239；Sort popup 具备单一选中项，按 Tab 后弹层关闭且焦点前进到 Filter。无权限弹窗、无遗留 ffmpeg/ffprobe/helper 子进程。
