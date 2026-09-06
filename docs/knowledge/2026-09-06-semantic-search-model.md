---
status: canonical
stage: implementation-backed
retrieved_at: 2026-09-06T06:34:20Z
freshness: normal
valid_until: 2026-09-16
confidence: high
---

# SigLIP2 公开 ONNX 模型资产

本记录是 2026-09-06 搜索模型缺口修复的外部知识来源。固定模型事实长期有效；远端可达性按 10 天重新验证。不是对上游最新版本的声明。

## 来源与版本

- [Google 模型卡](https://huggingface.co/google/siglip2-base-patch16-256)：原始模型 `google/siglip2-base-patch16-256`，Apache-2.0，支持图文检索。
- [ONNX Community 固定发布](https://huggingface.co/onnx-community/siglip2-base-patch16-256-ONNX/tree/d1114256522a37ffa257a0a58017348ab0058db2)：由 Hugging Face 的 Xenova 发布，模型卡明确来源为上述 Google 模型；不是 OpenTake 自行托管或同名替代模型。
- [固定 revision 文件 API](https://huggingface.co/api/models/onnx-community/siglip2-base-patch16-256-ONNX/revision/d1114256522a37ffa257a0a58017348ab0058db2?blobs=true)：提供 LFS SHA-256 和准确长度。必须使用 `lfs.sha256`，不能将 Xet 存储 hash 或 Git blobId 当文件 SHA-256。
- [固定预处理配置](https://huggingface.co/onnx-community/siglip2-base-patch16-256-ONNX/blob/d1114256522a37ffa257a0a58017348ab0058db2/preprocessor_config.json)：256×256、双线性缩放、RGB mean/std 均为 0.5。
- [固定 tokenizer 配置](https://huggingface.co/onnx-community/siglip2-base-patch16-256-ONNX/blob/d1114256522a37ffa257a0a58017348ab0058db2/tokenizer_config.json)：Gemma tokenizer，无 BOS、有 EOS，右填 `<pad>` id 0，长度 64。
- [Transformers 4.50.3 GemmaTokenizerFast](https://github.com/huggingface/transformers/blob/v4.50.3/src/transformers/models/gemma/tokenization_gemma_fast.py)：特殊 token 后处理；截断应在加入 EOS 前完成。所下载 tokenizer.json 包含完整词表、后处理和空格正规化，不需要 tokenizer.model 或联网加载配置。

项目使用 `ort = 2.0.0-rc.11`、`tokenizers = 0.21.4`（锁文件实际解析版本），ONNX 配置记载导出工具 `transformers 4.50.0.dev0`；这几者不代表上游最新版本。本次没有改依赖。

## 选用资产

基础地址：`https://huggingface.co/onnx-community/siglip2-base-patch16-256-ONNX/resolve/d1114256522a37ffa257a0a58017348ab0058db2`。

| 相对路径 | 准确字节数 | 文件 SHA-256 |
|---|---:|---|
| `onnx/vision_model.onnx` | 371992072 | `f5cb16728a704703f05516ded628397e11dbca4de2eb5db04b0c0bcee988aa7a` |
| `onnx/text_model.onnx` | 1129469657 | `d3de4a6bbbfcb429b6615ac496790353cf4a4fc0f19fbbe7179e523ae60daaef` |
| `tokenizer.json` | 34363039 | `cb9140fae3ac5122c972d37adf83e1248471a38147ad76f8215c8872c6fd8322` |

合计 **1,535,824,768 字节**（约 1.43 GiB）。以上值已与实际匿名公开下载重新计算的 SHA-256、文件长度逐一比对。只下载 FP32 两个独立编码器，不下载整个 11.4 GB 模型仓库、不使用量化变体、不生成修改后的 ONNX。

## 实际推理契约

| 图 | 输入 | 使用的输出 |
|---|---|---|
| Vision | `pixel_values`, float32 `(1,3,256,256)` | `pooler_output`, float32 `(1,768)` |
| Text | `input_ids`, int64 `(1,64)` | `pooler_output`, float32 `(1,768)` |

图内还暴露 `last_hidden_state`，不能拿它当最终检索向量。两个池化输出没有 L2 归一化；零图像与示例文本测得范数约 14.08 和 24.39，必须图外归一化后再交给现有余弦点积排名。短文本右填 0，长文本截断保留 EOS。沿用 Rust 的黑底 alpha 合成、双线性正方形缩放。完整真实图像/文本和 Rust 验证见[审计记录](../audit/2026-09-06/semantic-search-model.md)。

旧文档曾要求等待 OpenTake 自托管资产，并假定图内归一化；这属于旧占位实现。本次选用公开 ONNX 分离编码器，因此明确更正接口契约；配置中的安装/索引版本升至 2，旧 v1 索引需重新生成。没有改模型名称、维度、分辨率或上下文长度。

## 有效性边界

已验证 macOS ARM64 CPU 原生 ONNX Runtime 与 Python ONNX Runtime。Windows 使用项目现有 ort-tract 后端，其对这两个实际图的兼容性需要 Windows 运行证据；不能由 macOS 验证推断已通过。公开 Hugging Face 链路在受限网络可能不可达，离线安装可使用相同字节与校验值。
