# Motion Canvas 供应链门禁

- 日期：2026-08-08（Asia/Shanghai）
- 范围：`plugins/motion-canvas-studio/package-lock.json` 与 Release quality gate
- 结论：**PASS**

## 发现与根因

最终发布自检首次执行 `npm audit --audit-level=moderate` 时稳定失败：
`postcss@8.5.25` 的 `^3.3.16` 依赖在锁文件中解析到 `nanoid@3.3.16`，命中
`GHSA-2v37-7h3g-55p8`（`<3.3.17`，high）。这是传递依赖锁定版本问题，
不是 Motion Canvas 业务代码问题。

## 最小修复

仅把锁文件中的 `nanoid` 从 `3.3.16` 更新到同一兼容范围内的 `3.3.18`；
没有升级 Vite、Motion Canvas、PostCSS 或其他依赖，也没有运行会改写整个依赖图的
`npm audit fix`。

## 验证

- `npm ci --ignore-scripts`：PASS。
- `npm audit --audit-level=moderate`：PASS，0 vulnerabilities。
- `npm run licenses`：PASS，123 个锁定包许可证记录。
- `npm test`：PASS，2/2。
- `npm run build`：PASS；runner bundle 构建并规范化。
- `git diff --exit-code -- bundle/runner.html`：PASS，生成产物无漂移。
- 锁文件差异：仅 `nanoid 3.3.16 → 3.3.18` 的 version/resolved/integrity。

复验过程中本机磁盘被 Rust debug 构建缓存占满；已使用 `cargo clean` 删除可再生成的
本地构建缓存后重跑以上完整门禁。未删除源码、工程、媒体或用户数据。
