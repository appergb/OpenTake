# Models 移植规格

**职责**:
- 定义时间线数据模型:Timeline(fps/宽/高/settingsConfigured/tracks)、Track(类型/muted/hidden/syncLocked/clips)、Clip(媒体引用/时间区间/trim/speed/volume/fade/opacity/transform/crop/链接组/字幕组/文本/六条关键帧轨道)
- 实现片段的派生计算属性:endFrame、sourceFramesConsumed(=round(duration*speed))、sourceDurationFrames(=consumed+trimStart+trimEnd)、totalFrames(取所有轨道 endFrame 最大值)
- 实现按帧采样属性:opacityAt/rotationAt/topLeftAt/sizeAt/transformAt/cropAt/volumeAt/rawVolumeAt,把关键帧采样与静态值/淡入淡出包络组合
- 实现淡入淡出包络 fadeMultiplier(线性 or smoothstep,取头尾两端较小值)
- 实现源时间↔时间线帧的双向换算(timelineFrame(sourceSeconds:)、timelineFrame(sourceFrame via trim/speed))
- 实现关键帧系统:KeyframeTrack 的 upsert/remove/move(保持按 frame 升序、同帧覆盖、移动到已存在帧则忽略),以及带插值类型(linear/hold/smooth)的 sample
- 实现 Clip 上的关键帧增删改查:绝对时间线帧↔片段相对偏移转换(frame-startFrame)、allKeyframeFrames 并集、按属性枚举 AnimatableProperty 路由到对应轨道
- 实现关键帧维护:clampKeyframesToDuration(丢弃 [0,durationFrames] 之外的帧)、rescaleKeyframes(按比例缩放帧号并四舍五入)、clampFadesToDuration(头尾不超过总时长)
- 实现 Transform 的几何:中心点/左上角/尺寸互转、旋转(度,顺时针为正)、翻转、边界吸附 snapToBoundary/snapToCanvasEdges/snapCenterToCanvasCenter,以及旧工程文件(x/y→centerX/centerY)的迁移解码
- 定义文字样式 TextStyle(字体/字号/缩放/颜色 RGBA/对齐/阴影/背景/边框)与 RGBA 颜色解析(hex #RGB/#RRGGBB/#RRGGBBAA、与 NSColor/SwiftUI Color 互转)
- 实现文字自然尺寸测量 TextLayout.naturalSize(按 canvasHeight/1080 缩放字号,加阴影 padding 与 4px 余量)
- 定义媒体资产 MediaAsset(可观察 class)及其异步元数据加载(时长/分辨率/帧率/缩略图/是否有音轨)
- 定义工程媒体清单序列化:MediaManifest(version/entries/folders)、MediaManifestEntry、MediaSource(project 相对路径 / external 绝对路径)、GenerationInput(AI 生成参数,纯数据)
- 实现资产 ID→文件 URL 解析 MediaResolver(相对路径基于工程目录拼接,检查文件是否存在,判断 isMissing)
- 为所有可序列化模型提供缺键容错的 Codable 实现(老工程文件用默认值补齐新增字段)

**核心类型**:
- `Timeline` (struct) — 工程时间线根模型。持有 fps(默认30)、width/height(默认1920×1080)、settingsConfigured、tracks 数组;totalFrames 取所有轨道 endFrame 的最大值。Codable/Sendable/Equatable。
- `Track` (struct) — 一条轨道。持有 id、type(ClipType)、muted/hidden/syncLocked、clips 数组、非序列化的 displayHeight;endFrame 取片段 endFrame 最大值;contiguousClipIds 求从某帧起首尾相接的连续片段链(用于波纹/链接)。自定义容错解码。
- `Clip` (struct) — 时间线上的片段(视频/音频/图片/文本/Lottie)。核心字段:mediaRef、mediaType、sourceClipType、startFrame、durationFrames、trimStartFrame/trimEndFrame、speed、volume、fadeIn/OutFrames+插值、opacity、transform、crop、linkGroupId、captionGroupId、textContent/textStyle,以及六条可空关键帧轨道(opacity/position/scale/rotation/crop/volume)。承载本模块大部分采样与时间换算算法。自定义容错解码。
- `Transform` (struct) — 片段在画布上的归一化变换(centerX/Y 默认0.5、width/height 默认1、rotation 度数顺时针为正、水平/垂直翻转)。提供中心↔左上角换算、边界与画布中心吸附、旧 x/y 字段迁移解码。
- `Crop` (struct) — 片段裁剪,以归一化(0–1)源坐标的四边内缩表示(left/top/right/bottom);visibleWidth/HeightFraction=max(0,1-两边)。实现 KeyframeInterpolatable 可逐分量插值。
- `KeyframeTrack<Value>` (struct) — 泛型关键帧轨道,keyframes 按 frame 升序排列;upsert(同帧覆盖否则插入到首个更大帧之前)、remove、move;Value 满足 KeyframeInterpolatable 时提供 sample(at:fallback:) 采样。
- `Keyframe<Value>` (struct) — 单个关键帧:frame(片段相对帧)、value、interpolationOut(默认 smooth)。
- `Interpolation` (enum) — 关键帧/淡变插值类型:linear、hold、smooth(smooth 使用 smoothstep)。
- `AnimPair` (struct) — 双分量关键帧值(a,b),用于 position(x,y)与 scale(width,height);实现逐分量线性插值。
- `AnimatableProperty` (enum) — 可动画属性枚举:opacity/position/scale/rotation/crop/volume,用于把 UI 操作路由到对应关键帧轨道。
- `ClipType` (enum) — 媒体/片段类型:video/audio/image/text/lottie;含 isVisual、isCompatible(同类或都可视)、按扩展名构造、SF Symbol 名等。
- `TextStyle` (struct) — 文字样式:fontName(默认 Helvetica-Bold)、fontSize(96)、fontScale、color(RGBA)、alignment、shadow、background/border(Fill)。含 RGBA hex 解析、NSColor/NSParagraphStyle/属性字典等渲染辅助。
- `TextLayout` (enum) — 文字自然包围尺寸计算的命名空间。naturalSize 按 canvasHeight/referenceCanvasHeight(1080)缩放字号,用 NSAttributedString.boundingRect 测量,加阴影 padding(12×2)与 4px 余量。
- `MediaAsset` (class) — @Observable @MainActor 的媒体资产引用类型(身份语义)。持有 url/type/name/duration/thumbnail/源宽高帧率/hasAudio/生成输入与状态/folderId/远程缓存 URL 与过期时间;loadMetadata 异步探测媒体元数据;与 MediaManifestEntry 互转。
- `MediaManifest` (struct) — 工程媒体清单(version=2、entries、folders)。容错解码。是工程文件 media.json 的根结构。
- `MediaManifestEntry` (struct) — 单条媒体清单项:id/name/type/source/duration/generationInput/源宽高帧率/hasAudio/folderId/远程缓存 URL+过期。
- `MediaSource` (enum) — 媒体位置:.project(relativePath)(随工程移动)或 .external(absolutePath)(外部引用)。决定序列化与解析方式。
- `GenerationInput` (struct) — AI 生成参数的纯数据快照(prompt/model/duration/aspectRatio/分辨率/质量/各类参考资产 URL 与 assetId/voice/lyrics 等)。本模块不发起任何网络请求。
- `MediaResolver` (class) — 资产 ID→文件 URL 解析器。external 直接用绝对路径,project 用工程目录拼接相对路径;resolveURL 校验文件存在,isMissing/displayName/entry 辅助。
- `MediaFolder` (struct) — 媒体库文件夹(id/name/parentFolderId),支持嵌套。

**核心算法/逻辑(供 Rust 复刻)**:
- 【单位与帧/秒换算】全局以整数帧为时间单位。frameToSeconds=frame/fps;secondsToFrame=Int(seconds*fps)(向零截断,非四舍五入)。fps 为 Int(默认30)。timecode 格式 HH:MM:SS:FF 由整除/取余得到(ff=frame%fps)。Rust 复刻须保持 secondsToFrame 的截断语义而非四舍五入。
- 【片段时间区间】endFrame=startFrame+durationFrames;片段占据 [startFrame, endFrame) 半开区间。contains(timelineFrame:)=frame>=startFrame && frame<endFrame。Track.endFrame/Timeline.totalFrames 为各自子项 endFrame 的最大值(空则0)。
- 【speed 与源帧消耗】sourceFramesConsumed=Int(round(durationFrames*speed));sourceDurationFrames=sourceFramesConsumed+trimStartFrame+trimEndFrame。即 speed 是‘源帧/时间线帧’比率:speed>1 表示快放(消耗更多源帧)。所有涉及 speed 的换算都用 Double 计算后 .rounded()(就近舍入,.5 进位)。
- 【源时间→时间线帧】Clip.timelineFrame(sourceSeconds t, fps):sourceFrame=t*fps;offsetFromTrim=sourceFrame-trimStartFrame,若<0 返回 nil;frame=Int(round(startFrame + offsetFromTrim/max(speed,0.0001)));若 frame 不在 [startFrame,endFrame) 返回 nil。speed 下限钳到 0.0001 防除零。
- 【关键帧存储坐标系】关键帧 frame 字段存的是‘片段相对偏移’=绝对时间线帧-startFrame。所有对外 API 用绝对帧,内部 toOffset(abs)=abs-startFrame、toAbs(off)=startFrame+off 转换。allKeyframeFrames 把六条轨道的相对帧 +startFrame 取并集后排序。
- 【关键帧采样 sample(at frame, fallback)】规则:空轨道→fallback;单帧→该帧值;frame<=首帧→首帧值;frame>=末帧→末帧值(端点 clamp,无外插);否则找首个 frame>查询帧的关键帧 b,a=b 的前一帧,raw=(frame-a.frame)/(b.frame-a.frame);按 a.interpolationOut 决定:hold→返回 a.value;linear→lerp(a,b,raw);smooth→lerp(a,b,smoothstep(raw))。smoothstep(t)=t*t*(3-2t)。注意:插值类型取自‘左端关键帧的 interpolationOut’。
- 【关键帧插值类型】Double 线性插值 a+(b-a)*t;AnimPair 逐分量线性;Crop 逐边(left/top/right/bottom)线性。新建 Keyframe 默认 interpolationOut=.smooth。
- 【KeyframeTrack.upsert】若存在同 frame 关键帧则原地替换;否则插入到首个 frame>新帧 的位置之前(保持升序)。move(from,to):若目标帧已被占用(且≠源帧)则放弃移动;否则移除再 upsert(隐含:若目标已存在会在 upsert 阶段覆盖,但 move 提前用 contains 拦截了冲突)。remove 删除所有匹配 frame 的关键帧。
- 【按属性删除关键帧并自动清空轨道】removeKeyframe(property, at) 删除后若该轨道 keyframes 为空,则把整条轨道置 nil(轨道 nil 表示‘无动画’,采样回退到静态字段)。clearKeyframes 直接把对应轨道置 nil。
- 【clampKeyframesToDuration】片段缩短后调用。对每条轨道:保留 frame 在闭区间 [0, durationFrames] 内的关键帧(注意是闭区间,含两端),逐个 upsert 到新轨道;若结果为空则该轨道置 nil。clampVolumeKfsToDuration 只处理音量轨。
- 【rescaleKeyframes(by scale)】用于变速等需要整体缩放关键帧时间轴。scale 须 finite 且>0,否则原样返回。对每个关键帧 frame'=Int(round(frame*scale)),upsert 进新轨道(同帧覆盖)。空则 nil。
- 【淡入淡出包络 fadeMultiplier(at frame)】rel=frame-startFrame;若 rel<0 或 rel>durationFrames 返回0。入端:若 fadeInFrames>0,t=min(1, rel/fadeInFrames),smooth 插值用 smoothstep(t) 否则 t,否则1。出端:outRem=durationFrames-rel,若 fadeOutFrames>0,t=min(1, outRem/fadeOutFrames),同样按插值;否则1。最终返回 min(入端,出端)。注意端点 rel==durationFrames 仍算在内(<=)。
- 【opacity 合成】opacityAt=rawOpacityAt(=opacityTrack.sample(off, fallback=opacity) ?? opacity);若 mediaType!=audio 且存在淡变(fadeIn>0||fadeOut>0)再乘 fadeMultiplier。音频片段不应用不透明度淡变。
- 【音量合成(dB 模型)】volumeAt=volume(静态线性外层增益)× kfGain × fadeMultiplier。kfGain:若 volumeTrack 激活,采样得到 dB 值(关键帧值单位是 dB,fallback=0dB),再 VolumeScale.linearFromDb(dB);否则1。rawVolumeAt 同上但不乘 fade。liveVolumeKfDb 返回当前帧的原始 dB(仅当 contains 且轨道激活)。VolumeScale:floorDb=-60、ceilingDb=15;dbFromLinear(l)= l<=0?-60:clamp(20*log10(l), -60..15);linearFromDb(db)= db<=-60?0:pow(10, min(db,15)/20)。Rust 须保持 -60dB→线性0 的硬截断与 +15dB 上限。
- 【Transform 几何】中心坐标系:topLeft=(centerX-width/2, centerY-height/2);构造可由 topLeft 或 center 推中心。rotation 单位为度、顺时针为正。transformAt(frame) 组合 topLeftAt(优先 positionTrack.sample,否则由 center 与 sizeAt 推算)、sizeAt(优先 scaleTrack 否则 transform.width/height)、rotationAt(优先 rotationTrack 否则 transform.rotation)。注意 positionTrack 存的是‘左上角’归一化坐标(a=x,b=y),scaleTrack 存的是宽高(a=w,b=h)。
- 【边界吸附】snapToBoundary(v,th):|v|<th→0,|v-1|<th→1,否则原值。snapToCanvasEdges 对左右边、上下边分别吸附(优先吸左/上,再吸右/下,通过平移 center 实现)。snapCenterToCanvasCenter 对 centerX/Y 分别在 |c-0.5|<阈值 时吸到0.5,返回是否吸附用于画辅助线。阈值来自像素/缩放换算(见 Snap 常量:thresholdPixels=8、stickyMultiplier=1.5、playheadMultiplier=1.5)。
- 【裁剪表示】Crop 四边内缩(0–1 源坐标);isIdentity=四边全0;可见宽高比例=max(0,1-left-right)/max(0,1-top-bottom)。可作为关键帧值逐边插值。
- 【片段分割(split,逻辑在 Editor 层但直接操作本模块 Clip 字段,须忠实复刻)】仅当 startFrame<atFrame<endFrame 才分割。splitOffset=atFrame-startFrame;leftSource=Int(round(splitOffset*speed));rightSource=Int(round((duration-splitOffset)*speed))。左半:durationFrames=splitOffset,trimEndFrame=原trimEnd+rightSource,fadeOutFrames=0 后 clampFades;右半:新 id,startFrame=atFrame,durationFrames=原duration-splitOffset,trimStartFrame=原trimStart+leftSource,fadeInFrames=0 后 clampFades。即把源消耗按 speed 折算后分配给两半 trim,使两半拼接仍等价于原片段。
- 【关键帧分割(splitKeyframeTrack)】在 splitOffset 处采样得 boundary。左轨=保留 frame<=splitOffset 的关键帧;若最后一个不在 splitOffset,追加 (splitOffset, boundary)。右轨=取 frame>=splitOffset 的关键帧并整体平移 -splitOffset(保留各自 interpolationOut);若首个 frame≠0,在0处插入 (0, boundary)。空则 nil。保证切割两侧曲线连续,不残留越界关键帧。
- 【淡变钳制】clampFadesToDuration:fadeInFrames=clamp(0..durationFrames);fadeOutFrames=clamp(0..(durationFrames-fadeInFrames)),即入端优先、头+尾不超过总时长。setFade(edge,frames) 取 max(0,frames) 后钳制。setDuration 改 duration 后会连锁调用 clampKeyframesToDuration+clampFadesToDuration。
- 【连续片段链 contiguousClipIds(fromEnd, excludeId)】对按 startFrame 升序、startFrame>=fromEnd 且 id≠excludeId 的片段:若某片段 startFrame≠当前链尾 chainEnd 则中断;否则把 chainEnd 推进到该片段 endFrame 并收集其 id。用于波纹/链接选区的相邻判定。
- 【文字自然尺寸 TextLayout.naturalSize】measured=空串则用单空格;canvasScale=canvasHeight/1080;renderSize=fontSize*fontScale*canvasScale;用 boundingRect(maxWidth × ∞, [usesLineFragmentOrigin, usesFontLeading]) 测量;宽=max(1, ceil(bw)+(阴影启用?12*2:0)+4),高=max(1, ceil(bh)+4)。换字体度量引擎(Rust)须复现该缩放基准与 padding。
- 【RGBA hex 解析】去空白与前导#;长度3→每位重复成字节(#RGB→#RRGGBB),长度6→RGB(a=1),长度8→RGBA;每分量 UInt8(hex)/255;其余长度返回 nil。
- 【工程序列化格式(JSON / Codable)】所有模型为 Codable。容错策略:绝大多数字段用 try? decode ?? 默认值,使旧工程文件兼容新增字段。MediaManifest.version 缺省按1(代码默认值2)。Transform 兼容旧 x/y 字段:centerX=oldX+width-0.5、centerY=oldY+height-0.5(把旧左上角语义迁移为中心语义)。Track.displayHeight 不序列化(CodingKeys 不含),打开工程重置为默认50。关键帧轨道为可空,nil 即省略=无动画。MediaSource 为带 case 的枚举(external/project),按 Swift 默认枚举编码(含 case 标签)。

**苹果框架使用**:
- Foundation [none] — Codable 序列化、UUID 生成片段/轨道/资产 ID、URL/FileManager 做路径解析与文件存在检查、Date 处理远程缓存过期、log10/pow/round 等数学。
- AVFoundation [medium] — MediaAsset.loadMetadata 用 AVURLAsset 加载视频时长、用 loadTracks(.video/.audio) 取 naturalSize+preferredTransform(校正旋转后的真实宽高)、nominalFrameRate(源帧率)、是否有音轨;AVAssetImageGenerator 抽首帧生成缩略图(maximumSize 320×320, appliesPreferredTrackTransform=true)。
- AppKit [medium] — NSImage 持有缩略图;NSColor 做 sRGB 颜色与 hex/SwiftUI Color 互转;NSFont 解析字体(失败回退 boldSystemFont);NSAttributedString+NSParagraphStyle 构建文字属性并测量包围尺寸(TextLayout)。
- CoreText/CoreAnimation [medium] — TextLayout.naturalSize 经 NSAttributedString.boundingRect(底层 CoreText)做按行折行的文字尺寸度量;TextStyle.Alignment.caTextAlignmentMode 暴露给 CATextLayer 渲染。
- SwiftUI [none] — 仅 TextStyle.RGBA 提供 swiftUIColor 与 init(Color) 供 UI 取色器使用,模型本身不依赖 SwiftUI 运行。
- CoreGraphics [low] — CGSize/CGFloat/CGImage 作为度量与图像数据载体(经 AppKit/AVFoundation 间接)。

**闭源云**:无。Models 目录内全部文件均无任何网络请求,未 import Convex/ConvexMobile/Clerk/ClerkKit,grep 确认无 URLSession/http/fetch。涉及云的仅为‘纯数据’:GenerationInput(记录 AI 生成参数,如 prompt/model/各类参考资产 URL 与 assetId)与 MediaAsset/MediaManifestEntry 上的 cachedRemoteURL+cachedRemoteURLExpiresAt(已下载远程素材的缓存直链与过期时间,toManifestEntry 时会丢弃过期项)。这些字段只被序列化存储,真正的生成式 AI 云调用发生在 Generation/Agent 等其它模块,不在本目录。

**移植策略**:时间线/关键帧/变换/裁剪/淡变/音量 dB/分割等核心算法全部是平台无关的整数帧+浮点数学,应在 Rust core 中一比一复刻为纯 struct/enum + 方法。建议:1) 用 serde 复刻 Codable,务必保留‘缺键回退默认值’的容错(serde 用 #[serde(default)] + Option;MediaManifest.version 缺省按1;Transform 旧 x/y→中心的迁移用自定义 Deserialize)。2) 浮点舍入务必与 Swift 一致:Swift .rounded() 是就近-银行家外的‘四舍五入(.5 远离0)’=Rust f64::round();secondsToFrame 用 (seconds*fps) as i32 的‘向零截断’而非 round。3) smoothstep、sample 的端点 clamp(无外插)、插值类型取左端关键帧 interpolationOut、clamp 用闭区间 [0,duration]、fade 取 min(in,out) 等边界条件逐一保留。4) speed 下限 0.0001、VolumeScale floor=-60→线性0 硬截断、ceiling=15 上限照搬。5) 片段分割按 round(offset*speed) 折算 trim 的逻辑与 splitKeyframeTrack 的边界关键帧插入须照搬(它在 Editor 层但属模型不变量)。需要替换/重建的只有 Apple 框架相关的‘辅助’部分:(a) MediaAsset.loadMetadata 用 FFmpeg(ffprobe)替代 AVFoundation 取时长/宽高(注意复刻 preferredTransform 旋转校正,用 ffprobe 的 rotate/display matrix)、帧率、音轨存在性、抽帧缩略图(ffmpeg -ss 0 -frames:v 1 缩放到 320),verdict 局部为 needs-replacement;(b) TextLayout/TextStyle 的文字度量与字体/颜色:用 Rust 文本栈(如 cosmic-text/fontdue 或浏览器侧 Canvas measureText)替代 CoreText/AppKit,务必复现 canvasHeight/1080 缩放基准与阴影 padding(12*2)+4px 余量,否则文本框尺寸会漂移,verdict 局部为 needs-replacement/ui-rebuild;(c) NSColor↔Color、SF Symbol 名、CATextLayerAlignmentMode 这类纯 UI 映射在前端(React/TS)或渲染层重建即可。GenerationInput 与 cachedRemoteURL 等字段按纯数据原样移植到工程格式,不触云。整体属高保真直译,风险集中在媒体探测与文字度量两处需用 FFmpeg+Rust 文本引擎对齐数值。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Models/Timeline.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Models/Keyframe.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Models/MediaAsset.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Models/MediaManifest.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Models/TextStyle.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Models/MediaResolver.swift

