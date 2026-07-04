# Export 移植规格

**职责**:
- 编排三条导出流水线并暴露统一的 export 入口(ExportService)，根据 ExportFormat 分流：xml 走早返回纯计算路径，h264/h265/prores 走 AVAssetExportSession 渲染路径，palmier 走打包路径
- 计算导出分辨率：以画布短边为基准把短边缩放到 720/1080/2160，长边按比例缩放并向下取偶数(编码器要求宽高为偶数)，最小 2x2(ExportResolution.renderSize)
- 把 ExportFormat+ExportResolution 映射成 AVFoundation 的导出预设名(exportPresetName)
- 驱动 CompositionBuilder 构建 AVComposition/audioMix/videoComposition，再用 TextLayerController.buildForExport 通过 AVVideoCompositionCoreAnimationTool 把文字图层烤进视频
- 以 200ms 轮询 AVAssetExportSession.progress 上报进度；区分用户取消(NSUserCancelledError)与真实失败
- 把 Timeline 序列化成 XMEML 4 XML：轨道→clipitem→file/filter/transition/link，覆盖裁剪/变速/音量/不透明度/变换/裁切/淡入淡出/AV 链接(XMLExporter)
- 把工程导出为 .palmier 包：去重收集所有可解析媒体到 media/ 目录，重写 manifest 的 source 为工程相对路径，附带 project.json/media.json/generation-log.json/缩略图/聊天记录(PalmierProjectExporter)
- 提供导出对话框 UI：格式/编码/分辨率选择、首帧预览、时长与体积估算、进度与错误展示、系统保存面板(ExportView)
- 导出开始/结束时通知 SearchIndexCoordinator 暂停/恢复后台索引

**核心类型**:
- `ExportService` (class) — @MainActor @Observable 导出协调器。持有 progress/isExporting/error 三个可观察状态。三个入口：export(视频与XML)、exportPalmierProject(打包)、私有 makeExportSession(组装 AVAssetExportSession)。isExporting 的 didSet 触发搜索索引暂停/恢复
- `ExportFormat` (enum) — 导出底层格式：h264/h265/prores/xml。携带 fileExtension(mp4/mov/xml) 与 utType(AVFileType；xml 为 nil)
- `ExportResolution` (enum) — 导出分辨率档位 720p/1080p/4K。shortSidePixels 给出目标短边像素；renderSize(for:) 按画布短边等比缩放并取偶数
- `ExportMode / VideoCodec` (enum) — UI 层选择枚举。ExportMode={video,xml,palmierProject}；VideoCodec={h264,h265,prores}。ExportView 把它们组合映射到底层 ExportFormat
- `XMLExporter` (enum) — XMEML 4 导出器命名空间。export() 是入口；内部私有 final class Builder 持有所有发射状态(已发射文件集合、clip 地址表、link 分组表、源起始时码缓存)并自底向上构建 XMLNode 树
- `XMLExporter.Builder` (class) — 真正的 XMEML 构建器。build() 产出文档骨架；逐 track→clipitem→file/filter/transition/link 发射；负责帧↔时码↔SMPTE 换算、NTSC 判定、坐标系转换、关键帧采样
- `XMLNode` (struct) — 极简 XML 树节点(name/attributes/text/children)。render() 独占缩进与转义；el/leaf/bool 是构造助手。保证没有任何片段硬编码空白字符
- `PalmierProjectExporter` (enum) — .palmier 包导出器命名空间。export() 在临时 staging 目录收集媒体并去重、重写 manifest、写三个 JSON、搬运附属文件，最后原子 move 到目标。Report 记录 collected/copiedInternal/missing/totalBytes
- `PalmierProjectExporter.Report` (struct) — 打包结果报告：collected(原 external 现已内联的 id)、copiedInternal(已内部媒体复制数)、missing(找不到源文件的条目)、totalBytes(复制总字节)
- `ExportView` (struct) — SwiftUI 导出对话框(860x560)。左侧设置面板+右侧首帧预览+底部信息栏与导出按钮。从 EditorViewModel 取 timeline/manifest/resolver/projectURL/generationLog
- `Clip(外部依赖,Models)` (struct) — 导出的核心数据单元。携带 startFrame/durationFrames/trimStart/trimEnd/speed/volume/opacity/transform/crop/fade/linkGroupId 及六条关键帧轨道。提供 endFrame、sourceFramesConsumed、sourceDurationFrames 等派生量与 *At(frame:) 采样函数
- `MediaResolver(外部依赖,Models)` (class) — 把 assetId 解析为文件 URL/显示名/manifest 条目。resolveURL 会校验文件存在性，不存在返回 nil(导出据此判定 offline)
- `CompositionBuilder(外部依赖,Preview)` (enum) — 视频导出的真正渲染引擎(非 Export 目录，但 makeExportSession 调用它)。把 Timeline 编译成 AVMutableComposition + audioMix + AVVideoComposition，负责变速/裁切/变换/不透明度/音量包络的逐帧指令发射
- `TextLayerController(外部依赖,Preview)` (class) — 文字图层控制器。buildForExport 生成一棵 CALayer 树(每个文字 clip 一个 CATextLayer + 离散 CAKeyframeAnimation 控制可见性)，交给 AVVideoCompositionCoreAnimationTool 烤进导出视频

