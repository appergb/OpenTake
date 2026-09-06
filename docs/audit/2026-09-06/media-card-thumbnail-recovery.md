# 媒体卡片缩略图加载恢复

> 状态：generated · 阶段：implementation-backed · 日期：2026-09-06
> 范围：MediaCard 图片加载恢复及小 poster 原子发布；Rust 补充更新：2026-09-07。原生 QA 复测与重新打包由主代理负责。

## 用户报告与只读证据

主代理在独立 Beta 6 `target/release/bundle/macos/OpenTake.app` 中报告：导入 `output/public-beta-2026-09-06/fixtures/playback-10s.mp4` 后，左侧卡片始终为蓝色问号破图，源预览与 V1 sprite 正常；完整播放 10 秒后破图仍在。

本任务没有控制电脑，也没有写入 QA 工程或素材。只读核对：

- QA 工程 `output/public-beta-2026-09-06/Beta6-Desktop-QA.opentake/media.json` 指向上述视频，asset id 为 `edb35602-6523-4979-b995-dc1a1cfc6784`。
- 按既有 `visual_file_identity_key` 的 path/size/mtime 规则定位到 cache key `9d7a7d4837ec2e05a65d12bb89028aca`。
- `/Users/trip/Library/Caches/com.opentake.desktop/media-cache/MediaVisualCache/9d7a7d4837ec2e05a65d12bb89028aca.thumb.png` 存在，3059 字节、100×68；PNG signature、IHDR/IDAT/IEND chunk CRC、IDAT zlib 解压通过。
- 同目录 `.preview.png` 存在，281760 字节、1584×1080，同样通过 PNG 结构/解压检查。卡片使用小 poster，源预览使用独立大 poster，不共用 React 图片节点。
- `assetUrl` 通过 `convertFileSrc(path, "opentake-asset")` 构造 URL。静态 scope 包含 `$APPCACHE/**/*`，协议将 app_cache_dir 列为 application-owned root，并允许 PNG MIME。未找到仅对该卡片 PNG 缺少授权的静态证据。
- 原生协议使用 `request.uri().path()` 解析并授权实际路径，URL 查询参数不会改变被授权文件。

## 已确认原因与证据边界

已确认的**持续破图原因**：MediaCard 直接挂载 `<img src=...>`，没有 onError 恢复；`item.thumbnail` 已有路径时跳过 lazy thumbnail 请求；即使列表刷新或预热完成，路径字符串不变也不会主动重新加载失败图片。因此一次资源加载失败会一直留下 WebKit 破图。

首次 asset 请求具体返回的状态码/错误尚未采集。上述检查不能证明首次失败属于权限、暂时忙、文件发布时序或其他协议错误；没有据此修改 Rust 或放宽授权。最终是否恢复原生 QA 中的实际图片，需要主代理用更新 Web 资源后的候选包复测。

## 最小修复

在 `MediaPanel.tsx` 内新增私有 `MediaCardThumbnail`，只用于原 MediaCard 的图片节点：

- 图片 error 后移除破图节点，显示原素材类型图标。
- 延迟 250ms、1000ms，最多重新加载两次同一 asset URL；附加 `opentake-thumbnail-retry=1/2` 查询参数，避免继续使用失败的请求结果。
- 重试不调用 generateThumbnail、不重新解码、不触发预览/落轨，也不修改源文件、缓存或媒体镜像。
- 两次仍失败后停在类型图标，不无限循环；卡片选择与源预览仍可使用。
- 恢复组件的 key 包含 projectEpoch、媒体 thumbnailKey 与实际 URL。工程、来源或 URL 变化时重建恢复状态；卸载会清理等待中的定时器。

实际写集仅：

1. `web/src/components/media/MediaPanel.tsx`
2. `web/src/components/media/MediaPanel.test.tsx`
3. 本记录

没有修改 asset helper、Rust、Windows 模型适配、PCM、QA 工程或其他 UI。未 commit/push。

## 验证

先新增 4 项回归。旧实现有 3 项明确失败：错误后仍显示坏 img、没有重试上限后的占位、没有可清理的恢复定时器。修复后覆盖：恢复后的新 URL、两次上限、卡片仍可预览、工程/源变化隔离、卸载清理。

执行：

```bash
pnpm --dir web test --run src/components/media/MediaPanel.test.tsx src/components/media/StickerPanel.test.tsx
pnpm --dir web exec tsc --noEmit --incremental false
git diff --check -- web/src/components/media/MediaPanel.tsx web/src/components/media/MediaPanel.test.tsx
```

结果：2 个文件 / 70 项通过（MediaPanel 51 项，Sticker 19 项）；TypeScript 检查与 diff 检查通过。没有运行全量测试、Cargo 构建或重新打包。

GUI 留给主代理：在更新后的候选包重现同一素材导入，确认卡片图片恢复；若仍停在类型图标，则需捕获该 `.thumb.png` 的原生 asset 请求错误，再决定是否需要 Rust 协议层修复。浏览器 DOM 的人工 error 事件验证了恢复流程，不替代这项原生验收。


