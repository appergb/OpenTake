---
status: canonical
stage: final-verified
updated_at: 2026-09-06
---

# 语义搜索模型首次安装修复

## 目标与写入范围

完成真实可下载、固定版本校验、离线安装、真实图文 embedding 与语义排名。保留主线已有 `embedder.rs` 的 `as_chunks` 修改。不提交、推送、发布，不修改其他 Markdown、marketing 或 output。

代码改动：`crates/opentake-media/src/search/{config,model_download,tokenizer,ort_embedder}.rs`、`src-tauri/src/search.rs`。其中 `ort_embedder.rs` 是调查后提前告知的必要新增写入范围。知识来源仅写 [模型知识记录](../../knowledge/2026-09-06-semantic-search-model.md)，详细模型 SHA/长度事实统一在那里维护。

读取范围：工作区和项目 AGENTS、docs/INDEX、opentake-media 和 src-tauri 的 OVERVIEW/INDEX、semantic-search 模块页、对应 Rust 文件，以及只读上游 `Search/SearchIndexConfig.swift`、`Search/Models/*.swift`、`Search/Query/VisualSearch.swift`。未修改上游。

## 根因与修复

1. 原 URL 指向未托管的 OpenTake ONNX 仓库，3 个 SHA-256 为空、长度为零。改用可信 ONNX Community 对应 Google 原模型的固定 revision FP32 资产，3 个文件共 1,535,824,768 字节，匿名下载后逐一验证通过。
2. 真实资产使用 `onnx/` 子目录和原始 `tokenizer.json`；旧下载器只支持根路径及 `tokenizer.zip`。现支持相对路径、原始 JSON，保留旧 zip 兼容性，不增加依赖。
3. 默认 I/O 名仍是 CoreML 的 `image/tokens/embedding`。改为真实图的 `pixel_values/input_ids/pooler_output`。
4. 原归一化分支为空操作；真实池化输出未归一化。现校验维度、有限值、非零范数，并按 `spec.normalized == false` 做 L2 归一化。
5. 原 tokenizer 编码后硬截断会丢失长查询末尾 EOS。现让 tokenizer 在特殊 token 后处理前执行长度限制，再按 0 右填到 64。
6. 原 `installed` 只判断三个路径存在，错误或部分文件也能被当作已安装。现要求与当前 manifest 相同的安装回执和准确尺寸；Tauri 加载前重新验证实际 SHA-256。安装目录与索引版本升至 2，使 v1 索引自动失效。

安装使用模型目录内的临时目录，下载结束逐个验证长度和 SHA-256，所有文件准备好后再发布整个目录；临时目录 RAII 清理覆盖请求失败和取消。替换旧目录失败时尝试恢复原目录。进度只在安装成功后到 1.0，超出 manifest 预期大小即拒绝写入更多内容。

## 实际外部证据

