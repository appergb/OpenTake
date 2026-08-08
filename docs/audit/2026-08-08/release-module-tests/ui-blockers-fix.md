# UI release blockers 修复记录

日期：2026-08-08（Asia/Shanghai）
范围：`UI-A11Y-01`（MediaCard 提取音频键盘路径）、`UI-A11Y-02`（Export alert、Library / Media / Preview empty state 对比度）及 final Web reviewer 的 popup keyboard/focus follow-up。
结论：**PASS — 两类 release blocker 与 reviewer MEDIUM 已由 RED→GREEN 回归覆盖并通过 Web 发布检查。**

## 修复结果

### UI-A11Y-01 — 零 hover 提取音频

- 可提取视频的“提取音频”按钮现在常驻 DOM 和 Tab 顺序，不再由 `hovered` 决定是否挂载。
- idle 状态保持 `opacity: 0` 与 `pointer-events: none`；鼠标 hover 或键盘焦点进入卡片后显现。键盘焦点不受 `pointer-events` 限制。
- 按钮仍使用原 24×24 尺寸、原位置、原图标和原保存/提取路径；键盘激活产生的 click 复用 `stopPropagation`，不会误触卡片预览。
- 新测试从零 hover 挂载卡片，证明按钮存在且可聚焦、idle 不可见、focus 后显现，并实际执行 `extractAudio(mediaId, path)`。

视觉权衡：每个可提取视频在 Tab 顺序中新增一个必要的等价操作，但 unfocused / unhovered 卡片不增加任何可见图标；卡片获得键盘焦点时显示动作属于可见焦点反馈，而非常驻视觉噪声。

### UI-A11Y-02 — 正常信息与错误小字对比度

| 场景 | 修复前实际组合 | 修复后实际组合 | AA 门槛 |
|---|---:|---:|---:|
| Export error alert | `--status-error` / 红色 8% tint + `--bg-elevated`：**3.307:1** | `--text-primary` / 同一 tint：**12.485:1** | 4.5:1 |
| Library / Media / Preview 信息空态 | `--text-muted` / `--bg-surface`：**3.115:1** | `--text-tertiary` / `--bg-surface`：**7.474:1** | 4.5:1 |

- 没有改全局 token；只把承载正常信息的三个 empty state 切到既有 `--text-tertiary`。
- Export alert 保留原红色 tint 背景、`role="alert"` 和 live-region 语义，只把小字前景切到 `--text-primary`。
- 字号、字重、内边距、换行规则、按钮布局均未改变。
- 测试 helper 按 WCAG 相对亮度公式解析 token，并先做前景/背景 alpha 合成，再断言实际组合 `>= 4.5`。

### Final reviewer MEDIUM — popup roving focus 与退出语义

- Media Import / Sort / Filter 的 `menuitem` / `menuitemradio` 和 Preview BadgeMenu 的 `option` 均使用真正 roving tabindex：任一时刻只有当前焦点项为 `tabIndex=0`，其他项为 `-1`；Arrow / Home / End 移动焦点时同步迁移 Tab stop。
- Tab 或 focusout 离开 popup 时关闭弹层并把 trigger 的 `aria-expanded` 更新为 `false`，但不把焦点错误抢回 trigger；默认 Tab 去向或明确的外部焦点得以保留。
- Escape 以及 Enter / Space 选择继续关闭并恢复 trigger 焦点；Preview option 新增显式 Enter / Space 激活，测试真实选择回调与恢复行为。
- Media popup 和 Preview BadgeMenu 各自广播同类 popup 打开事件；即使辅助技术直接触发另一个 trigger 的 click（没有 pointer mousedown），旧弹层也会无恢复地关闭，DOM 中始终最多一个同类 popup。
- 没有改变 popup 尺寸、位置、颜色或动画，也没有触碰 API、workflow、scheduler、Rust 或 playback capability。

Reviewer follow-up 的独立 RED：

