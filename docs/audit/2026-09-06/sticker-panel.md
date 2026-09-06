# Sticker 面板纵切记录

> 状态：generated · 阶段：implementation-backed · 日期：2026-09-06
> 工作树：OpenTake-generation · 分支：release/v1.0.0-beta.6
> 范围：本地贴纸面板接线与定向 Web 验证；不是原生安装包或发布验收。

## 实现结果

- Sticker 主标签已启用，在 MediaPanel 内提供独立贴纸内容区。
- 「导入贴纸」打开有贴纸格式筛选的原生多选文件对话框。接受 PNG/JPG/JPEG/TIFF/HEIC/WebP、Lottie JSON、`.lottie`；扩展名集合与既有 Rust `ClipType::from_file_extension` 一致，JSON/Lottie 内容校验继续由 Rust 负责。
- 面板从现有 `useMediaStore` 镜像筛选整个工程的 `image` / `lottie`，包括子文件夹素材。不新增贴纸模型或分类持久化字段，不引入商店、网络请求、生成素材或依赖。
- 复用 `MediaCard` 的缩略图、单击/键盘选中及源预览、拖拽 MIME、收藏和重新链接。增加可键盘访问的「将选中贴纸加入时间线」按钮；双击经同一受控回调调用既有 `addMediaToTimeline`，拖拽继续交由现有时间线接收。
- 落轨继续经既有 `placeMedia` 单事务处理，前端没有新增 Undo 栈或领域编辑逻辑。定向测试核对入口与既有事务行为；真实桌面 Undo 仍待主代理验收。
- 空态及状态使用 `role=status`，失败使用 `role=alert`，按钮通过 `aria-describedby` 关联提示。无工程、只读工程、素材离线、生成/下载中及生成失败/取消时给出原因并禁用相应操作。
- 整个导入手势（包括文件选择器）复用 `beginMediaImport` / `endMediaImport` token。重复点击被拦截；工程 epoch/path 变化后的选择结果、刷新和错误不会写回新工程，旧操作结束不会清除新工程的导入状态。
- StickerPanel 同工程跨标签保持挂载，切走时仅卸载卡片视图，保留 pending/ref/错误；贴纸局部状态以 epoch/path 为 React key 重建，旧工程落轨失败不进入新面板。共享方向键导航尊重卡片 `aria-disabled`，不会通过方向键预览不可用贴纸。

## Read Set 与修改范围

已阅读根 AGENTS.md、`docs/project/conventions.md`、Web OVERVIEW/INDEX 路由，以及现有 MediaPanel、MediaTabBar、mediaActions/mediaStore/editActions、类型、对话框/API 与 Rust 图片类型/导入实现。

只读上游参考：

- `../palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MediaPanelView.swift`
- `../palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MediaTab/AssetThumbnailView.swift`
- `../palmier-pro-upstream/Sources/PalmierPro/Editor/ViewModel/EditorViewModel+MediaLibrary.swift`

实际修改仅以下文件：

1. `web/src/components/media/MediaPanel.tsx`：贴纸内容、导入/落轨协调；为共享卡片增加可选回调与禁用原因。
2. `web/src/components/media/MediaTabBar.tsx`：启用标签。
3. `web/src/components/media/MediaTabBar.test.tsx`：新增可导航标签后的转场导航步数。
4. `web/src/components/media/StickerPanel.test.tsx`：新增真实 MediaPanel/MediaCard DOM 交互测试。
5. `web/src/i18n/dict.ts`：贴纸中英文文案。
6. 本记录。

没有修改测试配置、版本、Rust、其他文档；没有 commit/push/release。现有并行未提交改动保留。

## 验证证据

TDD：先写 11 项 Sticker 交互用例，旧面板下 11 项失败（置灰、缺少卡片和动作入口）；实现后通过。补键盘导航/工程失败隔离等边界至 16 项；方向键用例先复现失败，再补网格导航容器与禁用预览判断后通过。另补用例复现旧导入错误遮盖新落轨错误，改为同时展示后通过。

