# 分步实施清单与验收

> 依赖:`opentake-domain`(M1,已完成 media/clip_type/timeline)。本 crate 分 **Phase 2 子集**(缩略图/波形/解码/探测,易)与 **Phase 8 子集**(转写/语义搜索/ort worker)。每步独立可测,覆盖率 ≥80%(AGENTS.md / common testing)。

## Phase 2 子集(基础媒体,先做)

- **T2.1 cache_key + error**:`file_identity_key`(SHA256 path|mtime|size,前 16 字节 32 hex)+ `MediaError`。验收:同输入稳定、不同 mtime/size 变 key、缺文件 None。
- **T2.2 probe**:ffmpeg-sidecar 读时长/旋转校正宽高/fps/has_audio。验收:对一组样本(横屏/竖屏/旋转 90°/纯音频/无音轨视频)字段正确;旋转视频宽高已交换;与 `ffprobe` 交叉核对。
- **T2.3 decode_frame_at / decode_frames_at**:seek+tolerance+缩放(保宽高比)+ `t>lastTime` 去重。验收:指定秒取到最近帧、实际时间单调、越界 Err;批量去重生效。
- **T2.4 extract_pcm**:解音轨 → 16k mono f32,支持 range。验收:输出采样率/声道/长度正确;range `(a,b)` 长度≈`(b-a)*16000`;无音轨 `NoTrack`。
- **T2.5 thumbnail + sprite**:`videoThumbnailTimes` 公式、120×68、渐进回调、sprite 网格 + JSON sidecar(camelCase 字段)+ 原子写 + 读校验。验收:时间点序列与公式逐一相符;**写出的 `.thumbs.jpg/.json` 能被上游 `MediaVisualCache.loadThumbnails` 读回**(字段名/列数/tile 尺寸/times 一致);坏文件返回 None。
- **T2.6 waveform**:`waveform_sample_count` 公式逐字 + RMS 降采样 + 归一化「0=响,1=静」+ `.waveform` LE 缓存。验收:count 公式边界(0/1s/100s/133.3s/1000s)精确;全静音→≈1、满幅→≈0、单调;`.waveform` 字节 = `len*4`、可往返。
- **T2.7 encode + preset**:H.264/H.265/ProRes × 720/1080/4K 编码器 + 偶数尺寸(逻辑在 render,本 crate 接收偶数)+ BT.709。验收:导出 mp4 可播放、时长/分辨率正确、色彩 BT.709 tag;ProRes 带 LPCM。**画质逐预设逼近上游**留 §10 末持续校准(非阻塞)。

## Phase 8 子集(转写/语义搜索/AI worker)

