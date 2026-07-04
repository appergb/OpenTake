# 2026-06-27 UI Refresh Summary

## 更新范围

- 首页启动页与最近项目展示
- 编辑器标题栏按钮对齐
- 设置界面布局与视觉规范
- 前端视觉约束测试

## 更新内容

- 首页采用内嵌式深色视觉，并放大左上角 OpenTake logo。
- 有项目时保留左上宣传区，项目列表紧跟其下并按四列展示。
- 修复编辑器标题栏 Home / 菜单按钮与 macOS 交通灯不在同一平面的问题。
- 设置界面改为宽面板布局，左侧分类导航，右侧只显示当前分类内容。
- 新增首页、设置页、标题栏的视觉约束测试，防止后续回退。

## 更新原因

- 统一首页、编辑器、设置界面的 Vercel 风格与内嵌式 UI 方向。
- 减少不必要的分割线、厚重边框和色块。
- 让有项目状态下的首页同时具备品牌介绍和项目入口效率。
- 让设置界面更接近甲方参考图中的左侧导航结构。

## 验证

```bash
pnpm -C web test --run src/components/home/HomeView.visual.test.ts src/components/settings/SettingsView.visual.test.ts src/components/shell/TitleBar.visual.test.ts
pnpm -C web build
./web/node_modules/.bin/tauri build --bundles app
```

结果：测试通过，前端构建通过，macOS OpenTake.app 打包并预览通过。