最终执行：

```bash
pnpm --dir web test --run src/components/media/StickerPanel.test.tsx src/components/media/MediaTabBar.test.tsx src/components/media/MediaPanel.test.tsx src/store/editActions.test.ts
pnpm --dir web build
git diff --check -- web/src/components/media/MediaPanel.tsx web/src/components/media/MediaTabBar.tsx web/src/components/media/MediaTabBar.test.tsx web/src/i18n/dict.ts
```

- P2 修复后的最终结果：4 个测试文件 / 117 项通过，其中 Sticker 19 项（原 16 项 + 跨标签/工程隔离 3 项）。
- `tsc -b && vite build` 成功。仍有动态/静态 import 不能分块和大于 500 kB chunk 的构建告警；本切片未做打包重构。
- 最终 diff 已检查。未重复全量测试，留给主代理统一验证。
- 测试使用真实 React 卡片、Zustand 镜像和对话框/媒体 API 边界替身；Sticker DOM 用例中落轨 action 被 mock 来检验手势次数/失败门控，另运行既有真实 editActions 定向测试。不能把这些测试写成真实原生解码/撤销验收。

## 冻结 patch 审查 P2 修复

审查指出并经本地 DOM 用例复现：原 `active ? <StickerPanel /> : null` 在切换标签时卸载组件，重置 `placing` / `placementPending`，旧落轨尚未完成时返回已可再次提交；旧实例 `alive=false` 也会丢弃失败反馈。

最小修复：MediaPanel 对 StickerPanel 保持稳定挂载，传入 `active`；StickerPanel 完成所有 hooks 后在非活动状态返回 null，仅卸载卡片视图，不丢弃该工程的操作状态。父级继续用 epoch/path key 隔离工程，不新增全局 store。

本次新增 3 项回归：

- 可见期间失败和隐藏期间失败两种情形：落轨中切走再返回，导入/落轨按钮仍禁用，双击不重复提交；失败返回后可见；点击重试只新增一次调用，成功后解除 pending。
- 旧工程落轨在隐藏状态切换到新工程，新工程可独立提交；旧 promise 失败不显示到新工程，也不清除新工程的 pending。

TDD 证据：两个跨标签用例先在“返回后按钮仍禁用”断言失败；修复后原定向集合及新增用例全部通过（117 项），`pnpm --dir web build` 成功，原动态 import/大 chunk 告警仍在。测试 setup 对落轨 mock 使用 `mockReset`，防止失败用例未消费的 once promise 污染后续测试。

本次追加修改只有 `MediaPanel.tsx`、`StickerPanel.test.tsx` 和本记录，未改 Rust、配置、其他 MD；未运行全量测试。

## 待主代理候选包 GUI 验收

1. 在新 Beta 6 候选原生应用通过「导入贴纸」各导入透明 PNG、有效 Lottie JSON、`.lottie`，核对名称、缩略图/源预览、透明效果与动画播放；确认取消和无效 JSON 提示。
2. 单击、方向键、Tab 操作与选中按钮落轨；双击与拖拽各落轨一次，核对轨道/时长/合成画面；每次落轨执行一次 Undo、Redo，确认完整撤销/恢复本次操作。
3. 嵌套文件夹素材可见；将素材移走后确认离线状态及重新链接恢复。检查只读项目、生成/下载中及失败卡片提示。
4. 落轨 pending 时切走/返回，确认禁用持续、失败可见及重试一次；导入进行中切换面板/工程，核对旧结果/错误不覆盖新工程，连续点击不重复提交；检查中文/英文、窄面板滚动和屏幕阅读器导航。
5. 保存重开工程，确认贴纸仍来自同一媒体镜像，落轨片段及资源引用正常。

本任务未运行原生 GUI；不据此声明 Beta 6 已发布或全平台已验收。
