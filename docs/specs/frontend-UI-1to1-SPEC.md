# 前端 React UI —— 1:1 复刻上游 实现就绪规格 (Issue #12)

> **状态**:v1 硬要求规格。**已有功能的 UI 与交互必须 1:1 复刻上游 palmier-pro。**
> **范围**:`web/`（React + TypeScript + Vite + Zustand）的全部可视层与交互层。**不含** Rust core / Tauri command 的内部实现（只定义对接契约）。
> **证据基准**:全部数值/行为均引自 `palmier-pro-upstream/Sources/PalmierPro/`，并标注 `文件:行号`。凡本规格与上游源码冲突，**以上游源码为准**。
> **方法**:上游本质是「单一可观测状态容器 `EditorViewModel` + 帧本位不可变值模型 `Timeline` + AppKit 投影」。UI 是纯消费者（读 store、发命令）。本规格把这层「投影 + 手势」精确转写为 React。

---

## 目录

0. [总体复刻原则与验收方式](frontend/0-principles.md)
1. [设计令牌表（AppTheme → CSS variables）](frontend/1-design-tokens.md)
2. [窗口外壳与五面板布局](frontend/2-layout.md)
3. [组件地图（上游视图 → React 组件）](frontend/3-components.md)
4. [Toolbar（工具条）](frontend/4-toolbar.md)
5. [Timeline（时间线）—— 核心](frontend/5-timeline.md)
6. [Inspector（检查器）](frontend/6-inspector.md)
7. [MediaPanel（媒体面板）](frontend/7-media-panel.md)
8. [Preview（预览）](frontend/8-preview.md)
9. [交互细节逐项清单（1:1 关键）](frontend/9-interactions.md)
10. [Zustand 状态结构（只读镜像 + UI-only 态）](frontend/10-state.md)
11. [Tauri command / event 对接点](frontend/11-tauri.md)
12. [数据模型镜像（TS 类型）](frontend/12-data-models.md)
13. [实施清单与 1:1 验收方式](frontend/13-implementation.md)