## 2026-09-07：双生产者发布竞态的 Rust 修复

主代理授权扩展写集后，完成 `src-tauri/src/media.rs` 的 poster 写入/缓存读取及 `src-tauri/src/media/prewarm.rs` 的 GridPoster 发布修复。保留主代理原有两处 `ExportGuard<'_>` → `ExportGuard` 改动。未改 safe asset 协议、sprite 生产逻辑、export/transcribe、Windows 模型适配或 PCM。

### 复现证据

冷缓存导入后台 GridPoster 与 Web `generate_thumbnail(includeSprite=false)` 使用同一个 `{key}.thumb.png`。旧同步写入直接使用最终路径，后台预热则 rename 覆盖同一路径。

受控 channel 屏障先暂停后台生产者，再让同步 `write_png` 发布有效 PNG，记录其文件身份，最后放行后台提交。旧实现测试失败，实际读到的 inode 从 24552253 变为 24552255；另一个测试证明旧后台发布会覆盖已存在的损坏目标。旧 `write_png` 对损坏目标直接返回成功、列表暴露损坏路径的用例也先确认失败。

### 最小实现与现有缓存规则

- `StagedPoster` 使用既有 tempfile 依赖，在目标同目录生成唯一临时文件，完整验证/写入 PNG 后调用 `persist_noclobber` 发布。当前依赖在支持的 macOS/Linux 文件系统使用 no-replace rename，Windows 使用不覆盖移动；没有引入依赖或新协调器。
- 同步 `write_png` 和后台 `GridPoster` 复用此路径。已有有效同 key PNG 直接复用；其他生产者赢得发布时也只验证并复用，不能替换已发布 inode。
- GridPoster 在既有 scheduler epoch/token 锁内完成最后发布，取消任务不发布；临时文件由 RAII 清理。
- sprite 及其他原有可覆盖的 `commit_staged_bytes` 分支保持原行为。测试确认已有 sprite 仍可从 partial 更新为 complete。
- `cached_poster_dimensions` 检查 resident regular file、拒绝 symlink/reparse，并复用既有 safe asset 的保留句柄打开方式，对 PNG 完整解码验证。未修改或减弱 safe asset 验证。
- 损坏、截断、目录/符号链接等非法目标：保留原物，返回错误，不自动删除、修复覆盖或读取符号链接目标；媒体列表不返回其 thumbnail 路径，显式生成/缓存读取返回错误。恢复这类已损坏缓存需要通过既有缓存清理流程重新生成。
- 既有大 preview poster 也通过共用 `write_png` 获得完整、不覆盖发布，但路径与小 poster 仍分离；没有扩大预览 UI。

### 受控发布测试与结果

新增 7 项 Rust 回归、更新 1 项现有缓存列表用例，覆盖：

1. 完整临时文件写好后暂停于发布之前：最终路径不存在，`read_cached_poster` 无结果，媒体列表 thumbnail 为空；放行后为完整 PNG，临时文件已清理。
2. 同步生产者先发布、后台 GridPoster 后提交：文件身份、字节、Unix inode/ctime/ctime_nsec 全部保持不变。
3. 另一生产者在本生产者暂存后赢得发布：后者复用赢家，清理自身临时文件。
4. 损坏/截断 PNG 与符号链接拒绝且不修改原物；无效暂存内容不进入最终路径。
5. GridPoster 拒绝损坏目标，同时 sprite 的覆盖更新仍成功。

实际执行：

```bash
cargo test -p opentake-tauri --lib poster -- --nocapture
cargo test -p opentake-tauri --lib media::prewarm::tests -- --nocapture
cargo test -p opentake-tauri --lib media_item_uses_existing_cached_thumbnail_without_decoding -- --nocapture
cargo test -p opentake-tauri --lib cached_thumbnail -- --nocapture
rustfmt --edition 2021 --check src-tauri/src/media.rs src-tauri/src/media/prewarm.rs
git diff --check -- src-tauri/src/media.rs src-tauri/src/media/prewarm.rs
```

结果：poster 集合 12/12、prewarm 集合 13/13、缓存列表 1/1 通过；集合有重叠，共覆盖 23 个不同测试。`cached_thumbnail` 过滤器再次选中同一缓存列表用例，不计为额外覆盖。格式与 diff 检查通过。Cargo 仅报告既有 `block v0.1.6` future-incompatibility 警告。

早期测试草稿曾误用不存在的 fixture 构造函数，已在生产实现前修正；随后记录的红灯均为上述实际行为断言失败。没有跳过或关闭测试。未运行全 workspace 测试，也未重新打包/操作 GUI/修改 QA 工程。

### 验收边界

已复现并修复双生产者发布竞态，前端两次受限重试继续保留。原生首次蓝问号问题仍须主代理用新包、冷缓存导入及同文件新工程复验；当前自动化结果不能替代首次 GUI 请求的闭环证据。Windows/Linux 发布分支没有在本机实测。