- Media Import 当前得到 `[0, 0]`，预期单一 Tab stop `[0, -1]`。
- BadgeMenu 执行 End 后焦点已到最后一项，但 tabIndex 仍为 `[0, -1, -1]`，预期 `[-1, -1, 0]`。
- Media / Badge 的 Tab 后 popup 仍在 DOM 且 `aria-expanded=true`。
- 辅助技术 click 依次激活两个 trigger 后，Media menu 与 Badge listbox 均同时存在 2 层，预期 1 层。

Reviewer follow-up 的 GREEN：

- capability 定向组：5/5 PASS。
- focused：`MediaPanel.test.tsx` + `Preview.interaction.test.tsx`，2 文件 49/49；连续运行 3 次均通过（pass@1 / pass@3 / pass^3 均为 **1.00**）。
- 相关 UI：8 文件，111/111 PASS。
- 全 Web：138 文件，1157/1157 PASS。
- 对比度/键盘 blocker 定向组：6/6 PASS。
- `pnpm build`：PASS，`tsc -b` + Vite，1703 modules transformed。

## TDD 证据

### 基线

修复前 focused 基线：5 文件，96/96 tests PASS。

### RED

命令：

```bash
cd web
pnpm exec vitest run \
  src/components/media/MediaPanel.test.tsx \
  src/components/media/LibraryView.test.tsx \
  src/components/preview/Preview.test.tsx \
  src/components/shell/ExportDialog.test.ts \
  -t 'keyboard-reachable|WCAG AA'
```

旧实现按预期失败：

- MediaCard：零 hover 查询提取按钮得到 `null`。
- Export alert：received `3.307171079379349`, expected `>= 4.5`。
- Library / Media / Preview：received `3.114868963283787`, expected `>= 4.5`。

失败来自缺失行为/不足对比度，不是测试语法、mock 或环境错误。

### GREEN 与稳定性

- capability/regression 定向组首轮：6/6 PASS（5 个新增回归 + 1 个既有 CTA 对比度回归，pass@1 = **1.00**）。
- 原始 blocker focused 模块组：8 文件，108/108；连续独立运行 3 次均通过（pass@3 = **1.00**，pass^3 = **1.00**，3/3）。后续 reviewer follow-up 的最新总数见上节。
- 原始 blocker 全 Web：138 文件，1152/1152 PASS；reviewer follow-up 后最新为 1157/1157。
- `pnpm build`：PASS，`tsc -b` + Vite，1703 modules transformed。
- `git diff --check`：PASS，无 whitespace error。

构建仍报告项目既有的 ineffective dynamic import 与单个约 859.14 kB chunk 提示；没有 TypeScript/Vite 错误。本轮初次构建曾发现 test-only 对比度 helper 位于 `src/` 会进入产品 `tsconfig`，已移到 `web/test/`，没有新增依赖或产品 bundle 代码。

## 修改文件

- `web/src/components/media/MediaPanel.tsx`
- `web/src/components/media/MediaPanel.test.tsx`
- `web/src/components/media/LibraryView.tsx`
- `web/src/components/media/LibraryView.test.tsx`
- `web/src/components/preview/Preview.tsx`
- `web/src/components/preview/Preview.test.tsx`
- `web/src/components/shell/ExportDialog.tsx`
- `web/src/components/shell/ExportDialog.test.ts`
- `web/test/contrast.ts`
- `docs/audit/2026-08-08/release-module-tests/ui-blockers-fix.md`

指定文件中原有 scheduler、tab、focus、playback capability 等未提交改动均保留；未触碰 Rust、CI/release workflow，未提交或推送。

## 限制

- 本轮以 DOM 行为测试、token alpha 合成回归和生产构建为发布证据；未新增真实 Chromium accessibility-tree、VoiceOver 或新截图证据。
- **需要一次小范围独立 re-review**：仅复核 Media Import / Sort / Filter 与 Preview BadgeMenu 的 roving tabindex、Tab/focusout 与焦点恢复语义，以关闭 final reviewer 的 MEDIUM 循环；不需要重新执行完整 UI 视觉/对比度审计。
