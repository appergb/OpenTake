# preview-ui — 预览与播放（components/preview）

> 上级：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 覆盖 `web/src/components/preview/`。预览面板按**单表面 + 单时钟**模型工作：播放/暂停/拖拽共用同一组 `<video>/<audio>` DOM 元素，由唯一一个 `requestAnimationFrame` 循环驱动（对标上游 `AVPlayerLayer`）。

---

## 一句话职责

把后端 `timeline` 镜像在当前帧合成出画面与声音，并以单一时钟推进播放头（`uiStore.activeFrame`），支持单素材预览与时间线合成两种模式。

---

## 单表面 + 单时钟（核心设计）

- **单表面**：合成不靠每帧调 Rust `composite_frame` 出 PNG，而是直接挂载浏览器解码的 `<video>/<audio>` 元素。播放、暂停冻结帧、拖拽预览都用同一套元素，避免渲染器切换带来的颜色/尺寸跳变。
- **单时钟**：一个 rAF 循环是所有播放的唯一权威，消除双时钟仲裁。三态：
  - **PLAY**：主时钟从活跃音频元素读 `currentTime` 推进 `activeFrame`，其余元素跟随。
  - **SCRUB**：所有元素暂停并实时 seek 到拖拽帧（无声）。
  - **PAUSE**：所有元素冻结在当前解码帧。

> Rust 的 `compositeFrame` 仍保留（GPU 合成一帧 PNG，见 [ipc-api.md](ipc-api.md)），用于取静帧等场景；常态播放不走它。

## 模块构成

- `previewEngine.ts`：单一真理时钟（SPEC §8.4）。导出 Hook `useTimelinePlaybackEngine()`，在 `App.tsx` 顶层挂载一次；内部用一个 rAF 循环按 PLAY/SCRUB/PAUSE 统一调度所有注册的媒体元素并推进播放头。
- `timelinePlayback.ts`：纯逻辑库（无副作用），被引擎与播放层共用——`ActiveMedia` 类型、`activeVisualClips()`/`activeAudioClips()` 选择器、`sourceTimeSec()`（帧→源秒）、`advancePlayhead()`（单 tick 推进）、`clipVolume()` 等。
- `TimelinePlaybackLayer.tsx`：被动的 DOM 元素注册器，**不拥有时钟**。按当前帧挂载活跃视频/音频片段为 `<video>/<audio>`，把每个 ref 注册进共享 `previewElements` 表，并渲染剪裁遮罩与媒体变换。
- `previewLayerStyles.ts`：纯样式计算，从 `Clip` + `activeFrame` 采样出每片段的 CSS（位置/缩放/旋转/翻转/不透明度、剪裁遮罩、媒体变换补偿）。
- `Preview.tsx`：预览面板主容器。顶部 Timeline/Media 标签 + 画布区 + 搜索条 + 运输控制条。单素材模式直接用 HTML5 `<video>/<audio>/<img>` 解码；时间线模式挂 `TimelinePlaybackLayer`。订阅 `uiStore`（`activeFrame`/`isPlaying`/`isScrubbing`/`previewMediaId`），消费 `projectStore.timeline` 与 `mediaStore.items`。

## 数据流

播放头 seek：标尺刮擦或键盘 →（`uiStore.setActiveFrame` / `setScrubbing`）→ 引擎进入 SCRUB → 元素 seek → 松手回 PAUSE/PLAY。播放：`togglePlay` → 引擎 PLAY → 主音频时钟推进 `activeFrame` → 所有订阅组件（预览、时间线播放头、关键帧面板）跟随重渲染。组件只渲染状态、不持时钟逻辑。

---

## 完成状态

- **已实现**：单时钟引擎（三态）、单素材与时间线合成两种模式、变换/剪裁/不透明度采样、音量、拖拽实时预览、播放到末尾回绕。
- **计划中**：与 `opentake-render` 的像素级一致性核对、`compositeFrame` 取静帧的更多接线；高负载多轨合成的流式引擎是后续工作。

## 相关文档

- 播放头/选择等 UI 态来源 → [state-stores.md](state-stores.md)
- `compositeFrame`/`getWaveform` 封装 → [ipc-api.md](ipc-api.md)
- 预览规格 → [SPEC.md](SPEC.md)（§8 Preview）
- 后端渲染对端 → [../opentake-render/INDEX.md](../opentake-render/INDEX.md)

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
