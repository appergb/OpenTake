# MediaPanel 移植规格

**职责**:
- 素材库三种视图渲染：folder(面包屑钻入)/flat(全部素材)/grouped(按文件夹分区折叠)，含网格列数/瓦片宽度自适应计算
- 素材与文件夹的选择模型：单选/Shift 多选/橡皮筋框选(marquee)，键盘方向键导航(基于已发布的有序 id 列表 + 列数)
- 拖拽：自定义 URI 协议(palmier-asset://id 与 palmier-folder://id，搜索片段带 #start-end 源秒)，asset→时间线/文件夹、folder→folder 移动
- 导入：NSOpenPanel/Finder 拖放/剪贴板粘贴(URL 与 PNG/TIFF 图像数据)，目录递归镜像为文件夹树，单次导入作为一个撤销步骤
- 文件夹 CRUD：新建/重命名/删除(级联删除子树及其素材与引用片段)/移动(含环路防护)，全部走 UndoManager
- 搜索：本地三类结果——文件名匹配、Spoken(本地转写关键词命中)、Moments(本地 SigLIP 视觉向量检索)，250ms 防抖
- 素材缩略图卡片：缩略图/时长徽章/AI 徽章/离线缺失态/生成中态/失败态、双击重命名、右键菜单(Relink/Reveal/Copy Path/Delete/AIEdit)、悬停加入聊天
- 媒体替换(swap)模式横幅：高亮兼容素材(同 mediaType)，点击完成替换
- 字幕生成：源选择(选中片段/指定轨道/Auto 主讲轨)、样式(字体/字号/颜色/背景/大小写/脏话过滤)、画布归一化定位(带 0.5 吸附)、调用本地转写并把转写段切分为屏显字幕片段放到新建文本轨
- 音乐生成：视频转音乐/文本转音乐两种模式、模型选择、按所选时间线范围或整条时间线取源、积分成本估算、提交到云端生成服务并落到音频轨
- 可视搜索模型(SigLIP)下载/索引进度状态条展示

**核心类型**:
- `MediaPanelView` (struct) — 面板根视图：左侧竖直 tab rail(Media/Captions/Music)+ 内容区，处理 tab 切换动画与悬浮标签
- `MediaTab` (struct) — 媒体库主视图(824 行，通过 +Grids/+Drag/+Search/+IndexStatus 扩展拆分)。持有视图状态：sortMode/filterTypes/filterAI/searchQuery/thumbnailSize/viewMode/currentFolderId/选区/框选/搜索命中等
- `MarqueeSelection` (struct) — 橡皮筋框选状态：当前矩形、是否激活、Shift 扩选时的基线选中集(资产+文件夹)
- `MediaTab.MediaCell / GridLayoutInfo / GridDimensions` (struct) — 网格单元(folder/asset 二选一)与布局结果(列数/瓦片宽/间距/单元数组/有序 id)
- `AssetThumbnailView` (struct) — 单个素材缩略图卡片：渲染所有素材状态、拖拽源、重命名、右键菜单、点击/Shift 点击选择与 swap 完成
- `FolderTileView` (struct) — 文件夹瓦片：图标+子项计数徽章、内联重命名、单击选中/双击打开(自管双击间隔)、右键菜单
- `MediaPanelDropArea` (struct) — NSViewRepresentable 包裹 NSHostingView，用原生 AppKit 拖放接收 Finder 文件 URL(规避 SwiftUI 父级 .onDrop 遮蔽子级的 macOS 缺陷)
- `CaptionTab` (struct) — 字幕生成 UI：源/语言/字体/字号/颜色/背景/大小写/脏话过滤/画布定位预览，组装 CaptionRequest 调用 editor.generateCaptions
- `CaptionBuilder` (enum) — 纯算法(无 UI/无苹果框架)：把一段转写文本递归切分为屏显短语并按字符数分配时间、施加最小时长与防重叠位移、再映射为文本片段 spec
- `MusicTab` (struct) — AI 配乐 UI：模式/模型/时长/提示词/源范围/成本估算，组装 MusicGenerationSubmission 提交云端生成
- `MomentThumbnail` (struct) — 搜索结果异步抽帧缩略图(AVAssetImageGenerator 在指定时间点取帧)
- `AssetFramePreferenceKey` (struct) — SwiftUI PreferenceKey：收集每个单元在网格坐标系中的 frame，供框选命中测试与滚动定位

**核心算法/逻辑(供 Rust 复刻)**:
- 【网格列数与瓦片宽度】gridDimensions(width): spacing=AppTheme.Spacing.xl; outerPadding=Spacing.md*2; usable=max(0,width-outerPadding); cols=max(1, floor((usable+spacing)/(thumbnailSize+spacing))); tileWidth=max(thumbnailSize, (usable-(cols-1)*spacing)/cols)。thumbnailSize 预设 small/medium/large/xlarge = 80/110/150/200。folder 模式单元顺序=先子文件夹(按名称本地化升序)后资产(经 sortAndFilter)。
- 【排序与过滤】sortAndFilter: 先 filter(passesFilters) 再排序。passesFilters: typeOk = filterTypes 为空或包含 asset.type; aiOk = 非 filterAI 或 asset.isGenerated; nameOk = 查询去空白后为空或 name 本地化不区分大小写包含。排序 dateAdded=保持原序(不排); name=名称本地化升序; duration=时长降序; type=type.rawValue 升序。
- 【橡皮筋框选】DragGesture minimumDistance=3，坐标系 named\"mediaGrid\"。起点若落在任一已记录单元 frame 内则不启动框选(让位给单元拖拽)。启动时若按住 Shift 则以当前选中集为基线扩选，否则清空基线。每次变化重算矩形=由起点与当前点取 min 角与 abs 宽高，遍历 assetFrames 中与矩形相交者：key 若是 \"folder-<id>\" 加入 folderIds，否则加入 assetIds，并写回 editor 选区(变化才写)。结束时 reset。
- 【键盘方向导航】moveMediaSelection(direction): 基于已发布的 mediaPanelOrderedItemIds 与 mediaPanelColumnCount。step: left=-1/right=+1/up=-cols/down=+cols。锚点取有序列表中最后一个被选中项；raw=idx+step，target=clamp 到 [0,count-1]；若 target==idx 不动。无选中时 left/up 从末尾、right/down 从开头开始。
- 【拖拽 URI 协议】folderDragScheme=\"palmier-folder://\"，assetDragScheme=\"palmier-asset://\"。资产串=assetScheme+id；搜索片段串=assetScheme+id+String(format:\"#%.3f-%.3f\",start,end)(源秒，3 位小数)。多选拖拽=每行一个资产串以\\n连接。解析 assetId 取 scheme 后到 '#' 前；解析 segment 取 '#' 后按 '-' 分两段，要求 count==2、start>=0、end>start，返回 start...end。
- 【文本拖放解析(移动)】resolveTextDrop: 按\\n分行，folderId(fromDragString) 命中→folderIds；否则 assetId 命中且素材存在→assetIds。非空则分别调用 moveAssetsToFolder / moveFoldersToFolder 到目标文件夹。
- 【剪贴板粘贴导入】handleClipboardPaste 优先读 NSURL 列表当作 Finder 导入；否则依次尝试 .png/.tiff 数据，写入临时/项目 media 目录后作为新图像素材导入，并移动到当前文件夹。clipboardHasImportableMedia: 存在 .fileURL/.png/.tiff 之一。
- 【目录导入镜像】importFolder 递归：为该目录 createFolder，contentsOfDirectory(skipsHiddenFiles) 后按 lastPathComponent 的 localizedStandardCompare 升序遍历；子目录递归，文件经 ClipType(fileExtension:) 识别后 addMediaAsset。整个 importFinderItems 期间 disableUndoRegistration，结束后只注册一次撤销快照(动作名 Import Media)。
- 【支持的文件扩展名→类型】mov/mp4/m4v=video; mp3/wav/aac/m4a=audio; png/jpg/jpeg/tiff/heic/webp=image; json/lottie=lottie(json/lottie 还需 LottieVideoGenerator.isLottie 校验)。其它=不支持(toast 提示)。NSOpenPanel 允许类型: movie/image/audio/json + lottie 扩展。
- 【文件夹树与环路防护】MediaFolderIndex 用 byId 与 childrenByParent 两张表。path(for:) 自底向上收集祖先(visited 去重防环)再反转。isDescendant(folderId,of ancestorId): 自 folderId 向上找到 ancestorId 即真(visited 防环)。moveFoldersToFolder 跳过: 父未变、目标是自身后代(防环)、目标==自身。idsIncludingDescendants 用于级联删除。
- 【级联删除文件夹】deleteFolders: 取 ids 及其全部后代 → 计算其下所有资产 id → 计算时间线上引用这些资产的片段 id；先从选区与各轨道移除这些片段并 pruneEmptyTracks，再删资产/manifest 条目/文件夹，更新选区，关闭相关预览 tab。整体用 mediaLibraryUndoSnapshot 前置快照注册撤销(动作名 Delete Folder)。删除媒体资产 deleteMediaAssets 同理(会连带删除引用片段)。
- 【撤销策略】文件夹父级变更(移动)用 applyParentChanges：记录每项旧值作为 inverse，写新值，撤销时以 inverse 反向再调用自身。重命名/新建用轻量逐字段反向闭包。涉及时间线结构变化(删除/字幕轨插入)用整体快照 MediaLibraryUndoSnapshot(timeline+manifest+mediaAssets+各类选区+预览 tab+源播放头)做前后替换。
- 【时间换算 helper】secondsToFrame(seconds,fps)=Int(seconds*fps)(截断取整，非四舍五入)。frameToSeconds=frame/fps。formatTimecode 为 HH:MM:SS:FF。搜索结果里 timecode(seconds) 显示为 m:ss 或 h:mm:ss(四舍五入到秒)。
- 【字幕分句(CaptionBuilder.split)】对一段文本(去首尾空白)：若 fits(整段)则不切；否则 breakOnce 一次，若切出>1 段则对每段递归 split，否则原样返回(单个超长词不再切)。breakOnce 优先级: 句末标点 .!? → 从句标点 ,;: → 词中点 breakAtMidWord。
- 【按标点切分(breakOn)】仅在‘标点且其后是空格或文末’处断开(因此 U.S.、3.14 不被切)；逐字符累积，命中断点则裁剪当前片段(去空白非空才收)。返回片段数>1 才算成功(否则 nil 让位下一级)。breakAtMidWord: 按空格分词，词数>1 时在 count/2 处对半分。
- 【字幕计时分配(distribute)】把 segment 的 [start,end] 按各片段字符数(每段至少计 1)等比分配，首尾相接：dur=span*max(len,1)/总字符数，依次累加 t。span=max(end-start,0)。
- 【字幕最小时长与防重叠(enforceMinDuration)】逐个：若 end-start<minDuration 则 end=start+minDuration(minDuration 取 AppTheme.Caption.minDisplayDuration=0.7s)；若下一条 start< 当前 end，则把下一条整体右移 shift=当前 end-下一条 start(start 与 end 同步加)。注意只看相邻一对，是单向级联。
- 【字幕片段→时间线映射(CaptionBuilder.specs)】对每个短语 p(源秒): 计算源片段可见源区间 visibleStartSource=trimStartFrame, visibleEndSource=visibleStartSource+durationFrames*max(speed,0.0001)(源帧)。phraseStartSource=p.start*fps, phraseEndSource=p.end*fps；若短语与可见区间无交叠(phraseEnd<=visStart 或 phraseStart>=visEnd)则丢弃。再用 Clip.timelineFrame(sourceSeconds:fps:) 把 p.start/p.end 映射为时间线帧；映射失败回退到片段 start/end 帧。durationFrames=max(minDurationFrames=1, min(clip.endFrame,e)-max(clip.startFrame,s))，即裁剪到所属片段范围内。
- 【源秒↔时间线帧(Clip.timelineFrame)】sourceFrame=t*fps; offsetFromTrim=sourceFrame-trimStartFrame，若<0 返回 nil; frame=round(startFrame+offsetFromTrim/max(speed,0.0001)); 若 frame 不在 [startFrame,endFrame) 内返回 nil。这是字幕落点与搜索预览跳转的统一换算。
- 【字幕生成总流程(generateCaptions)】候选=autoDetect?所有可转写片段:指定 id 的可转写片段。可转写判定(captionTargets/captionCanTranscribe): mediaType 为 video/audio；且素材为 audio 或(video 且 hasAudio)；对带 linkGroup 的 video，若该组存在 audio 片段则排除该 video(优先用音频轨)。按 startFrame 升序。逐 mediaRef 转写(同 ref 只转一次)，转写范围=该 ref 在候选中所有片段可见源区间的并集±1s 余量(/fps 转秒、下限 0)。autoDetect 时统计各轨道命中词数(取每词中点落在片段可见源区间内)，选词数最多的轨道为唯一保留轨。把转写段经 CaptionBuilder.phrases 切分，按‘与片段可见源区间重叠最大且重叠>=短语时长一半’归属到某片段，套用大小写后生成 spec，最后在时间线索引 0 处插入一条新 video 轨放置全部文本片段(整体作为一个 Generate Captions 撤销步骤)。
- 【字幕样式与定位】TextStyle 默认 fontName=Helvetica-Bold；面板默认 fontSize=48(AppTheme.Caption.defaultFontSize)，范围 12..300。center 默认 (0.5,0.9) 归一化画布坐标。X/Y 输入以百分比显示(displayMultiplier=100)，范围 0..1。吸附: |v-0.5|<0.02 时吸到 0.5，并显示中心参考线。文本框自然尺寸 TextLayout.naturalSize: 以 1080 为参考画布高做缩放，按 NSAttributedString.boundingRect 测量，宽度上限=画布宽*0.9，开启阴影加 shadowPadding(12)*2，再加 4px slack；最终 Transform 用 natural.width/canvasW、natural.height/canvasH 归一化。captionLineFits: 单行自然宽 <= 画布宽*0.9 视为放得下。
- 【媒体替换 swap】isAssetCompatibleWithPendingSwap: clip.mediaType==asset.type(严格同类型，非 isVisual 宽松)。completeMediaSwap: 类型不符给 toast 并保持挂起；相同 mediaRef 直接返回；否则 replaceClipMediaRef。replaceClipMediaRef 默认 resetTrim=false——只改 mediaRef，保留 trim/speed/keyframes/transform 等全部状态，并对同组链接且共享同一旧媒体的片段一并替换(撤销恢复旧 mediaRef 与旧 trim)。
- 【离线/缺失判定】isMediaOffline = offlineMediaRefs∪unprocessableMediaRefs∪mediaResolver.isMissing。生成中/下载中/失败态的素材不算 offline(各有专属态)。缩略图边框: 缺失=红色 thick；swap 模式=兼容且悬停才高亮主色 thick；否则选中=主色 thick。
- 【搜索三段式】trimmedSearchQuery 非空时进入搜索视图。Moments=本地 SigLIP 向量检索(VisualSearch.search 用 cblas_sgemv 算 query·向量得分，按 shotStart 每镜头只留最高分帧，按分降序，top 限 20，相对截断 floor=最高分*0.85，并有 minScore 余弦下限)。Spoken=TranscriptSearch 在磁盘缓存转写中做‘所有词项不区分大小写/变音命中’匹配(limit 20)。Files=文件名 sortAndFilter 结果。查询变更 250ms 防抖后并行执行，任务可取消。点击结果调用 selectMediaAsset(atSourceFrame: secondsToFrame(seconds,fps)) 跳转预览。
- 【缩略图生成(MediaAsset.loadMetadata)】image: duration=Defaults.imageDurationSeconds，用 ImageEncoder 取尺寸与缩略图(maxPixel 1568)。lottie: LottieVideoGenerator.inspect 取时长/尺寸/帧率/缩略图。video: AVURLAsset 取首个视频轨 naturalSize 经 preferredTransform 校正得正确朝向尺寸、nominalFrameRate、timeRange 时长；AVAssetImageGenerator(maximumSize 320、appliesPreferredTrackTransform) 在 .zero 取首帧作缩略图(用真实像素尺寸避免 16:9 挤压)；并探测是否有音频轨。audio: 直接取 duration。

**苹果框架使用**:
- SwiftUI [high] — 全部三个标签页 UI、LazyVGrid/LazyVStack 网格、DragGesture 框选、.draggable 拖拽源、PreferenceKey 收集单元 frame、ScrollViewReader 滚动定位、@Observable/@Environment 状态绑定
- AppKit [high] — NSOpenPanel(导入选择)、NSPasteboard(剪贴板粘贴/复制路径)、NSWorkspace.activateFileViewerSelecting(在 Finder 显示)、NSEvent(modifierFlags/doubleClickInterval/keyCode)、NSHostingView/NSView 原生拖放(MediaPanelDropArea/KeyCommandSink)、NSImage 缩略图、NSBitmapImageRep PNG 编码
- AVFoundation [medium] — AVURLAsset/AVAssetImageGenerator 抽帧缩略图(素材首帧、搜索 moment 指定时间帧、整帧导出为素材)；AVAssetReader/AVAssetReaderTrackOutput 解码音频轨为 16k 单声道 PCM、AVAudioFile/AVAudioPCMBuffer 写 caf 供转写；AVAssetExport/合成参与帧捕获
- CoreMedia [low] — CMTime/CMTimeRange 表示抽帧时间与转写源区间、CMSampleBuffer 取 PCM 数据与格式描述
- Speech [blocker] — SpeechAnalyzer + SpeechTranscriber 做完全本地化语音转写：supportedLocales、AssetInventory 模型下载安装、etiquetteReplacements 脏话过滤、audioTimeRange 词级时间戳、按 Result(段)与 run(词)解码为 TranscriptionResult
- CoreText/NSAttributedString [medium] — TextLayout.naturalSize 用 NSAttributedString.boundingRect + NSFont(name:size:) 测量字幕文本框尺寸，决定字幕换行/是否放得下/Transform 归一化尺寸
- CoreImage/ImageIO [low] — 经 ImageEncoder.metadata 读取图片尺寸与生成缩略图(maxPixel 1568)
- CoreGraphics [low] — CGImage(缩略图/抽帧)、CGContext 合成视频帧+文本层为 PNG、CGRect 框选与坐标计算
- Accelerate(BLAS) [low] — VisualSearch.search 用 cblas_sgemv 做 query 向量与素材帧向量矩阵的点积打分(视觉语义检索核心)
- UniformTypeIdentifiers [none] — UTType 判定导入类型(movie/image/audio/json/lottie)、拖放标识符(.fileURL/.text)、剪贴板类型

**闭源云**:是。MediaPanel 本身只在两处触达闭源云：(1) Music 标签页：MusicGenerationSubmission.run 通过 GenerationService/GenerationBackend 工作——视频转音乐会先本地渲染低分辨率 mp4，再 GenerationBackend.uploadReference(经 ConvexMobile 申请 Convex 存储票据 + URLSession 上传)，随后提交云端生成任务；成本估算/积分/登录态来自 AccountService(ClerkKit+ClerkConvex+ConvexMobile，Google OAuth)。(2) 三个标签页的 AI 入口：Media 的 Generate 与 Organize with Agent、Captions/Music 的 Agent Mode、缩略图悬停"加入聊天"，都会唤起 agentService/GenerationView，最终走 Convex+Clerk 后端的生成式 AI(PalmierClient)。其余功能全部本地：语音转写(Speech 框架，按需下载苹果模型)、视觉搜索索引与检索(本地 SigLIP/CoreML，模型从 SearchIndexConfig.baseURL 这一静态 CDN 下载，非生成式 AI 云)、缩略图/波形/导入/文件夹操作/字幕分句计时全部离线。

**移植策略**:整体定位为 ui-rebuild：UI 用 React/TS 重写，但内嵌的纯算法应 direct-port 到 Rust，闭源云需 cloud-rebuild。分项策略——(A) direct-port 到 Rust core：CaptionBuilder 全套(split/breakOn 标点规则/breakAtMidWord/distribute 按字符等比分配/enforceMinDuration 单向级联防重叠)、Clip.timelineFrame 源秒↔帧映射、CaptionBuilder.specs 裁剪逻辑、网格列数/瓦片宽公式、文件夹树 MediaFolderIndex(path/isDescendant/级联删除/环路防护)、拖拽 URI 编解码(palmier-asset://、#%.3f-%.3f)、键盘方向导航、sortAndFilter/passesFilters、撤销快照模型(在 Rust 用命令/快照栈替代 NSUndoManager)、TranscriptSearch 关键词匹配、VisualSearch 打分(cblas_sgemv 换 ndarray/nalgebra 或 BLAS crate，逻辑一致)。注意 secondsToFrame=Int(seconds*fps) 是截断不是四舍五入，timelineFrame 内部才 round——必须逐处对齐取整方式。(B) needs-replacement：所有 AVFoundation/CoreMedia 媒体操作(缩略图抽帧、指定时间取帧、音频抽轨为 16k 单声道 PCM、整帧导出)改用 FFmpeg(ffmpeg/ffprobe 或 ffmpeg/ffprobe via ffmpeg-sidecar)；文本测量 NSAttributedString.boundingRect 改用前端 Canvas measureText 或 Rust 端 cosmic-text/rusttype/harfbuzz 复刻换行与尺寸(字幕落点依赖测量结果，需保证与渲染一致)；图片元数据/缩略图用 image crate。(C) blocker→cloud-or-on-device-replacement：Speech 框架本地转写无法直接移植，换 whisper.cpp/whisper-rs(本地)或转写服务，须自行产出 segment(段级时间)+word(词级时间戳)以驱动 CaptionBuilder 与 Spoken 搜索；视觉搜索 SigLIP+CoreML 换 ONNX Runtime/candle 跑同类图文对比模型，向量检索逻辑可保留。(D) cloud-rebuild：Music 生成与所有 Agent/Generate 入口依赖 Convex+Clerk 闭源后端，需替换为 OpenTake 自有后端/MCP server(上传参考、提交生成、积分与鉴权)。(E) UI 平台适配：NSOpenPanel→Tauri dialog、NSPasteboard→Tauri clipboard、Finder 显示→Tauri opener、原生拖放遮蔽问题在 Web 端不存在(用标准 HTML5 DnD/文件拖放即可)。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MediaTab/MediaTab.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MediaTab/MediaTab+Grids.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MediaTab/MediaTab+Drag.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MediaTab/MediaTab+Search.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/CaptionsTab/CaptionBuilder.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MusicTab.swift

