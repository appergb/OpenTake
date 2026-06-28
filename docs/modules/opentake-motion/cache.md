# cache — 内容寻址帧缓存

> 上级：[模块目录 INDEX.md](INDEX.md) · [总览 OVERVIEW.md](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
> 源码：[`../../../crates/opentake-motion/src/cache.rs`](../../../crates/opentake-motion/src/cache.rs)

---

## 职责

为已渲染的动效帧提供**内容寻址缓存**：缓存键是 SHA-256 over 一切影响像素的输入——源（代码 或 模板 id + 参数）+ fps + 宽 + 高 + 透明标志。同输入 ⇒ 同键 ⇒ 复用已渲染帧；改源或任一参数 ⇒ 键变 ⇒ 下次未命中重算。这是标准的 content-addressed cache 模式：键 path-independent、self-invalidating。

> 完成状态：**全部已实现并全测**（哈希纯函数无文件系统；`MotionCache` 是薄目录包装 + 完整性判定）。

---

## `content_hash(req)`（纯函数）

返回小写 hex SHA-256。喂入一个**规范、无歧义**的字节流：

- **版本前缀** `b"opentake-motion/v1\n"`——将来改"哈希什么"时整体失效旧条目，而非静默碰撞。
- **数值/标志先行**（定宽、无歧义）：`fps` / `duration_frames` / `width` / `height`（`to_le_bytes`）+ `transparent`（1 字节），各带定界标签。
- **源**：
  - `Code`：`source=code;len=<u64>;body=<bytes>`（长度前缀，防 `("a","bc")` 与 `("ab","c")` 碰撞）。
  - `Template`：`source=template;id_len=…;id=…;params=…`；`BTreeMap` 按键排序迭代 ⇒ 确定性。
- **参数值**（`hash_param_value`）带**类型标签**，使字符串 `"1"` 与数字 `1` 哈希不同：
  - `String` → `s:` + 长度 + bytes
  - `Number` → `n:` + bit pattern（`-0.0` 归一为 `0.0`，防发散）
  - `Bool` → `b:` + 1 字节
  - `Color` → `c:` + 长度 + **小写** bytes（`#ABC == #abc`）

测试覆盖：64 hex 字符、同请求稳定、改 body/尺寸/fps/透明/时长都变键、参数插入顺序不影响键（BTreeMap 规范化）、参数值类型变键变、Code 与 Template 同字符串不碰撞。

---

## `MotionCache`

根目录下的内容寻址帧缓存。每个渲染键映射到 `root/<hash>/`，渲染器往该目录填帧文件，缓存报告命中/未命中。

- `new(root)`：建缓存（磁盘目录直到 `ensure_dir`/写入才创建）。
- `dir_for(req)` = `root/content_hash(req)`；`dir_for_hash(hash)` = `root/<hash>`。
- `ensure_dir(req)`：`create_dir_all` 并返回路径。
- `frame_file(dir, i)` = `dir/frame_{i:05}.png`——**零填充**使字典序 == 播放序。
- `is_cached(req)`：当且仅当目录存在且恰好含 `duration_frames` 个 `frame_*.png` 文件时为 `true`。**partial 渲染（中途崩溃）视为 miss** 而非供出截断结果——重算而非半成品。

`count_frame_files`（私有）：数目录里 `frame_*.png`，不存在返回 `None`。

测试覆盖：`dir_for` = root join hash、`frame_file` 零填充、缺目录 miss、**只有帧数完全匹配才命中**（2/3 帧仍 miss）。

---

## 在管线中的位置

```text
MotionRenderRequest
  └─ content_hash ──▶ MotionCache.is_cached?
        ├─ 命中 → 复用 root/<hash>/frame_*.png
        └─ 未命中 → MotionRenderer::render 写帧 → 下次命中
```

`StubRenderer` 与（计划中的）`HeadlessChromiumRenderer` 都用 `cache.ensure_dir` + `frame_file` 写帧；产物 `RenderedClip.content_hash` 即此键（见 [renderer.md](renderer.md) / [manifest-source.md](manifest-source.md)）。

---

## 移植铁律落地

- **确定性 = 预览==导出**：键覆盖一切影响像素的输入，纯函数；同请求同字节同键。
- **path-independent + self-invalidating**：键不含路径，输入变即失效。
- **改哈希内容必须升版本前缀**（`opentake-motion/v1`）。
- **完整性优先**：partial 视为 miss，不供截断结果。
- **离线可测**：哈希纯函数；`sha2`/`hex` 已 vendored，build/test 全离线。

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
