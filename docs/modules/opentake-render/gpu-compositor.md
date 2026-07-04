# GPU 合成器 — wgpu 逐帧合成 render_to_rgba

> 上级：本模块目录 [INDEX.md](INDEX.md)

## 职责

把单帧 `FramePlan` 画成一张画布 RGBA8 帧：为每个 `LayerDraw` 画一个带仿射变换的纹理 quad，按序预乘 alpha-over 叠到离屏 render target，再回读为像素。这是上游全部委托给 AVFoundation `AVVideoComposition` 黑盒、OpenTake **从零自建**的部分——也正因自建，特效/调色/抠像/蒙版这类「在片元着色器对像素做数学」的能力天生可做（上游做不到，ADVANCED-FEATURES A 层）。

**单一 render pipeline**：quad 是 4 个常量顶点（triangle-strip），所有几何都在 uniform 的仿射里；每个 draw 只换一个 bind group（纹理 + uniform）。

## 关键类型与算法

### 设备（`gpu/device.rs`）
`RenderDevice { device, queue }`，`try_new()` 经 `pollster::block_on` 同步获取适配器/设备，**无 GPU 时返回 `Err(RenderError::NoAdapter)` 而非 panic**——CI/headless/沙箱里测试据此优雅跳过（host-capability 门控，非 `should_panic`）。macOS 上跑 Metal。

### 纹理（`gpu/texture.rs`）
- `GpuTexture { texture, view, width, height }`，`Rc` 引用计数让缓存条目与在飞 draw 共享。
- `upload_rgba(device, queue, frame, srgb, label)`：上传 `DecodedFrame` 为 RGBA8 纹理。`srgb=true` 用 `Rgba8UnormSrgb`（硬件 sRGB 解码、采样返回线性）；PoC 在 sRGB 非线性域混合，调用方传 `srgb=false` 直接采样原始字节。
- `TextureCache`：content-hash → `Rc<GpuTexture>` 的 LRU（容量下限 1），防显存膨胀。图片/文字/Lottie 帧长期缓存；视频帧不长缓存、当前帧按需上传。

### 合成器（`gpu/compositor.rs`）
`Compositor { pipeline, bind_group_layout, sampler }`，`new(device)` 构建：
- 着色器从 `shader.wgsl` `include_str!` 编入。
- **预乘 alpha-over blend**（SPEC §3.6）：color 与 alpha 均 `src_factor=One, dst_factor=OneMinusSrcAlpha, op=Add`。
- sampler：`linear` + `ClampToEdge`（crop 子矩形边缘 clamp，防越界采样）。
- 工作色彩格式 `RT_FORMAT = Rgba8Unorm`：PoC 存原始编码字节直接混合，最贴近 AVFoundation；回读即这些字节。

`render_to_rgba(device, queue, size, frame_plan, resolver)`：
1. 建离屏 RT（`RENDER_ATTACHMENT | COPY_SRC`）。
2. **预先**为每个 draw 解析纹理（经 `TextureResolver`）+ 组装 uniform + 建 bind group（让 render pass 借用干净，`Rc` 纹理保活到 pass 结束）。纹理解析不出（离线/不可处理源）则跳过该 draw，等价上游离线处理。
3. clear 成 `frame_plan.clear_rgba`（不透明黑），按序逐 draw `set_bind_group` + `draw(0..4)`（后者在上）。
4. RT → buffer（行 256 对齐 `COPY_BYTES_PER_ROW_ALIGNMENT`）→ `map_async` 回读 → 去对齐填充拷成紧凑 RGBA → 返回 premultiplied `DecodedFrame`。

`TextureResolver` trait：`resolve(source, source_frame) -> Option<Rc<GpuTexture>>`，让合成器**与解码无关**——集成层（或测试）按 `TextureSource` + 源帧供像素（通常 `FrameProvider` + `TextureCache`，见 [source-size.md](source-size.md)）。