**核心算法/逻辑(供 Rust 复刻)**:
- [单位约定] 全系统时间单位是整数帧。秒→帧 secondsToFrame = Int(seconds * fps)(向零截断，非四舍五入)；帧→秒 = frame/fps。Clip.endFrame = startFrame + durationFrames(半开区间[start,end))。Track.endFrame = 各 clip endFrame 最大值。Timeline.totalFrames = 各 track endFrame 最大值。
- [导出分辨率算法 ExportResolution.renderSize] canvasShort=min(画布宽,高)，若<=0 直接返回原画布；scale = 目标短边像素 / canvasShort；w = (Int((画布宽*scale)四舍五入) / 2) * 2；h 同理；最终 max(2, w) x max(2, h)。注意是先四舍五入再整除2乘2向下取偶，保证编码器要求的偶数宽高。
- [导出预设映射 exportPresetName] h264: 720p→1280x720, 1080p→1920x1080, 4K→3840x2160；h265: 720p 和 1080p 都→HEVC1920x1080(720p 实际上被提升到1080p), 4K→HEVC3840x2160；prores: 恒为 AppleProRes422LPCM(忽略分辨率档位，由 renderSize 决定实际尺寸)。Rust+FFmpeg 应改为直接用 libx264/libx265/prores_ks 编码器并按 renderSize 设宽高，码率自定。
- [视频导出主流程 ExportService.export 非xml分支] 1)由 timeline.width/height 组画布尺寸，算 renderSize；2)CompositionBuilder.build 产出 composition+audioMix+videoComposition；3)取 utType(nil 则抛 invalidFormat)；4)AVAssetExportSession 失败若文件已存在，故先 try? 删除 outputURL；5)session.audioMix=结果音轨混音；6)TextLayerController.buildForExport 得到(parent, videoLayer)，把 videoComposition 做 mutableCopy 后设 animationTool=AVVideoCompositionCoreAnimationTool(postProcessingAsVideoLayer:videoLayer,in:parent)，再赋回 session.videoComposition；7)开后台 Task 每 200ms 读 session.progress 写回 self.progress；8)session.export(to:as:)；成功 progress=1，失败区分 NSCocoaErrorDomain+NSUserCancelledError(显示'Export was cancelled')与其它错误。
- [XMEML 文档骨架 build()] 根 <xmeml version=4> > <sequence id=sequence-1> 含 name='Timeline Export'、duration=totalFrames、rate、timecode(00:00:00:00/NDF)、<media>。<media> 内 <video>(格式节点+视频轨节点) 与 <audio>(numOutputChannels=2、格式节点、outputs、音频轨节点)。文件头固定 '<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE xmeml>\n'。render 缩进步长=2 空格。
- [XMEML 轨道顺序 关键] 视频轨：模型存储为上→下，FCP XML 要求下→上，所以 videoTracks = timeline 中 type.isVisual 的轨道 reversed()。音频轨：保持原序(type==.audio)。每条轨内 clip 按 startFrame 升序排序，且只保留 resolver.resolveURL 非 nil 的 clip(sortEmittable 丢弃离线媒体，使 link 索引与实际发射一致)。
- [XMEML clip 可见性过滤] sortEmittable 过滤掉无法解析 URL 的 clip。文字 clip 不在此被特判，但因 XMEML 不支持文字且文字媒体通常无法解析为视频/音频文件，实践上不会进入。文档明确声明文字不导出。
- [XMEML clipitem 发射 clipItemNode] 子节点顺序固定：masterclipid、name(=resolver.displayName)、enabled=TRUE、duration(=源时长帧 sourceDurationFrames(for:)，读不到则用 clip.sourceDurationFrames)、rate、start=clip.startFrame、end=clip.endFrame、in=trimStartFrame、out=trimStartFrame+sourceFramesConsumed、file 节点，然后追加(若 speed!=1)Time Remap 滤镜、音/视频滤镜、link 节点。clipitem id='clipitem-<clip.id>'。
- [XMEML in/out 与变速的关系 关键] in/out 是源帧偏移，跨度 = sourceFramesConsumed = round(durationFrames*speed)。start/end 是时间线帧，跨度 = durationFrames。二者比例即变速，但 Premiere 不会自行推断，必须显式发 Time Remap 滤镜(见下)。
- [XMEML masterclipId 规则] 若 clip.linkGroupId 存在 → 'masterclip-<group>'(让 A/V 共享 masterclip)；否则 → 'masterclip-<mediaRef>-<audio|video>'(按媒体类型分离)。
- [XMEML file 节点与去重 fileNode] fileId='file-<mediaRef>-<audio|video>'(必须按媒体类型分离 id，否则 Premiere 拒绝 clipitem 指向错误类型的 file)。用 Set<FileKey{mediaRef,isAudio}> 去重：首次完整发射，重复出现只发自闭合 <file id=.../>。完整节点含 name、pathurl、rate、duration、timecode、media。
- [XMEML pathurl 形式 关键坑] 路径 = url.absoluteString 把 'file://' 替换为 'file://localhost//'(Premiere 需要这种多斜杠主机形式，规范的单斜杠会失败)；解析不到 url 时回退 'media/<mediaRef>'。Rust 移植需复刻这个非标准前缀。
- [XMEML file duration] 图片(entry.type==.image)恒为 1 帧；否则 = max(0, secondsToFrame(entry.duration, fps))，读不到 entry 则 0。图片的 <media><video> 额外内嵌一个 <duration>1。
- [XMEML 源帧率→timebase/ntsc rateTags] timebase = max(1, round(rawFps))；ntscRate = timebase*1000/1001；若 |rawFps-ntscRate| < |rawFps-timebase| 则 ntsc=TRUE。即 29.97/23.976/59.94 这类判为 NTSC。源 fps 取 entry.sourceFPS，缺省用时间线 fps。
- [XMEML 源起始时码读取 readStartTimecodeFrame] 用 AVAssetReader 读 .timecode 轨第一个 sample 的 data buffer 前 4 字节(大端 UInt32)作为起始帧。跳过没有 data buffer 的前导编辑边界。结果按 mediaRef 缓存(startFrameCache)，video/audio 共用一次读取。无时码轨返回 nil→用 0。FFmpeg 移植：用 ffprobe 读 timecode 流或 tmcd，缺失则 0。
- [XMEML SMPTE 时码格式化 formatTimecode] 非丢帧用 ':' 分隔；丢帧(ntsc 且 timebase 是 30 的倍数)用 ';' 分隔并补偿丢帧。丢帧补偿:drop=round(fps*0.066666)(30→2,60→4)；d=f/(fps*600), m=f%(fps*600)；f += drop*9*d + (m>drop ? drop*((m-drop)/(fps*60)) : 0)。然后 ff=f%fps, ss=(f/fps)%60, mm=(f/(fps*60))%60, hh=f/(fps*3600)，格式 %02d<sep>%02d<sep>%02d<sep>%02d。
- [XMEML 淡入淡出→单边转场 fadeTransition] 淡入淡出不走 clip-to-clip，而是发单边 dissolve 到黑/静音。frames=clip.fadeFrames(edge)，0 则不发。左边(淡入):start=clip.startFrame, end=start+frames, alignment='start-black', cutFrames=0；右边(淡出):start=endFrame-frames, end=endFrame, alignment='end-black', cutFrames=frames。音频用 effect 'Cross Fade ( 0dB)'/id KGAudioTransCrossFade0dB；视频用 'Cross Dissolve' 并额外发 cutPointTicks=Int64(cutFrames)*(254016000000/fps)(Premiere 私有切点，单位 ticks=254016000000/秒) 及 wipecode/wipeaccuracy/startratio/endratio/reverse 参数体。转场节点名 <transitionitem>，发射位置：淡入在 clipitem 之前，淡出在之后。
- [XMEML 变速→Time Remap 滤镜 timeRemapFilter] speed==1 不发。参数:variablespeed=0、speed=value(格式 '%.4f' 的 speed*100，即百分比)、reverse=FALSE、frameblending=FALSE。effect id=timeremap, type=motion。
- [XMEML 音量→Audio Levels 滤镜 volumeFilters] level 是线性值(1=0dB)，clamp 到 [0,3.98]。无关键帧时:若 volume==1.0 不发；否则发静态 level=clamp(volume)。有关键帧时:取 volume 关键帧绝对帧集合(keyframeFrames(.volume))，每帧 when=帧-startFrame, value=clamp(rawVolumeAt(frame))(注意用 rawVolumeAt 即不含淡入淡出，因为淡入淡出已单独走转场)，base=首关键帧值。格式 '%.4f'。effect id=audiolevels。
- [XMEML rawVolumeAt 语义] = volume(静态外层增益) * kfGain，其中 kfGain：若 volumeTrack 激活则 VolumeScale.linearFromDb(采样dB) 否则 1.0。不含 fadeMultiplier。VolumeScale.linearFromDb(db): db<=-60 返回 0；否则 pow(10, min(db,15)/20)。dbFromLinear 反向:linear<=0→-60；否则 clamp(20*log10(linear), -60, 15)。
- [XMEML 变换→Basic Motion 滤镜 motionFilter 关键] 缩放百分比 scalePct(width): 若源宽>0 则 (seqWidth/sourceWidth)*width*100 否则 width*100(把归一化宽换算成相对源像素的百分比)。中心 center(t)=(centerX-0.5, centerY-0.5)(FCP7 用以画布中心为 0 的归一化坐标)。旋转取负(-rotation；FCP7 逆时针为正，模型顺时针为正)。采样帧集合=position∪scale∪rotation 关键帧并集排序。无关键帧时只在超过阈值才发:needsCenter=|cx|>0.001 或 |cy|>0.001；needsScale=|scaled-100|>0.1；needsRotation=|rotated|>0.05；都不满足返回 nil。有关键帧时三个参数(scale/rotation/center)都按并集帧逐帧采样发射。effect id=basic。
- [XMEML 裁切→Crop 滤镜 cropFilter] 模型 crop 存 0–1 边距，导出转 0–100 百分比。无关键帧且 crop.isIdentity 则不发。四个参数 left/right/top/bottom，每个静态=crop[edge]*100 或逐帧采样 cropAt(frame)[edge]*100(关键帧帧集合=keyframeFrames(.crop))。effect id=crop, type=motion, category=motion。
- [XMEML 不透明度→Opacity 滤镜 opacityFilter] FCP7 不透明度独立于 Basic Motion。无关键帧时 opacity==1.0 不发，否则静态=opacity*100；有关键帧时逐帧 rawOpacityAt(frame)*100(注意 raw 不含淡入淡出)。格式 '%.1f'。effect id=opacity。
- [XMEML AV 链接 linkNodes/link 索引] indexAddresses 给排序后每条轨每个 clip 编 (trackIndex,clipIndex) 均 1-based,分 video/audio 两套。indexLinkGroups 把所有带 linkGroupId 的 clip 按 group 归集(注意用原始 timeline.tracks 全部 clip,不过滤)。发射时:若 clip 有 group 且同组 partner>1 个,对每个 partner(含自身)发 <link>:linkclipref='clipitem-<partner.id>'、mediatype=partner 的 audio/video、trackindex、clipindex。partner 若不在 clipAddresses(被 sortEmittable 丢弃)则跳过。
- [XMEML 关键帧 when 坐标] 所有滤镜关键帧的 when = 绝对时间线帧 - clip.startFrame(转成 clip 相对偏移)。keyframeFrames(for:) 返回的是绝对帧(内部 offset+startFrame)，发射时再减回去。XMEML 不携带插值曲线(linear/hold/smooth)，导入端按默认缓动,这是已知信息损失。
- [XMEML 不导出项 明确] 文字叠加、水平/垂直翻转(flipHorizontal/Vertical)、关键帧插值曲线 均不进 XMEML。Rust 移植 XMEML 时同样省略,但若改走 FCPXML 可补回文字。
- [.palmier 打包流程 PalmierProjectExporter.export] 1)在系统临时目录建 staging='palmier-export-<UUID>' 及其下 media/ 子目录；defer 删除 staging；2)遍历 manifest.entries:用 sourceURL(source,projectURL) 解析源(external→绝对路径,project→projectURL+相对路径)；若解析失败或文件不存在→记 missing 并原样保留该(悬空)条目;3)对存在的源:key=标准化绝对路径,用 relativePathBySource 去重——同一源只复制一次;首次复制到 media/ 下用 uniqueURL 防重名,relativePath='media/<文件名>',累加 totalBytes,若源是 .project 则 copiedInternal++;4)若源是 .external 记入 collected;5)把条目 source 重写为 .project(relativePath);6)写 project.json(timeline)/media.json(新manifest)/generation-log.json(均 JSONEncoder 默认);7)若有 sourceProjectURL,搬运 thumbnail.jpg 与 chat/ 目录(存在才搬);8)目标已存在先删,建父目录,最后 fm.moveItem(staging→destURL) 原子落地。
- [.palmier 文件命名 filename] project 源:保留原 lastPathComponent;external 源:base='import-<entry.id 前8位>',有扩展名则 base.ext 否则 base。uniqueURL 防冲突:同名则追加 '-1','-2'...(保留扩展名)。
- [.palmier 包结构] 目录形式的 bundle(typeIdentifier='io.palmier.project',扩展名 'palmier')。内含 project.json(Timeline)、media.json(MediaManifest version=2)、generation-log.json(GenerationLog version=1)、media/(所有媒体)、可选 thumbnail.jpg、可选 chat/。所有 JSON 用 Swift JSONEncoder 默认设置(键名=结构体属性名,无排序,Date 默认编码为 ISO 时间戳数值-Codable 默认是 referenceDate 起的秒数 Double)。
- [ExportView 体积估算 estimatedFileSize] seconds=totalFrames/max(1,fps);按(codec,resolution)查表 bytesPerSec(如 h264/1080p=1.3e6, prores/4K=65e6 等9种组合);估算字节=bytesPerSec*seconds,用 ByteCountFormatter .file 格式化。纯展示估算,与真实编码码率无关。
- [ExportView 首帧预览 loadPreview] 找第一条 video 轨第一个能解析 URL 且含视频轨的 clip,用 AVAssetImageGenerator(maximumSize 480x270, appliesPreferredTrackTransform=true)在 time=CMTime(trimStartFrame, timescale=fps)异步取一帧 NSImage。Rust 移植用 FFmpeg seek 到 trimStartFrame/fps 秒取一帧缩略图。
- [ExportView palmier 摘要 computePalmierSummary] 预扫 manifest:每条解析 url(external/project),不存在记 missing;external 且存在记 collect;累加文件字节。用于对话框显示'X media files missing'与预计体积。逻辑与 PalmierProjectExporter 的统计一致但独立实现。
- [搜索索引联动] ExportService.isExporting 的 didSet:变 true 调 SearchIndexCoordinator.exportDidBegin()(暂停后台索引),变 false 调 exportDidEnd()(恢复)。仅在值真正变化时触发。Rust 移植中若有后台索引/缩略图任务同理应在导出期间让路。