- **T8.0 模型资产准备**(前置):SigLIP2-base-patch16-256 转/取 ONNX(image+text encoder)+ tokenizer.json;whisper 多语种 ggml/gguf;自托管 + 计算 sha256/bytes 填 `Manifest`。验收:`installed()` 能识别、`verify_sha256` 通过、`OrtEmbedder` warm-up `encode_text("warm up")` 成功(对齐 `VisualModelLoader.load` 的 warm-up,`VisualModelLoader.swift:99`)。
- **T8.1 transcribe 数据模型 + offsetting + locale**:`TranscriptionResult/Word/Segment` serde(字段名与上游互读)、`offsetting`、`match_locale`/`best_supported_locale`。验收:`offsetting(0)` 恒等、`offsetting(k)` 全时码 +k、None 保持;locale 同语言选同地区否则首个(对拍 `Transcription.matchLocale` 用例)。
- **T8.2 whisper 后端**:`extract_pcm` → whisper-rs(token timestamps)→ segments/words/text/language;range→offsetting。验收:已知短音频转写文本合理、segment start<end、word 时码落在 segment 内、`text` = segments 拼接 trim;range 转写后时码回到源时间。
- **T8.3 TranscriptCache + TranscriptSearch**:内存 LRU=4(满清空)、磁盘 JSON、`filter` 半开重叠、`terms`/`matches`(AND 子串、大小写/变音不敏感 NFD)。验收:`filter` 段/词重叠判定精确、`text` 空格 join;`terms("budget, plan")→["budget","plan"]`;`matches` 变音不敏感(café~cafe)、AND 语义;磁盘 JSON 与上游 `TranscriptCache` 互读。
- **T8.4 FrameSampler + LumaGrid**:高分辨率间隔翻倍、stride 候选、tolerance、`t>lastTime` 去重、luma 8×8 Rec.601、meanDiff>12 镜头、coverageFloor 8、lastKeptTime 只在保留时推进。验收:对合成测试视频(已知镜头切点)产出帧的 `is_new_shot` 与切点对应;候选时间序列与公式相符;高分辨率源 interval 翻倍。
- **T8.5 EmbeddingStore**:PALMEMB1 LE 布局、rowBytes=24+dim*2(dim768→1560)、f16 落盘/f32 内存、严格长度校验、atomic、key/is_current。验收:`save→load` 往返 bit 级(f16 量化后)一致;**上游 `EmbeddingStore.load` 能读本 crate 写的 `.embed`**(magic/headerLen/JSON/行布局逐字节)、反之亦然;截断/篡改→`StoreCorrupt`;`is_current` 版本三元组判定。
- **T8.6 Embedder(ort)+ 预处理 + tokenizer**:squash-resize 黑底 256²、NCHW、mean/std、tokenize 截断 64 右填 0、输出 dim768 断言。验收:预处理输出尺寸/通道/范围正确;tokenize 长度恒 64、超长截断、短补 0;`encode_image/encode_text` 返回 768 维;**(若有 candle 后端)同图同文两后端余弦 >0.999**;`normalized` 开关行为正确。
- **T8.7 VisualSearch 排名**:矩阵·向量、best-per-shot(同分保留先出现)、minScore 过滤、`prefix(limit)` 再 `filter(floor)` 顺序、top≤0 空。验收:构造已知向量集断言命中顺序/去重/截断边界与上游 `VisualSearch.search` 逐用例一致(尤其「先 limit 后 floor」最终 ≤ limit)。
- **T8.8 VisualIndexer**:视频 shot 累积(首镜头起点强制 0、shotEnd=下一镜头起点/末镜头=duration)、图片单 embedding 零长 shot、needsIndex 幂等、导出让路 + 取消。验收:对已知镜头视频断言 rows 的 `shot_start/shot_end`;重复 index 跳过(幂等);取消中途 `Cancelled`。
- **T8.9 ModelDownloader**:reqwest 流式下载 + 加权进度 + 流式 SHA256 + zip 解压(单顶层条目)+ 原子安装 + 幂等。验收:mock HTTP 下载三文件、进度单调到 1、错校验和→`Checksum`、安装目录结构正确、二次 install 直接返回。
- **T8.10 ort worker 框架**:`OrtModel::{load,run}`、`OrtWorker::submit`(序列化 + 导出暂停)、`OrtModelRegistry` 懒加载、EP 回退 CPU、tensor 辅助。验收:加载 SigLIP image encoder 跑通(即 `OrtEmbedder` 复用它);EP 不可用回退 CPU 有日志;worker 串行执行、导出暂停期间不取模型。
- **T8.11 IndexCoordinator**:schedule 条件、单 worker、dequeue 跳过失效 id、index_one 进度分配(转写 0.5)、并发转写+视觉、failed 集、search 快照 off-thread、ExportPause 引用计数 + 2s 轮询。验收:入队/去重/失败重试(failedIds 仅批内去重)行为与上游一致;导出 begin 后 worker 暂停、end 后恢复;search 空 query 返回空、命中合理。

## 持续校准(非阻塞,跨 Phase)

- **导出画质对齐**:逐预设调码率/profile/色彩,与上游 `AVAssetExportSession` 导出对比(画质/时长/音画同步)。`docs/_analysis/02` 风险登记列为 🟠 中、非阻塞核心。
- **L2 归一化标定**:确认导出 SigLIP2 是否图内归一,据此设 `EmbedderSpec.normalized`,使裸点积分数语义与上游一致(§0.8)。
- **波形视觉等价**:若后续要求与上游逐位一致,切 `WaveformMode::Peak` 并标定缩放(§4.3)。
