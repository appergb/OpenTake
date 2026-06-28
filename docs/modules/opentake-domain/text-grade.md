# 子系统：文字与调色（TextStyle / TextLayout / ColorGrade）

> 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)

文字片段的样式数据与（近似）布局度量，以及高端浮点调色链的参考实现。

## 职责

- 定义文字片段的样式值 `TextStyle`（字体/字号/颜色/对齐/阴影/背景/边框）、颜色 `Rgba` 与 hex 解析、`TextLayout` 自然尺寸**近似**估算。
- 定义在线性光空间运行的浮点调色 `ColorGrade`（曝光/白平衡/Lift-Gamma-Gain/对比/饱和）及其参考像素数学（render 层 WGSL 镜像之）。

不做：真实字形度量与文本栅格化（render 层 cosmic-text）、`NSColor`/`swiftUIColor`/字体解析等平台 UI 映射（前端/render 层）、调色着色器接入与对应 command（render/ops 层）。

## 关键类型与算法

源文件：[`text.rs`](../../../crates/opentake-domain/src/text.rs)、[`grade.rs`](../../../crates/opentake-domain/src/grade.rs)（调色部分；同文件的抠像/蒙版/Effect 见 [split-subtitle.md](split-subtitle.md)）

### 文字（text.rs）
- `Rgba { r, g, b, a }`：sRGB 直通 alpha，默认不透明白。`from_hex`：解析 `#RGB`/`#RRGGBB`/`#RRGGBBAA`（`#` 可选，3 位 nibble 复制如 `f`→`ff`），格式错误返回 `None`，1:1 复刻上游 `init?(hex:)`。
- `TextAlignment { Left, Center, Right }`，默认 `Center`，小写线上名。
- `Shadow { enabled, color, offset_x, offset_y, blur }`：默认启用、黑 `0.6` alpha、`offset_y = -2`、`blur = 6`。`Fill { enabled, color }`：可开关纯色（文本框背景/边框），默认关闭。
- `TextStyle { font_name, font_size, font_scale, color, alignment, shadow, background, border }`：默认 `Helvetica-Bold` / `96` / `scale 1` / 白 / 居中 / 阴影开 / 背景关 / 边框关 —— 全部对齐上游默认。
- `TextLayout::natural_size(content, style, max_width, canvas_height)`（**近似**）：`canvas_scale = canvas_height / 1080`，`render_size = font_size * font_scale * canvas_scale`；用固定 `APPROX_ADVANCE_FACTOR = 0.6` 估字宽、`APPROX_LINE_HEIGHT_FACTOR = 1.2` 估行高，贪心折行进 `max_width`；末端加阴影 padding（`SHADOW_PADDING(12) * 2`，仅阴影启用时）与 `+4` 余量，下限 1。

### 调色（grade.rs · ColorGrade）
- `Rgb { r, g, b }`：三通道乘子，默认恒等 `(1,1,1)`；`Rgb::zero()` = 加性恒等 `(0,0,0)`（lift 用）。
- `LiftGammaGain { lift, gamma, gain }`：ASC-CDL 风格三轮（lift 加性恒等 0、gamma 幂恒等 1、gain 乘性恒等 1）。单通道算子 `gain*(x+lift)` 后取 `^(1/gamma)`（gamma>0 且≠1 时，且对负值先夹 0 再取幂）。
- `ColorGrade { exposure, temperature, tint, lift_gamma_gain, contrast, saturation }`：默认全恒等（`is_identity()` 为真，render 层据此整段跳过）。`CONTRAST_PIVOT = 0.18`（场景线性 18% 灰）。
- `apply_linear(r,g,b)`（**线性光输入/输出，夹 `[0,1]`**，render 层 WGSL 镜像）：固定顺序 **曝光 → 白平衡 → Lift/Gamma/Gain → 对比 → 饱和**。
  - 曝光：线性增益 `2^exposure`。
  - 白平衡：`white_balance_gain()` 把 `temperature`/`tint`（各 `±1`，系数 `*0.25` 控制 ±25% 摆幅）化为乘性 RGB 增益（温度红↔蓝、色调绿↔品红）。
  - 对比：绕 `0.18` 枢轴，斜率 `1 + contrast`（枢轴为不动点）。
  - 饱和：保亮度向灰 lerp，`saturation` 倍（0=灰度、>1=增强），灰度用 `luma709`。

## 关键不变量与上游对齐点

- **`TextLayout` 是近似，非像素一致**：复刻了 `canvas_height/1080` 缩放基准、阴影 padding（`12*2`）与 `+4` 余量的**公式形状**，但宽度与上游 CoreText 不一致，render 层文本引擎（cosmic-text）必须重算（见 [OVERVIEW.md](OVERVIEW.md) 完成状态、`MODULE-PORT-MAP.md` 文字度量 needs-replacement）。
- **hex 解析逐字节复刻**：3/6/8 位长度、单 nibble 复制、容错返回 `None`，与上游一致。
- **调色顺序锁定**：曝光→白平衡→LGG→对比→饱和，且**在线性光空间**运行（render 层负责 BT.709↔线性转换包裹）。顺序错或空间错都会偏色。
- **恒等即 no-op**：所有调色默认值构成恒等变换，`default()` 不改像素（有单测）；这是 render 层跳过优化的前提。
- **`f64` 精度**：domain 用 `f64` 与采样层一致，GPU 侧消费 `f32`（8-bit 输出下精度损失无关）。
- **serde**：`TextStyle`/`Rgba`/`Shadow`/`Fill` camelCase（`fontName`/`offsetY`…）缺键回退默认；`ColorGrade`/`Rgb`/`LiftGammaGain` camelCase（`liftGammaGain`…），缺字段解码为恒等。

## 与其他子系统关系

- `TextStyle`/`text_content` 是 [timeline-model.md](timeline-model.md) 的 `Clip` 文字字段；`TextStyle` 也是 [split-subtitle.md](split-subtitle.md) caption-group 批量同步的目标值。
- `ColorGrade` 与同在 `grade.rs` 的 `ChromaKey`/`Mask`/`Effect` 同属 `Clip` 的进阶特效字段；抠像/蒙版/通用特效的参考数学见 [split-subtitle.md](split-subtitle.md)。
- `luma709`、`smoothstep01` 等 `grade.rs` 共享数值助手被调色与抠像/蒙版共用。

---

页脚：[INDEX.md](INDEX.md)