**苹果框架使用**:
- AVFoundation (AVAssetExportSession) [blocker] — 视频导出的实际编码器:按预设名渲染 composition+videoComposition+audioMix 到 mp4/mov,并提供 progress 进度
- AVFoundation (AVMutableComposition / AVVideoComposition / AVMutableAudioMix) [blocker] — 由 CompositionBuilder 把 Timeline 编译成可渲染的合成对象:轨道拼接、变速 scaleTimeRange、空隙 insertEmptyTimeRange、逐帧变换/裁切/不透明度/音量包络指令
- AVFoundation (AVVideoCompositionCoreAnimationTool) [blocker] — 把 TextLayerController 生成的 CALayer 文字树烤进导出视频(postProcessingAsVideoLayer)
- AVFoundation (AVAssetReader + .timecode 轨) [medium] — XMEML 导出时读取源媒体 QuickTime tmcd 时码轨首帧,写入 file 节点的 startframe/timecode
- AVFoundation (AVAssetImageGenerator) [low] — ExportView 生成时间线首帧预览缩略图
- CoreMedia (CMTime/CMTimeRange/CMBlockBuffer) [medium] — 全部时间运算的载体;解析时码 sample 的大端字节;分数 CMTime 做平滑关键帧细分避免整数帧坍塌
- QuartzCore/CoreAnimation (CATextLayer/CAKeyframeAnimation) [high] — 文字排版与渲染:字体/对齐/背景/边框/阴影,离散关键帧动画控制每帧可见性,烤入导出视频
- AppKit (NSSavePanel/NSWorkspace) [low] — 系统保存面板选择输出路径;导出成功后在 Finder 选中 .palmier 包
- AppKit (NSImage/NSColor/NSAttributedString/NSScreen) [medium] — 预览图承载;文字样式颜色与富文本属性;contentsScale 取屏幕缩放
- SwiftUI [high] — ExportView 整个导出对话框 UI(设置面板/预览/进度/底栏)
- UniformTypeIdentifiers (UTType) [low] — 保存面板的允许内容类型(.mp4/.movie/.xml/工程包 UTType)
- CoreGraphics (CGAffineTransform/CGSize/CGRect) [low] — 归一化变换→渲染坐标的仿射矩阵;裁切矩形换算;尺寸数学
- Foundation (JSONEncoder/FileManager) [none] — .palmier 打包的文件复制/移动/JSON 序列化

