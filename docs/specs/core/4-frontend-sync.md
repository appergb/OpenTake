## 4. 前端只读镜像 + 版本号同步协议

### 4.1 协议(三条规则)

1. **快照都带版本号**:`get_timeline` 返回 `{ timeline: TimelineDTO, version: u64 }`。前端 Zustand 存 `{ mirror, mirrorVersion }`。
2. **写命令返回新版本号**:`edit_apply` / `undo` / `redo` 返回 `EditResult`(含 `timeline_version`)。前端可乐观地用返回值直接判断是否需重取,或纯靠事件(见 4.3)。
3. **事件触发重取**:收到 `timeline_changed{version}`,若 `version > mirrorVersion`,则 `invoke('get_timeline')` 重取并替换镜像 + 更新 `mirrorVersion`。`version <= mirrorVersion` 的事件**幂等忽略**(防重复/乱序)。

> 对应 ARCHITECTURE §2 原话:「每次 `edit_apply` 广播 `timeline_changed{version}`,前端据此重取。」`version` 即上游 `timelineRenderRevision` 的跨进程版,只是上游靠 `@Observable` 自动 diff,OpenTake 靠"版本号比较 + 显式重取"。

### 4.2 为何"重取整棵 timeline"而非增量 patch(明确取舍)

- 上游撤销/重做 = **整棵 timeline 替换**(`vm.timeline = undoState`,`ClipMutations.swift:232`),`Timeline` 是值类型整树(ARCHITECTURE §4「全是 Clone+Serialize+PartialEq 的值类型」)。OpenTake 沿用整树快照模型(§2.3、ARCHITECTURE §5「撤销栈在 Rust,整树快照」)。
- 因此**最简单且与撤销模型一致的同步 = 整树重取**。timeline 是元数据(帧号/引用/关键帧),非像素,JSON 体量可控;媒体像素走 §3.3 旁路,从不进 timeline DTO。
- **不做增量 patch**(CLAUDE.md「Simplicity First / 不做未要求的灵活性」):整树重取在正确性上零歧义(版本号即一致性令牌),patch 会引入顺序/丢失/合并的复杂度。若未来 profiling 证明大工程重取过重,再在**不破坏版本号契约**的前提下加 patch 通道。

### 4.3 乱序与并发(跨进程必须显式处理,上游不存在)

跨进程下,多个客户端(UI + 若干 MCP 连接)可并发 `edit_apply`。core 的 `Mutex` 串行化所有 `apply`,故 `version` 严格单调、无并发写竞争。但前端可能:
- 在 `get_timeline` 在途时收到更新的 `timeline_changed` → **以更高 `version` 为准**(收到的快照若 `version < latestEventVersion`,丢弃并重取)。
- 收到旧 `version` 事件(broadcast 缓冲)→ **幂等忽略**(规则 3)。

> 这是上游"单进程 @Observable"模型免费给的、OpenTake 必须自己保证的一致性。`version` 单调 + "永远向最高版本收敛" 是协议正确性的全部基础。**必须有集成测试**:并发触发 N 个 `edit_apply`,断言前端最终镜像 `version` == core 最终 `version` 且内容一致。

### 4.4 TimelineDTO 边界

`get_timeline` 返回的不是 Rust `Timeline` 本体,而是**面向前端的 DTO**(serde 序列化)。Phase 6 前端态(ARCHITECTURE §2:「Timeline 只读镜像 + UI-only 态(selection/zoom)」)只读 DTO,**永不写回**——所有变更经 `edit_apply`。DTO 可直接 `serde` 复用 domain 的 `Serialize`(与 `.opentake/project.json` 同 schema,ARCHITECTURE §9),保证"持久化格式 = 线上格式",减少一套映射。

---