- 获取时间：2026-09-06；固定 revision 与逐个校验值见知识记录。Python 使用公开 HTTPS 请求，不读取 Hugging Face 用户配置或凭据。
- Python ONNX Runtime 1.23.2 CPU 加载两个真实图成功，不需要外部 `.onnx_data` 文件。
- 视觉输入 `(1,3,256,256)`，文本输入 `(1,64)`，二者 `pooler_output` 均为 `(1,768)`。
- 未归一化输出范数：全零像素张量约 14.08375，示例文本约 24.39208。归一化后再排名，不能将裸点积当余弦。
- 图片来源：[猫图](https://huggingface.co/datasets/huggingface/documentation-images/resolve/main/coco_sample.png)、[鹦鹉图](https://huggingface.co/datasets/huggingface/documentation-images/resolve/main/hub/parrots.png)，只放任务临时目录，不加入仓库。

Python 真实图文余弦分数：

| 查询 | 猫图 | 鹦鹉图 | 预期 |
|---|---:|---:|---|
| a photo of two cats on a couch | 0.155847 | -0.012872 | 猫图第一 |
| a photo of colorful parrots | -0.004647 | 0.133400 | 鹦鹉图第一 |
| 沙发上的两只猫 | 0.115520 | 0.013697 | 猫图第一 |
| 彩色鹦鹉 | -0.043940 | 0.115521 | 鹦鹉图第一 |
| a photo of an airplane | -0.018885 | 0.006155 | 均低于 0.05 门槛 |

## Rust 验证与复现

配置修复前，针对公开 API 数据的 manifest 回归检查失败，原因是缺少固定 revision。修复后通过。Tokenizer 长序列回归在修改前实际失败：`[2,2,2,2]`，期望 `[2,2,2,1]`；修改后通过。

为避免争用主线 Cargo，先使用 `/tmp` 中的 `rustc --test` harness 引用实际模块并只读复用已编译依赖。最初 harness 误选 reqwest 0.13（项目用 0.12）导致 TLS provider 初始化失败；按项目 0.12 重新链接后 34 项通过。这是测试装配错误，没有为其修改产品依赖或添加 TLS 绕过。

主线随后明确释放 Cargo 锁，执行正式命令：

```sh
cargo test -p opentake-media --features model-download,ort-backend --lib search:: -- --test-threads=1
```

最终结果：84 passed、0 failed、1 ignored（真实资产测试默认 opt-in）；包括模型下载、HTTP 404/截断/超长/错误 hash、离线安装、回执、实际字节损坏拒绝、tokenizer 与排名/存储现有回归。

真实模型 Cargo 复现命令（本轮真实模型实际使用下述独立 Rust harness 执行；此命令供标准 Cargo 重跑，无下载、无凭据）：

```sh
OPENTAKE_SEARCH_MODEL_TEST_DIR=/tmp/opentake-semantic-model \
  cargo test -p opentake-media --features model-download,ort-backend --lib \
  real_model_offline_install_embeddings_and_ranking -- --ignored --nocapture --test-threads=1
```

该目录应包含 `onnx/vision_model.onnx`、`onnx/text_model.onnx`、`tokenizer.json`、`cats.png`、`parrots.png`。测试从固定 manifest 校验离线来源，安装至临时模型目录，以产品 OrtEmbedder 加载，运行实际图片与中英文查询，断言 768 维/有限值/单位范数，将图像 embedding 经 PALMEMB1 f16 编码解码，再运行产品 ranker 并检查五组结果。

真实 Rust 模块回归已完成：`/tmp/opentake-semantic-model/rust_tests.py` 用 `rustc --test` 引用本工作树实际 `config/embedder/tokenizer/model_download/ort_embedder/embed_store/ranker` 及依赖模块，启用 `model-download` 与 `ort-backend`，连接项目已编译的 `ort 2.0.0-rc.11` 原生运行库。macOS 初始化函数与库内一致为空函数。结果 **72 passed、0 failed、0 ignored**，包含真实模型 opt-in 测试，耗时 301.84 秒（debug 构建对模型源、暂存副本、安装文件的重复 SHA-256 校验占主要时间）。没有以 Python 推理代替 Rust 实测，也没有为测试关闭 checksum。

产品 Rust 路径经历真实图像预处理、768 维向量归一化、f16 索引编码解码和 ranker 门槛后的结果：

| 查询 | 第一名 | 分数 |
|---|---|---:|
| a photo of two cats on a couch | cats | 0.15572338 |
| a photo of colorful parrots | parrots | 0.13344961 |
| 沙发上的两只猫 | cats | 0.11564892 |
| 彩色鹦鹉 | parrots | 0.11663653 |
| a photo of an airplane | 无结果 | 所有候选低于门槛 |

Tauri 正式验证：

```sh
cargo test -p opentake-tauri --lib search:: --jobs 1 -- --test-threads=1
```

结果 **14 passed、0 failed**；跨 crate 的 `verify_installed` 已正常编译链接。原主线编译中间态错误不再出现。仅有已有依赖 `block 0.1.6` 的未来 Rust 兼容性提示。

最后执行受限文件 `rustfmt --check` 与 `git diff --check`，均通过。主线原有 `embedder.rs` 的 `as_chunks` 改动保留，本任务未再编辑该文件。Cargo 本轮完成后已明确通知主线继续。

本地证据保留在 `/tmp/opentake-semantic-model/`：`api.json`、三个原始模型文件、两张公开图片、`python-evidence.json`、`rust-red.log`、`rust-unit.log`、`rust-real.log`、`cargo-search.log`、`tauri-tests.log` 及复现脚本。它们不加入 Git；真实测试生成的安装副本已随 tempfile 清理。

## 离线使用方式

- 用户可在可联网机器完成一次应用内模型下载，然后将完整 `siglip2-base-patch16-256-v2/` 目录复制至离线机器的应用 `Models` 目录（`MediaEngine::models_dir()`）。目录包含两个 ONNX、`tokenizer/tokenizer.json`、`spec.json`、`manifest.json`。应用识别回执和尺寸，并在推理加载前重新验 hash；不需要联网或账号。
- 对预先下载的原始资产，调用公开 Rust 函数 `model_download::install_from_directory(models_dir, &config::manifest(), source_dir)`。它读取上述三个 repository-relative 文件，验证源和暂存副本，再创建同一安装回执。真实模型 opt-in 测试覆盖此路径。
- 没有增加离线导入 UI 或额外 Tauri 命令；也没有新增前端类型或注册项。不可只放几个未校验的空文件伪造“已安装”。

## 剩余边界

- 此记录验证本任务纵向链路，不宣称 workspace、完整 UI、发布包、Windows ort-tract 已通过；主线仍负责公开 Beta 发布门槛。
- Hugging Face 在受限网络可能不可达，应用返回请求错误，离线复制/导入保留相同校验。
- 模型净资产约 1.43 GiB；离线安装测试需要源与安装目录同时存在，逻辑峰值约 2.86 GiB，临时安装目录测试结束后清理。未下载整个模型仓库。
- 加载前重新计算两个大 ONNX 的 SHA-256 有 CPU/IO 成本；快速状态查询只读回执和尺寸。原有查询构造新 embedder 的行为未在本任务扩为跨请求缓存。

## 2026-09-06 独立 ort-tract 资格验证：失败，阻塞当前 Windows 语义搜索

获取/执行时间：2026-09-06T06:58:44Z 起完成汇总。范围仅为临时 harness 和本节审计；冻结产品代码没有在本轮修改，主 workspace target 未使用。主线另行处理查询缓存版本过滤、模型失败恢复与下载大小显示，这些修改不属于本次后端结论。

### 环境与可重复性

- 实际宿主：macOS ARM64，16 GiB RAM，`rustc 1.97.1 (8bab26f4f 2026-07-14)`。
- 锁定后端：`ort 2.0.0-rc.11`、`ort-sys 2.0.0-rc.11`、`ort-tract 0.2.0+0.22`、`tract-onnx/tract-core 0.22.3`。
- 临时 Cargo.lock 的所有 registry 包版本及 checksum 均与主 workspace 当时锁文件一致，差异列表为空，证据 `lock-comparison.json`。本地原先没有 tract 源码，只获取构建该后端所需的锁定 Rust 依赖。
- Cargo 实际 feature：`ort = [alternative-backend, ndarray, std]`；`ort-sys = [disable-linking, std]`；没有启用 native ONNX Runtime 的 download-binaries、copy-dylibs 或 TLS feature。测试中 `ort::set_api(ort_tract::api())` 断言成功。
- 复制产品相关模块到 `/tmp/opentake-tract-qualification/src/imported/` 作为稳定快照，复用实际 `OrtEmbedder`、`Embedder` trait、预处理、tokenizer、f16 index 与 ranker。初始化函数逐句使用产品 Windows 分支的 `Once + ort::set_api`，没有人为设置 `cfg windows`。
- ort-tract 可以在 macOS 原生编译。其 `api.rs:66-72` 的 Windows cfg 只分支文件路径 UTF-16 解码；后续模型解析、类型推导和执行走共同代码。本次结果是相同后端的兼容性预检，**不是 Windows OS/MSVC 实机通过证据**。
- 模型与两张图继续使用 `/tmp/opentake-semantic-model/` 中前轮已校验的同一批文件。未额外下载或重写模型、未固化 ONNX 维度、未关闭 shape/finite/dim/normalization/ranking 断言。

临时构建：

```sh
CARGO_TARGET_DIR=/tmp/opentake-tract-qualification/target \
  cargo build --manifest-path /tmp/opentake-tract-qualification/Cargo.toml \
  --locked --offline --jobs 1
```

最终构建成功。临时 harness 的初版日志曾误用 ort Session 私有字段，改为锁定 API 的 `inputs()` / `outputs()` 后通过；修正仅发生在 `/tmp`。为节省资源使用 `debug=0`、`incremental=false`、`opt-level=1`、单编译任务。所有临时构建/日志约 731 MiB；本轮开始可用磁盘约 25 GiB，结束约 23 GiB。前轮模型目录仍约 1.7 GiB，没有模型复制增量。

### 三个真实运行阻塞

| 路径 | 实际失败阶段及结果 | 判定 |
|---|---|---|
| 原样产品 OrtEmbedder → Windows init → tract | `model install: ort threads: Unimplemented`；图像、文本独立入口也均返回 `Unimplemented`，进程退出码 1 | 模型尚未加载即失败，当前 Windows 产品路径必需先处理 |
| 临时诊断模式仅省略线程数量性能选项，加载原图像图 | `Failed to parse model: Failed analyse for node #232 "/vision_model/embeddings/patch_embedding/Conv" ConvHir`，退出码 1 | 即使处理线程选项，原图像图仍无法加载 |
| 同一临时诊断模式加载原文本图并用真实 tokenizer 输入 `(1,64)` | Session 加载成功，执行失败：`Failed to run session: Evaluating #11 "/text_model/embeddings/Slice" StridedSlice`，退出码 1 | 原文本图无法输出 embedding |

线程选项的具体源码链：

- 产品 `crates/opentake-media/src/search/ort_embedder.rs::build_session` 调用 `.with_intra_threads(...)`。
- 锁定 crate `ort-tract-0.2.0+0.22/api.rs:531-590` 构造 API 表，没有覆盖 `SetIntraOpNumThreads`，其余字段来自 `..ort_sys::stub::api()`。
- `ort-sys-2.0.0-rc.11/src/stub.rs:194` 的 `SetIntraOpNumThreads` 明确返回 `OrtErrorCode::ORT_NOT_IMPLEMENTED`、`"Unimplemented"`。

为了得到 ort-tract API 错误包装省略的完整原因，临时诊断又直接调用了**同版本 tract 的相同解析/into_typed/into_runnable/run 流程**，没有修改输入事实或模型：

```text
Vision:
Failed analyse for node #232 "/vision_model/embeddings/patch_embedding/Conv" ConvHir:
Infering facts:
Applying rule inputs[0].shape[1] == 1*{inputs[1].shape[1]}:
Impossible to unify Sym(num_channels) with Val(3).

Text (typed graph 成功，实际执行失败):
Evaluating #11 "/text_model/embeddings/Slice" StridedSlice:
Running legacy eval:
Evaluating #5 "adhoc" Slice:
Undetermined symbol in expression: <Sym1>
```

`ort-tract/session.rs:57-64` 在加载时 `model_for_proto_model` 后立即 `into_typed()`（或启用优化时 `into_optimized()`）；`session.rs:35-43` 的执行计划构造只设置 input names，没有根据运行时 tensor 绑定 input shape。该代码与上述错误相互印证。这里只记录已观察到的动态 shape 失败，不将它泛称为整个 Conv/Slice 算子不受支持，也不预先声称某一种修复方案已可行。

### 内存与验收结果

使用 `/usr/bin/time -l` 测量，每个图在独立进程顺序运行，没有 OOM 或进程被杀：

| 诊断 | 最大 RSS | 结果 |
|---|---:|---|
| ort-tract 图像加载（省略线程选项） | 1,500,954,624 bytes（约 1.40 GiB） | shape 推导失败 |
| ort-tract 文本加载 + `(1,64)` 执行（省略线程选项） | 2,383,986,688 bytes（约 2.22 GiB） | Slice 动态符号失败 |
| 直接 tract 文本完整原因链诊断 | 2,925,232,128 bytes（约 2.72 GiB） | 同一 Slice 失败；time 另报 peak memory footprint 4,105,866,744 bytes |

**没有产生有效 tract 图像/文本 embedding 或排名结果。** 原样产品排名入口实际运行并在 `OrtEmbedder::new` 返回线程设置错误，因此不能以先前 native-ORT 的正确猫/鹦鹉排名替代本后端验收。由于执行提前失败，也不能据此宣称完整模型推理的内存上限已通过。

结论：**阻塞当前 Windows 语义搜索发布资格。** 同一个锁定 tract 后端的共享代码已在宿主预检稳定复现三个阻塞；不能仅修复 `.with_intra_threads` 就宣布 Windows 支持。修复需要主线协调产品后端/API 或 shape 适配策略；本轮没有修改 `ort_embedder.rs`、`model_download.rs` 或模型文件。

### 最小 Windows 原生 qualification

在真实 `windows-2022`/Windows x64 runner、VS C/C++ Build Tools + Windows SDK、项目 stable Rust MSVC 工具链上运行。无需 Python ONNX Runtime、native onnxruntime.dll、FFmpeg、Whisper 或付费服务。fixture 目录预先准备前文三个固定 SHA 模型文件以及 cats.png/parrots.png，测试本身不联网下载；需保留足够空间供源文件与临时安装副本并存。

```powershell
$env:OPENTAKE_SEARCH_MODEL_TEST_DIR = Join-Path $env:RUNNER_TEMP 'semantic-model-fixtures'
$env:CARGO_TARGET_DIR = Join-Path $env:RUNNER_TEMP 'semantic-qualification-target'
cargo test --locked -p opentake-media `
  --target x86_64-pc-windows-msvc `
  --features model-download,ort-backend --lib `
  search::ort_embedder::tests::real_model_offline_install_embeddings_and_ranking `
  -- --exact --ignored --nocapture --test-threads=1
if ($LASTEXITCODE -ne 0) { throw 'Windows semantic model qualification failed' }
```

该命令自然选中产品 Windows `ort-tract` 依赖和初始化分支，覆盖实际 hash/安装、预处理、768 维单位向量、f16 索引和五条查询。需在协调修复后真正通过，并保留资源峰值及日志，才能解除 Windows 资格阻塞。这里提供命令，未提交/运行 CI 或改工作流。

本轮原始证据：`/tmp/opentake-tract-qualification/{Cargo.toml,Cargo.lock,metadata.json,lock-comparison.json,build.log,vision.log,text.log,ranking.log,diagnostic-vision.log,diagnostic-text.log,direct-vision.log,direct-text.log}`；可执行文件为 `target/debug/opentake-tract-qualification`，模式分别是 `vision`、`text`、`ranking`、`diagnostic-vision`、`diagnostic-text`、`direct-vision`、`direct-text`，运行时设置 `OPENTAKE_SEARCH_MODEL_TEST_DIR=/tmp/opentake-semantic-model`。