**闭源云**:无。整个 Export 目录及其导出路径不含任何 Convex/ConvexMobile/Clerk/ClerkKit 引用，也没有 URLSession/HTTP/生成式 AI 云请求(grep 全目录仅命中 XMLExporter 文件头里两条苹果文档 URL 注释)。导出是纯本地操作:读本地媒体文件、用 AVFoundation 本地编码、写本地文件。MediaManifestEntry 上虽有 cachedRemoteURL 字段，但导出代码从不读取它，仅依赖本地解析的文件 URL。GenerationLog 仅记录历史模型名与积分成本，不触发任何网络。

**移植策略**:分三块,移植难度差异极大。(1) .palmier 打包(PalmierProjectExporter)——direct-port,纯文件IO+JSON,Rust 用 std::fs+serde_json 一比一复刻:临时 staging 目录、按标准化绝对路径去重、external→'import-<id前8>.<ext>' 命名、project→保留原名、重名追加 -1/-2、重写 manifest.source 为相对路径、写三个 JSON、搬运 thumbnail/chat、最后原子 rename。唯一坑:Swift JSONEncoder 默认把 Date 编成 Apple referenceDate(2001-01-01)起的秒数 Double,Rust 侧若要双向兼容旧 .palmier 需自定义 serde 序列化匹配该数值语义(或统一改用 ISO8601 并接受不与上游互通)。(2) XMEML 导出(XMLExporter)——direct-port 级别的算法,几乎全是确定性纯计算,Rust 用自建 XMLNode 树+手写 render(复刻2空格缩进与5种实体转义)即可逐函数照搬:轨道 reversed、in/out vs start/end 的变速比、masterclip/file id 按媒体类型分离、pathurl 的 'file://localhost//' 非标准前缀、rateTags 的 NTSC 判定、formatTimecode 的丢帧补偿、各滤镜阈值(scale 0.1/rotation 0.05/center 0.001)、关键帧 when=帧-startFrame、Cross Dissolve 的 cutPointTicks=cutFrames*254016000000/fps、中心坐标 -0.5 偏移与旋转取负、缩放 (seqWidth/sourceWidth)*width*100。唯一需替换的子步骤是读源时码:把 AVAssetReader 读 tmcd 改成 ffprobe(-show_streams 取 timecode tag,或读 tmcd 流首样本),读不到回退 0。建议优先实现 XMEML,它对 Premiere 用户价值高且无渲染依赖。(3) 视频渲染导出(ExportService 非xml路径)——这是真正的 needs-replacement/重写:整条 AVFoundation 渲染管线(AVComposition/AVVideoComposition/CoreAnimationTool/CATextLayer)在 Rust+FFmpeg 下要重建。方案:用 FFmpeg filter_complex 搭建合成图——黑底 color 源打底,每条视频轨各 clip 做 trim(in/out=trimStart..trimStart+sourceFramesConsumed)+setpts(变速)+scale/rotate/crop/overlay(对应 Basic Motion/Crop)+ format=yuva 配合 fade/alpha(对应 opacity 与淡入淡出 envelope),音频走 atrim+asetpts+volume(线性,clamp 3.98)+afade 后 amix;文字叠加用 drawtext 或预渲染 PNG 序列 overlay(替代 CATextLayer,字体/阴影/边框需逐项映射,这是最难复刻的视觉一致性点,建议用 cosmic-text/skia 离线渲染文字位图再 overlay 以接近 CATextLayer 效果);关键帧动画 FFmpeg 原生支持弱,需把 CompositionBuilder 的 trackOps/emitEnvelopeRamps 平滑细分逻辑(smoothSegments=8、smoothstep)在 Rust 侧采样成密集的 sendcmd/expr 时间函数或逐帧参数。颜色管线锁定 BT.709(对应 AVVideoColor*_709)。编码:libx264(h264)/libx265(h265)/prores_ks(prores),宽高=renderSize(短边缩放取偶),帧率=timeline.fps。进度从 FFmpeg stderr 的 -progress 解析。(4) UI(ExportView)——ui-rebuild:用 React/TS 重写对话框,体积估算表/分辨率换算/首帧预览(Rust 经 FFmpeg 出缩略图)逻辑照搬。(5) 搜索索引联动——infra,Tauri 侧若有后台任务在导出期间暂停即可。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Export/ExportService.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Export/XMLExporter.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Export/PalmierProjectExporter.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Export/ExportView.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Preview/CompositionBuilder.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Preview/TextLayerController.swift

