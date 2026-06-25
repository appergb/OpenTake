# 数据模型镜像（TS 类型）

> 来源:`Models/Timeline.swift` / `Keyframe.swift` / `ClipType.swift`。**字段名与 Rust serde / 工程 JSON 保持一致**(ARCHITECTURE §4)。前端 TS 类型 = 镜像反序列化目标。**前端不实现派生算法**(在 Rust),但需要少量纯 UI 派生(clip rect / 标签),可读这些字段。

```ts
type ClipType = 'video' | 'audio' | 'image' | 'text' | 'lottie';
type Interpolation = 'linear' | 'hold' | 'smooth';

interface Timeline {              // Timeline.swift:9-23
  fps: number;                    // 默认 30
  width: number;                  // 默认 1920
  height: number;                 // 默认 1080
  settingsConfigured: boolean;
  tracks: Track[];
}

interface Track {                 // Timeline.swift:25-59
  id: string;
  type: ClipType;
  muted: boolean;                 // 默认 false
  hidden: boolean;                // 默认 false
  syncLocked: boolean;            // 默认 true
  clips: Clip[];
  // displayHeight 不在 JSON, 前端 UI 态(默认 50, 范围 32..200)
}

interface Keyframe<V> {           // Keyframe.swift:7-11
  frame: number;                  // ★ 存储用 clip 相对偏移
  value: V;
  interpolationOut: Interpolation; // 默认 smooth
}
interface KeyframeTrack<V> { keyframes: Keyframe<V>[]; }
interface AnimPair { a: number; b: number; } // 位置(x,y)/缩放(w,h), Keyframe.swift:53-63

interface Transform {             // Timeline.swift:364-498
  centerX: number; centerY: number; // 默认 0.5/0.5
  width: number; height: number;    // 默认 1/1 (归一画布比例)
  rotation: number;                 // 度, 顺时针正
  flipHorizontal: boolean; flipVertical: boolean;
}
interface Crop {                  // Timeline.swift:501-510 (归一边距 0..1)
  left: number; top: number; right: number; bottom: number;
}

interface Clip {                  // Timeline.swift:75-117
  id: string;
  mediaRef: string;               // = asset id, 永不存路径
  mediaType: ClipType;            // 默认 video
  sourceClipType: ClipType;       // 色彩用, 默认 video
  startFrame: number;
  durationFrames: number;
  trimStartFrame: number;         // 默认 0
  trimEndFrame: number;           // 默认 0
  speed: number;                  // 默认 1.0
  volume: number;                 // 默认 1.0 (线性)
  fadeInFrames: number; fadeOutFrames: number; // 默认 0
  fadeInInterpolation: Interpolation;  // 默认 linear
  fadeOutInterpolation: Interpolation; // 默认 linear
  opacity: number;                // 默认 1.0
  transform: Transform;
  crop: Crop;
  linkGroupId?: string;           // A/V 链接组
  captionGroupId?: string;
  textContent?: string;           // text clip
  textStyle?: TextStyle;          // (见 TextStyle.swift)
  opacityTrack?: KeyframeTrack<number>;
  positionTrack?: KeyframeTrack<AnimPair>;
  scaleTrack?: KeyframeTrack<AnimPair>;
  rotationTrack?: KeyframeTrack<number>;
  cropTrack?: KeyframeTrack<Crop>;
  volumeTrack?: KeyframeTrack<number>; // 值是 dB
}
```

**前端需要的派生（纯 UI，读字段即可）**：
- `endFrame = startFrame + durationFrames`（`Timeline.swift:119`）。
- clip 时长时间码（标签栏用）。
- 是否 linked（`linkGroupId != null` → 标签下划线）。
- **关键帧偏移↔绝对帧**:存储是相对偏移,绘制 clip 上的关键帧标记要 `+startFrame`（`ClipRenderer.swift:165-169`）;Inspector 关键帧控件用绝对帧。
- 采样/插值(`*_at`、smoothstep、fade)**全部在 Rust 算**;前端预览帧来自 Rust,Inspector 显示的"当前帧值"可由 Rust 提供或前端按同公式算(若需即时反馈)。**若前端算,必须逐字复刻** `KeyframeTrack.sample`(端点 clamp 无外插 + 按左端点 interpolationOut，`Keyframe.swift:231-250`)、`smoothstep(t)=t*t*(3-2t)`(`:40`)、`fadeMultiplier`(in/out 取 min，`Timeline.swift:211-226`)。

**VolumeScale**（音量 dB↔线性，`Models` 内 `VolumeScale`）：floor=-60dB（→线性 0 硬截断）、ceiling=15dB；`dbFromLinear`/`linearFromDb`。Inspector/橡皮筋显示用（`ClipRenderer.swift:236,314`）。前端需取这两个常量与转换（从 Rust 取或复刻）。