### 着色器（`gpu/shader.wgsl`）
- **顶点**：quad `[0,1]²` → 乘 `nat` 得源像素 `[0,natW]×[0,natH]` → 行向量仿射 `p'=p·M`（CG 语义）映到画布像素（原点左下 y 上）→ NDC（wgpu NDC y 也向上，几何无需额外翻 y）。UV 取 crop 子矩形并**翻 v 一次**（`1-v`，对齐「纹理行 0=顶部」与「y 上」）；另算 `canvas_uv`（原点左上 y 下）供蒙版求值。
- **片元像素链**（顺序固定，1:1 镜像 `opentake_domain::grade`，进阶效果落地部分）：
  1. 取样后统一回到**直通（非预乘）**色：`FLAG_PREMULTIPLY` 决定是否需我方预乘（直通源）或先 un-premultiply（已预乘源），保证整条链数学无歧义。
  2. **绿幕抠图**（`FLAG_CHROMA`）：CbCr 色度距离 `smoothstep` 出 matte 缩 alpha + spill 抑制。
  3. **调色**（`FLAG_GRADE`）：调色定义在线性光，故 `srgb_to_linear` → `apply_grade_linear`（曝光 2^stops → 白平衡逐通道增益 → Lift/Gamma/Gain → 0.18 pivot 对比 → 709 luma 保亮饱和）→ `linear_to_srgb`。白平衡已在 CPU 端预解为逐通道增益。
  4. **蒙版**（线性/圆形）：每个 SDF 出覆盖（羽化 `smoothstep01` + 反相），多蒙版取交（乘积）缩 alpha；上限 `MASK_CAP=4`。
  5. 末尾预乘一次 + 全局 `opacity`（预乘下同时缩 rgb 与 a）。
- uniform 全部按 `vec4` 对齐，Rust 端 `Uniforms`/`MaskGpu`（`bytemuck::Pod`）与 WGSL `struct U`/`MaskGpu` 字段顺序逐一对应；flag 位 `f32::from_bits` 打包进 `canvas_op_flags.w`。

## 源文件
- [`crates/opentake-render/src/gpu/compositor.rs`](../../../crates/opentake-render/src/gpu/compositor.rs) — `Compositor` / `render_to_rgba` / `TextureResolver` / uniform 打包 / 回读。
- [`crates/opentake-render/src/gpu/device.rs`](../../../crates/opentake-render/src/gpu/device.rs) — `RenderDevice::try_new`。
- [`crates/opentake-render/src/gpu/texture.rs`](../../../crates/opentake-render/src/gpu/texture.rs) — `GpuTexture` / `upload_rgba` / `TextureCache`（LRU）。
- [`crates/opentake-render/src/gpu/color.rs`](../../../crates/opentake-render/src/gpu/color.rs) — `srgb_to_linear` / `linear_to_srgb`（IEC 61966-2-1，备线性光增强）。
- [`crates/opentake-render/src/gpu/shader.wgsl`](../../../crates/opentake-render/src/gpu/shader.wgsl) — 顶点 + 片元（投影约定 + 调色/抠像/蒙版数学）。
- [`crates/opentake-render/src/gpu/mod.rs`](../../../crates/opentake-render/src/gpu/mod.rs) — `RenderError` + re-export。

## 不变量
- **投影约定是像素 diff 命脉**：行向量左乘 `p'=p·M`、画布原点左下/y 上、纹理 v 翻转**只在着色器一次**——与上游 `CGAffineTransform`/`CGPoint.applying` 逐像素一致，不得擅改半像素中心或加额外 y 翻转（SPEC §1.3/§3.3/§3.4）。
- **uniform 的 `natW/natH` 必须是 `LayerDraw.nat_size`**（构 affine 所用），不是 `tex.width/height`；UV 在 0..1 采样，纹理真实像素尺寸与几何无关（#125）。
- **预乘混合**：所有源在片元里收敛成预乘再 over；合成器输出 `DecodedFrame.premultiplied = true`。
- **解析不出的纹理 = 不贡献像素**，不报错（对齐上游离线处理）。
- **Rust POD ↔ WGSL 布局必须同步**：改 uniform 字段时 `Uniforms`/`MaskGpu` 与 `struct U`/`MaskGpu`、`MASK_CAP`、flag 位三处齐改。

## 关系
- 输入 `FramePlan` 来自 [render-plan.md](render-plan.md)；其中 `color_grade/chroma_key/masks` 在片元链消费。
- 纹理像素经 `TextureResolver`（通常包 [source-size.md](source-size.md) 的 `FrameProvider` + `TextureCache`）注入；文字 clip 的纹理来自 [text-rasterizer.md](text-rasterizer.md) 的预乘 RGBA。
- 输出 `DecodedFrame` 给导出后端编码（src-tauri #112）或预览贴 canvas（计划中 P1-9）。

## 计划中
- **多边形（钢笔）蒙版**：变长点列不适配固定 uniform，当前编码为全画布 no-op；穿 storage buffer 是 render 侧 TODO（domain 已存储且单测）。
- **`effects: Vec<Effect>` 链**：透传到 `LayerDraw`，尚无对应片元 pass；转场（相邻 clip 重叠区 pass）亦未实现。
- **线性光混合**：当前 sRGB 非线性域混合贴上游；线性光（RGBA16F）为增强项，仅在像素 diff 通过后切换（`color.rs` 已备转换）。

---

> 上级：本模块目录 [INDEX.md](INDEX.md)
