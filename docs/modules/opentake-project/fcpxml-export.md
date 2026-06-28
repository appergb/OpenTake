# fcpxml-export — 时间线导出（XMEML 4 / FCP7 XML）

> 上级：本模块目录 [INDEX.md](INDEX.md)

源文件：[`fcpxml.rs`](../../../crates/opentake-project/src/fcpxml.rs)。1:1 端口自上游 `Export/XMLExporter.swift`。

## 职责

把内存中的 `Timeline` 序列化为**时间线交换 XML**，供 Premiere Pro / DaVinci / Final Cut 导入。覆盖片段位置与裁剪、变速、音量（静态 + 关键帧）、不透明度、变换（缩放 / 旋转 / 位置）、裁切、淡入淡出、链接的 A·V 片段、源帧率 NTSC 判定。

### 命名 vs 产物（重要）

任务约定的命令 / 前端名沿用 `export_fcpxml` / `exportFcpxml`，但**本模块的公开入口是 [`export_xmeml`]，产物是 XMEML 4（`.xml`，即 FCP7 XML），不是 FCPXML**。原因（源码注明）：上游两种交换格式 XMEML 与 FCPXML，FCPXML 更新但 **Premiere Pro 不原生支持**——选 FCPXML 用户得拿 DaVinci 当桥或第三方转换。Premiere 是当前优先级，故上游（及本端口）选了已弃用但 Premiere/DaVinci/FCP 都能读的 XMEML。

## 关键类型与结构

- **`export_xmeml(timeline, manifest, project_base) -> String`** — 纯函数入口：构造 domain 的 `MediaResolver`，交给 `Builder` 产出完整 XML 文本。
- **`Builder<'a>`** — 真正的构建器，持有所有发射状态：`emitted_files`（已发文件去重集）、`clip_addresses`（clip id → 媒体类型内地址，供 link 交叉引用）、`clips_by_link_group`（link 组 → 片段）、`fps` / `seq_width` / `seq_height`。
- **`XmlNode`（私有）** — 整份文档建成一棵节点树；`render` **独占**全部缩进与转义（步长 2 空格），任何片段不自带空白。辅助构造器：`el` / `el_attrs` / `leaf` / `leaf_i` / `boolean`，渲染 `render` + `escape_xml`。

源文件按 `// MARK:` 分段，自上而下即文档结构：Document shell → Tracks→clipitems → File elements → Links → Transitions(fades) → Filters → Indexing helpers → Effect & parameter builders → XML rendering。

## 关键算法（与上游对齐的要点）

- **`seconds_to_frame(s, fps)`** — `(s * fps) as i32`，**截断取整**，1:1 对应上游 `secondsToFrame`（`Int(seconds*fps)`），不是四舍五入。
- **轨道顺序（关键坑）** — 视频轨：模型存 top→bottom，FCP XML 要求 bottom→top，故 `video_tracks` 取 `is_visual()` 的轨道 **`.rev()`**；音频轨保持原序。轨内 clip 按 `start_frame` 升序（`sort_emittable`）。
- **文件存在性过滤（跨平台降级）** — 上游 `resolveURL` 过滤解析不到的离线 clip；domain 的 `MediaResolver` 是**零 IO** 的（只算 `expected_path`），本模块在层内用 `expected_path() + is_file()` 复刻过滤，不污染 domain 的零 IO 约束。过滤后再 `index_addresses`，保证 link 的 trackindex/clipindex 与实际发射的 clip 一致。
- **clipitem 的 in/out 与变速（关键坑）** — `start`/`end` 是时间线帧（跨度 `duration_frames`），`in`/`out` 是源帧偏移（`trim_start_frame` 起，跨度 `source_frames_consumed`）。二者比例即变速，但 Premiere 不会自行推断，**必须显式发 Time Remap filter**（`speed==1` 不发）。
- **`<file>` 去重与类型分离（关键坑）** — `file_id = file-<media_ref>-<audio|video>`：视频 / 音频用**不同** id，否则 Premiere 拒绝 clipitem 指向类型不符的 file。按 `(media_ref, is_audio)` 去重，首次发完整节点，重复折叠为自闭合 `<file id="…"/>`。
- **pathurl 形式（关键坑）** — `file://localhost//<path>`（双斜杠 host 形式），Premiere 需要这种非标准前缀，规范单斜杠会解析失败；解析不到时回退 `media/<media_ref>`。
- **淡入淡出 → 单边转场** — 不走 clip-to-clip，而是发单边 dissolve 到黑 / 静音（视频 Cross Dissolve、音频 Cross Fade），淡入发在 clipitem 前、淡出发在后。
- **滤镜映射** — 变速→Time Remap、音量→Audio Levels、不透明度→Opacity、变换→Basic Motion、裁切→Crop；静态值低于阈值不发，有关键帧则按帧并集逐帧采样。坐标系：Basic Motion 用以画布中心为 0 的归一化坐标，旋转取负（FCP7 逆时针为正）。
- **源帧率 → NTSC（`rate_tags`）** — `timebase = max(1, round(rawFps))`；若更接近 `timebase*1000/1001` 则 `ntsc=TRUE`（29.97 / 23.976 / 59.94 判 NTSC）。源 fps 取 `entry.source_fps`，缺省用时间线 fps。
- **SMPTE 时码（`format_timecode`）** — 非丢帧 `:` 分隔；丢帧（NTSC 且 timebase 为 30 的倍数）`;` 分隔并做丢帧补偿。

