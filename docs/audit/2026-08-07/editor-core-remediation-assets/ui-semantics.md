# UI-03 / UI-04 语义与键盘修复证据

日期：2026-08-08
范围：素材面板、素材主/副标签、预览标签、预览设置 BadgeMenu。此次仅补语义、焦点与键盘契约，没有改版、扩写文案或改动 Rust/调度器。

## RED

先加入 7 个新测试并加强 1 个既有预览测试。修复前同一轮全 Web 结果为：

- 137 个测试文件中 4 个失败。
- 1139 个测试中 1131 通过、8 失败。
- 失败分布：MediaPanel 3、MediaTabBar 2、Preview 3。
- 失败原因与审计一致：搜索没有 `type=search`/明确名称；Import/Sort/Filter 没有 popup ARIA 与焦点键盘路径；breadcrumb 点击目标不足 24px；素材 tabs 没有 roving/controls/panel；Preview 没有 tablist/tabpanel；BadgeMenu 不管理打开焦点与 Escape 恢复。

## GREEN

| 检查项 | 修复结果 |
|---|---|
| 素材搜索 | `type="search"`，并以本地化搜索文案提供 `aria-label` |
| Import / Sort / Filter | trigger 提供 `aria-haspopup`、`aria-expanded`、`aria-controls`；打开后焦点进入；支持 ArrowUp/ArrowDown/Home/End；Escape 关闭并恢复 trigger 焦点 |
| Breadcrumb | 所有可点击层级至少 24×24 CSS px；返回按钮保持 24×24 |
| 素材主 tabs | `tablist/tab/tabpanel`、`aria-controls`/`aria-labelledby`、单一 tab stop；方向键与 Home/End roving，跳过禁用 tab |
| 素材副 tabs | 同样提供 roving tab 与对应动态 tabpanel |
| Preview tabs | 时间线/来源使用 `tablist/tab/tabpanel`；Home/Arrow 返回时间线，焦点与选择同步 |
| BadgeMenu | trigger/listbox 关联；打开聚焦活动项；ArrowUp/ArrowDown/Home/End roving；Escape 关闭并恢复 trigger |

验证结果：

- 聚焦测试：4 文件、62/62 通过。
- 全 Web：137 文件、1139/1139 通过。
- `pnpm build`：通过（TypeScript build + Vite production build）。仅输出项目既有的大 chunk / dynamic import 提示，无新增构建错误。
- `git diff --check`：通过。

## 范围纪律

- `HoverButton` 只增加 popup ARIA、button ref 与 key handler 透传。
- 保留资源调度 Agent 已加入的 `derivedResourceScheduler`、缩略图、波形与 poster 变更，没有回滚或改写其逻辑。
- 未触碰 LibraryView、ExportDialog、scheduler 文件、Rust、主题 token 与全局 CSS。

## TypeScript reviewer follow-up

统一 reviewer 追加确认了两个 MEDIUM，均按 TDD 修复：

1. 菜单项由键盘 Enter/Space 选择时，原实现只关闭菜单，没有把焦点恢复到 Import/Sort/Filter trigger。新增失败测试后，menu item 的 Enter/Space 统一走 action，并由所属菜单执行 close-and-restore。
2. 非活动主/副 tab 的 `aria-controls` 原来指向未挂载的 panel。现在所有受控 panel 节点始终存在；非活动节点为 `hidden` 且不挂载重内容，活动 panel 保持原渲染与 flex 布局。

Follow-up RED：MediaPanel 2 个测试失败。
Follow-up focused GREEN：MediaPanel + MediaTabBar 共 42/42 通过。

Follow-up 最终门禁：

- 全 Web：137 个测试文件、1143/1143 通过。
- `pnpm build`：通过（包含 `tsc -b` 与 Vite production build）；仅项目既有 bundle 提示。
- `git diff --check`：通过。

资源调度器同时收紧 typed-kind 合约；按协调仅在本文件责任内补齐：卡片缩略图使用 `derivedResourceKinds.thumbnail`，音频波形使用 `derivedResourceKinds.waveform`。未修改 scheduler 实现。
