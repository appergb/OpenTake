# 短 ID 系统（出站缩短 / 入站展开）算法

> **来源**：`ToolExecutor+ShortId.swift`（逐行可译）。ARCHITECTURE §7 `:152`「省 token 的关键设计，必须复刻」。

## 3.1 为什么

实体 ID 是完整 UUID（36 字符），verbatim 发送会撑爆 `get_timeline`/`get_transcript`。方案：**出站把每个已知 UUID 替成「在全工程唯一的最短前缀（≥8 字符）」；入站把任意前缀展开回完整 UUID（歧义则报错）**。系统提示词专门叮嘱「原样传回前缀，别补全」（`AgentInstructions.swift:19-20`）。

## 3.2 ID 宇宙（`currentIdUniverse`，`+ShortId.swift:26-39`）

每次工具执行时从当前 `Timeline` + 媒体库 + 文件夹收集**所有 Agent 可见/可命名的 ID**：

```
for track in timeline.tracks:
    ids += track.id
    for clip in track.clips:
        ids += clip.id
        ids += clip.caption_group_id (if Some)
        ids += clip.link_group_id    (if Some)
for asset in media_assets: ids += asset.id
for folder in folders:     ids += folder.id
```

返回 `HashSet<String>`。

## 3.3 出站缩短（`shorteningIds` + `shortIdMap`，`:43-64`）

```rust
const ID_PREFIX_FLOOR: usize = 8;

// 每个 id → 不与任何其它 id 共享的最短前缀（≥8）
fn short_id_map(ids: &HashSet<String>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for id in ids {
        let mut len = ID_PREFIX_FLOOR;
        while len < id.len()
            && ids.iter().any(|other| other != id && other.starts_with(&id[..len])) {
            len += 1;
        }
        out.insert(id.clone(), id[..len].to_string());
    }
    out
}
```
注意：按 **char**（UTF-8 字节）切片需小心，UUID 全 ASCII 故 `[..len]` 安全；若 ID 含非 ASCII（不应发生），改用 `chars().take(len)`。

应用：用正则扫描结果文本里的**完整 UUID**，逐个查 map 替换（不在 map 的 UUID——如嵌在文件名里的——原样透传）。

```
uuid_regex = r"[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}"
for block in result.content where block is Text:
    block.text = uuid_regex.replace_all(text, |m| map.get(m).cloned().unwrap_or(m.to_string()))
```
**关键时序**（`ToolExecutor.execute:69`）：缩短在**工具运行后**的 timeline 状态上做，这样新建实体的 ID（出现在 summary 里）也能被缩短。

## 3.4 入站展开（`expandingIdPrefixes` + `expandOne`，`:68-99`）

工具执行前，把入参里**指定键**的前缀展开为完整 ID：

- **标量键**（`scalarIdKeys`，`:10-15`）：`clipId, sourceClipId, mediaRef, startFrameMediaRef, endFrameMediaRef, sourceVideoMediaRef, videoSourceMediaRef, folderId, parentFolderId`。
- **数组键**（`arrayIdKeys`，`:16-20`）：`clipIds, assetIds, folderIds, referenceMediaRefs, referenceImageMediaRefs, referenceVideoMediaRefs, referenceAudioMediaRefs`。

递归遍历入参 JSON：遇到 scalar 键的字符串值 → `expand_one`；遇到 array 键的字符串数组 → 逐元素 `expand_one`；其它对象/数组递归下探（覆盖 `entries[].mediaRef`、`moves[].clipId` 等嵌套）。

```rust
fn expand_one(reference: &str, universe: &HashSet<String>) -> Result<String, ToolError> {
    if universe.contains(reference) { return Ok(reference.to_string()); }      // 已是完整 ID
    let matches: Vec<&String> = universe.iter().filter(|id| id.starts_with(reference)).collect();
    match matches.len() {
        1 => Ok(matches[0].clone()),
        0 => Ok(reference.to_string()),  // 未知 → 原样传，让工具自己报 not-found
        _ => Err(ToolError::new(format!(
            "Ambiguous id '{reference}' matches {} items; re-read with get_timeline or get_media for current ids.",
            matches.len()))),
    }
}
```

> **测试对拍**（与 Swift 必须一致）：① 两个 UUID 共享前 8 字符 → 各自缩短到第 9 字符；② 传一个唯一前缀 → 展开为完整 ID；③ 传一个被两个 ID 共享的前缀 → 报「Ambiguous」；④ 出站文本里嵌在文件名中的 UUID（不在宇宙）→ 不被替换。