## 不会传输的内容（与上游一致）

- **文本叠加**：FCPXML 支持、XMEML 不支持，故文本 clip 不导出（且文本媒体通常无法解析为视频 / 音频文件，实践上也不会进入发射）。
- **翻转**（水平 / 垂直）。
- **关键帧插值曲线**（linear / hold / smooth）：导入端用默认缓动。

## 两处跨平台降级（语义对齐）

1. **源起始时码**：上游用 AVFoundation 读 QuickTime `tmcd` 轨；Rust/Tauri 无等价实现，这里 1:1 降级为 `startFrame=0` + `00:00:00:00`（正是上游读不到 tmcd 时的回退分支）。后续可用 ffprobe 补读。
2. **文件存在性检查**：如上，用 domain resolver 的 `expected_path() + is_file()` 在层内复刻，不改 domain 零 IO 约束。

## 不变量

- 文档头固定 `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE xmeml>\n`，根 `<xmeml version="4">`。
- 渲染缩进步长 2 空格，转义集中在 `render`/`escape_xml`，节点片段不自带空白。
- link 的 track/clip 索引在过滤后重建，永远与实际发射的 clip 对齐。

## 文件规模与拆分建议

`fcpxml.rs` 约 **1489 行**，超过项目约定的单文件 800 行上限（见全局 `CLAUDE.md` 代码风格）。当前是单文件 1:1 端口，可读性靠 `// MARK:` 维持。**建议**（非阻塞）：按现有 MARK 边界拆成子模块，例如 `fcpxml/mod.rs`（入口 + `Builder` 骨架）、`fcpxml/clipitem.rs`、`fcpxml/filters.rs`（滤镜 / 关键帧采样，最大的一段约 518–797 行）、`fcpxml/timecode.rs`（`rate_tags` / `format_timecode`）、`fcpxml/xml.rs`（`XmlNode` + `render` + `escape_xml`），测试随之分散。拆分时保持对拍测试与上游行为不变。

## 与其他子系统的关系

- 输入来自 domain：`Timeline` / `Track` / `Clip` / `Transform` / `Crop` / `MediaManifest` / `MediaResolver` 等。
- 与 [`archive`](bundle-archive.md) 共享 `.external`/`.project` 的素材定位语义（archive 直接拼路径，本模块经 domain `MediaResolver`）。
- 经 `src-tauri` 命令暴露给前端导出对话框；成片**视频**导出（H.264 等逐帧合成）在 `opentake-render`/`src-tauri`，与本模块（纯 XML 文本）无关。

---

> 上级：本模块目录 [INDEX.md](INDEX.md)
