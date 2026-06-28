# OpenTake 模块移植地图

> 由 20 个 max-思考子 Agent 对 palmier-pro-upstream 逐模块拆解生成。verdict 含义:direct-port=Rust直写 / needs-replacement=换跨平台库 / ui-rebuild=React重建 / cloud-rebuild=自建后端。

## 总览

| 模块 | 分层 | 移植判定 | 一句话职责 |
|---|---|---|---|
| **Models** | core-domain | direct-port | PalmierPro 的领域模型层(纯数据 + 少量渲染/元数据辅助)。定义视频编辑器的核心可序列化数据结构:时间线/轨 |
| **Project** | mixed | needs-replacement | 这是 PalmierPro 的「工程文件格式 + 主屏启动器 + 示例工程下载」层。它定义了 .palmier 工程包( |
| **Editor** | mixed | needs-replacement | PalmierPro 编辑器模块,是整个 AI 视频编辑器的"编辑领域核心 + 编辑器 UI 外壳"。它围绕一个巨型 @ |
| **Timeline** | ui | ui-rebuild | PalmierPro 的时间线 UI 与交互层：基于 AppKit 自绘(NSView + CGContext)渲染多轨 |
| **Preview** | mixed | needs-replacement | Preview 是 PalmierPro 的"预览/合成"子系统:把领域模型 Timeline(轨道/片段/关键帧)实时 |
| **Export** | mixed | needs-replacement | 导出子系统，把内存中的 Timeline 落地为三种产物：(1) 渲染好的视频文件 (.mp4 H.264/H.265  |
| **Generation** | mixed | cloud-rebuild | PalmierPro 的「生成式 AI」子系统:把文/图/音/视频生成请求、AI 二次编辑(放大 Upscale、重跑  |
| **Agent** | mixed | needs-replacement | PalmierPro 的 AI 智能体子系统：把自然语言剪辑意图翻译成对编辑器领域模型(Timeline/Clip)的工 |
| **MediaPanel** | mixed | ui-rebuild | 左侧停靠面板，承载三个标签页：Media（媒体素材库浏览器：文件夹/扁平/分组三种视图、搜索、拖拽、选区、导入、AI 整 |
| **Inspector** | ui | ui-rebuild | Inspector 是 PalmierPro 右侧的属性检查器面板：根据当前选区（单/多 clip、纯文字 clip、媒 |
| **Account** | cloud-client | cloud-rebuild | 账户/订阅/计费/鉴权的云客户端层。负责 Google OAuth 登录(Clerk)、把账户信息从 Convex 后端 |
| **Search** | engine | needs-replacement | 完全本地 (on-device) 的语义化媒体搜索子系统:用 SigLIP2(CLIP 风格的图文双编码器,经 Core |
| **Settings** | ui | ui-rebuild | Palmier Pro 的"设置"窗口模块:一个独立 NSWindow,左侧侧边栏 + 右侧详情,承载 5 个分页(Ac |
| **Help** | ui | ui-rebuild | 应用内"帮助/支持"模块,纯展示型 UI。提供三块内容:(1) 键盘快捷键速查表(静态硬编码),(2) MCP serv |
| **App** | infra | needs-replacement | 这是 PalmierPro(AI 原生 macOS 视频编辑器)的"应用外壳/引导层"。它负责进程启动与依赖装配(日志、 |
| **Utilities** | infra | needs-replacement | PalmierPro 的通用基础设施工具集,集中放置不属于任何具体业务域的横切能力:有界并发信号量、磁盘缓存目录管理、图 |
| **UI** | ui | ui-rebuild | PalmierPro 的共享设计系统与通用 SwiftUI/AppKit 表现层组件目录。它集中定义全局设计令牌(颜色/ |
| **Transcription** | engine | needs-replacement | 封装"音频/视频 → 文字稿"的全链路：从媒体里抽取音频轨、调用 Apple 设备端语音识别(macOS 26 新 Sp |
| **Telemetry** | infra | needs-replacement | 对 Sentry Cocoa SDK 的一层极薄静态封装,负责崩溃/错误/异常上报、面包屑(breadcrumb)日志、 |
| **Toolbar** | ui | ui-rebuild | Toolbar 是编辑器顶部的工具栏 UI 条，提供撤销/重做、指针/剃刀工具模式切换、在播放头处分割、把入点/出点裁到 |

---

## Models  ·  `core-domain` → **direct-port**

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

## Project  ·  `mixed` → **needs-replacement**

**职责**:
- 定义并实现 .palmier 工程包的磁盘格式(目录式文件包):读取/写入 project.json(Timeline)、media.json(MediaManifest)、generation-log.json(GenerationLog)、thumbnail.jpg、media/ 目录(素材)、chats/ 目录(对话会话)
- NSDocument 生命周期:read(反序列化,off-main 解码 on-main 应用)、save/fileWrapper(主线程抓快照后可 off-main 写盘)、autosavesInPlace、变更计数同步到 EditorViewModel.isDocumentEdited
- 工程窗口构建:创建 NSWindow + NSHostingController 承载 EditorView,安装暗色外观、透明标题栏、左右两侧 SwiftUI 标题栏配件(带圆角安全区适配)、键盘监视器
- 工程缩略图生成:遍历时间线首个可用图片/视频片段,图片直接缩放,视频用 AVAssetImageGenerator 在 trimStartFrame 对应时间抽帧,JPEG 编码缓存
- 素材恢复:打开工程时按 MediaManifest 把每个条目解析为 MediaAsset,触发波形/缩略图/元数据生成,统计 restored/missing
- 最近工程注册表(ProjectRegistry):记录工程 URL/创建时间/最近打开时间,JSON 持久化到 ~/Documents/Palmier Pro/project-registry.json,支持注册/移除/删除到废纸篓/重命名后改 URL,带加载期间挂起变更队列
- Home 主屏 UI:侧边栏(登录/新建/打开/设置)、工程卡片网格(缩略图/名称/相对时间/缺失态/删除确认/右键菜单)、示例工程横条、欢迎浮层、版本更新浮层
- 示例工程服务(SampleProjectService):从 Convex HTTP 后端拉取示例列表并把单个示例「物化」成本地 .palmier 包(并发下载素材+对话,带进度回调,失败清理)
- 工程设置不匹配对话框:导入视频片段时检测其 FPS/分辨率与时间线是否一致,提供「保持当前」或「改为匹配」(后者会按比例重算所有片段帧值)
- 应用级工程编排(AppState):新建(NSSavePanel)、打开(NSOpenPanel/URL)、打开示例、Home/Editor 窗口切换、通知点击后定位生成资产

**核心类型**:
- `VideoProject` (class) — NSDocument 子类,工程包的核心。负责 .palmier 文件包的读写序列化、窗口创建、缩略图生成、素材恢复、变更计数。持有唯一的 EditorViewModel 实例。autosavesInPlace=true。
- `Project` (enum) — 命名空间常量(在 Utilities/Constants.swift):fileExtension="palmier"、typeIdentifier="io.palmier.project"、各子文件名(project.json/media.json/generation-log.json/thumbnail.jpg)、mediaDirectoryName="media"、registryFilename、storageDirectory=~/Documents/Palmier Pro。定义整个工程格式的契约。
- `ProjectRegistry` (class) — @Observable @MainActor 单例,最近工程列表的内存+磁盘管理。entries 按 lastOpenedDate 倒序;通过 actor ProjectRegistryDisk 做后台 IO 与废纸篓删除;加载期间用 pendingMutations 队列保证不丢写。
- `ProjectEntry` (struct) — 注册表条目:id(UUID)、url、createdDate、lastOpenedDate;name 从 url 文件名派生,isAccessible 检查文件是否存在。Codable/Sendable。
- `SampleProjectService` (class) — @MainActor @Observable 单例,示例工程的列表拉取与物化。Summary(slug/title/posterUrl);从 Convex HTTP 后端 v1/samples 与 v1/samples/resolve 下载并组装本地工程包。
- `Timeline` (struct) — 工程核心数据模型(被序列化为 project.json):fps=30/width=1920/height=1080/settingsConfigured/tracks[]。totalFrames 取所有轨道 endFrame 最大值。Codable/Sendable/Equatable。
- `Track` (struct) — 轨道:id/type(ClipType)/muted/hidden/syncLocked/clips[];displayHeight 不序列化。endFrame 取片段 endFrame 最大值;contiguousClipIds 求连续片段链。自定义 init(from:) 容错解码。
- `Clip` (struct) — 片段:mediaRef/startFrame/durationFrames/trimStart/trimEnd/speed/volume/fade/opacity/transform/crop/linkGroupId/captionGroupId/文本/6 条关键帧轨。含大量帧换算与采样方法(opacityAt/volumeAt/fadeMultiplier/timelineFrame)。自定义容错解码。
- `MediaManifest` (struct) — 素材清单(序列化为 media.json):version=2、entries[]、folders[]。自定义 init(from:) 对缺失字段降级到 version=1/空数组。
- `MediaManifestEntry` (struct) — 素材条目:id(String)/name/type/source(MediaSource)/duration/generationInput?/源宽高FPS/hasAudio/folderId/cachedRemoteURL+过期时间。
- `MediaSource` (enum) — 素材定位:.external(absolutePath) 或 .project(relativePath)。决定素材是工程内 media/ 目录还是外部绝对路径引用。
- `GenerationLog` (struct) — 追加式 AI 生成记录(序列化为 generation-log.json):version=1、entries[]。GenerationLogEntry 含 model/costCredits/createdAt,旧格式 cost(美元)迁移为 credits(×100 向上取整)。

**核心算法/逻辑(供 Rust 复刻)**:
- 【工程包磁盘格式】.palmier 是一个目录式文件包(macOS NSDocument package,UTI=io.palmier.project,conformsTo .package)。包内固定结构:project.json(Timeline 的 JSON)、media.json(MediaManifest 的 JSON)、generation-log.json(GenerationLog 的 JSON,可选)、thumbnail.jpg(JPEG 封面,可选)、media/(目录,存放工程内素材文件)、chats/(目录,每个对话会话一个 <uuid>.json)。Rust 复刻时按目录读写即可,不需要 NSFileWrapper 机制。
- 【读取流程 read()】必须存在 project.json 否则抛 fileReadCorruptFile;用 JSONDecoder 解码 Timeline;若存在 media.json 解码 MediaManifest(解码失败抛 fileReadCorruptFile);若存在 generation-log.json 用 try? 宽松解码(失败则忽略=nil)。解码在后台线程做,真正赋值给 EditorViewModel 在主线程的 makeWindowControllers() 里完成。
- 【保存流程 save/fileWrapper】关键时序:必须先在主线程调用 captureSaveSnapshot() 把 4 份数据 JSON 编码成 Data 快照(snapshotTimeline/Manifest/GenerationLog/Thumbnail)+ 收集非空对话会话编码为 [(name,data)],置 snapshotPreparedForFileWrapper=true;之后 fileWrapper() 可在后台线程运行,把快照写入包(replaceChild 逐个替换子文件)。若 fileWrapper() 在非主线程运行但快照未准备好则抛 fileWriteUnknown。media/ 目录通过对现有磁盘目录做 FileWrapper(url:options:.immediate) 整体纳入。Rust 实现:保存时对 Timeline/Manifest/GenerationLog 各 serde_json::to_vec 写入对应文件,缩略图重新抓帧,对话会话各写一个 json,media/ 目录原样保留。
- 【JSON 序列化容错策略(必须一比一复刻)】所有 Codable 自定义 init(from:) 都对缺失字段降级:Timeline 字段缺失用默认(fps30/1920×1080);Track 缺 id 生成新 UUID、缺 muted/hidden 用 false、syncLocked 用 true;Clip 缺 id 生成 UUID、mediaType/sourceClipType 缺用 .video、trim/fade 缺用 0、speed/volume/opacity 缺用 1.0、interpolation 缺用 .linear,transform/crop 缺用默认;MediaManifest 缺 version 视为 1、缺 entries/folders 视为空数组;GenerationLogEntry 优先读 costCredits(Int),若无则读旧字段 cost(Double 美元)并换算 credits=ceil(dollars×100)。Transform 还兼容旧 x/y 键:centerX = oldX + width - 0.5(同理 y)。Rust 用 serde 的 #[serde(default)] + 自定义 Deserialize 复刻这些默认与迁移。
- 【帧/时间换算单位】整个数据模型以「帧」为时间单位(Int),fps 存在 Timeline.fps。片段 endFrame = startFrame + durationFrames(半开区间 [start,end))。sourceFramesConsumed = round(durationFrames × speed);sourceDurationFrames = sourceFramesConsumed + trimStartFrame + trimEndFrame。timelineFrame(sourceSeconds t):sourceFrame = t×fps;offsetFromTrim = sourceFrame − trimStartFrame(必须≥0);frame = round(startFrame + offsetFromTrim / max(speed,0.0001));要求 startFrame ≤ frame < endFrame 否则返回 nil。视频缩略图抽帧时间 = CMTime(value: trimStartFrame, timescale: fps)。
- 【缩略图生成算法】遍历所有 video 类型轨道的所有片段(轨道顺序、片段顺序),取第一个能解析出 URL 的片段:若是图片用 ImageEncoder.thumbnail(maxPixelSize:640) 缩放后 JPEG(quality 0.7);若是视频用 AVAssetImageGenerator(maximumSize 320×180, appliesPreferredTrackTransform=true)在 trimStartFrame/fps 处同步抽一帧(5 秒超时,超时则取消并跳过),成功则 NSBitmapImageRep 转 JPEG(0.7)。结果缓存到 cachedThumbnail。Rust+FFmpeg:用 ffmpeg seek 到对应秒数抽单帧并缩放编码 JPEG。
- 【素材恢复 restoreAssetsFromManifest】打开工程后,对 MediaManifest.entries 逐条:用 mediaResolver.expectedURL(for: id) 求期望路径(无法解析记 missing);构造 MediaAsset(entry, resolvedURL) 加入 mediaAssets;检查文件是否真实存在(不存在记 missing 但仍保留 asset);存在则按类型触发:audio/video→generateWaveform,video→generateVideoThumbnails,image→generateImageThumbnail,并异步 loadMetadata()。统计 restored/missing 上报遥测。
- 【FPS 变更时的全片段帧重算(applyTimelineSettings 核心算法)】当新 fps≠旧 fps 且两者>0:scale = newFps/oldFps。currentFrame 与 sourcePlayheadFrame 各 ×scale 取整。对每条轨道按 startFrame 升序处理片段,维护 previousEnd:scaledStart=round(start×scale),scaledEnd=round(end×scale);新 startFrame=max(scaledStart, previousEnd ?? scaledStart)(防止重叠);durationFrames=max(1, scaledEnd−newStart);trimStart/trimEnd 各 round(×scale);关键帧 rescaleKeyframes(×scale)(每个 kf.frame=round(frame×scale) 后 upsert);fadeIn/fadeOut 各 round(×scale);再 clampKeyframesToDuration + clampFadesToDuration;更新 previousEnd=新 endFrame。这是有损但确定性的重采样,Rust 必须逐帧复刻取整与 max 防重叠逻辑。
- 【分辨率变更时的自动适配】当 width/height 改变:对每个片段,若其 transform 恰好等于 fitTransform(asset, 旧 canvas)(即此前是自动适配的),则替换为 fitTransform(asset, 新 canvas);手动调过的 transform 保持不变。判定依据是 Transform 的 Equatable 全等。
- 【设置不匹配判定 checkProjectSettings】传入待导入 assets:若没有 video 资产→proceed。若 timeline.settingsConfigured==false(史上第一个片段)→静默自动检测:fps=round(firstVideo.sourceFPS)、width/height=源宽高(缺则保持当前),applyTimelineSettings 后 proceed。若时间线非空→proceed(不打扰)。仅当「时间线为空但已配置过设置」时才比较:fpsMismatch = 源 fps 存在且≠时间线 fps;resMismatch = 源宽存在且≠时间线宽 或 源高存在且≠时间线高;有任一不匹配则返回 .mismatch(用源值,缺失项回退时间线值),由 UI 弹 ProjectSettingsMismatchView,用户选「改为匹配」则 applyTimelineSettings(源值)。这些都登记 undo(action name "Change Project Settings")。
- 【ProjectRegistry 持久化与并发】注册表 JSON 存到 ~/Documents/Palmier Pro/project-registry.json([ProjectEntry] 数组)。所有 URL 比较用 standardizedFileURL。register:已存在则更新 lastOpenedDate,否则追加新 UUID 条目。所有变更走 mutate(apply):若正在异步加载(isLoading)则把闭包加入 pendingMutations 队列延后执行,否则立即 apply 并 save。加载完成 finishLoading 后依次回放挂起变更再保存一次。delete 走后台 actor trashItem 到废纸篓成功后才从注册表移除。工程文件 URL 改名(VideoProject.fileURL setter)会调 updateURL 同步注册表并刷新 lastOpenedDate。
- 【示例工程物化 SampleProjectService.materialize】GET base/v1/samples/resolve?slug=,返回 JSON 含 title/project/manifest/downloads[]/可选 chat[]/generationLog/posterUrl。downloads 每项{id,relativePath,url};chat 每项{name,url}转成 relativePath=chats/name。在 ~/Library/Application Support/PalmierPro/Samples/<safeSlug>/ 下建 <safeTitle>.palmier 包:写 project.json、media.json、(有则)generation-log.json,下载 posterUrl→thumbnail.jpg;用 withThrowingTaskGroup 并发下载所有 downloads(到 dest/relativePath,建父目录、覆盖旧文件),每完成一个回调进度 completed/total。任一步失败则删除整个 slug 目录并抛错。safeName:把 / : \ 替换为空格并 trim,空则用 "Sample"。Rust 复刻:HTTP 拉 JSON + 并发下载文件落盘组装目录。
- 【新建工程流程 AppState.createNewProject】NSSavePanel(默认名 "Untitled Project",目录 ~/Documents/Palmier Pro,内容类型 io.palmier.project)→用户确认后 new VideoProject,设 fileURL/fileType,makeWindowControllers+showWindows,addDocument,save(.saveOperation) 落盘后 register 到注册表。打开:VideoProject(contentsOf:ofType:) 解码后同样建窗口并 register。

**苹果框架使用**:
- AppKit / NSDocument [high] — VideoProject 继承 NSDocument,用其文件包读写机制(read/fileWrapper/save)、自动保存(autosavesInPlace)、变更计数(updateChangeCount)、undoManager 注入、NSDocumentController 文档管理。这是工程持久化的骨架。
- AppKit / NSWindow+NSHostingController+NSWindowController [high] — makeWindowControllers 创建编辑器窗口:暗色外观、透明全尺寸标题栏、左右 SwiftUI 标题栏配件(带圆角安全区适配的 CornerAdaptiveView)、最小尺寸、frameAutosaveName。HomeWindowController/SettingsWindowController 同理。
- SwiftUI [high] — HomeView/ProjectCard/SampleProjectsStrip/WelcomeOverlay/UpdateOverlay/ProjectSettingsMismatchView 全部用 SwiftUI;含 @Observable/@Bindable/@AppStorage 状态、LazyVGrid 网格、glassEffect 玻璃态、ultraThinMaterial、spring 动画、contextMenu/alert/sheet。
- AVFoundation [medium] — captureThumbnail 用 AVURLAsset 检查视频轨、AVAssetImageGenerator(maximumSize/appliesPreferredTrackTransform)同步抽帧;CMTime(value:timescale:) 用 fps 做帧↔时间换算。
- UniformTypeIdentifiers [low] — 定义工程包 UTType(io.palmier.project, conformsTo .package),NSSavePanel/NSOpenPanel 的 allowedContentTypes,以及素材 MIME/扩展名识别。
- ImageIO (via ImageEncoder) [low] — 图片缩略图用 CGImageSource 缩放(kCGImageSourceThumbnailMaxPixelSize),CGImageDestination 编码 JPEG(质量梯度 0.85/0.7/0.55/0.4 控制在 3.5MB 内)。
- Foundation (JSONEncoder/Decoder/FileManager/FileWrapper/URLSession) [none] — 工程 JSON 序列化、注册表读写、文件包目录操作、示例工程 HTTP 下载与并发落盘、废纸篓删除(trashItem)、相对时间格式化。

**闭源云**:有。SampleProjectService 通过 BackendConfig.convexHttpURL 访问 Convex HTTP 后端:GET v1/samples(示例列表)与 GET v1/samples/resolve?slug=(解析单个示例的 project/manifest/downloads/chat/posterUrl),再用 URLSession 并发下载示例素材到本地。AsyncImage 也会按 posterUrl 拉取海报缩略图。账号侧通过 BackendConfig.clerkPublishableKey/convexDeploymentURL + AccountService(signInWithGoogle/account/aiAllowed/isMisconfigured)与 Clerk/Convex 鉴权耦合,Home 侧边栏与欢迎浮层据此显示登录/Get started。本模块自身不直接调用生成式 AI 生成接口(genAI 生成在 Generation/Agent 模块),此处仅触达 Convex(示例分发)与 Clerk(身份)。

**移植策略**:分三类处理。(1) 工程文件格式与领域模型(Project 常量、Timeline/Track/Clip/Transform/Crop、MediaManifest/Entry/Source、GenerationLog、ProjectEntry)→ direct-port:在 Rust core 用 serde 定义同构 struct/enum,严格复刻字段默认值与迁移逻辑(#[serde(default)] + 自定义 Deserialize 处理:Track/Clip 缺字段默认、MediaManifest version 降级、Transform 旧 x/y→centerX/Y 换算、GenerationLogEntry cost 美元→credits=ceil(×100)、MediaSource 的 external/project 二选一 tagged enum)。工程包改为普通目录(.palmier/ 内含 project.json/media.json/generation-log.json/thumbnail.jpg/media//chats/),Rust 直接按路径读写,丢弃 NSFileWrapper;保存时序简化为同步序列化即可(Rust 无 NSDocument 的主线程/后台快照约束),但要保留『先组装内存快照再原子写盘』以防半成品。(2) NSDocument 生命周期/窗口/标题栏配件/自动保存/undoManager 注入 → ui-rebuild:在 Tauri 侧用前端管理打开的工程与窗口,Rust core 暴露 open_project/save_project/create_project 命令;autosave 改为前端 debounce 或 Rust 定时;undo/redo 重做栈在 core 用命令模式自实现(NSUndoManager 不可移植)。(3) Home 主屏/卡片/浮层/示例条/设置不匹配对话框 → ui-rebuild:React/TS 重建,玻璃态/spring 动画用 CSS/Framer Motion,相对时间用 Intl.RelativeTimeFormat。(4) 缩略图生成 → needs-replacement:图片缩略图用 image crate 缩放编码 JPEG;视频抽帧用 FFmpeg(seek 到 trimStartFrame/fps 秒抽单帧→缩放→JPEG),复刻『遍历首个可用片段、视频用 trimStartFrame 处』规则与 5s 超时保护。(5) applyTimelineSettings 的 FPS 重采样与分辨率自动适配、checkProjectSettings 不匹配判定 → direct-port:纯算术,Rust 逐帧复刻 round/max 防重叠/clamp 逻辑与判定分支。(6) ProjectRegistry → direct-port:JSON 数组持久化,异步加载用 tokio,挂起变更队列与 standardizedFileURL 归一化(Rust 用 canonicalize/标准化路径)照搬;trashItem 删除→用 trash crate 跨平台回收站。(7) 闭源云:SampleProjectService 的 Convex 拉取/下载 → cloud-rebuild,在 Rust 用 reqwest 复刻 v1/samples 与 v1/samples/resolve 协议+并发下载组装目录;账号/Clerk 鉴权按 OpenTake 自有方案重做(可改为可选/本地无云)。关键坑:① JSON 字段默认值与旧版本迁移必须一比一,否则老工程打不开;② 半开帧区间 [start,end) 与所有 round 取整点要与 Swift 完全一致以保证跨端工程往返无漂移;③ Transform 用归一化画布坐标(0–1, centerX/Y/width/height),移植渲染时坐标系约定别改。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Project/VideoProject.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Project/ProjectRegistry.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Project/SampleProjectService.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Project/HomeView.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Project/ProjectSettingsMismatchView.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Utilities/Constants.swift

## Editor  ·  `mixed` → **needs-replacement**

**职责**:
- 时间线编辑领域逻辑:放置/创建/移动/分割/删除剪辑,覆盖式清区(clearRegion/OverwriteEngine),波纹删除/插入/闭合空隙(RippleEngine),sync-lock 跨轨道联动
- 撤销重做基础设施:三套并存的撤销策略——整时间线快照交换(withTimelineSwap/registerTimelineSwap)、多剪辑状态交换(mutateClips)、单剪辑属性交换(commitClipProperty),以及拖拽期 live/commit/revert/debounce 模式
- 裁剪与速度:trim 把源帧增量按 speed 折算回时间线帧;变速重算 durationFrames 并对接触链做波纹位移;裁剪/速度对关键帧与淡入淡出做夹取与重定基
- 关键帧动画系统:opacity/position/scale/rotation/crop/volume 六条 KeyframeTrack 的 stamp/remove/move/插值切换;animation-aware 写入(有激活轨道则写 kf,否则写静态值);分割时在切点插边界 kf 保持曲线连续
- A/V 链接组(linkGroupId):选择/移动/裁剪/删除作为整体;分割后右半重新成组;out-of-sync 偏移计算;拖放分轨路由(视频区/音频区镜像)
- 媒体库与文件夹:导入(文件/目录树/粘贴图像/截帧)、重命名、删除(连带删引用剪辑)、文件夹层级移动与防环、离线媒体重链(relink)
- 字幕生成:调用 Apple Speech 做端上转写,按可见源区间归属短语到剪辑,生成文本剪辑轨道
- 项目设置:FPS/分辨率变更时按比例重算所有帧值并重排剪辑、重拟合自动适配的变换;导入设置不匹配对话框
- 剪贴板与复制:复制选区(相对锚点偏移)、playhead/指针粘贴、Option 拖拽复制,均带链接组重映射
- 预览标签页、布局预设切换、面板焦点/最大化、播放控制转发、时间线范围(I/O 入出点)选择
- AI 编辑入口(右键菜单):upscale/edit/重跑/生成视频/视频转音频,经云端生成服务回填替换剪辑源;把剪辑/范围另存为媒体(AVFoundation 导出)
- 新手引导 Tour:步骤模型、面板/控件锚点高亮、scrim 挖洞;项目活动 AI 花费日志展示

**核心类型**:
- `EditorViewModel` (class) — 编辑器的中枢状态对象(@Observable @MainActor),持有 timeline/mediaManifest/generationLog/mediaAssets 及全部瞬态 UI 状态;通过 20+ 个 extension 把所有编辑操作挂在其上。是 Rust 端要复刻的核心领域服务。
- `RippleEngine` (enum) — 纯函数波纹引擎:computeRippleShifts(删剪辑后回填)、computeRippleShiftsForRanges(按帧区间回填)、computeRipplePush(插入后整体右推)、mergeRanges(区间合并)。无副作用,最易直接移植。
- `OverwriteEngine` (enum) — 纯函数覆盖引擎:computeOverwrite 给定区间返回对每个重叠剪辑的动作(remove/trimEnd/trimStart/split),含源帧按 speed 折算。clearRegion 的算法内核。
- `Clip` (struct) — 剪辑值类型(Models/Timeline.swift):startFrame/durationFrames/trimStart/trimEnd/speed/volume/opacity/transform/crop/fades/6条关键帧轨道/linkGroupId/captionGroupId/text*。含大量派生计算(endFrame、sourceFramesConsumed、各属性 At(frame) 采样、fadeMultiplier、时间换算)。Codable。
- `Timeline / Track` (struct) — 工程数据根:Timeline{fps,width,height,settingsConfigured,tracks};Track{id,type,muted,hidden,syncLocked(默认true),clips,displayHeight(不序列化)}。Track.contiguousClipIds 求接触链。均 Codable+Equatable(Equatable 是整时间线快照撤销的基础)。
- `KeyframeTrack / Keyframe / Interpolation / AnimPair` (struct) — 关键帧模型(Models/Keyframe.swift):帧为剪辑相对偏移;upsert 有序插入;sample(at:fallback:) 按 hold/linear/smooth 插值;smoothstep=t*t*(3-2t);AnimPair 承载 position(x,y)/scale(w,h)。
- `ClipShift / FrameRange / GapSelection` (struct) — 波纹引擎的 DTO:ClipShift(clipId→newStartFrame);FrameRange 半开区间 [start,end);GapSelection(trackIndex+range)用户选中的空隙。
- `OverwriteEngine.Action` (enum) — 覆盖动作枚举:remove / trimEnd(newDuration) / trimStart(newStartFrame,newTrimStart,newDuration) / split(leftDuration,rightId,rightStartFrame,rightTrimStart,rightDuration)。
- `Transform / Crop` (struct) — Transform 归一化画布坐标(centerX/Y,width,height 默认1=满画布,rotation度顺时针,flipH/V),含边界吸附与中心吸附;Crop 归一化边缘内缩。都 Codable(Transform 有旧键 x/y 迁移)。
- `EditorSplitViewController` (class) — AppKit NSSplitViewController 子类,搭建 default/media/vertical 三种多面板布局、面板折叠/最大化、Tour 高亮帧计算。纯 macOS UI。
- `EditorWindowController` (class) — NSWindowController:用 NSEvent.addLocalMonitorForEvents 拦截键盘(空格/方向键/C/V/I/O/[/]/Delete 等)与鼠标点击设面板焦点;实现 EditorActions 响应链(复制/剪切/粘贴/导出/面板切换)。
- `TourController / TourStep / TourOverlay` (class) — 新手引导:步骤序列(intro/spotlight/outro)、面板与控件锚点(NSView 弱引用)、SwiftUI scrim 挖洞高亮。纯 UI。
- `GenerationLog / GenerationLogEntry` (struct) — AI 生成花费日志(持久化 generation-log.json):model+costCredits+createdAt,含旧版美元→credits 迁移(dollars*100 上取整)。

**核心算法/逻辑(供 Rust 复刻)**:
- 【单位换算 帧↔秒】secondsToFrame = Int(seconds*fps)(截断,非四舍五入);frameToSeconds = frame/fps。剪辑时长:clipDurationFrames = max(1, secondsToFrame(segment时长 或 asset.duration))。时间码 formatTimecode 为 HH:MM:SS:FF(FF=frame%fps)。注意:导出/seek 用 CMTime(value:frame, timescale:fps),即帧号直接作为分子、fps 作分母。
- 【源帧↔时间线帧 折算(贯穿全模块,极重要)】时间线上 1 帧对应 speed 个源帧。核心公式:源帧增量 = round(时间线帧增量 * speed);时间线帧增量 = round(源帧增量 / speed)。Clip 派生:sourceFramesConsumed = round(durationFrames*speed);sourceDurationFrames = sourceFramesConsumed+trimStartFrame+trimEndFrame。所有 trim/分割/覆盖/裁剪到 playhead 都用此折算,且都用 .rounded()(四舍五入到偶数?Swift 默认 .toNearestOrAwayFromZero)。
- 【分割 splitClip / splitSingleClip】仅当 startFrame < atFrame < endFrame 才分。splitOffset=atFrame-startFrame;leftSource=round(splitOffset*speed);rightSource=round((durationFrames-splitOffset)*speed)。左半:duration=splitOffset, trimEndFrame += rightSource, fadeOut 清零;右半:新 id, startFrame=atFrame, duration=原-splitOffset, trimStartFrame += leftSource, fadeIn 清零;两半都 clampFadesToDuration。关键帧:六条轨道各在 splitOffset 处 sample 出边界值,左半保留 frame<=splitOffset 的 kf 并确保末尾有 splitOffset 边界 kf,右半取 frame>=splitOffset 的 kf 把 frame 减去 splitOffset 重定基并确保首端有 frame=0 边界 kf(保持曲线跨切口连续)。链接组:先对组内每个剪辑各自分割,再把所有右半重新分配一个新 linkGroupId(左右各成独立 A/V 对)。撤销:移除右半并把左半还原为原 clip。
- 【覆盖清区 OverwriteEngine.computeOverwrite + clearRegion】区间 [regionStart,regionEnd) 对每个剪辑(cs=startFrame, ce=endFrame)判定:ce<=start 或 cs>=end → 跳过;cs>=start && ce<=end → remove(整体在区内);cs<start && ce>end → split(跨区:左 duration=start-cs,右 startFrame=end, rightTrimStart=trimStartFrame+round((end-cs)*speed), rightDuration=ce-end);cs<start(仅左重叠)→ trimEnd(newDuration=start-cs);else(仅右重叠)→ trimStart(newStartFrame=end, newTrimStart=trimStartFrame+round((end-cs)*speed)? 实际为 round((end-cs)*speed)…注意右重叠用 trimAmount=end-cs)。clearRegion 把这些动作落地:trimEnd 时反推 trimEndFrame += round((oldDuration-newDuration)*speed);split 动作走 splitClip 再删右段(若右段还越过 end 则再 split 一次)。clearRegion 是 addClips/move/paste/duplicate/placeTextClips 放置前的统一让位手段。
- 【波纹删剪辑 rippleDeleteSelectedClips】被删剪辑的 [start,end) 收集为 globalRemovedRanges。逐轨道:若该轨道自身有被删剪辑 → computeRippleShifts(删后剩余剪辑按 start 升序,每个剪辑左移量 = 所有 end<=clip.startFrame 的已合并删除区间长度之和);否则若 track.syncLocked → 用 computeRippleShiftsForRanges 按 globalRemovedRanges 左移,并先 validateShifts 干跑(任一剪辑移后 start<0 或与前一剪辑重叠则整体拒绝:NSSound.beep+log,不执行)。全部校验通过后在一个 withTimelineSwap 内 removeClips 再 applyShifts。
- 【波纹删区间 rippleDeleteRangesOnTrack(MCP/agent 用)】对锚轨道的任意 [start,end) 区间(可跨多剪辑)mergeRanges 后:收集锚轨道 + 所有被触及的链接剪辑的伙伴所在轨道 id 作为 clearTrackIds(保证 A/V 同步切除);先对非清除的 sync-locked 跟随轨道 validateShifts,任一不能吸收则 refuse 返回。执行时对每个 clearTrackIds 轨道逐区间 clearRegion(prune:false),再对(清除轨道∪sync-locked 轨道)applyShifts 左移闭合。返回报告含 removedFrames/clearedTracks/shiftedClips/新生成与存活的 fragments/被删 clipIds。
- 【波纹插入 rippleInsertClips】totalPush = 各待插剪辑时长之和。对 (目标轨道 ∪ 所有 sync-locked 轨道 ∪ 链接音频落点轨道) :若有剪辑跨越 atFrame(start<atFrame<end)先 splitClip 在 atFrame 切开使右半随推移;再 computeRipplePush(把 start>=atFrame 的剪辑 start += totalPush);最后从 atFrame 起按游标顺序 placeClip。简化版(MediaAsset 列表)不切跨越剪辑,只推+建。
- 【裁剪 trimClipInternal / commitTrim】入参为新的源帧 trimStart/trimEnd。deltaStartSource=新trimStart-旧;deltaEndSource=新trimEnd-旧;转时间线:deltaStartTimeline=round(deltaStartSource/speed),deltaEndTimeline=round(deltaEndSource/speed);newDuration=旧duration-两者;newStartFrame=旧start+deltaStartTimeline。覆盖式:同轨相邻不让位、不推 sync-locked。commitTrim 把边沿拖拽 deltaFrames 经 trimValues 折算:sourceDelta=round(delta*speed);左沿 newTrimStart=旧+sourceDelta,右沿 newTrimEnd=旧-sourceDelta;image/text 为无源材料,trim 可为负(不夹 0),video/audio 夹 max(0,_)。propagateToLinked 开启则对链接伙伴同样处理。trim 是 begin/end undo grouping 包裹、自反注册的递归撤销。
- 【变速 setClipSpeed】basis = 拖拽起点快照 dragBefore(没有则当前);sourceFrames=basis.duration*basis.speed;newDuration=max(1, round(sourceFrames/newSpeed));写入 speed 与 newDuration 后 clampKeyframesToDuration+clampFadesToDuration。rippleDelta=(start+newDuration)-oldEnd,若非 0 则对从 oldEnd 起的接触链(contiguousClipIds:从该 end 起严格首尾相接、start 升序)整体平移 rippleDelta(变速会推后续紧邻剪辑)。commitClipSpeed 用 preDragTimeline→after 注册整时间线交换撤销。
- 【关键帧采样与写入(动画系统)】采样 KeyframeTrack.sample(at:fallback:):空→fallback;单 kf→该值;frame<=首kf→首值;frame>=末kf→末值;否则取首个 frame>目标的 kf 作 b,a=前一个,raw=(frame-a.frame)/(b.frame-a.frame),按 a.interpolationOut:hold→a 值,linear→线性,smooth→smoothstep(raw)。kf.frame 存的是剪辑相对偏移(=绝对帧-startFrame),公开 API 用绝对帧、内部 toOffset/toAbs 转换。stampKeyframe 在当前帧采当前值 upsert;animation-aware 写入(applyOpacity/Rotation/Volume/Position/Scale/Transform/Crop):若对应 track.isActive 则在 activeFrame upsert kf,否则写静态字段。position 写入既更新 kf 又同步 transform.center(=topLeft+尺寸/2);scale 用 mediaCanvasAspect 把单一 scale 拆成 (w=scale, h=scale/aspect)。volume kf 存 dB,静态 volume 存线性(VolumeScale.linearFromDb)。
- 【淡入淡出 fadeMultiplier】rel=frame-startFrame,越界返回0。inMul:fadeInFrames>0 时 t=min(1,rel/fadeInFrames),linear 直接 t、smooth 用 smoothstep;outMul:outRem=duration-rel,t=min(1,outRem/fadeOutFrames) 同理;最终 = min(inMul,outMul)。clampFadesToDuration:fadeIn=clamp(0,duration);fadeOut=clamp(0, duration-fadeIn)。opacity 最终 = rawOpacity(kf 或静态) * fadeMultiplier(audio 不乘);volume 最终 = 静态volume * kfGain(dB→线性) * fadeMultiplier。
- 【撤销重做三套数据结构】(1) withTimelineSwap:抓 before=timeline 副本→关 undo 注册→执行→after;before!=after(整时间线 Equatable 比较)才 registerTimelineSwap 注册双向交换闭包(undo 还原 before 并递归注册反向 redo)。用于结构性多步变更(增删轨/移动/波纹/粘贴)。支持嵌套抑制(外层在抑制注册则内层跳过)。(2) mutateClips:对一组 id 抓 before 全剪辑快照→modify→采 after→registerClipStateSwap 双向回写。(3) commitClipProperty:单剪辑 before(取自 dragBefore 拖拽起点或当前)→modify→registerClipPropertySwap。拖拽期 applyClipProperty 只写值+刷新预览不入栈、revertClipProperty 丢弃、debouncedCommitClipProperty 静默期后提交一条(给无 drag-end 的连续控件)。文本剪辑走 CATextLayer 旁路(syncTextLayers),不重建合成。
- 【链接组与拖放分轨路由】linkIndex 一遍扫出 group→[clipId];expandToLinkGroup 把选择扩到整组;partnerMoves 单剪辑移动算伙伴同 delta 平移(伙伴 start=max(0,_));linkGroupOffsets 以组内最小 (start-trimStart) 为参考算各剪辑失同步偏移。zones 把轨道分视频区[0,firstAudio)与音频区[firstAudio,end);drop 时视频半与音频半分别路由,光标落在异区会跨分界镜像(V1↔A1, V2↔A2);insertTrack 用 partitionedInsertionIndex 强制视觉轨永远在音频轨之上。link 需 >=2 且类型>=2 种;放置后 pruneEmptyTracks 删空轨。
- 【字幕生成 generateCaptions】目标剪辑筛选:可转写(video 有音轨或 audio)、且 video 若其链接组里已有 audio 剪辑则跳过(避免重复)。逐 mediaRef 用 Apple Speech 转写(可见源区间并集±1s padding,秒制);autoDetect 时按词数选主讲轨道。短语用 CaptionBuilder.phrases 按可见宽度(<画布宽*比例)切行、最短显示时长约束;每短语归属于与其源区间重叠最大且重叠>=短语半长的剪辑;TextStyle 大小写应用后生成 TextClipSpec,统一插一条新视频轨(index 0 顶部)放置,整体一条 Generate Captions 撤销。
- 【项目设置变更 applyTimelineSettings(FPS 重定标)】scale=新fps/旧fps。currentFrame/sourcePlayheadFrame、每剪辑 startFrame/endFrame(round 后)按比例缩放并以 previousEnd 防重叠(start=max(scaledStart, previousEnd))、duration=max(1, scaledEnd-start)、trimStart/trimEnd/fadeIn/fadeOut 缩放、rescaleKeyframes(by:scale 整体缩放 kf.frame)。分辨率变更:仅对 transform 仍等于旧画布 fitTransform 的剪辑重算新画布 fitTransform(自动适配的才重拟合,用户改过的不动)。首个视频剪辑且未配置过则静默用其源 fps/宽高;空时间线但配置过且不匹配则弹 mismatch 对话框。
- 【自动适配变换 fitTransform / 裁剪适配 cropFittingAspect】源宽高有效时:|画布纵横比-源纵横比|<0.02 视为相等返回单位 Transform;源更宽→Transform(width:1, height:画布比/源比);源更高→Transform(width:源比/画布比, height:1)。cropFittingAspect 求源内最大居中目标比裁剪:源比>目标→左右内缩 (1-目标/源比)/2;否则上下内缩。Transform 吸附:snapToBoundary(|v|<阈值→0, |v-1|<阈值→1)用于贴画布边;snapCenterToCanvasCenter 贴中心(0.5)。
- 【剪贴板复制 cloneClipsAt】复制时记录相对最小轨道/最小起始帧的 trackOffset/frameOffset;粘贴在目标 (track,frame) 重建:先逐放置点 clearRegion 让位,再克隆(新 id、新 start);链接组若本次复制了同组>=2 个剪辑则重映射到新组 id 保持组关系,否则置 nil(单个复制断链)。键盘粘贴落 playhead、指针粘贴落鼠标点、Option 拖拽落拖放点。
- 【序列化(工程文件格式)】工程是 NSDocument 包(目录),内含:timeline.json(JSONEncoder 编码 Timeline,即 fps/width/height/settingsConfigured/tracks→clips 全字段)、media-manifest.json(MediaManifest,媒体条目+文件夹)、generation-log.json(可选)、media/ 媒体目录、缩略图、chat 会话目录。读取若缺 timeline.json 抛 corruptFile。Clip/Track/Transform 的 Decoder 大量用 try? + 默认值做向后兼容(老工程缺字段不报错);Transform 有 legacy x/y→centerX/Y 迁移(centerX=oldX+w-0.5)。displayHeight 不序列化、开项目重置 50。

**苹果框架使用**:
- AVFoundation [high] — saveClipAsMedia/saveTimelineRangeAsMedia 用 AVMutableComposition 拼源区间、scaleTimeRange 做变速、AVAssetExportSession 导出 mp4/m4a;captureCurrentFrameToMedia 用 AVAssetImageGenerator 抽帧(零容差);VideoEngine 用 AVPlayer 预览。是媒体引擎核心。
- CoreMedia [medium] — 全程用 CMTime(value:帧号, timescale:fps) 表达时间、CMTimeRange 表达源区间做导出与 seek。换算语义需在 Rust/FFmpeg 用有理数时间基(num/den)精确复刻。
- Speech [high] — 字幕生成的端上转写(SpeechAnalyzer+SpeechTranscriber,见 Transcription 模块),Editor 经 generateCaptions 调用。完全本地、无云。
- AppKit [high] — EditorSplitViewController(NSSplitViewController 多面板布局/折叠/最大化)、EditorWindowController(NSEvent 全局键鼠监听做快捷键与面板焦点、EditorActions 响应链)、Tour 锚点用 NSView 弱引用、NSSound.beep 波纹拒绝反馈。纯桌面 UI 外壳。
- SwiftUI [high] — EditorView 包装 AppKit 控制器;TitleBar/ProjectActivity/TourOverlay 等视图;@Observable 驱动响应式刷新;NSViewRepresentable 注册 Tour 锚点。
- QuartzCore/CoreGraphics [medium] — 截帧合成:CALayer.render 把文字层渲到翻转的 CGContext 再与视频帧合成为 CGImage、NSBitmapImageRep 编 PNG;文本剪辑预览走 CATextLayer。
- Foundation/Observation [medium] — UndoManager(三套撤销注册的宿主)、FileWrapper+JSONEncoder/Decoder(工程包序列化)、UserDefaults(面板可见性/布局持久化)、RelativeDateTimeFormatter(活动时间显示)。

**闭源云**:Editor 目录代码本身不含网络请求,但通过两条路径间接接触闭源生成式 AI 云:(1) EditorViewModel+AIEdit 的 aiEditAllowed 读取 AccountService.shared.isSignedIn/isMisconfigured——AccountService 用 ClerkKit/ClerkConvex/ConvexMobile 做 Clerk 登录 + Convex 后端(BackendConfig 配 clerkPublishableKey/convexDeploymentURL/convexHttpURL);(2) 右键 AI 编辑/重跑/upscale/视频转音频与生成面板回填(beginAIEdit/runAIUpscale/beginAIVideoAudio/beginAIRerun/beginAICreateVideo/seedGenerationPanel)经 Generation 模块(GenerationService/EditSubmitter/GenerationBackend)与 Agent 模块(AgentService/PalmierClient)走 Convex 云完成生成。注意:字幕转写(generateCaptions→Transcription)是 Apple Speech 端上推理,不走云。纯本地编辑操作(增删改/波纹/覆盖/裁剪/关键帧/撤销/导出/另存为媒体)均不触云。

**移植策略**:分层处置。【直接移植/core-domain → Rust crate】RippleEngine、OverwriteEngine 两个纯函数引擎可几乎逐行移植(只是整数运算与区间合并,注意 Swift .rounded() 默认 round-half-away-from-zero,Rust 用 f64::round() 语义一致)。Clip/Timeline/Track/Keyframe/Transform/Crop 及其全部派生计算(endFrame、sourceFramesConsumed、sample 插值、fadeMultiplier、源帧↔时间线帧折算、splitKeyframeTrack、clamp/rescale 关键帧、fitTransform/cropFittingAspect/吸附)是纯值语义 + 数学,直接移植为 Rust struct + 方法,用 serde 复刻 Codable(务必保留同名 JSON 键与 try?+默认值的向后兼容,以及 Transform 的 legacy x/y 迁移)。EditorViewModel 的编辑算法主体(placeClip/createClips/clearRegion/moveClips/splitClip/trimClipInternal/setClipSpeed/各 ripple* /链接组/分轨路由/字幕归属/applyTimelineSettings 重定标/cloneClipsAt)迁到 Rust core 作为纯函数式时间线变换(输入 Timeline+参数→输出新 Timeline,符合不可变原则)。【needs-replacement】撤销重做:Swift 用 UndoManager+闭包注册三套策略;Rust 端建议统一改为命令模式或基于整 Timeline 快照的 undo/redo 栈(withTimelineSwap 已是快照交换语义,最易复刻——前后 Timeline 做 Eq 比较,变化才入栈;mutateClips/commitClipProperty 可统一收敛为快照栈,牺牲一点内存换简单)。媒体引擎:AVFoundation 的 composition/export/抽帧/变速(saveClipAsMedia/saveTimelineRangeAsMedia/captureCurrentFrameToMedia)全部用 FFmpeg 重写(filter_complex 拼接、setpts/atempo 变速、抽帧→PNG);CMTime 用有理时间基(rational)表达。字幕转写 Apple Speech 需替换为 whisper.cpp/faster-whisper 等端上模型(保留'可见源区间转写→短语切行→按重叠归属剪辑'的上层算法不变,只换 ASR 后端)。【ui-rebuild】EditorSplitViewController/EditorWindowController/EditorView/TitleBar/ProjectActivity/Tour* 全是 AppKit/SwiftUI,在 Tauri2+React 前端重建:多面板布局用 CSS Grid/可拖拽分隔条;键盘快捷键在前端监听并经 Tauri command 调 Rust;Tour 用前端 overlay+锚点高亮;焦点/最大化为前端 UI 状态。【cloud-rebuild】AI 编辑/生成/upscale/登录门禁(AIEdit 扩展、GenerationLog、aiEditAllowed)依赖 Clerk+Convex 闭源云,需替换为自有鉴权+生成后端(或保留为可选外部 provider),OpenTake 的 MCP server(Rust)对应原 Agent/ToolExecutor 暴露同样的编辑工具集。【关键坑】(1) 帧/源帧折算的 round 方向与 max(1,_)/max(0,_) 边界必须逐处对齐,否则跨剪辑会累积 1 帧漂移;(2) sync-locked 默认 true,波纹会联动所有同步轨并可能拒绝(validateShifts:移后 start<0 或重叠);(3) 关键帧 frame 存剪辑相对偏移、分割要插边界 kf 保连续;(4) 工程 JSON 必须保持键名与缺省兼容以读旧工程;(5) image/text 剪辑 trim 可负(无源材料约束)。

**关键文件**:Sources/PalmierPro/Editor/RippleEngine.swift、Sources/PalmierPro/Editor/OverwriteEngine.swift、Sources/PalmierPro/Editor/ViewModel/EditorViewModel.swift、Sources/PalmierPro/Editor/ViewModel/EditorViewModel+ClipMutations.swift、Sources/PalmierPro/Editor/ViewModel/EditorViewModel+Ripple.swift、Sources/PalmierPro/Editor/ViewModel/EditorViewModel+Keyframes.swift

## Timeline  ·  `ui` → **ui-rebuild**

**职责**:
- 自绘渲染整条时间线:轨道背景与分隔线、片段卡片(圆角填充+左侧色条+裁剪手柄+标签栏时码)、视频缩略图平铺条、音频波形(峰值检测+dB 轴)、音量橡皮筋折线/淡入淡出楔形/音量关键帧菱形、不透明度淡变、关键帧黄色菱形标记、out-of-sync 偏移红色徽章、缺失媒体红色蒙版、生成中片段的 SwiftUI 覆盖层
- 绘制标尺(自适应主/次刻度、~80px 目标间距、时码标签)、播放头(CAShapeLayer + 三角形把手 + withObservationTracking 自动更新)、吸附指示线(虚线黄色 CAShapeLayer)
- 视口虚拟化绘制:document view 保持透明,实际绘制发生在一个跟随滚动的 viewport 尺寸 canvas 子视图上,只重绘脏矩形与可见 bar/tile
- 把鼠标手势分发为拖拽状态机(DragState):播放头拖拽(scrub)、片段移动(含多选/链接伴随/跨轨/新建轨道)、左/右裁剪、音量关键帧 2D 拖拽、淡入淡出 knee 拖拽、框选(marquee)、范围选择(timelineRange)
- 裁剪/移动/吸附的实时预览(ghost 片段)与释放时的提交(commitTrim/moveClips/duplicateClipsToPositions/insertTrack)
- 片段吸附引擎 SnapEngine:收集片段边缘+播放头为吸附目标,多探针(片段头尾)就近吸附,带 sticky 滞回与播放头优先权,触发触觉反馈
- 纯几何换算 TimelineGeometry:帧↔像素、轨道 Y、片段矩形、拖放目标(已有轨道/新建轨道前)、插入线 Y、ghost Y、音量关键帧/淡变 knee 命中矩形
- 缩放与平移:Option+滚轮以光标为锚缩放、Cmd+滚轮水平平移、触控板 pinch 缩放、播放头锚定缩放、边缘自动滚动(scrub 与拖拽时)
- 外部拖入(从媒体面板)落点解析:计算 DropPlan(视觉/音频轨道目标),绘制 ghost,支持普通插入与 Cmd 波纹插入
- 右键上下文菜单:复制/粘贴/链接/解链/换源/存为媒体/AI 编辑子菜单(Upscale/Edit/生成音乐SFX/Rerun/Create Video)/范围加入聊天等
- 轨道头(TimelineHeaderView):轨道标签、静音/隐藏/同步锁切换按钮、拖拽改轨道高度
- 媒体可视化缓存 MediaVisualCache:异步生成并磁盘缓存视频缩略图(雪碧图)、音频波形、图片缩略图,带并发闸门与基于 路径|大小|mtime 的 SHA256 缓存键

**核心类型**:
- `TimelineView` (class) — NSView 自绘文档视图。持有 EditorViewModel(unowned)、inputController、playheadOverlay、snapOverlay,负责 drawContent 全部绘制逻辑、外部拖入(NSDraggingDestination)、右键菜单构建、@objc 菜单动作。内部用 TimelineCanvasView(透明、hitTest 返回 nil)做 viewport 虚拟化绘制。
- `TimelineInputController` (class) — @MainActor 输入控制器。把 mouseDown/Dragged/Up/Moved/scrollWheel/magnify 翻译成 DragState 状态机,执行命中测试(hitTestClip/hitTestGap/fadeKneeHit/audioVolumeKfHit)、实时吸附、裁剪/移动钳制、缩放、播放头自动滚动定时器。是交互手感的核心。
- `DragState` (enum) — 拖拽状态机。变体:idle/scrubPlayhead/moveClip(MoveClipDrag)/trimLeft(TrimDrag)/trimRight(TrimDrag)/audioVolumeKf/fadeKnee/marquee/timelineRange。各自携带拖拽起始快照(原始帧、原 trim、grab 偏移、deltaFrames、dropTarget、isDuplicate 等),用于无副作用地计算预览与提交。
- `SnapEngine` (enum) — 纯吸附算法(无状态命名空间)。collectTargets 收集片段头尾边缘+可选播放头;findSnap 用多探针就近匹配,带 sticky 滞回(2.5x 阈值内保持)、播放头优先(1.5x 阈值)、命中即触觉反馈。SnapState 在拖拽间持久化记忆当前吸附目标。
- `TimelineGeometry` (struct) — 纯布局数学(struct,值类型)。预算 cumulativeY;提供 frameAt/xForFrame(帧↔像素)、clipRect、trackY/trackHeight、trackAt、dropTargetAt(拖放目标解析)、insertionLineY/ghostY、fadeKneeRect/audioVolumeKfRect 命中盒。被绘制与命中测试共用。
- `ClipRenderer` (enum) — 片段卡片渲染器(无状态)。draw 绘制圆角填充+色条+边框+缩略图/波形+音量橡皮筋/淡变楔形+关键帧+标签栏+裁剪手柄。含 dB↔Y 换算(y(forDb:)/db(forY:))、波形峰值检测、缩略图/图片平铺、淡变曲线采样(linear/hold/smooth)。
- `MediaVisualCache` (class) — @MainActor 媒体可视化缓存。异步(Task.detached + AsyncSemaphore 闸门)用 AVAssetImageGenerator 抽视频缩略图、DSWaveformImage 抽波形、ImageIO 抽图片缩略图;磁盘缓存为 JPEG 雪碧图+JSON 边栏/二进制波形,缓存键= SHA256(path|size|mtime)。
- `TimelineGeometry.TrackDropTarget` (enum) — 拖放目标:existingTrack(Int) 落到已有轨道 / newTrackAt(Int) 在该索引前新建轨道。贯穿移动、外部拖入、ghost 绘制、插入线绘制。
- `PlayheadOverlay / SnapIndicatorOverlay` (class) — CAShapeLayer 覆盖层。PlayheadOverlay 用 withObservationTracking 监听 playheadState.timelineFrame/zoomScale 自动重画红色播放头(竖线+三角)。SnapIndicatorOverlay 画虚线黄色吸附线,localX(本地拖拽)/externalX(外部拖入)两个来源。
- `TimelineContainerView` (struct) — SwiftUI NSViewRepresentable 容器。组装固定轨道头(左)+ NSScrollView(右,内含 TimelineView)+ 分隔线;用 RenderState(revision/zoom/选择/范围/pending)做 needsRender diff 触发重绘;播放时把播放头滚入视口;监听 scrollView bounds/frame 变化同步重绘与轨道头滚动。
- `TimelineRangeSelection` (struct) — 时间线范围选择(Sendable)。startFrame/endFrame,normalized 规范化、isValid、contains(frame:) 半开区间。用于'存范围为媒体/范围加入聊天/清除范围'。

**核心算法/逻辑(供 Rust 复刻)**:
- 单位换算(全局以帧为唯一时间单位):frame=Int(seconds*fps)(secondsToFrame,向零截断,非四舍五入);seconds=frame/fps;像素 x=headerWidth+frame*pixelsPerFrame(pixelsPerFrame 即 zoomScale);frameAt(x)=max(0,Int((x-headerWidth)/pixelsPerFrame))(向零截断)。时码 formatTimecode=HH:MM:SS:FF,FF=absFrame%fps,SS=(absFrame/fps)%60,MM=(absFrame/fps/60)%60,HH=absFrame/fps/3600,负值加'-'前缀,各字段两位补零(>=10 原样)。
- 片段数据模型 Clip 的关键派生量:endFrame=startFrame+durationFrames(半开区间 [startFrame,endFrame));sourceFramesConsumed=round(durationFrames*speed)(可见部分消耗的源帧);sourceDurationFrames=sourceFramesConsumed+trimStartFrame+trimEndFrame(引用的源总帧)。trimStartFrame/trimEndFrame 是'源媒体两端被裁掉的帧数'(源帧单位,非时间线帧)。contains(timelineFrame f)= f>=startFrame && f<endFrame。
- 裁剪(trim)实时预览(TimelineInputController.mouseDragged):候选边=fps 帧位置经吸附后得到。左裁剪:deltaFrames=snappedStart-originalStartFrame,钳制 [minDelta,maxDelta];maxDelta=originalDuration-1(至少留 1 帧);minDelta= 图片/文本(hasNoSourceMedia)取 -originalStartFrame(可拉到时间线 0),否则取 -originalTrimStart(不能拉出源头部)。右裁剪:candidateEnd=max(originalStartFrame+1,frame),deltaFrames=snappedEnd-originalEndFrame;minDelta=-(originalDuration-1);若 hasNoSourceMedia 只夹下限,否则上限=originalTrimEnd(不能拉过源尾部)。预览片段:左裁剪时 startFrame+=deltaFrames、trimStartFrame+=round(deltaFrames*speed)、durationFrames-=deltaFrames;右裁剪时 durationFrames+=deltaFrames、trimEndFrame-=round(deltaFrames*speed)。
- 裁剪提交(commitTrim→trimClips→trimClipInternal):入参是新的源帧 trim 值。deltaSource=新 trim-旧 trim;deltaTimeline=round(deltaSource/speed);newDuration=prevDuration-deltaStartTimeline-deltaEndTimeline;newStartFrame=startFrame+deltaStartTimeline。trimValues 计算左/右目标 trim:sourceDelta=round(delta*speed);左:newTrimStart=trimStartFrame+sourceDelta(图片/文本不夹,否则 max(0,_));右:newTrimEnd=trimEndFrame-sourceDelta(同样图片/文本不夹)。trim 是'overwrite 式':同轨相邻片段不位移、不推其它轨。setDuration 后调用 clampKeyframesToDuration+clampFadesToDuration。若 propagateToLinked 则对链接伙伴用同一 deltaFrames 各自按各自 speed 重算。整个 trim 注册为可逆 undo(registerUndo 反向再注册自身)。
- 片段移动(moveClip)实时:candidateFrame=cursorFrame-grabOffsetFrames。吸附探针=每个被拖片段相对 lead 的偏移 baseOffset 及 baseOffset+durationFrames(头尾都能吸)。deltaFrames=(吸附帧-探针偏移)-lead.originalFrame 或 candidateFrame-lead.originalFrame;再 deltaFrames=max(-minOrigFrame,deltaFrames) 防止任何片段被推到负帧。跨轨:光标在已有轨道时 trackDelta=clampedTrackDelta(逐步回退到所有非 pinned 片段都落在类型兼容轨道的最大可行 delta);光标在轨道间隙/顶部/底部则 dropTarget=newTrackAt。
- 移动中的 pinned 伙伴规则(pinnedCompanionIds):与 lead 同 linkGroup 的伙伴=pinned(保持原轨,只随水平 delta 移动);类型与 lead 轨道不兼容的选中片段也 pinned。ghost 绘制:被拖片段原位以 0.3 alpha(复制模式 1.0)、ghost 以 0.7 alpha;新建轨道落点时 lead 行非 pinned 片段画在插入线上方一个轨道高。
- 移动提交(mouseUp):若落在原轨且 deltaFrames=0 则无操作。existingTrack:刚性平移,非 pinned 片段 toTrack=trackIndex+delta、pinned 保持本轨,toFrame=originalFrame+frameDelta;调 moveClips 或(复制模式)duplicateClipsToPositions。newTrackAt:先 insertTrack(at:type=lead 轨道类型)得到 newIdx,lead 行非 pinned 片段跳到 newIdx、其余按 >=newIdx 则+1 平移,再提交;整体包在一个 undo group。
- moveClips 算法(EditorViewModel):先校验目标轨道类型兼容(isCompatible:同类型或都 isVisual);把所有被移动片段先从源轨道摘除(使后续 clearRegion 不会误伤它们);对每个目标轨道范围 clearRegion(overwrite 清空目标占用区间,prune:false);再把每个片段 startFrame 设为 toFrame 追加到目标轨;所有轨道 sortClips(按 startFrame 升序);pruneEmptyTracks。整个为单条 timeline-swap undo。
- 分割(splitClip atFrame):仅当 atFrame>startFrame && atFrame<endFrame。若片段有 linkGroup,则对组内所有片段都在同一 atFrame 分割,并把右半片段重新 stamp 一个新 linkGroupId(各侧自成一对)。splitSingleClip:splitOffset=atFrame-startFrame;leftSource=round(splitOffset*speed);rightSource=round((durationFrames-splitOffset)*speed)。左半:durationFrames=splitOffset、trimEndFrame=原 trimEnd+rightSource、fadeOutFrames=0;右半:新 id、startFrame=atFrame、durationFrames=原-splitOffset、trimStartFrame=原 trimStart+leftSource、fadeInFrames=0。两半各自 clampFadesToDuration。每条关键帧轨在切点处插入边界关键帧后分配到两侧(保持曲线连续):左侧保留 frame<=splitOffset 并在末尾补 splitOffset 处采样值;右侧取 frame>=splitOffset 并整体减 splitOffset、首部补 0 处采样值。注册可逆 undo。
- overwrite 清区算法(OverwriteEngine.computeOverwrite,clearRegion 调用):对区间 [regionStart,regionEnd) 内每个片段判断:完全在区外(ce<=start||cs>=end)跳过;完全在区内→remove;跨越整个区间(cs<start && ce>end)→split(右半 trimStart=原 trimStart+round((regionEnd-cs)*speed)、右半 start=regionEnd);仅左侧重叠(cs<start)→trimEnd(newDuration=regionStart-cs);仅右侧重叠→trimStart(newStart=regionEnd、newTrimStart=原+round((regionEnd-cs)*speed)、newDuration=ce-regionEnd)。trimEnd 提交时还要把 trimEndFrame+=round((原 duration-新 duration)*speed)。
- 波纹删除(rippleDeleteSelectedClips):被删片段区间=各自 [startFrame,endFrame]。每轨:有自身删除→computeRippleShifts;无自身删除但 syncLocked→computeRippleShiftsForRanges(用全局合并后的被删区间),且若 validateShifts 检出碰撞或负帧则 NSSound.beep 拒绝整次操作。RippleEngine.computeRippleShiftsForRanges:合并区间后,对每个片段 shift=Σ(所有 end<=clip.startFrame 的被删区间长度),newStart=startFrame-shift。mergeRanges:按 start 排序,相邻 range.start<=last.end 即合并取 max(end)。
- 波纹插入(rippleInsertClips):totalPush=Σ各资产 clipDurationFrames。对目标轨+所有 syncLocked 轨(以及 specs 版:链接音频落点轨)执行 computeRipplePush(把 startFrame>=insertFrame 的片段全部 +pushAmount);插入式还会先对每条被推轨道上跨越 atFrame 的片段 splitClip(让其右半随波纹一起后移而不是被覆盖);随后顺序 placeClip 填入。
- 范围波纹删除(rippleDeleteRangesOnTrack):合并 ranges,totalRemoved=Σlength;clearTrackIds=锚轨+所有被触及链接片段的伙伴所在轨;先 dry-run 校验所有未清空的 syncLocked 跟随轨能否吸收(validateShifts);再对 clearTrackIds 逐区 clearRegion,然后对 clearTrackIds∪syncLocked 轨 applyShifts。validateShifts:把 shift 后的区间排序,任一 start<0 或相邻 start<前 end 即返回拒绝原因字符串。
- 吸附 findSnap 算法:baseFrameThreshold=baseThreshold(8px)/pixelsPerFrame。sticky 滞回:若已吸附到 snapped,只要探针位置(position+currentProbeOffset)距 snapped<=baseFrameThreshold*1.5 且 snapped 仍是有效目标,就保持吸附;否则解除。否则遍历所有(探针,目标)对求最近:阈值 playhead 目标=base*1.5、clipEdge=base*1;dist<=阈值且更近则更新 best。命中即触觉 .alignment 反馈并记忆 sticky。返回 SnapResult{frame=目标帧, probeOffset, x=目标帧*pixelsPerFrame}。snapTargets=所有(非排除)片段 startFrame 与 endFrame,加可选 playheadFrame。
- 音量橡皮筋 dB↔Y 换算(ClipRenderer):橡皮筋显示范围 volumeRubberBandTopDb=+6、bottomDb=-60(注意与 VolumeScale 的 floorDb=-60/ceilingDb=+15 不同——绘制范围窄于编辑范围)。y(forDb)=body.minY+((6-clamp(db))/(6-(-60)))*body.height(Y 轴翻转,高 dB→小 Y);db(forY)=6-frac*(6-(-60)),frac=clamp((y-body.minY)/body.height,0,1)。音量关键帧 2D 拖拽:dB 由 cursorY 经 db(forY) 反算并夹到 [floorDb=-60,ceilingDb=15];帧位置夹在左右相邻关键帧之间(leftBound=max neighbor.frame+1,rightBound=min neighbor.frame-1)。
- 音量/淡变模型:有效线性音量 volumeAt(frame)=volume*kfGain*fadeMultiplier;kfGain=linearFromDb(volumeTrack.sample(at:offset))或 1。VolumeScale:dbFromLinear=clamp(20*log10(linear),-60,15)(linear<=0 时 -60);linearFromDb=db<=-60?0:pow(10,min(db,15)/20)。fadeMultiplier:inMul=fadeInFrames>0? (smooth?smoothstep(t):t), t=min(1,rel/fadeInFrames);outMul 同理用 (durationFrames-rel)/fadeOutFrames;返回 min(inMul,outMul);rel<0 或 rel>durationFrames 返回 0。淡变 knee 拖拽:proposed=左则 originalFrames+delta、右则 originalFrames-delta;cap=max(0,durationFrames-对侧淡变);clamp 到 [0,cap]。setFade 后 clampFadesToDuration(fadeIn 夹 [0,dur],fadeOut 夹 [0,dur-fadeIn])。
- 关键帧采样(KeyframeTrack.sample):空轨返回 fallback;单帧返回该值;frame<=首帧返回首帧值;frame>=末帧返回末帧值;否则找首个 frame>查询帧的 b,a=b-1,raw=(frame-a.frame)/(b.frame-a.frame),按 a.interpolationOut:hold→a.value、linear→lerp、smooth→lerp(smoothstep(raw))。smoothstep(t)=t*t*(3-2*t)。关键帧以'片段相对偏移'存储(offset=absFrame-startFrame),公共 API 用绝对帧、内部 toOffset/toAbs 转换。upsert 按 frame 有序插入或替换同帧。
- 缩放:applyZoom(factor,anchorDocX):frameUnderCursor=anchorDocX/zoomScale;newScale=clamp(zoomScale*factor,[minZoomScale,Zoom.max=40]);更新后令光标下帧保持视口位置(scrollX=frameUnderCursor*newScale-anchorViewportX)。Option+滚轮 factor=exp(scrollingDeltaY*0.04);pinch factor=1+magnification*1.5。minZoomScale=clamp(availableWidth/(totalFrames*fitAllBuffer=3),[Zoom.floor=0.0001,40]),视口/总帧为 0 时取 Zoom.min=0.05。内容宽=zoomScale*totalFrames+visibleWidth*0.5。
- 拖放目标解析 dropTargetAt(y):无轨道→newTrackAt(0);y<首轨顶→newTrackAt(0);轨间边界区(下轨底-insertThreshold(10) 到 上轨顶+10)→newTrackAt(i+1);超过末轨底→newTrackAt(trackCount);否则落在某轨→existingTrack(i)。轨道分区铁律(partitionedInsertionIndex):视觉轨(video/image/text/lottie)必须在 firstAudioIndex 之前,音频轨必须在其后;插入索引据此夹紧。
- 外部拖入落点(resolveDropPlan):顺序为每个资产分配 [cursor,cursor+dur) 区间(dur=segment 时长或 asset.duration 经 secondsToFrame,min 1);视觉目标=resolveVisualDropTarget(光标在音频区时跨分隔线镜像到对应视觉轨 V1↔A1),音频目标=resolveAudioDropTarget(光标指向的音频轨或镜像音频轨,无则底部新建)。materialize 时若先插了视觉轨需 shiftAfterVisualInsertion 把音频目标索引+1。Cmd 修饰=波纹插入,否则 overwrite 式 addClips。
- 几何细节:clipRect=NSRect(x=headerWidth+startFrame*pxPerFrame, y=trackY+2, w=durationFrames*pxPerFrame, h=trackHeight-4)。trackY 用预算 cumulativeY(起点=rulerHeight(24)+dropZoneHeight(60),逐轨累加 displayHeight)。裁剪手柄宽 Trim.handleWidth=4,圆角 3。标尺自适应:目标主刻度间距 80px,候选 frames=[1,2,5,10,15,30,60,120,300,600,1200,1800,3600]*fps 取首个>=rawFrames;次刻度尝试 10/5/4/2 取首个每格>=12px。撤销重做总体策略:withTimelineSwap 捕获整个 Timeline 值快照做双向 swap(disableUndoRegistration 包住实际改动,嵌套时跳过);细粒度操作用 registerClipPropertySwap/registerClipStateSwap 保存 before/after 整片段做双向 undo。

**苹果框架使用**:
- AppKit (NSView/NSEvent/NSScrollView) [high] — 整个时间线是 NSView 子类自绘 + NSEvent 手势(鼠标/滚轮/pinch/拖放),NSScrollView 提供滚动视口,NSTrackingArea 做 mouseMoved 光标
- CoreGraphics / Quartz (CGContext) [medium] — drawContent 用 CGContext 画轨道、片段卡片、波形、橡皮筋、楔形、关键帧、标尺;CGPath 路径;CGImage 缩略图绘制与裁剪(雪碧图 cropping)
- CoreAnimation (CAShapeLayer/CATransaction) [low] — 播放头(竖线+三角)与吸附虚线用 CAShapeLayer 单独图层,CATransaction 关动画即时更新
- AVFoundation (AVAssetImageGenerator/AVURLAsset/CMTime) [high] — MediaVisualCache 抽视频缩略图(maximumSize 120x68、tolerance 1s、appliesPreferredTrackTransform)与读取时长/轨道
- ImageIO + CGImageSource/CGImageDestination [medium] — 图片缩略图生成(kCGImageSourceCreateThumbnail*,MaxPixelSize 120)、缩略图雪碧图 JPEG 编解码(质量 0.75)
- Observation (withObservationTracking) [low] — PlayheadOverlay 监听 playheadState.timelineFrame/zoomScale 变化自动重画播放头(Swift 宏观察)
- CryptoKit (SHA256) [none] — 磁盘缓存键= SHA256(url.path|size|mtime) 前 16 字节十六进制
- NSHapticFeedbackManager [low] — 吸附命中时触发 .alignment 触觉反馈(trackpad)
- SwiftUI (NSViewRepresentable/NSHostingView) [medium] — 把 AppKit 时间线桥接进 SwiftUI;生成中片段用 NSHostingView 叠 SwiftUI 动画覆盖层
- DSWaveformImage (第三方) [medium] — WaveformAnalyzer().samples 抽音频波形(0=loud,1=silence 归一化)
- NSMenu / NSImage SF Symbols [low] — 右键上下文菜单与 AI 子菜单;轨道头静音/隐藏/同步锁用 SF Symbol 图标

**闭源云**:无。Timeline 目录全量 grep(convex/clerk/URLSession/http/fetch/api.)无任何网络或闭源云访问。唯一与 AI 相关的是 TimelineView+AIEditMenu.swift 仅构建 NSMenu 并把动作转发给 editor.beginAIEdit/runAIUpscale/beginAICreateVideo 等(实际云调用发生在 Agent/生成模块,不在本目录)。媒体缩略图/波形均为本地 AVFoundation/ImageIO/DSWaveformImage 离线生成并落本地磁盘缓存。

**移植策略**:分两层处理。(1) 纯算法层可 direct-port 到 Rust core(放进 Rust 领域 crate,React 前端调 Tauri command):SnapEngine(收集边缘+多探针+sticky 1.5x 滞回+播放头 1.5x 优先)、TimelineGeometry(帧↔像素、trackY 累加、dropTargetAt 阈值 10px、insertThreshold)、RippleEngine/OverwriteEngine(已是纯函数,mergeRanges/computeRippleShifts*/computeRipplePush/computeOverwrite)、裁剪/移动/分割/波纹的钳制与提交规则、关键帧 sample(hold/linear/smooth+smoothstep)、VolumeScale(20*log10 / pow(10,db/20),floor -60/ceiling 15)、淡变 fadeMultiplier、时码 formatTimecode、secondsToFrame(务必复刻 Int 截断而非四舍五入,trim/source 换算处复刻 round() 行为)。注意 Swift Int 截断 vs round 的混用要逐处对齐(frameAt 用截断,trim 源帧换算用 round)。(2) 渲染与输入层必须 ui-rebuild:用 React + Canvas2D/WebGL(或 wgpu via Tauri)重画轨道/片段/波形/橡皮筋/楔形/缩略图条/标尺/播放头;鼠标手势状态机(DragState)用 TS 重写或保留在前端,吸附/几何调用 Rust。(3) 媒体可视化 needs-replacement:AVAssetImageGenerator→FFmpeg 抽帧(ffmpeg -ss -i -vframes,或 ffmpeg 滤镜批量),DSWaveformImage 波形→FFmpeg(astats/showwavespic 或解码 PCM 自算 RMS/peak,复刻 0=loud/1=silence 归一化与峰值检测),ImageIO 缩略图→image crate/FFmpeg;磁盘缓存(SHA256(path|size|mtime) + JPEG 雪碧图 + JSON 边栏 + 二进制 f32 波形)可在 Rust 用 sha2+image+serde 1:1 复刻,缓存目录与文件名约定照搬以便缓存兼容。(4) CAShapeLayer 播放头/吸附线→前端单独 canvas 图层或 SVG。(5) 触觉反馈在桌面 Web 无对应,drop 之(none)。(6) undo/redo:Swift 用 NSUndoManager+整 Timeline 值快照双向 swap;Rust 复刻为 command 模式或快照栈(每次编辑前后存 Timeline 快照,withTimelineSwap 语义=值相等则不记录、嵌套合并),建议在 Rust core 实现统一 undo 栈而非依赖前端。关键风险:波形/缩略图视觉与抽帧时间点要与 FFmpeg 对齐(原实现缩略图间隔 <10s 用 1s、否则 2s;波形采样数=duration<133s 时 max(4000,duration*150) 否则 20000),否则画面对不上但不影响剪辑数据正确性。

**关键文件**:Sources/PalmierPro/Timeline/TimelineInputController.swift、Sources/PalmierPro/Timeline/SnapEngine.swift、Sources/PalmierPro/Timeline/TimelineGeometry.swift、Sources/PalmierPro/Timeline/ClipRenderer.swift、Sources/PalmierPro/Timeline/TimelineView.swift、Sources/PalmierPro/Timeline/MediaVisualCache.swift

## Preview  ·  `mixed` → **needs-replacement**

**职责**:
- 把 Timeline 编译成 AVMutableComposition(逐轨逐片段插入源轨,处理间隙/变速/重叠剔除),CompositionBuilder
- 把片段的不透明度/音量/位置/缩放/旋转/裁剪关键帧 + 淡入淡出 编译成 AVVideoComposition 的 layer instruction 与 AVAudioMix 的音量斜坡
- 实时播放引擎:AVPlayer 封装、播放/暂停/逐帧、精确 seek 与拖拽 seek 的节流+容差策略,VideoEngine
- 画布交互叠加层:变换框(移动/缩放/旋转/吸附/中心参考线)与裁剪框(平移/缩放/锁定宽高比/三分线),TransformOverlayView/CropOverlayView
- 文字层渲染:预览用长期存活的 CATextLayer 树(带 preroll 预热),导出用一次性 CAKeyframeAnimation 离散关键帧,TextLayerController
- 媒体烘焙缓存:静态图→长时长 .mov、纯黑背景 .mov、Lottie→ProRes4444 alpha .mov、直通道 alpha→预乘 alpha .mov(ImageVideoGenerator/LottieVideoGenerator/AlphaVideoNormalizer)
- 片段范围导出到临时 mp4(含文字烘焙、可选音频、按短边缩放),TimelineRenderer
- 预览顶部标签页/传输条/进度拖拽条/工程设置(画幅/帧率/分辨率/缩放)UI,PreviewContainerView/PreviewTab
- 离线/不可处理媒体的占位与重新链接 UI,生成中/失败态占位

**核心类型**:
- `CompositionBuilder` (enum) — 无状态命名空间。核心:build() 把 Timeline 编译为 AVMutableComposition+AVVideoComposition+AVAudioMix;buildVisuals() 只重建视觉/音量属性(变换/不透明度/音量/裁剪)而不重插源轨;还含关键帧→斜坡指令的全部算法(trackOps/emitTransform/emitCrop/emitOpacity/emitVolumeEnvelope/affineTransform)。
- `CompositionResult` (struct) — build() 的产物聚合:composition、audioMix、videoComposition、trackMappings、每片段 naturalSize/preferredTransform、离线 mediaRef 集合、不可处理 mediaRef 集合。
- `TrackMapping` (struct) — 一条 AVMutableCompositionTrack 与其语义来源的映射:.timeline(轨道下标, 该合成轨承载的 clipId 集合) 或 .blackBackground(时间区间);记录 naturalSize/endTime/isVideo。变速片段会单独占一条合成轨。
- `VideoEngine` (class) — @MainActor 播放引擎。持有 AVPlayer、TextLayerController、当前 trackMappings;负责 rebuild/refreshVisuals、play/pause/seek、周期时间观察器(回写 currentFrame)、拖拽 seek 的节流(30fps)与按活跃图层数动态放大的 seek 容差。
- `TextLayerController` (class) — @MainActor 文字层管理。预览:维护 layersByID 的 CATextLayer 树,按 preroll 窗口惰性materialize/回收,tick() 改不透明度;导出:buildForExport() 产出带离散 CAKeyframeAnimation 的一次性层树喂给 AVVideoCompositionCoreAnimationTool;buildSnapshot() 出单帧层。
- `ImageVideoGenerator` (enum) — 把静态图烘焙成 30 分钟长的 .mov(只写首尾两帧),纯黑背景 .mov;按 alpha 选 ProRes4444 或 H.264;按 mediaRef+尺寸+alpha 后缀命名做磁盘缓存;提供图片原生尺寸/是否含 alpha 探测。
- `LottieVideoGenerator` (enum) — 用 Lottie 库把 .json/.lottie 逐帧渲染到 CGContext→CVPixelBuffer,写成 ProRes4444 alpha .mov,末尾把最后一帧定格到极远时间点(freeze-frame);提供 inspect/sampleFrames 给媒体库。
- `AlphaVideoNormalizer` (enum) — 检测源视频是否为直通道(straight)alpha(只信编解码器的 ContainsAlphaChannel 扩展),若是且无旋转,用 AVAssetReader 逐帧 vImage 预乘 alpha 后用 ProRes4444 重新编码,供正确合成;按文件大小+修改时间做缓存键。
- `TransformOverlayView` (struct) — SwiftUI 画布变换叠加层:选中片段的边框+四角手柄,移动/缩放手势,缩放时锁定媒体宽高比、边缘吸附、中心参考线;文字片段缩放改 fontScale 而非尺寸;旋转时跳过吸附并用旋转命中区。
- `CropOverlayView` (struct) — SwiftUI 裁剪叠加层:在片段矩形内绘制裁剪框+四周遮罩+三分线,平移/四角缩放手势;支持按 CropAspectLock 锁定宽高比;把屏幕拖拽量逆旋转回片段本地轴以适配旋转片段。
- `TimelineRenderer` (enum) — 把 Timeline 的 [startFrame, startFrame+frameCount) 区间用 AVAssetExportSession 导出为临时 mp4,复用 CompositionBuilder + 文字烘焙(动画工具),可选音频、可按短边缩放。
- `PreviewNSView` (class) — AppKit 宿主视图:承载 AVPlayerLayer + 文字 CALayer 树;布局时把 playerLayer 铺满并把文字根 frame 对齐 videoRect;cmd+滚轮缩放画布(以光标为锚点计算偏移)。
- `PreviewTab` (enum) — 预览标签页模型:.timeline 或 .mediaAsset(id,name,type);提供稳定 id、显示名、是否可关闭、配色。

**核心算法/逻辑(供 Rust 复刻)**:
- 【时间单位】全程以整数帧为权威单位。CMTime 用 timescale = fps(timescale=CMTimeScale(timeline.fps)),frame→CMTime 即 CMTime(value: frame, timescale: fps)。秒→帧:secondsToFrame = Int(seconds*fps)(向零截断,非四舍五入)。timecode 格式 HH:MM:SS:FF:absFrame=|frame|; ff=absFrame%fps; ss=(absFrame/fps)%60; mm=(absFrame/fps/60)%60; hh=absFrame/fps/3600;各段两位补零,负帧加'-'。
- 【合成总体流程 CompositionBuilder.build】前置校验 fps>0 && width>0 && height>0,否则抛 InvalidTimelineError。逐轨道遍历(保持 timeline.tracks 顺序);每轨 clips 先按 startFrame 升序排序,并过滤掉 mediaType==.text(文字永不进合成轨,只走 CATextLayer)。空轨跳过。轨道类型决定 AVMediaType(audio/video)。
- 【视频轨道插入规则】为该视频轨建一条合成轨,cursor=0,previousEndFrame=Int.min。逐片段:要求 durationFrames>0 且 startFrame>=previousEndFrame(即丢弃与前一片段重叠的片段,重叠剔除是'后者让位前者');加载源(见媒体烘焙);读取源轨 naturalSize 与 preferredTransform,计算显示尺寸=natSize.applying(preferredTransform) 的包围盒绝对宽高,存入 clipNaturalSizes[clip.id];把 preferredTransform 复合一个平移使包围盒原点归零,存入 clipTransforms[clip.id]。插入成功则 insertedCount++、记录 clipId、previousEndFrame=clip.endFrame。若整轨无插入则移除该合成轨。轨道 naturalSize 取合成轨 naturalSize,无效则回退 renderSize。
- 【音频轨道插入规则】音频分两类轨:speed==1.0 的片段共用一条'normalTrack'顺序拼接;speed!=1.0 的片段每个单独占一条合成轨(以便 scaleTimeRange 变速)。normalTrack 若最终无片段则移除。每个映射记录其承载的 clipId 集合,用于后续音量包络只作用于本轨片段。
- 【单片段插入 insertClip】clipStart=帧→CMTime。trimStart:图片片段用 max(0,trimStartFrame),其它用 trimStartFrame。若 clipStart>cursor 先插入空区间(透明间隙)CMTimeRange(cursor, clipStart-cursor)。源时长 sourceFrames:speed==1 时=durationFrames;否则=max(1, Int(durationFrames*speed))(注意:speed>1 表示更快→消耗更多源帧)。sourceRange=(trimStart, sourceFrames)。insertTimeRange(sourceRange, of: sourceTrack, at: clipStart) 失败则记录日志并跳过该片段(返回 false,不致命)。若 speed!=1 则 scaleTimeRange((clipStart,sourceFrames) → durationFrames) 实现变速。cursor=clipStart+durationFrames。
- 【黑底背景层】所有视频映射的 endTime 取最大值 lastVideoEnd;desiredDuration=max(CMTime(totalFrames,fps), lastVideoEnd)。若>0 则插入一条最底层不透明黑色视频轨(由 ImageVideoGenerator.blackVideo 烘焙的 .mov),区间[0,desiredDuration],作为 .blackBackground 映射。它保证空白处是黑而非透明。
- 【视觉编译 buildVisuals(可独立重算)】renderSize=画布宽高。颜色全程标注 BT.709(primaries/transfer/matrix 均 ITU_R_709_2)。frameDuration=CMTime(1,fps)。对每条视频映射生成一个 AVVideoCompositionLayerInstruction;图层顺序即 trackMappings 中视频映射的顺序(黑底最先加入故在最底)。每个 instruction 先 setOpacity(0,at:.zero)(默认隐藏)。
- 【黑底图层指令】在 range.start setOpacity(1);若 range.end<compositionDuration 在 range.end setOpacity(0)。
- 【时间线视频图层指令】若轨道 hidden 则不发任何可见指令(整轨隐藏)。否则对本映射承载的、非文字、durationFrames>0、startFrame>=prevEndFrame 的片段(再次重叠剔除),按 start=startFrame、end=endFrame:emitOpacity(关键帧+淡入淡出)→在 end setOpacity(0)→emitTransform→emitCrop。映射的 endTime<compositionDuration 时在 endTime setOpacity(0)。
- 【关键帧通用规则 normalizedKeyframes】只保留 frame∈[0,durationFrames] 的关键帧,按帧排序,同帧后者覆盖前者(用字典去重)。关键帧 frame 是'片段内相对偏移',发指令时统一 +clip.startFrame 转绝对时间线帧。Keyframe.interpolationOut 默认 .smooth;Clip.fadeIn/OutInterpolation 默认 .linear。
- 【单属性关键帧→斜坡 trackOps(opacity/crop 用)】track 不活跃(无关键帧)→[setStatic(fallback, clipStart)]。否则:若首关键帧绝对时间>clipStart,先 setStatic(首值, clipStart)(保持)。相邻关键帧 a→b 按 a.interpolationOut:.hold→setStatic(a.value, aT);.linear→若区间>0 则 ramp(a,b,(aT,bT));.smooth→把区间均分为 8 段(smoothSegments=8),每段端点值用 V.keyframeInterpolate(a,b, t=smoothstep(s/8)) 计算(smoothstep(t)=t*t*(3-2t)),逐段发线性 ramp(用分数 CMTime 细分:nextT=aT+CMTime(seconds: span.seconds*t, timescale: span.timescale*8) 避免整数帧塌缩);末关键帧绝对时间<clipEnd 时 setStatic(末值, lastT)。Double 的 keyframeInterpolate=a+(b-a)*t;Crop 逐分量线性;AnimPair 逐分量线性。
- 【不透明度 emitOpacity】无淡入淡出时直接走 trackOps(opacityTrack, fallback=clip.opacity)。有淡入淡出时改走 emitEnvelopeRamps:把 clip.opacityAt(frame) 作为采样函数(它已折叠 静态opacity×关键帧×fade)。值统一 clamp 到[0,1] 且必须 isFinite,时间必须 numeric 且>=0。每片段在 end 处强制 setOpacity(0) 防止越界显示。
- 【音量包络 emitVolumeEnvelope】无关键帧且无 fade:整段一条常量 ramp,值=clip.volumeAt(startFrame)。否则走 emitEnvelopeRamps,采样=clip.volumeAt(startFrame+offset)。轨道 muted→整轨 setVolume(0)。volumeAt(frame)=静态 volume × 关键帧增益 × fadeMultiplier;关键帧值以 dB 存储,linearFromDb(dB)=dB<=-60 时为0(硬静音),否则 pow(10, min(dB,15)/20)。
- 【包络采样点集合 emitEnvelopeRamps】offset 集合初始{0,durationFrames}∪所有关键帧帧;对每对相邻关键帧按出插值补点:.smooth→加 8 等分细分点;.hold→若间隔>1 加 (b.frame-1);.linear→不加。淡入:加 min(dur,fadeInFrames),若 fadeInInterpolation==.smooth 再加 [0,该点] 的 8 等分;淡出:加 max(0,dur-fadeOutFrames),smooth 同理加细分。排序后相邻点之间发线性 ramp(端点各自采样)。
- 【fadeMultiplier】rel=frame-startFrame,rel<0或>durationFrames→0。inMul:fadeInFrames>0 时 t=min(1, rel/fadeInFrames),smooth 则 smoothstep(t) 否则 t;outMul:outRem=durationFrames-rel,fadeOutFrames>0 时 t=min(1, outRem/fadeOutFrames),同理。结果=min(inMul,outMul)。淡入淡出对音频不参与 opacity(opacityAt 对 audio 跳过 fade)。
- 【变换矩阵 affineTransform(t, natSize, renderSize)】把归一化(0–1 画布坐标)的 Transform 映射为 AVFoundation 图层 CGAffineTransform。sx=(renderSize.w/natSize.w)*t.width*(flipH?-1:1);sy=(renderSize.h/natSize.h)*t.height*(flipV?-1:1);tx=(flipH? tl.x+t.width : tl.x)*renderSize.w;ty=(flipV? tl.y+t.height : tl.y)*renderSize.h。placed=scale(sx,sy)∘translate(tx,ty)。若 rotation!=0:绕中心(cx=centerX*renderSize.w, cy=centerY*renderSize.h)旋转 rotation 度(转弧度*π/180,正=顺时针):placed∘translate(-cx,-cy)∘rotate∘translate(cx,cy)。最终图层变换=clipTransforms[clip](源 preferredTransform 归零) ∘ affineTransform。
- 【变换关键帧 emitTransform】无变换动画(position/scale/rotation 轨均不活跃)→setTransform(clip.transform 的 affine, at:start)。否则取三轨关键帧帧的并集(clamp 到[0,dur])。若首偏移>0 先 setTransform(transformAt(start+首偏移), at:start) 保持。每相邻偏移段统一用 8 等分 + 分数 CMTime 细分发 transform ramp,值用 clip.transformAt(帧)(它对每个属性独立按各自轨采样);末偏移<end 时 setTransform 保持到片段末。注意:变换的平滑细分是按'采样 transformAt'实现,而 transformAt 内部各属性按 KeyframeTrack.sample 用各自 interpolationOut(hold/linear/smoothstep)插值——故变换的曲线由各属性关键帧自身的插值模式决定,而 emitTransform 的 8 段细分只是把结果离散成线性 ramp 逼近。
- 【裁剪关键帧 emitCrop】Crop 是源坐标系归一化边距(left/top/right/bottom)。rect(crop)=CGRect(x:left*natW, y:top*natH, w:max(1, visibleWidthFraction*natW), h:max(1, visibleHeightFraction*natH)).applying(preferredTransform.inverted())——即把裁剪矩形变换回'源像素坐标系'(因为 AVFoundation setCropRectangle 作用于源)。visibleWidthFraction=max(0,1-left-right),visibleHeightFraction=max(0,1-top-bottom)。指令同样走 trackOps(cropTrack, fallback=clip.crop)。
- 【KeyframeTrack.sample(核心采样)】空→fallback;单关键帧→其值;frame<=首帧→首值;frame>=末帧→末值;否则找第一个 frame>查询帧的关键帧 b,a=b前一个,raw=(frame-a.frame)/(b.frame-a.frame),按 a.interpolationOut:hold→a;linear→lerp(a,b,raw);smooth→lerp(a,b,smoothstep(raw))。
- 【VideoEngine.rebuild】取消上一次 rebuildTask;收集每个 MediaAsset 的 sourceWidth/Height 做 resolveSourceSize;后台 Task 调 CompositionBuilder.build,完成后回写 trackMappings/sizes/transforms/compositionDuration/离线集/不可处理集;新建 AVPlayerItem(asset:composition) 挂 audioMix/videoComposition,replace 当前 item;syncTextLayers();seek 到 currentFrame;若 isPlaying 则 play。任意阶段 Task.isCancelled 即放弃。
- 【VideoEngine.refreshVisuals(快路径)】仅当已有 currentItem 且 trackMappings 非空时,用 buildVisuals 只重算 audioMix/videoComposition 并赋给现有 item(不重插源轨、不换 item),用于纯视觉属性变更;否则回退全量 rebuild。
- 【seek 策略】exact:取消节流与待定 seek,直接 player.seek(time, toleranceBefore/After=.zero) 精确;interactiveScrub:容差=min(0.75, 0.15*max(1,活跃视频图层数)) 秒(timescale 600),并以 30fps(interval=1/30s)节流——记录 lastInteractiveDispatchTime,若距上次<间隔则起一个 sleep 任务延迟派发,只保留最后一次 pending。每次 seek 前 cancelPendingSeeks。活跃视频图层数=当前帧落在[startFrame,endFrame)内的 video/image 片段所在的可见 video 轨数。seek 时同步 textController.tick(frame)。
- 【周期时间观察器】interval=CMTime(1,fps)。回调里:仅当 isPlaying 且 !isScrubbing 才回写;frame=secondsToFrame(time.seconds,fps);duration=activePreviewDurationFrames;clamped=duration>0? min(frame,duration):frame;timeline 标签写 currentFrame 并 tick 文字,源标签写 sourcePlayheadFrame;若 duration>0 且 frame>=duration 则 pause(到尾自动停)。
- 【播放起点 playbackStartFrame】duration<=0→0;当前帧>=duration(已在尾部)→归零重播,否则从当前帧继续。
- 【文字层预览 reconcile】只有落在 preroll 窗口[startFrame-30, endFrame) 内的片段才拥有 CATextLayer(惰性创建/超出回收),避免播放时排版卡顿。每层 zPosition=可见文字片段在列表中的下标(决定叠放层级)。可见(frame>=startFrame)时 opacity=clip.opacityAt(frame),否则 0(preroll 期保持透明)。restyle 时重应用样式。CATransaction 关闭隐式动画。CATextLayer 通过把 contents/bounds/position/opacity/transform/string 的 action 设为 NSNull 抑制隐式交叉淡入。
- 【文字样式 applyStyle】scale=containerSize.height/1080(以 1080 高为参考缩放)。layer.frame 用 clip.transform.topLeft 与 width/height 乘容器尺寸(文字框用静态 transform,不随变换关键帧动)。fontSize=(style.fontSize*style.fontScale)*scale。string=NSAttributedString(content, 字体+段落对齐+(可选)前景色)。背景/边框/阴影按 enabled 开关并都乘 scale;阴影 shadowOpacity 恒为1,颜色 alpha 充当不透明度,默认阴影 offset(0,-2)、blur 6。根层 isGeometryFlipped=true(左上原点)。
- 【文字层导出 buildForExport】父层 frame=renderSize、isGeometryFlipped、beginTime=AVCoreAnimationBeginTimeAtZero;内嵌一个 videoLayer 供 AVVideoCompositionCoreAnimationTool postProcessingAsVideoLayer。每个可见文字片段建层并加'离散'CAKeyframeAnimation(keyPath=opacity, calculationMode=.discrete):values[i] 在 [keyTimes[i],keyTimes[i+1]) 保持,故 values.count==keyTimes.count-1;按 0..<totalFrames 逐帧给 visible? opacityAt(frame):0,keyTimes[i]=i/totalFrames。fillMode=.both,isRemovedOnCompletion=false。totalFrames=round(totalSeconds*fps)。
- 【画布交互坐标系】视频内容矩形 videoContentRect:在视图内按 timeline 宽高比做 aspect-fit 居中。clipFrame=videoRect 原点 + transform.topLeft*videoRect 尺寸,宽高=transform.width/height*videoRect 尺寸。所有交互以归一化(0–1)画布坐标存回 Transform/Crop。
- 【变换移动 movedTransform】centerX += Δx/videoRect.w;centerY += Δy/videoRect.h。未旋转时:snapToCanvasEdges(阈值=8px/videoRect.w 或 /h)把片段四边吸附到 0/1;snapCenterToCanvasCenter(阈值同上)把中心吸到 0.5 并返回是否吸附(用于画中心参考线)。旋转(rotation!=0)时跳过所有吸附。snapToBoundary(v,th):|v|<th→0;|v-1|<th→1;否则原值。snapToCanvasEdges:优先吸左/上边,否则吸右/下边,通过调 centerX/Y 平移整框。
- 【变换缩放 resizedTransform】minSize=0.05。把角拖拽量(Δ/videoRect)加到对应边(left/top/right/bottom);随后夹住被拖边不越过对边(矩形永不反转)。若有 mediaCanvasAspect(源宽高比/画布宽高比),按角维持宽高比:比较当前 w 与 h*aspect,取较大者驱动另一维(顶角调 top、底角调 bottom 或左右同理)。未旋转时做边缘吸附(8px 归一化),吸附后按 aspect 反算另一维。输出 width/height 至少 0.05,保留原 rotation。
- 【文字缩放 textScale】不改尺寸而改 fontScale:wSign/hSign 按角符号;wRatio=max(0.01,(start.w+wSign*Δx)/start.w),hRatio 同理;newScale=max(0.05, startScale*sqrt(wRatio*hRatio))(几何均值各向同性缩放),并随后 fitTextClipToContent。
- 【裁剪交互】Crop 存 left/top/right/bottom 边距(源归一化)。屏幕拖拽先 clipLocal() 逆旋转回片段本地轴:dx'=Δw*cos+Δh*sin, dy'=-Δw*sin+Δh*cos(r=rotation*π/180)。平移 pannedCrop:dx=Δx/clipRect.w, dy=Δy/clipRect.h;可见宽=1-left-right,可见高=1-top-bottom;L=clamp(left+dx, 0, 1-visW),T 同理;right=1-visW-L, bottom=1-visH-T(保持可见尺寸不变只移位)。
- 【裁剪缩放(自由)resizedCrop】minVis=0.05;按角对 L/T/R/B 加减 dx/dy(topLeft: L+=dx,T+=dy; topRight: R-=dx,T+=dy; bottomLeft: L+=dx,B-=dy; bottomRight: R-=dx,B-=dy);各值 clamp 到[0, 1-minVis-对边]。
- 【裁剪缩放(锁宽高比)resizedCropLocked】aspectN=目标像素宽高比/源像素宽高比。以'宽度等效量's 驱动:按角定 widthDelta/heightDelta 符号;sFromW=startVisW+widthDelta, sFromH=aspectN*(startVisH+heightDelta);谁动得多用谁。s 上限=min(锚定边到画布的可用宽, aspectN*可用高);下限=max(minVis, minVis*aspectN)。newVisW=s, newVisH=s/aspectN;按角反算 L/T/R/B(以未动的对边为锚)。
- 【apply vs commit(撤销语义)】overlay 拖拽中调 apply*(实时改模型,合并进同一撤销步,不设 action 名);拖拽结束调 commit*(落一个撤销条目并 setActionName)。写入策略 writeTransform/writeCrop:若对应关键帧轨'活跃'(已有关键帧)则在 activeFrame upsert 一个关键帧(关键帧帧=绝对帧-startFrame);否则直接改 clip 的静态 transform/crop。即'有动画就打点,无动画就改基值'。文字缩放写 fontScale 同理(scaleTrack 活跃则打 scale 关键帧,否则改 textStyle.fontScale)。
- 【片段范围导出 TimelineRenderer】renderSize:无 shortSide→画布取偶;有则按短边 scale=min(1, shortSide/canvasShort) 等比缩小后取偶(even=max(2, round(v)/2*2))。复用 CompositionBuilder.build;AVAssetExportSession(preset 默认 MediumQuality);includeAudio=false 时移除合成里所有音频轨;文字用 buildForExport+AVVideoCompositionCoreAnimationTool 烘焙;timeRange=[startFrame, frameCount];导出 .mp4 到临时目录。
- 【媒体烘焙缓存键】图片:'{mediaRef}_{w}x{h}{_a|_o}.mov'(_a=含alpha用ProRes4444,_o=不透明用H.264+BT.709/sRGB传递);黑底:'_black_{w}x{h}.mov';Lottie:'{mediaRef}_{w}x{h}.mov'(ProRes4444);预乘alpha:'{mediaRef}_{size}_{mtime}_premul.mov'。编码尺寸 clampedForEncoder:长边>4096 按比例缩,且宽高强制为偶数(max(2, floor 后去奇))。图片/黑底用首尾两帧(t=0 与 t=ceil(1800s)-1)铺满 30 分钟以便任意拉伸;Lottie 末帧定格到 max(1800s, duration+1)。写入用'.writing-UUID.mov'临时文件再原子 move,move 前若目标已存在则放弃(并发安全)。
- 【直通道 alpha 预乘 premultiply】仅当 track 的编解码器扩展 ContainsAlphaChannel==true 且 preferredTransform 为 identity(有旋转则跳过以免丢失原始 transform)且尺寸>=2 才处理。AVAssetReader 逐帧读 32BGRA,vImagePremultiplyData_RGBA8888 原地预乘(RGB←RGB*A/255),ProRes4444 重编码。用 requestMediaDataWhenReady + CheckedContinuation + OSAllocatedUnfairLock 保证只 resume 一次。
- 【cmd+滚轮画布缩放】factor=exp(deltaY*灵敏度);newZoom=clamp(oldZoom*factor, 0.1, 8.0);以光标点为锚保持视觉不动:fitW/H=viewSize/oldZoom;dx=fitW*(newZoom-oldZoom)/2 + pointTopDown.x*(1-newZoom/oldZoom),dy 同理;canvasOffset += (dx,dy)。灵敏度:精确滚轮0.005 否则0.05。
- 【UI 工程设置预设】画幅 AspectPreset:16:9=1920x1080,9:14=1080x1680,9:16=1080x1920,1:1=1080x1080,4:3=1440x1080,2.4:1=2560x1080。帧率档:24/25/30/50/60。分辨率 QualityPreset 按短边缩放保持宽高比:720/1080/1440(2K)/2160(4K),宽高取偶。画布缩放档:0.25/0.5/0.75/Fit(=1.0)/1.25/1.5/2.0,Fit 判定 |zoom-1|<0.01。质量徽标:短边<=720→HD,<=1080→FHD,<=1440→2K,否则4K。画幅徽标用 gcd 约分。

**苹果框架使用**:
- AVFoundation [blocker] — 全模块核心:AVMutableComposition/AVMutableCompositionTrack 排布片段、insertTimeRange/insertEmptyTimeRange/scaleTimeRange 实现间隙与变速;AVVideoComposition + AVVideoCompositionLayerInstruction(setOpacity/addOpacityRamp/setTransform/addTransformRamp/setCropRectangle/addCropRectangleRamp)做逐帧合成;AVMutableAudioMix/AVMutableAudioMixInputParameters(setVolume/setVolumeRamp)做音量;AVPlayer/AVPlayerItem 播放与 seek;AVAssetWriter/AVAssetWriterInputPixelBufferAdaptor 烘焙图片/Lottie/预乘视频;AVAssetReader 读源帧;AVAssetExportSession 导出;AVVideoCompositionCoreAnimationTool 烘焙文字层;AVURLAsset.load(.naturalSize/.preferredTransform/.formatDescriptions)。
- CoreMedia [medium] — CMTime/CMTimeRange 作为时间表示(timescale=fps);CMFormatDescriptionGetExtension 读 ContainsAlphaChannel;CMSampleBufferGetImageBuffer/PresentationTimeStamp。
- CoreAnimation/QuartzCore [high] — CATextLayer 渲染文字(string/alignmentMode/shadow/border/background/contentsScale/isWrapped);CAKeyframeAnimation(.discrete)做导出文字不透明度;CATransaction 抑制隐式动画;CALayer 树叠加;CGAffineTransform 全部变换数学。
- CoreVideo [medium] — CVPixelBuffer/CVPixelBufferPool 作为烘焙与预乘的像素载体(32BGRA),CVPixelBufferLockBaseAddress/GetBytesPerRow/SetAttachment(色彩空间)。
- CoreGraphics [medium] — CGContext(premultipliedFirst+byteOrder32Little)绘制图片/Lottie 到像素缓冲;CGImageSource 读图片尺寸与 alpha;CGColorSpace(sRGB)。
- Accelerate/vImage [low] — vImagePremultiplyData_RGBA8888 在 CPU 上把直通道 alpha 原地预乘成预乘 alpha。
- AppKit [high] — NSView/AVPlayerLayer 宿主、NSImage 解码、NSColor/NSFont 文字样式、NSOpenPanel 重新链接文件、NSCursor 光标、NSEvent 滚轮缩放、NSScreen.backingScaleFactor。
- SwiftUI [high] — PreviewContainerView/TransformOverlayView/CropOverlayView 全部叠加层交互与传输条/标签页 UI;DragGesture 手势、Canvas 绘制遮罩/三分线、GeometryReader 取尺寸。
- Lottie(第三方) [high] — 加载 .json/.lottie 并逐帧渲染为位图用于烘焙与缩略图/取样。

**闭源云**:无。Preview 目录内不含任何 Convex/ConvexMobile/Clerk/ClerkKit 调用,也无任何网络请求或生成式 AI 云访问;仅触及本地文件系统(磁盘缓存于 ~/Library/Caches/PalmierPro、临时目录导出)与本地媒体。生成相关仅出现 UI 占位(activeMediaAsset.isGenerating / generationStatus / retryDownload),真正的云调用在本模块之外的 generationService(不在此目录)。

**移植策略**:分三层处理。1) 纯算法层(可 direct-port 到 Rust core,与平台无关,价值最高、必须一比一复刻):合成时间轴排布(间隙/变速 sourceFrames=max(1,round(dur*speed))/重叠剔除 startFrame>=previousEndFrame/cursor 推进)、关键帧采样 KeyframeTrack.sample(hold/linear/smoothstep)、smoothstep=t²(3-2t)、8 段平滑细分(smoothSegments=8)、淡入淡出 fadeMultiplier=min(in,out)、音量 dB↔线性(floor -60dB 硬静音、ceil 15dB、pow10(dB/20))、affineTransform 归一化→像素的仿射数学(含 flip/绕中心旋转)、crop rect=源坐标×inverse(preferredTransform)、overlay 的 movedTransform/resizedTransform/吸附(阈值 8px 归一化)/resizedCrop(自由+锁宽高比)/textScale 几何均值/clipLocal 逆旋转、timecode 与 frame↔秒换算(注意 secondsToFrame 是 Int 截断不是四舍五入)。这些用纯 Rust 的 f64 + 自定义 AffineTransform(2x3 矩阵)实现,无需任何外部库;CMTime 用 (i64 value, i32 timescale=fps) 的有理数表示,平滑细分处 Swift 用 timescale*8 的分数 CMTime 防塌缩,Rust 用有理数或直接用 f64 秒+足够精度即可等价。2) 媒体引擎层(needs-replacement,用 FFmpeg):AVMutableComposition/AVVideoComposition 的'声明式逐帧合成'在 FFmpeg 里没有等价物——要把 layer instruction 序列(每片段在每帧的 opacity/transform/crop)转译成 filtergraph(scale+rotate/perspective+crop+overlay+colorchannelmixer 控 alpha,或用 zscale 做 BT.709)或自渲染:推荐用 wgpu/纹理合成做实时预览(每帧按片段 z 序把解码纹理用仿射矩阵+裁剪+不透明度画到画布),导出再走 FFmpeg 编码;音量斜坡用 ffmpeg volume/afade 或自混音。图片→静态源:FFmpeg 直接把图片作为输入循环(-loop 1)或 Rust 端生成单帧纹理,无需烘焙 30 分钟 .mov(那是 AVFoundation 的规避手段)。Lottie:Lottie 库无 Rust 等价的成熟实现,选项是用 rlottie(C 库,thorvg/Samsung,FFI 绑定)逐帧渲染为 RGBA,再走纹理/编码;若 rlottie 覆盖不足则降级为'导入时在前端用 lottie-web 烘焙'。直通道 alpha 预乘:FFmpeg premultiply 滤镜或 Rust 端按 RGB*=A/255 处理纹理(wgpu 着色器里做更简单),vImage 用 SIMD/glam 或着色器替代。3) UI 层(ui-rebuild,用 React/TS):PreviewContainerView/传输条/标签页/工程设置、TransformOverlayView/CropOverlayView 全部在 React 画布上重建(可用 HTML+CSS transform 或 canvas/WebGL 画手柄与遮罩),但务必把 movedTransform/resizedTransform/resizedCrop/吸附/textScale 这些纯几何决策放进 Rust core 经 Tauri command 调用以保证与导出一致,前端只做手势与绘制。apply/commit 的'拖拽中合并、松手落一个撤销条目'语义、以及'有关键帧轨就在当前帧打点否则改基值'的写入策略,必须在 Rust 编辑层一比一实现(撤销栈以 Timeline 不可变快照或命令模式承载)。文字渲染:CATextLayer 的换行/对齐/阴影/边框/背景在 Rust 端可用 cosmic-text/glyphon(wgpu)排版,或导出时用 FFmpeg drawtext(功能弱)——更稳是 wgpu 文本渲染并把 fontSize=fontSize*fontScale*(canvasH/1080) 的参考缩放规则照搬;坐标系注意 Swift 文字根 isGeometryFlipped=true(左上原点),wgpu/HTML 默认即左上,反而省事。颜色管理:全程 BT.709 + sRGB 传递,FFmpeg 用 zscale/colorspace 显式标注以避免色差。坑:(a) 变速 sourceFrames 用 round 且至少 1,音视频变速路径不同(音频变速片段单独成轨);(b) trimStartFrame 对图片 clamp 到 >=0;(c) 黑底层保证空白为黑而非透明,Rust 合成时画布初始填黑即可;(d) seek 容差/30fps 节流是 AVPlayer 性能权衡,Rust+FFmpeg 预览按需重绘可用更简单的'丢帧到最新请求'策略;(e) 重叠片段在三处都被同一条件剔除(插入、音量、视觉),复刻时要保持一致否则音画错位。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Preview/CompositionBuilder.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Preview/VideoEngine.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Preview/TextLayerController.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Preview/TransformOverlayView.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Preview/CropOverlayView.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Preview/ImageVideoGenerator.swift

## Export  ·  `mixed` → **needs-replacement**

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

## Generation  ·  `mixed` → **cloud-rebuild**

**职责**:
- 统一的生成入口 GenerationService.generate:创建占位 MediaAsset、上传引用(可缓存)、构造后端参数、提交任务、订阅状态、下载结果、落地导入、发通知、N 图只首张回调替换
- 引用预处理:视频裁剪段导出(VideoTrimExtractor 用 AVFoundation 把 clip 可见帧区间导出为临时 mp4)、参考视频降采样(VideoCompressor 限制长边 ≤1100px)
- 引用上传缓存:对未裁剪/未预处理的纯净资产按 MediaAsset 缓存远端 URL,TTL=6 天
- 模型目录与能力约束 ModelCatalog/各 *ModelConfig:从 Convex 订阅 models:list,解析出 video/image/audio/upscale 四类模型的 caps(时长/分辨率/宽高比/参考槽位上限/互斥规则等)与计价
- 参数序列化:Video/Image/Audio/Upscale GenerationParams 编码为带 kind 判别字段的后端 JSON
- 成本估算 CostEstimator:按模型计价规则(每秒/每张/每千字符/平铺矩阵)算出 credits,向上取整
- AI 编辑动作矩阵 EditAction:按资产类型/时长/是否已放大/是否生成中,决定 Upscale/Edit/Music/SFX/Rerun/CreateVideo 是否可用及禁用原因
- Rerun/Upscale 提交装配 EditSubmitter:从已存 GenerationInput 复原参数并重新提交,或为编辑面板生成种子输入
- 四类提交装配器(Video/Image/Audio/Music Submission):把 UI/Agent 的输入资产映射成上传顺序、占位时长、buildParams 闭包、snapshot 闭包
- 生成面板 UI GenerationView(SwiftUI):类型切换、模型选择、参考拖拽槽、@提及参考补全、设置弹窗、成本显示、提交;面板高度自适应;种子回填
- 模型启用偏好 ModelPreferences:本地 UserDefaults 持久化被禁用的模型 ID
- 音乐生成 MusicGenerationSubmission:视频转音乐时先渲染选区为低分辨率 mp4 上传,再生成并把占位音频片段放上时间线
- 原生 AppKit 拖放目标 DropTargetOverlay:绕开 SwiftUI 父级 onDrop 遮蔽问题

**核心类型**:
- `GenerationService` (class) — @MainActor 单一生成入口。generate() 是整个流水线的总调度:创建 N 个占位资产→(可选)裁剪/预处理/上传引用(带缓存)→buildParams→runJob 提交并订阅→finalizeSuccess 逐个下载落地→onComplete/onFailure 回调→发系统通知。还含 retryDownload、上传缓存记录、contentType 推断。
- `GenerationBackend` (enum) — @MainActor 的 RPC 薄层,封装 Convex。subscribe(jobId) 订阅单个任务;uploadReference 三步上传(generateUploadTicket→POST 字节→commitUpload 得永久 URL);submit(model,params,projectId) 提交得 jobId。定义 BackendGenerationJob/Params/Status 与错误类型。
- `BackendGenerationParams` (enum) — video/image/audio/upscale 四种参数的统一封装,singleValueContainer 编码;每个具体 Params 自带 kind 判别字段。是发往后端的最终载荷。
- `GenerationInput` (struct) — (定义在 Models/MediaManifest.swift)生成的完整可序列化输入快照:prompt/model/duration/aspectRatio/resolution/quality/各类 URL 与 assetId 列表/音频专属字段/createdAt。随占位资产持久化,是 Rerun 的唯一数据来源。
- `ModelCatalog` (class) — @Observable @MainActor 单例。订阅 Convex models:list 得 [CatalogEntry],分拣为 video/image/audio/upscale 四个数组与 byId 字典。所有 *ModelConfig.allModels 都读它。是「模型能力的运行时真相源」。
- `CatalogEntry` (struct) — 单个模型的后端描述:id/kind/displayName/allowedEndpoints/responseShape/uiCapabilities(四选一 caps)/计价字段(creditsPerSecond/PerImage/PerSecondUpscale/audioPricing/audioDiscountRate)。自定义 Decodable。
- `VideoModelConfig / ImageModelConfig / AudioModelConfig / UpscaleModelConfig` (struct) — 对 CatalogEntry+caps 的强类型封装,提供 validate() 校验与计价访问器。Video 含参考槽位/互斥/源视频要求等最复杂的约束;Audio 含 tts/music/sfx 分类与多种计价。
- `VideoGenerationSubmission` (struct) — 视频提交装配器。InputAssets 子结构承载 sourceVideo/frames/imageRefs/videoRefs/audioRefs 并做完整 validate();make() 根据 requiresSourceVideo 走两条装配路径,产出上传顺序约定、buildParams、snapshotRefs、preprocessRef。
- `EditSubmitter` (enum) — @MainActor。submitUpscale 与 rerun 两大动作:rerun 从存储的 GenerationInput 按模型类型(video/image/audio/upscale)分别复原参数重提交;还提供 editSeed/createVideoSeed/videoAudioSeed 给面板回填。
- `EditAction` (enum) — AI 编辑动作枚举与可用性矩阵。available(for:) 按资产类型给候选,availability() 给每个动作算可用/禁用原因(含 4K 上限、Edit≤10s、已放大、生成中等边界)。
- `CostEstimator` (enum) — 纯函数成本估算器。videoCost/imageCost/audioCost/upscaleCost + cost(for:GenerationInput)。计价规则各异,统一 ceil 向上取整为整数 credits。
- `TrimmedSource` (struct) — 描述 clip 在源媒体中的可见帧区间(trimStartFrame/trimEndFrame/sourceFramesConsumed/fps),供 VideoTrimExtractor 只导出该段上传。durationSeconds = sourceFramesConsumed/fps。
- `VideoTrimExtractor / VideoCompressor` (enum) — 两个 AVFoundation 媒体工具:前者把帧区间导出临时 mp4(并钉死输出帧率防重量化),后者把长边超限的参考视频降采样到 960x540。
- `GenerationView` (struct) — SwiftUI 生成面板(1884 行)。承载全部表单状态、模型/参考槽 UI、@提及补全、成本显示、拖放校验、种子回填(populatePanel)与提交编排(submitGeneration),是 UI 层最大的复刻/重写目标。
- `ModelPreferences` (class) — @Observable @MainActor 单例,用 UserDefaults 持久化被禁用的模型 ID 集合,面板按它过滤模型菜单。

**核心算法/逻辑(供 Rust 复刻)**:
- 【时间/帧单位换算 — 全局约定】生成域内时长基本以『秒(Double 或 Int)』为单位,只有写回时间线时才转帧。秒→帧统一走 editor.secondsToFrame(seconds,fps) 且结果 max(1,…)(见 placeGeneratingAudioClip/finalizeGeneratingClip)。TrimmedSource.durationSeconds = sourceFramesConsumed / max(1,fps)。EditSubmitter.submitUpscale 的 effectiveDuration = trim 有裁剪时 max(1, round(trim.durationSeconds)) 否则 max(1, round(asset.duration));图片占位时长恒为 Defaults.imageDurationSeconds=5.0。Rust 复刻:统一约定『面向模型/成本=秒,面向时间线=帧』,换算函数 frame = max(1, round(seconds*fps))。
- 【GenerationService.generate 主流程 — 必须一比一复刻的顺序与边界】1) count = clamp(numImages,1,4);baseName = name ?? prompt 前30字符;resolvedFolderId 仅当该 folder 存在才保留。2) 目标目录:有 projectURL 则 projectURL/'media' 并创建,否则系统临时目录;占位文件名 'gen-<id前8>.<ext>'。3) 同步创建 count 个占位 MediaAsset(generationStatus=.generating, folderId 设好, 追加进 editor.mediaAssets),记 primaryId=占位[0].id 并立即 return(异步在后台 Task 继续)。4) 异步:若给了 preUploadedURLs 则直接用;否则:取 references 的 url 列表 urlsToUpload;若 trimmedSourceOverride.hasTrim 且非空则用 VideoTrimExtractor 替换 urlsToUpload[0] 并登记待清理;若有 preprocessRef 则对每个 reference 并发执行(TaskGroup)可能产出改写 URL;计算 cacheKeys(只有既无 preprocessRef 且不是被裁剪的第0项时,才用该 MediaAsset 作缓存键);uploadReferences 并发上传(命中 freshRemoteURL 缓存则跳过实际上传,上传成功则写回缓存 TTL=6天)。5) finalGenInput:若给 snapshotRefs 则调用它写回各类 URL,否则把 uploaded 整体塞进 imageURLs;createdAt 为空则置 now;把 finalGenInput 赋给所有占位的 generationInput。6) params=buildParams(uploaded)→runJob。7) 任意环节抛错:所有占位 generationStatus=.failed('Upload failed: …'),调 onFailure。8) defer 清理所有临时文件。
- 【runJob 提交+订阅状态机】生成 runId(UUID前8)。submit(model,params,projectId) 拿 jobId,失败→占位.failed + onFailure。subscribe(jobId) 拿 Combine publisher(nil→.failed('Backend not configured'))。把 publisher 包成 AsyncStream 在主线程消费:job.status == .queued/.running 时 continue;== .succeeded → finalizeSuccess 后 return;== .failed → 占位.failed(job.errorMessage ?? 'Generation failed') + onFailure + return。Rust 复刻:用一个状态轮询/推送通道(WebSocket 或轮询 REST)驱动同一状态机。
- 【finalizeSuccess 下载落地 — N 图分发规则】resultUrls 为空→全部占位 .failed('No URL in response')+onFailure。若 resultUrls.count < 占位数,多出的占位标记失败(只 log 不报错)。逐 i 配对占位[i]↔resultUrls[i]:URL 非法→.failed('No URL for placeholder');否则 downloadAndFinalize 成功才 onComplete?(占位) 并计入 finalized。最后只要有 finalized 就对 finalized.first 发 generationComplete 通知(count=finalized.count);全失败→onFailure。
- 【downloadAndFinalize 落地细节】先置 .downloading。URLSession 下载到临时文件;若远端扩展名非空、与占位 url 扩展名不同、且该扩展名是已知 ClipType,则把目标 url 改成新扩展名(纠正真实类型)。删除旧文件→移动临时文件到目标。清 pendingDownloadURL、status=.none、importMediaAsset(skipAppend:true)→appendGenerationLog→finalizeImportedAsset(异步补全元数据如尺寸/帧率/音轨)。失败:记 pendingDownloadURL=远端 URL、status=.failed(message),供 retryDownload 重试。
- 【uploadReferences 并发+缓存+顺序保持】对每个 url 起 TaskGroup 任务:cacheKey.freshRemoteURL 命中则直接产出该 URL;否则按文件扩展名/回退类型推断 contentType(jpg→image/jpeg, png, webp, heic, gif, mp4/m4v→video/mp4, mov→video/quicktime, mp3→audio/mpeg, wav, m4a→audio/mp4;回退:image→image/jpeg, video→video/mp4, audio→audio/mpeg, text→octet-stream, lottie→application/json),上传后写缓存。最终按原始下标排序返回(顺序对后续 frames/refs 切分至关重要)。
- 【上传引用缓存 freshRemoteURL】MediaAsset.cachedRemoteURL + cachedRemoteURLExpiresAt;freshRemoteURL 仅当存在且 expiresAt>now 才返回。TTL=6*24*60*60 秒。缓存只对『字节纯净』的资产生效(未裁剪、未预处理)。Rust 复刻:按资产内容哈希做上传去重缓存更稳。
- 【VideoTrimExtractor.extract 帧区间导出 — 关键边界】要求 fps>0、sourceFramesConsumed>0。timescale=CMTimeScale(fps);sourceStart=CMTime(value=trimStartFrame,timescale);sourceDuration=CMTime(value=sourceFramesConsumed,timescale);把该 timeRange 从源视频/音轨插入到新 composition 的 0 处;保留 preferredTransform。输出帧率钉死逻辑:读源 nominalFrameRate,若在[24,60]则 round 取整否则用 30,videoComposition.frameDuration=1/targetFps —— 不钉会因 composition timescale 重量化出分数帧率。导出 AVAssetExportPresetHighestQuality 为 mp4。Rust+FFmpeg 复刻:ffmpeg -ss (trimStartFrame/fps) -t (sourceFramesConsumed/fps) -r targetFps,或精确按帧 trim+设置 -r。
- 【VideoCompressor.compressIfNeeded 长边限制】读 naturalSize 与 preferredTransform,display=size.applying(transform),longSide=max(|w|,|h|);若 longSide ≤ maxLongSide(默认1100,调用方对参考视频用默认)→返回 nil(不压);否则用 AVAssetExportPreset960x540 导出临时 mp4(长边落到 960,安全低于 Seedance ~1112 上限),只缩小不放大。Rust+FFmpeg:scale='if(gt(iw,ih),960,-2)':'if(gt(iw,ih),-2,960)' 并仅在超限时执行。
- 【VideoGenerationSubmission.make 两条装配路径 + 上传顺序契约】(A) requiresSourceVideo 模型:references = [sourceVideo?] + imageRefs;buildParams 把 uploaded.first 当 sourceVideoURL,uploaded.dropFirst() 当 referenceImageURLs。(B) 文生视频/图生视频模型:references = frames + imageRefs + videoRefs + audioRefs(顺序固定!);上传后切分:前 frameCount 个是 frames(frames[0]→startFrameURL, frames[1]→endFrameURL),其后依次 imageRefCount/videoRefCount/audioRefCount 切给三类引用 URL。视频参考非空时挂 preprocessRef=VideoCompressor.compressIfNeeded。snapshotRefs 把这套切分同样写回 GenerationInput 的各 URL 字段。Rust 复刻必须严格保持这个『frames 在前、再 image、再 video、再 audio』的扁平上传顺序与切分。
- 【InputAssets.validate 视频引用全部约束(逐条)】源视频路径:必须有 sourceVideo 且 type==video;不允许同时给 frames/videoRefs/audioRefs;不 supportsReferences 时不许 imageRefs;imageRefs.count ≤ maxReferenceImages。文生视频路径:不许 sourceVideo;frames ≤2;有 frames 需 supportsFirstFrame;frames>1 需 supportsLastFrame;framesAndReferencesExclusive 模型 frames 与 任何 ref 不可并存;imageRefs/videoRefs/audioRefs 各自 ≤ 对应上限;maxTotalReferences 总量上限;视频参考累计秒数 ≤ maxCombinedVideoRefSeconds、音频参考累计秒数 ≤ maxCombinedAudioRefSeconds;最后逐资产类型校验(frame/image 必须 image、video 必须 video、audio 必须 audio)。
- 【EditSubmitter.rerun 复原逻辑(按模型类型分支)】前置:必须 isSignedIn、必须有 stored generationInput;复制并清空 createdAt。Video 模型:先 validate(duration/ar/resolution);若 requiresSourceVideo 则 preUploaded.first 为源、其余为 imageRefs,占位时长= asset.duration>0?asset.duration:max(1,duration);否则 startFrame=preUploaded.first、endFrame=preUploaded[1]、其余 referenceX 直接取 stored 的各 URL,并把所有 URL 串成 bundled 作为 preUploadedURLs。Image 模型:count=clamp(numImages,1,maxImages),validate 后用 preUploaded 作 imageURLs。Audio 模型:判断是否需要视频源(inputs 含 video 且 (不含 text 或有 referenceVideoAssetIds 或有 sourceURL));占位时长 fallback 到 music=60s/tts=10s;durationSeconds 仅当模型有 durations 或需视频源且 duration>0 时传。Upscale 模型:preUploaded.first 为源,图片 durationSeconds=1 否则=duration。都用同一 GenerationService.generate 提交。
- 【CostEstimator 计价规则(逐类型,均向上取整 ceil,≤0 记0)】videoCost:rate=creditsPerSecond[resolution]??creditsPerSecond[''];若不生成音频且有 audioDiscount(resolution)则 rate*=折扣;cost=ceil(rate*duration)。imageCost:先查二维矩阵 creditsPerImage['<res>|<quality>'];否则若模型有 qualities 查 creditsPerImage[quality];否则按 resolution(或'')查;再 *max(1,numImages)。audioCost:perThousandChars→ceil(rate*chars/1000)(chars 取 prompt.count,需>0);perSecond→ceil(rate*secs)(需 secs>0);flat→ceil(price);unknown→nil。upscaleCost=ceil(creditsPerSecond*max(1,duration))。
- 【EditAction 可用性矩阵(边界)】候选:image=[upscale,edit,rerun,createVideo];video=[upscale,edit,generateMusic,generateSFX,rerun];audio/text=[upscale,edit,rerun];lottie=[]。upscale:仅 video/image;video 需 sourceHeight>0 且 <2160(≥2160 视为已4K禁用);已是 upscale 结果(model∈UpscaleModelConfig.allIds)禁用;生成中禁用。edit:video 时 effectiveDuration(=asset.duration>0?:generationInput.duration)必须 >0 且 ≤10s(editMaxDurationSeconds=10.0);image 无时长限制;audio/text/lottie 不支持。generateMusic/SFX:仅 video,需 duration>0,需有对应 provider 模型,且 model.validate(spanSeconds) 通过(minSeconds≤s≤maxSeconds,默认1..900)。rerun:需 isGenerated 且 model 仍存在于 ModelRegistry。
- 【VideoToAudioEditKind 模型选择】music→provider 'Sonilo' 偏好 id 'sonilo-v1.1-video-to-music';sfx→provider 'Mirelo' 偏好 id 'mirelo-sfx-v1.5-video-to-audio'。先精确匹配 (id==preferred && category 匹配 && inputs 含 video),否则回退到 category 匹配 + inputs 含 video + id/displayName 含 providerName。
- 【MusicGenerationSubmission.run 视频转音乐】videoToMusic 模式:先 TimelineRenderer.render(timeline, resolver, startFrame, frameCount, shortSide=360, includeAudio=false) 渲染选区低分辨率无声 mp4,上传得 videoURL(用完删临时文件)。durationSeconds=max(1,round(spanSeconds));构造 AudioGenerationParams 与 GenerationInput(createdAt=now)。提交后 onComplete 里 finalizeGeneratingClip(把占位片段时长改为真实);同时立即 placeGeneratingAudioClip 在 startFrame 放占位音频片段。Rust+FFmpeg:用本地渲染管线导低分辨率代理上传。
- 【时间线写回:placeGeneratingAudioClip / finalizeGeneratingClip】place:durationFrames=max(1,secondsToFrame(spanSeconds,fps));先 snapshot timeline,关闭 undo 注册→resolveOrCreateAudioTrack→placeClip(addLinkedAudio:false)→开启 undo 注册;失败回滚 timeline;成功用 registerTimelineSwap(before,after,actionName) 注册为单步可撤销并 notifyTimelineChanged。finalize:按 mediaRef==placeholderId 找到片段,realFrames=max(1,secondsToFrame(asset.duration,fps)),关闭 undo 注册改 durationFrames 并清 trimStart/EndFrame=0,再 notify。这是『占位先放、真长后改、且不污染撤销栈』的核心模式。
- 【N 图替换 clip 的 FirstOnly 语义】当从时间线某 clip 触发替换(pendingEditReplacementClipId):makeOnComplete 返回的回调内用 FirstOnlyFlag.fire() 保证只有第一张成功的资产真正 replaceClipMediaRef(clipId,newAssetId,resetTrim),其余忽略;resetTrim 取 trimmedSource?.hasTrim==true(视频)或 false(图/音)。无替换目标时则 autoOpenPreview 选中新资产。失败回调统一 clearPendingReplacement。
- 【面板 @提及参考补全(GenerationView)】仅在 showsRefSections 且有 availableRefTags(由各类参考数量按 tagNoun 生成 @Image1/@Video1/@Audio1/@<referenceTagNoun>N)时启用。query 提取:取最后一个 '@',其后不含空白且 '@' 前是行首或空白;匹配 label 包含(忽略大小写);上下键移动高亮、回车/点选插入 '@label '、Esc 取消。
- 【面板高度自适应】chromeHeight=measuredPanelHeight-measuredPromptHeight(两次同帧测量恢复出除 prompt 外的固定高度);maxPromptExtra=maxPanelHeight - 2*Spacing.sm - chromeHeight - promptMinHeight(下限0);promptHeight=promptMinHeight+clamp(liveExtra??promptExtra,0,maxPromptExtra)。拖拽 resizeHandle 改 liveExtra,松手落到 @AppStorage promptExtra。
- 【populatePanel 种子回填】按 stored.model 定位模型与类型(upscale/未知直接 return);isPopulatingPanel=true 抑制所有 onChange 副作用,defer 在下一 runloop 复位。回填 prompt/ar/res/quality/duration(同时填 video 和 audio duration)/numImages/voice/lyrics/style/instrumental/generateAudio;clearReferences 后用 mediaAssets 的 id→asset 字典把各 assetId 列表还原成 MediaAsset 槽位(源视频/首末帧/三类参考/图参考/音频源),并据有无引用决定 framesRefsMode。最后 resetSettings 兜底非法值。

**苹果框架使用**:
- AVFoundation [medium] — VideoTrimExtractor 用 AVURLAsset/AVMutableComposition/AVAssetExportSession(HighestQuality)按 CMTime 帧区间导出 mp4 并用 AVMutableVideoComposition 钉死帧率;VideoCompressor 用 AVAssetExportPreset960x540 降采样参考视频,并读 naturalSize/preferredTransform/nominalFrameRate。
- CoreMedia [low] — CMTime/CMTimeScale/CMTimeRange/CMTimeValue/kCMPersistentTrackID_Invalid 做基于帧的精确时间区间计算(timescale=fps)。
- SwiftUI [high] — GenerationView(1884行)整个生成面板、AIEditMenu 上下文菜单、各 ModelConfig 的 @Observable 目录、ModelPreferences、@AppStorage/@State/@FocusState/.popover/.onKeyPress/.onGeometryChange/LazyVGrid 等。
- AppKit [medium] — DropTargetOverlay 用 NSViewRepresentable+NSView 的 registerForDraggedTypes/draggingEntered/performDragOperation 实现原生拖放(绕开 SwiftUI 父级 onDrop 遮蔽);MediaAsset.thumbnail 为 NSImage;SettingsWindowController。
- Combine [low] — GenerationBackend.subscribe 返回 AnyPublisher<BackendGenerationJob?,ClientError>,GenerationService 把它桥接成 AsyncStream 消费;ModelCatalog 用 AnyCancellable 订阅 models:list。
- Foundation [low] — URLSession.download/upload(下载结果、上传引用字节)、FileManager(目录/临时文件/移动删除)、JSONDecoder/Encoder、UserDefaults(模型偏好)、Date/TimeInterval(缓存 TTL)、UUID。

**闭源云**:是 —— 强依赖 Convex 闭源云。GenerationBackend 通过 ConvexMobile 调用 generations:submit(提交任务)、generations:byId(订阅任务状态)、uploads:generateUploadTicket / commitUpload(引用上传三步握手),并用 URLSession 向 Convex 返回的暂存 URL POST 字节、从结果 URL 下载产物。ModelCatalog 订阅 Convex models:list 动态获取全部模型能力与计价。鉴权/额度经 AccountService.shared.convex(Clerk/Convex 账户:isSignedIn/budgetCredits/spentCredits/signInWithGoogle)。所有实际生成模型(视频/图/音/放大)都在该闭源云后端背后调度,客户端不直接接触模型厂商。Agent 的 MCP 工具(ToolExecutor+Generate)也经同一路径。

**移植策略**:整模块是『闭源云客户端 + AVFoundation 媒体预处理 + SwiftUI 面板』的混合体,需分层处理:

1) 云后端层(GenerationBackend / ModelCatalog / Convex 调用):必须 cloud-rebuild。OpenTake 不应保留 Convex。两条路:(a) 自建 Rust core 内的生成网关(reqwest/tokio),定义自有 REST:POST /generate→job_id、GET /generate/{id} 或 WebSocket 订阅状态、POST /uploads 三步或直传对象存储(S3/R2 预签名 URL),自己对接 fal.ai/Replicate/各厂商;(b) 若仍要复用某 SaaS,则封装其 SDK。BackendGenerationJob/Params 这套 DTO 与带 kind 判别字段的 JSON 可直接用 serde 复刻。Combine→改用 tokio 通道/Stream 驱动 runJob 状态机。

2) 编排层(GenerationService 主流程 / EditSubmitter / 四个 *Submission / CostEstimator / EditAction / TrimmedSource 语义 / 上传顺序与切分契约 / N图FirstOnly / 占位先放后改时间线 / 缓存TTL):direct-port,这是最有价值且与平台无关的『编辑逻辑』,应在 Rust core 忠实复刻。注意上传顺序(frames→image→video→audio 扁平化)与切分、ceil 计价、各 validate 边界、6天缓存(建议改为内容哈希去重)。回调 onComplete/onFailure/snapshotRefs/preprocessRef 用 trait 对象或 enum + async 闭包表达;@MainActor 约束在 Rust 里改为把时间线写回派发到拥有 timeline 的 actor/单线程。

3) 媒体预处理层(VideoTrimExtractor / VideoCompressor):needs-replacement,用 FFmpeg 复刻——trim:-ss start_sec -t dur_sec 并 -r targetFps(targetFps 规则:源帧率∈[24,60] 则 round 否则 30);compress:仅当长边>1100 时 scale 到长边960。注意 preferredTransform/旋转要靠读取 rotate/side_data 并相应处理。

4) UI 层(GenerationView / AIEditMenu / DropZoneView / ModelPreferences):ui-rebuild,用 React/TS 重写。模型菜单/参考拖拽槽/@提及补全/设置弹窗/成本显示/面板高度自适应/种子回填全部按上面记录的规则在前端实现;ModelPreferences 改 localStorage/前端持久化;拖放用 HTML5 DnD,不再需要 AppKit 绕路。模型目录由 Rust core 暴露给前端。

5) GenerationInput/MediaManifestEntry 的 Codable 持久化:direct-port 为 serde struct,保持字段名以兼容工程文件(decodeIfPresent→Option,version 兼容旧值)。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Generation/GenerationService.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Generation/GenerationBackend.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Generation/Submission/VideoGenerationSubmission.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Generation/Edit/EditSubmitter.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Generation/Catalog/ModelCatalog.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Generation/UI/GenerationView.swift

## Agent  ·  `mixed` → **needs-replacement**

**职责**:
- 对话编排：维护多会话(ChatSession)、消息历史、流式 SSE 解析、工具调用循环(runLoop)、孤儿 tool_use 修复、提示缓存边界设置
- LLM 客户端抽象(AgentClient)：AnthropicClient 直连 api.anthropic.com；PalmierClient 经 Clerk JWT 鉴权访问自有 Convex 后端 /v1/agent/stream(计费/积分路径)
- 工具定义(ToolDefinitions)：31 个工具的名称、自然语言描述、JSON Schema 输入约束(同时供 Anthropic tools 与 MCP)
- 工具执行(ToolExecutor + 13 个扩展文件)：参数解码/未知字段拒绝/有限性校验、ID 前缀展开、调用 EditorViewModel 完成编辑、结果 ID 缩短
- 帧/秒/源-时间线坐标换算：trim/speed/startFrame 之间的双向映射(剪辑、波纹删除、转写词映射、上采样裁剪)
- 时间线只读视图压缩：get_timeline/get_transcript/inspect_* 的默认值剥离、字幕组折叠、关键帧行化、窗口分页、UUID→最短唯一前缀压缩
- 媒体读取与理解：AVFoundation 抽帧、OverviewRenderer 故事板雪碧图、on-device 转写(Speech)、语义检索(search_media)
- 生成/上采样/导入：把 generate_* / upscale / import_media 转成后台异步提交，返回占位 assetId
- 本地 MCP HTTP server(MCPHTTPServer/MCPService)：仅绑 127.0.0.1:19789，Origin/ContentType/协议版本校验，暴露同一套 ToolExecutor
- SwiftUI 聊天面板 UI(Panel/*)：输入框、@提及弹窗、消息渲染、Markdown、思考动画
- 撤销治理：agentUndoStack 仅允许撤销助手自己本会话产生的编辑，拒绝撤销用户手动编辑
- API Key 安全存取：AnthropicKeychain 经 Keychain 读写，DEBUG 下可读环境变量

**核心类型**:
- `AgentService` (class) — @MainActor @Observable 对话主控。持有 sessions/messages/draft/mentions，选择后端(selectClient)，跑工具循环(runLoop)，做 SSE→消息块累积、孤儿 tool_use 合成、提示缓存、会话持久化、@提及上下文注入。
- `ToolExecutor` (class) — @MainActor 工具执行核心。execute() 统一做：ID前缀展开→分发到具体工具→对比 timeline 变化决定是否压入 agentUndoStack→缩短结果中的ID→遥测。被应用内 agent 与 MCP server 共用。
- `AgentClient/AnthropicClient/PalmierClient` (protocol) — 流式后端抽象。两实现都构造 Anthropic Messages API body(含 ephemeral 提示缓存)，用 URLSession.bytes 拿 SSE，交给 AnthropicSSE.parse。PalmierClient 额外走 Clerk 鉴权 + Convex 后端。
- `ToolDefinitions/AgentTool/ToolName` (enum) — 31 个工具的单一事实源：枚举名→rawValue 工具名、长描述、JSON Schema。MCP 与 Anthropic 共用；ToolArgsBridge 在 MCP Value 与 [String:Any] 间互转。
- `AnthropicSSE/AnthropicRequestBody` (enum) — 共享 SSE 解析器(解析 text_delta/input_json_delta/content_block_stop/message_delta/error，按 index 累积 tool_use 的分片 JSON)与请求体构造器(在 system+tools 末尾与会话最后一块打 cache_control ephemeral)。
- `ToolResult` (struct) — 工具返回值：[Block] 内容(text 或 base64 图像) + isError。可转成 MCP CallTool.Result，也可 Codable 持久化进消息历史。
- `Clip/Timeline/Track` (struct) — (引用自 Models/Timeline.swift)被工具读写的核心领域模型。Clip 含 startFrame/durationFrames/trimStart/trimEnd/speed/volume/opacity/transform/crop/linkGroupId/captionGroupId/textContent/textStyle 及 6 条关键帧轨。totalFrames=各轨 max(clip.endFrame)。
- `OverviewRenderer` (enum) — 把一段视频抽成单张故事板雪碧图：密集抽候选帧→用 LumaGrid 亮度网格丢弃近重复帧(meanDiff>12 才保留)→6 列网格、最多 36 块、CoreText 烧入时间码。
- `AgentMentionContext/AgentMention/AgentTimelineRangeMention` (struct) — @提及上下文：把被引用的媒体资产/时间线 clip/选中时间范围序列化成 JSON hint 注入用户消息；图像提及内联为 base64 image block。范围为半开区间(start 含 end 不含)。
- `MCPHTTPServer/MCPService` (actor) — 本地 MCP HTTP 服务：NWListener 仅绑 127.0.0.1:19789，每 TCP 连接一对 Server+StatelessHTTPServerTransport；手写 HTTP 解析，处理 /mcp、/.well-known/oauth-protected-resource、SSE GET。
- `RippleEngine/OverwriteEngine` (enum) — (引用自 Editor/)纯函数引擎。RippleEngine 算波纹位移(合并范围、按 end<=clip.start 累加左移量、push 右移)；OverwriteEngine 算覆盖落点的 remove/trimEnd/trimStart/split 动作。

**核心算法/逻辑(供 Rust 复刻)**:
- 【帧/秒基本换算】timeline 有固定 fps；frame = round(seconds × fps)。所有工具时间单位默认是项目帧(timeline fps)，不是源媒体 fps。clip.endFrame = startFrame + durationFrames(半开区间 [start,end))。timeline.totalFrames = 各 track 的 max(clip.endFrame)，空则 0。Swift Int(Double.rounded()) 默认 round-half-away-from-zero，Rust 复刻需用 (x).round()。
- 【trim/speed 与源-时间线映射(最关键)】clip.sourceFramesConsumed = round(durationFrames × speed)。源帧→时间线帧：timelineFrame = round(startFrame + (sourceFrame − trimStartFrame)/max(speed,0.0001))；要求 sourceFrame≥trimStartFrame 且结果落在 [startFrame,endFrame) 否则视为不可见。get_transcript 的项目帧 P → 该 clip 源 trim 偏移：trimStartFrame + (P − startFrame)×speed。
- 【spanFrames(转写词→项目帧)】先把源秒区间钳到 clip 可见窗口：visStart=trimStartFrame(源帧)，visEnd=visStart + durationFrames×max(speed,0.0001)；s=max(start×fps, visStart)，e=min(end×fps, visEnd)，若 e<=s 丢弃；再 toTimeline(x)=round(startFrame + (x−visStart)/max(speed,0.0001))，返回 (a, max(a,toTimeline(e)))，保证 end>=start。边界跨越的词只产出真实碎片，零长词(start==end 四舍五入成 0 帧)会被丢弃——系统提示明确要求按词级而非段级去重。
- 【get_transcript 词归属】对每个音/视频 clip 取其源转写词，按词中点判定归属：midFrame=(s+e)/2×fps，要求 visStart<=midFrame<visEnd，故跨 clip 缝的词只发射一次；再经 spanFrames 映射；窗口过滤 f.end<=startFrame 或 f.start>=endFrame；按 (start,end) 排序；全局 10000 词上限，超出给 nextStartFrame 分页。每源仅转写一次(缓存)，单资产失败跳过不致命。
- 【add_clips 覆盖式落点】对每个 entry：校验 trackIndex 在范围、资产类型与轨道类型兼容(video/image 可互换；audio 需 audio 轨)、durationFrames>=1、startFrame>=0、trim>=0。trackIndex 全省略=自动建轨(视觉类共享一条 video 轨、音频类共享一条 audio 轨，都插在 index 0)；混用(部分给部分省略)直接拒绝。放置前对落点区间 clearRegion(覆盖式裁/分/删已有 clip)，再 placeClip。批内按(音频优先, trackId, startFrame)排序放置。带音轨的视频放到 video 轨会自动在 audio 轨创建 linkGroupId 关联的镜像音频 clip。整批一个 undo。
- 【clearRegion/OverwriteEngine 覆盖决策】对落点区间 [regionStart,regionEnd) 内每个相交 clip：完全被包含→remove；clip 跨整个区间(cs<start 且 ce>end)→split(左半 duration=regionStart−cs；右半 startFrame=regionEnd, rightTrimStart=trimStart+round((regionEnd−cs)×speed), rightDuration=ce−regionEnd)；仅左缘相交(cs<start)→trimEnd(newDuration=regionStart−cs)；仅右缘相交→trimStart(newStartFrame=regionEnd, newTrimStart=trimStart+round((regionEnd−cs)×speed), newDuration=ce−regionEnd)。
- 【insert_clips 波纹插入】trackIndex 必填。entries 从 atFrame 起首尾相接铺放；总推移量=各 entry duration 之和。推移施加到目标轨 + 所有 syncLocked 轨 + 自动镜像音频落点轨。插入前对每条被推轨上跨 atFrame 的 clip 做 split(使其右半随波纹走而非被覆盖)；splitClip 也会切并重组其链接伙伴。RippleEngine.computeRipplePush：startFrame>=insertFrame 的 clip 一律 +pushAmount。
- 【ripple_delete_ranges(波纹删除，两模式)】恰好二选一传 clipId 或 trackIndex。clipId 模式：ranges 钳到该 clip 可见区间，units 可为 seconds(源秒，经 toFrame=startFrame+(v×fps−trimStart)/max(speed,0.0001) 映射)或 frames(项目帧)。trackIndex 模式：ranges 必须是 frames(项目帧)，可跨该轨任意多 clip。每个 range 要求 end>start，二元组。合并重叠范围(mergeRanges：排序后 range.start<=last.end 即并)。被触及 clip 的链接 A/V 伙伴在同范围被切以保持同步。删除后剩余 clip 左移闭合空隙；syncLocked 轨随之左移以保对齐(其内容不被切)。若某 syncLocked 轨吸收位移会越过帧 0 或产生碰撞→整体 refused 不改任何东西。返回 anchor 轨删后布局(clip ids/frames)免重读。
- 【RippleEngine.computeRippleShiftsForRanges 左移量】对每个剩余 clip：左移量 = 所有满足 range.end<=clip.startFrame 的已删范围长度之和；>0 才产生 ClipShift(newStartFrame=startFrame−shift)。validateShifts 干跑：任一 clip 移后 start<0 报“移过时间线起点”；排序后相邻区间 start<前一 end 报“无空间波纹”。
- 【split_clip 与关键帧切分】atFrame 必须严格在 (startFrame,endFrame) 内。splitOffset=atFrame−startFrame；leftSource=round(splitOffset×speed)，rightSource=round((duration−splitOffset)×speed)。左 clip：duration=splitOffset, trimEnd+=rightSource, fadeOut=0；右 clip：新id, startFrame=atFrame, duration=duration−splitOffset, trimStart+=leftSource, fadeIn=0。每条关键帧轨在切点采样出边界关键帧并各自重基(左保留<=splitOffset 并补边界点；右过滤>=splitOffset 后整体减 splitOffset 并补 0 帧边界点)以保曲线连续。有链接组则同时切所有伙伴并把右半重组为新链接组。
- 【move_clips】每个 move 至少给 toTrack 或 toFrame 之一；toTrack 必须与 clip 媒体类型兼容。链接伙伴跟随：startFrame 以增量传播(toFrame−当前 startFrame，伙伴新帧=max(0, 伙伴 start+delta))以保 l-cut/j-cut 偏移；轨道变化不传播。实现先把被移 clip 从源轨摘除→对各目标区间 clearRegion(覆盖式)→按精确目标帧落下→各轨排序→剪空轨。
- 【set_clip_properties】对 clipIds 施加同一组值(duration/trim/speed/volume/opacity/transform/文本字段)。文本专用字段(content/fontName/fontSize/color/alignment)若 clipIds 含非文本 clip→拒绝。speed 改动：若未同时给 durationFrames 且 speed>0，按 sourceConsumed=duration×旧speed 重算 duration=max(1, round(sourceConsumed/新speed))。设 volume/opacity 标量会清空该属性已有关键帧轨。改 duration/speed 都会 clampKeyframesToDuration + clampFadesToDuration。文本改 content/font 且未显式给 transform 时自动 refit 包围盒(fitTextClipToContent)。timing 类(duration/trim/speed)传播到链接伙伴(伙伴若是文本则跳过 trim/speed)。
- 【set_keyframes 行解析与插值】property∈{volume,opacity,rotation,position,scale,crop}。行格式 [frame, ...values, interp?]，interp∈{linear,hold,smooth}(默认 smooth)。value 数量按属性：scalar=1(volume/opacity/rotation)、pair=2(position 是左上角 x,y 归一化；scale 是归一化宽高，非缩放因子)、crop=4(top,right,bottom,left 归一化边距)。帧是 clip 相对(0=clip 首帧)。内部按 frame 排序并去重(同帧后者覆盖)。空数组清空轨。采样算法 KeyframeTrack.sample：空→fallback；单点→该值；frame<=首/>=尾→边界值；否则取首个 frame>查询的 b，a=b前一，raw=(frame−a.frame)/(b.frame−a.frame)，按 a.interpolationOut：hold→a.value；linear→lerp(a,b,raw)；smooth→lerp(a,b,smoothstep(raw))，smoothstep(t)=t²(3−2t)。运动关键帧(position/scale/rotation)激活时覆盖静态 transform。
- 【fade 包络(与关键帧叠加)】fadeMultiplier(rel)：rel=frame−startFrame，越界(rel<0 或 rel>duration)返回 0；inMul=fadeIn>0 时 t=min(1,rel/fadeIn)，smooth 则 smoothstep(t)；outMul=fadeOut>0 时 t=min(1,(duration−rel)/fadeOut)，取 min(inMul,outMul)。clampFadesToDuration：fadeIn=clamp(0..duration)，fadeOut=clamp(0..duration−fadeIn)。有效音量=volume × dB转线性(关键帧采样,VolumeScale.linearFromDb) × fadeMultiplier；音频 clip 不应用 fade 到 opacity。
- 【ID 前缀压缩(双向)】实体 UUID 在输出文本里替换为最短(>=8 字符)且全集内唯一的前缀(shortIdMap：从 8 起逐字符加长直到无他者共享该前缀)。输入侧 expandingIdPrefixes：扫描已知 scalar/array ID 键，前缀唯一则展开成全 UUID，多义则抛 Ambiguous 错误，未知则原样透传让具体工具报 not-found。ID 全集 = 所有 track.id/clip.id/captionGroupId/linkGroupId/asset.id/folder.id。
- 【get_timeline 负载压缩】默认值剥离：mediaType=video、sourceClipType=mediaType、speed=1、volume/opacity=1、trim/fade=0、单位 transform/crop、默认 textStyle、track muted/hidden=false、syncLocked=true。文本 clip 不报 trim。关键帧轨折叠成 keyframes 行(frame,值...,非 smooth 才附 interp)。同 captionGroupId 的 clip 折叠成 captionGroups：众数残差属性提到 shared，每行 [clipId,startFrame,durationFrames,text]，字幕框宽高(自动 fit)剔除，每组上限 200 行超出分页，偏离众数的字幕 clip 单独列出。窗口 [startFrame,endFrame) 只返回相交 clip，浮点数四舍五入到 3 位。
- 【undo 治理】每次工具执行后若 timeline 真的变了且非 undo 工具且无错，记录 undoManager.undoActionName 入 agentUndoStack。undo 工具只在栈顶动作名等于当前 undoManager.undoActionName 时执行 undoManager.undo() 并弹栈；否则拒绝(“最近改动不是助手做的”)，保护用户手动编辑。底层撤销用 withTimelineSwap：禁登记跑改动→对比 before/after timeline→登记一个双向 timeline 整体快照 swap(registerTimelineSwap 自递归注册逆向)。
- 【add_captions 流程】on-device 转写(Speech)候选 clip→若 autoDetect 选说话词数最多的轨(dominantSpeechTrack)→CaptionBuilder.phrases 按行宽是否 fit(captionLineFits：自然尺寸宽<=画布宽×比率)与最小时长断句→短语按与 clip 可见区间重叠最大且重叠>=短语一半归属(bestClip)→应用大小写(auto/upper/lower)→CaptionBuilder.specs 生成共享 captionGroupId 的文本 clip→在 index 0 插新 video 轨并 placeTextClips，整体一个 undo。语言经 Transcription.matchLocale 校验支持。
- 【import_media 安全约束】source 恰好二选一 url/path/bytes。url 必须 https、无内嵌凭据、有 host；类型由扩展名推断或 mimeType 覆盖；后台下载(超 1GB 取消)。bytes 需 mimeType，base64 上限约 15MB，写入项目 media 目录。path 可为目录(递归镜像子文件夹为媒体文件夹)。支持类型：video(mov/mp4/m4v)、audio(mp3/wav/aac/m4a)、image(png/jpg/jpeg/tiff/heic)、json(Lottie)，其余拒绝。
- 【generate_*/upscale 异步提交】先检查 AccountService.isSignedIn && hasCredits，否则报错引导登录/充值。经 list_models 校验 model 支持的 duration/aspectRatio/resolution/references/voice/类型；构造 GenerationInput→对应 *GenerationSubmission.make().submit() 到 editor.generationService(走 Convex 后端)，立即返回占位 assetId，后台完成后在 get_media 可见。video-to-audio 给时间线 span 时会自动把结果放到该 span。

**苹果框架使用**:
- AVFoundation [high] — AVURLAsset/AVAssetImageGenerator 在 inspect_media 抽视频帧、OverviewRenderer 抽候选帧；CompositionBuilder 产出 AVComposition+AVVideoComposition 供 inspect_timeline 渲染合成帧；读 video track、loadTracks、appliesPreferredTrackTransform、时间容差控制
- CoreMedia [low] — CMTime(seconds:preferredTimescale:600)/CMTimeScale(fps) 做抽帧时间点与时基换算，requestedTimeTolerance 控制抽帧精度
- CoreGraphics [medium] — CGContext 合成 Lottie 透明帧到灰底、OverviewRenderer 拼网格雪碧图、inspect_timeline 合成视频+文本层；CGImage 像素操作与 sRGB 色彩空间
- ImageIO [low] — CGImageSourceCreateWithURL + CopyPropertiesAtIndex 读图片像素尺寸/EXIF/方向/色彩模型(inspect_media 图像分支)
- CoreText [low] — OverviewRenderer 用 CTFontCreateWithName/CTLineCreateWithAttributedString/CTLineDraw 把时间码烧入雪碧图块
- Speech [high] — on-device 语音转写(经 Transcription/TranscriptCache)，支撑 inspect_media/get_transcript/add_captions/search_media spoken；matchLocale/supportedLocales 校验语言
- Network [low] — MCPHTTPServer 用 NWListener 仅绑 127.0.0.1:19789、NWConnection 收发；NWParameters.requiredLocalEndpoint 锁回环防 LAN 访问
- AppKit [medium] — 波纹拒绝时 NSSound.beep()；UndoManager 全套(beginUndoGrouping/registerUndo/undoActionName/disableUndoRegistration) 是撤销治理的核心；NSAttributedString
- SwiftUI [high] — 整个 Panel/* 聊天 UI(AgentPanelView/AgentInputBox/AgentMessageView/ChatHistoryList/MarkdownText/MentionPopover/ThinkingDots)，需 React/TS 重建
- Observation [low] — @Observable 让 AgentService/MCPService 状态驱动 SwiftUI 刷新
- Foundation [low] — URLSession.bytes 拿 SSE 流、JSONSerialization/JSONEncoder 编解码、URLSession.download 导入下载、Regex(/UUID/)、FileManager、UserDefaults、NotificationCenter

**闭源云**:是。三处触达闭源云：(1) AnthropicClient 直连 https://api.anthropic.com/v1/messages(用户自带 key 时，流式生成式 AI)；(2) PalmierClient 经 ClerkKit 取 JWT 后请求自有 Convex 后端 BackendConfig.convexHttpURL + /v1/agent/stream(无 key 的付费/积分代理路径，本质仍是 Anthropic 模型)；(3) 所有 generate_video/image/audio 与 upscale_media 经 editor.generationService/GenerationBackend 提交到 Convex 后端调度第三方生成式模型(Seedance/Kling/Veo/Nano Banana/ElevenLabs/Lyria 等)，AccountService(Clerk 登录+积分)门控。import_media 的 url 模式还会发起任意 https 下载。on-device 转写(Speech)与语义检索不触云。

**移植策略**:分层处理：①工具执行/校验/帧秒换算/ID 压缩/负载压缩(ToolExecutor 全家 + AgentMentionContext + ShortId + Timeline/Keyframe/RippleEngine/OverwriteEngine 算法)——可 direct-port 到 Rust core，是 OpenTake 领域模型与 MCP server 的核心，coreLogic 中的公式按整数帧 + round(half-away-from-zero) 一比一复刻即可。②LLM 客户端(AgentClient/SSE/RequestBody)——Rust 用 reqwest + eventsource 流重写，提示缓存 cache_control 边界逻辑照搬；自带 Anthropic key 直连可保留，Convex/Clerk 付费代理属 closed-cloud，需换成 OpenTake 自己的鉴权/计费或去掉只留 BYO-key。③MCP server(MCPHTTPServer/MCPService/ToolResult/ToolDefinitions)——改用 Rust MCP SDK(rmcp) + 仅绑 127.0.0.1，工具 schema(JSON)可几乎照抄，这是 OpenTake 暴露给外部 agent 的关键能力。④媒体读取(inspect_media 抽帧/overview/inspect_timeline 合成、OverviewRenderer 亮度去重 LumaGrid/雪碧图、Lottie)——AVFoundation→FFmpeg(ffmpeg -ss 抽帧、scale、合成滤镜) + image crate 拼图；CoreText 烧字→ab_glyph/cosmic-text。⑤Speech 转写与语义/视觉检索——Apple Speech 无 Rust 等价，需换 whisper.cpp(转写) + CLIP/embedding(视觉)+ 文本检索；这是工作量大头。⑥UndoManager 撤销治理——Rust 自建命令栈/timeline 快照 swap(本模块已是整 timeline diff swap 模式，易移植)，agentUndoStack 守卫逻辑照搬。⑦SwiftUI Panel/*——React/TS 全量重建(ui-rebuild)。⑧生成/上采样/导入——cloud-rebuild：重接 OpenTake 自己的生成后端或第三方 API。⑨AppKit NSSound.beep 等纯反馈可丢弃或换前端提示。

**关键文件**:Agent/AgentService.swift、Agent/Tools/ToolExecutor.swift、Agent/Tools/ToolExecutor+Clips.swift、Agent/Tools/ToolExecutor+Timeline.swift、Agent/Tools/ToolDefinitions.swift、Agent/Tools/AgentInstructions.swift

## MediaPanel  ·  `mixed` → **ui-rebuild**

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

**移植策略**:整体定位为 ui-rebuild：UI 用 React/TS 重写，但内嵌的纯算法应 direct-port 到 Rust，闭源云需 cloud-rebuild。分项策略——(A) direct-port 到 Rust core：CaptionBuilder 全套(split/breakOn 标点规则/breakAtMidWord/distribute 按字符等比分配/enforceMinDuration 单向级联防重叠)、Clip.timelineFrame 源秒↔帧映射、CaptionBuilder.specs 裁剪逻辑、网格列数/瓦片宽公式、文件夹树 MediaFolderIndex(path/isDescendant/级联删除/环路防护)、拖拽 URI 编解码(palmier-asset://、#%.3f-%.3f)、键盘方向导航、sortAndFilter/passesFilters、撤销快照模型(在 Rust 用命令/快照栈替代 NSUndoManager)、TranscriptSearch 关键词匹配、VisualSearch 打分(cblas_sgemv 换 ndarray/nalgebra 或 BLAS crate，逻辑一致)。注意 secondsToFrame=Int(seconds*fps) 是截断不是四舍五入，timelineFrame 内部才 round——必须逐处对齐取整方式。(B) needs-replacement：所有 AVFoundation/CoreMedia 媒体操作(缩略图抽帧、指定时间取帧、音频抽轨为 16k 单声道 PCM、整帧导出)改用 FFmpeg(ffmpeg/ffprobe 或 ffmpeg-next/rsmpeg)；文本测量 NSAttributedString.boundingRect 改用前端 Canvas measureText 或 Rust 端 cosmic-text/rusttype/harfbuzz 复刻换行与尺寸(字幕落点依赖测量结果，需保证与渲染一致)；图片元数据/缩略图用 image crate。(C) blocker→cloud-or-on-device-replacement：Speech 框架本地转写无法直接移植，换 whisper.cpp/whisper-rs(本地)或转写服务，须自行产出 segment(段级时间)+word(词级时间戳)以驱动 CaptionBuilder 与 Spoken 搜索；视觉搜索 SigLIP+CoreML 换 ONNX Runtime/candle 跑同类图文对比模型，向量检索逻辑可保留。(D) cloud-rebuild：Music 生成与所有 Agent/Generate 入口依赖 Convex+Clerk 闭源后端，需替换为 OpenTake 自有后端/MCP server(上传参考、提交生成、积分与鉴权)。(E) UI 平台适配：NSOpenPanel→Tauri dialog、NSPasteboard→Tauri clipboard、Finder 显示→Tauri opener、原生拖放遮蔽问题在 Web 端不存在(用标准 HTML5 DnD/文件拖放即可)。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MediaTab/MediaTab.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MediaTab/MediaTab+Grids.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MediaTab/MediaTab+Drag.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MediaTab/MediaTab+Search.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/CaptionsTab/CaptionBuilder.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/MediaPanel/MusicTab.swift

## Inspector  ·  `ui` → **ui-rebuild**

**职责**:
- 根据选区类型(可视clip/音频clip/纯文字clip/媒体资产/空)解析可用标签页与当前激活标签页(availableTabs/activeTab/resolvePreferredTab)
- 渲染并驱动变换区：位置(X/Y, 归一化topLeft)、缩放(%)、旋转(°)、不透明度(%)、裁剪(开关+宽高比预设菜单+画布编辑)、水平/垂直翻转、Reset Transform
- 渲染并驱动播放速度(0.25x–4x)、音量(dB, -∞..+15)、淡入/淡出(秒)
- 纯文字 clip 的文字检查器：内容多行输入、字体选择(NSMenu)、字号、颜色/背景/边框/阴影(NSColorPanel)、对齐、不透明度、位置
- 每个可动画属性行附带关键帧控制：上一/下一关键帧跳转、打点/删点(diamond)、范围内禁用判断
- 右侧关键帧小面板(KeyframesPanel)：ruler+clip条+每属性lane，菱形关键帧绘制、拖动移动(带吸附)、右键改插值/删点、红色playhead与黄色吸附虚线
- 媒体资产详情：文件信息(类型/尺寸/时长/大小/路径)、AI 生成元数据(模型/宽高比/分辨率/时长/prompt)、生成引用缩略图条
- AI Edit 标签：放大/编辑/重跑/图生视频/视频配乐/音效入口，作用域开关(替换clip源、仅用裁剪段、放到时间线)，成本估算展示
- ScrubbableNumberField：可拖拽数字输入(拖动改值/点击键入)，shift x10、command x0.1 精度修饰
- 只读展示工程元数据(分辨率/帧率/宽高比/时长)与多选数量汇总

**核心类型**:
- `InspectorView` (struct) — 检查器根视图。负责选区→标签页路由、变换/速度/音量/淡变各区的组装,以及 apply(实时)/commit(落undo)两段式属性写入的调度。内含 VolumeScale(dB↔线性换算)与 PromptCopyButton。
- `KeyframesPanel / KeyframesLaneRow / ClipRulerBlock` (struct) — 关键帧小面板族。KeyframesMetrics 提供 frame↔x 像素映射;LaneRow 用 Canvas 画菱形、处理拖动移动关键帧并调用 SnapEngine 吸附;RulerView 用 NSViewRepresentable 复用主时间线 TimelineRuler 绘制。
- `AIEditTab` (struct) — AI 编辑标签视图。计算各 EditAction 的可用性与禁用原因,管理作用域开关状态,通过 EditSubmitter / editor.seedGenerationPanel 把生成请求送往(闭源)生成服务,并在完成回调里替换 clip 源。
- `TextTab` (struct) — 纯文字 clip 的样式检查器,所有改动经 editor.applyTextStyle/commitTextStyle/debouncedCommitTextStyle,并在内容/字体/字号变化后调用 fitTextClipToContent 自适应包围盒。
- `ScrubbableNumberField` (struct) — 核心交互控件:可横向拖拽的数字字段。含 displayMultiplier/format/suffix/dragSensitivity,内部用 AppKit ScrubMouseArea 做鼠标跟踪(>3px 判定拖动),修饰键调精度,mixed(多选不一致)显示‘—’。
- `ColorField / ColorPanelBridge` (struct) — 色板控件+NSColorPanel 单例桥。用 colorDidChangeNotification 在拖动中实时回调(SwiftUI ColorPicker 仅 mouseUp 才回调),设初值时抑制一次回调避免回环。
- `FontPickerField` (struct) — 字体选择:用原生 NSMenu 列出 Featured(BundledFonts)+全部系统字族,高亮时实时预览(onPreview),关闭未选则 onCancel 回滚。
- `GenerationReferencesStrip` (struct) — AI 生成引用缩略图条。按模型类型把 GenerationInput 里的各类引用 assetId 解析为带语义标签(Source/First Frame/Reference/Image Ref...)的缩略图。
- `InspectorSection / InspectorRow / InspectorPositionFields / TextContentField` (struct) — 复用小组件:分区标题、图标+标签+尾随控件行、XY 位置双字段(displayMultiplier=画布宽高把归一化坐标显示为像素)、NSTextView 包装的多行文本框(规避 SwiftUI TextEditor 丢键问题)。

**核心算法/逻辑(供 Rust 复刻)**:
- 【单位约定】时间一律整数帧;秒=帧/fps。Clip 关键帧在存储中是 clip 相对偏移(timelineFrame - startFrame),对外 API 用绝对帧。endFrame = startFrame + durationFrames;contains(frame) 判定为 frame>=startFrame && frame<endFrame(右开)。
- 【两段式编辑+撤销】所有连续控件遵循 apply(拖动中,只改 timeline 不落 undo,首帧用 dragBefore[clipId] 存快照) 然后 commit(松手,注册一次双向 undo/redo swap,setActionName)。多 clip 编辑用 beginUndoGrouping/endUndoGrouping 合成一条。ScrubbableNumberField 的 onChanged→apply、onCommit→commit;ColorPicker 类无松手事件,改用 debouncedCommit(先 apply,400ms 静默后 commit)。Rust 复刻:命令模式,每次提交生成一个可逆 Command(before/after 全 clip 快照或字段差),压入 undo 栈;拖动期只改状态不入栈。
- 【缩放写入】scaleScrubField 的 value 取 sizeAt(frame).width,range 0.01..∞,显示 x100 取整成%。写入(writeScale):aspect = mediaCanvasAspect(for:clip) ?? 1.0;w=newScale, h=newScale/aspect(即用户拖的是宽度比例,高度按‘源像素宽高比/画布宽高比’联动保持源不变形)。mediaCanvasAspect = (源宽/源高)/(画布宽/画布高);源尺寸未知则 nil→aspect=1。若 scaleTrack 激活则写关键帧,否则直接写 transform.width/height。
- 【位置写入】InspectorPositionFields 的 X/Y = topLeftAt(frame)(归一化画布坐标),displayMultiplier=画布宽/高把它显示为像素整数,range -10..10。writePosition:newX/newY 缺省取当前 topLeft;sz=sizeAt(frame);若 positionTrack 激活则 upsert positionTrack=AnimPair(newX,newY);并始终把 transform.centerX=newX+sz.w/2, centerY=newY+sz.h/2(中心=左上+半尺寸)。
- 【旋转写入】value=rotationAt(frame),range -3600..3600,单位度,正=顺时针。激活 rotationTrack 则写关键帧,否则 transform.rotation=值。
- 【不透明度写入】value=rawOpacityAt(frame)(不含淡变包络),range 0..1 显示 x100%。激活 opacityTrack 写关键帧,否则 clip.opacity。注意渲染用 opacityAt=rawOpacity*fadeMultiplier(仅非音频且有淡变时)。
- 【音量/分贝换算(VolumeScale)】floorDb=-60, ceilingDb=+15。dbFromLinear(l)= l<=0?-60: clamp(20*log10(l), -60, 15);linearFromDb(db)= db<=-60?0: pow(10, min(db,15)/20)。UI 显示:db<=-60 渲染‘-∞ dB’。value 取 liveVolumeKfDb(at:frame)(有激活 volumeTrack 且 playhead 在范围内时为该帧采样 dB)否则 dbFromLinear(clip.volume)。写入(writeVolume):若该帧有激活 volume 关键帧则把 dB 直接 upsert 进 volumeTrack(关键帧值存的是 dB),否则 clip.volume = linearFromDb(db)。最终 volumeAt = volume * linearFromDb(kf采样dB) * fadeMultiplier。
- 【淡入淡出】fadeRow 显示秒(帧/fps),range 0..(单选时 duration/fps,否则60),写入时 frames=round(seconds*fps)。setFade 后 clampFadesToDuration:fadeIn=clamp(0..duration);fadeOut=clamp(0..(duration-fadeIn))(头+尾不得超过时长,头优先)。fadeMultiplier(rel=frame-start, 需 0<=rel<=duration 否则0):inMul = fadeIn>0 ? f(min(1, rel/fadeIn)) : 1;outRem=duration-rel;outMul = fadeOut>0 ? f(min(1, outRem/fadeOut)) :1;f = smooth?smoothstep:linear;返回 min(inMul,outMul)。淡变插值默认 linear。
- 【关键帧数据结构】每属性一个 KeyframeTrack<V>?(nil=无动画)。Keyframe{frame(相对), value, interpolationOut(默认.smooth)}。属性→值类型:opacity/rotation/volume=Double, position/scale=AnimPair(a,b), crop=Crop(left,top,right,bottom)。upsert:同帧覆盖,否则按 frame 升序插入(找第一个 frame>新帧的位置)。remove 删同帧;空轨道置 nil。move(from,to):目标帧已存在(且!=源)则放弃;否则取出改 frame 再 upsert。
- 【关键帧采样 sample(at frame, fallback)】空→fallback;1个→该值;frame<=首帧→首值;frame>=末帧→末值;否则取第一个 frame>查询帧的 b,a=b前一个,raw=(frame-a.frame)/(b.frame-a.frame),按 a.interpolationOut:hold→a.value;linear→lerp(a,b,raw);smooth→lerp(a,b,smoothstep(raw))。smoothstep(t)=t*t*(3-2*t)。lerp 对 Double 是 a+(b-a)t,对 AnimPair/Crop 是逐分量 lerp。
- 【打点 stampKeyframe】仅当 playhead 在 clip 范围内。各属性把‘当前采样值’固化为关键帧:opacity=rawOpacityAt;position=topLeftAt→AnimPair;scale=sizeAt→AnimPair;rotation=rotationAt;crop=cropAt;volume=当前 volumeTrack 采样 dB(无则0)。删点 removeKeyframe;清空 clearKeyframes 整轨置 nil。setInterpolation 改某帧 interpolationOut(linear/smooth/hold)。导航:previous=所有关键帧绝对帧中<当前帧的最大值,next=>当前帧的最小值。
- 【关键帧拖动移动(KeyframesLaneRow)】命中容差 hitTolerance=7px;按下找最近关键帧(<=7px 且最近)进入拖动,否则空白处=seek。拖动中:pxPerFrame=max(1e-4, width/span);raw=frameAt(x);经 SnapEngine 吸附后 clamp 到 [clipStart,clipEnd];若变化则 applyMoveKeyframe(from current→snapped),并核对该帧是否仍在(被占则不推进 currentFrame)。松手:若 current!=original 则 commitMoveKeyframe(一条 undo),否则 revertClipProperty(清快照)。span=max(1,endFrame-startFrame)。
- 【lane 吸附目标 & SnapEngine】lane 吸附阈值 baseThreshold=4px;目标=范围内 playhead(kind .playhead)+clip 两端(.clipEdge)+本 clip 其它属性的所有关键帧(.clipEdge)。SnapEngine.findSnap:baseFrameThreshold=baseThreshold/pxPerFrame;sticky:已吸附且 |probe-snapped|<=baseFrameThreshold*1.5(stickyMultiplier) 且该目标仍存在,则保持;否则解除。否则遍历 probe×target 取最近(playhead 阈值再 x1.5=playheadMultiplier;clipEdge 用基础阈值),命中触发对齐触感反馈并记 sticky 状态。命中且 clamp 后未变才显示黄色虚线吸附指示(snapX)。注意主时间线 Snap.thresholdPixels=8,lane 内部用 4。
- 【裁剪 Crop】Crop 是源归一化(0–1)四边内缩 {left,top,right,bottom};isIdentity=全0;可见宽=max(0,1-left-right)。宽高比预设(CropAspectLock):free=不改(用户自由拖),original=Crop()清零,具体比例→cropFittingAspect:source=源宽/源高,|source-target|<1e-4 则 Crop();source>target(源更宽)→水平内缩 inset=(1-target/source)/2 左右各 inset;否则垂直内缩 inset=(1-source/target)/2 上下各 inset(即在源内取最大居中目标比例区域)。pixelAspect 表:16:9=16/9,9:16,1:1,4:3,3:4,21:9。裁剪有关键帧则写 cropTrack 否则 clip.crop。
- 【Reset Transform】把 transform 置默认 Transform()(center 0.5,0.5 / w=h=1 / rot=0 / 不翻转)、opacity=1、清空 opacity/position/scale/rotation 四个 track、fadeIn/Out=0、淡变插值=linear(注意不清 cropTrack/volumeTrack)。
- 【翻转】flipHorizontal/flipVertical 为 transform 上的布尔,取选区首个 clip 的值做显示,toggle 后对全选区 commit(各 clip 独立提交,合成一条 undo)。翻转不进关键帧。
- 【文字自适应 fitTextClipToContent】用 TextLayout.naturalSize(content, style, maxWidth=画布宽*0.9, canvasHeight) 得自然像素尺寸→归一化 needW=natural.w/画布宽, needH/画布高;与当前 transform.w/h 差<1e-4 则跳过。保持垂直中心 cy=topLeft.y+curH/2;水平锚点按对齐:left→cx=tl.x+needW/2,right→cx=(tl.x+curW)-needW/2,center→cx=tl.x+curW/2;写 transform=Transform(center:(cx,cy),w:needW,h:needH)。内容/字号/字体改动后都会调用。
- 【Transform 表示与遗留迁移】Transform 以 centerX/centerY + width/height(均画布归一化)+rotation(度,顺时针)+flip 表示;topLeft=center-尺寸/2。解码兼容旧 x/y 键:centerX=oldX+w-0.5(同理 y)。提供 snapToCanvasEdges(阈值内把边吸到0/1)与 snapCenterToCanvasCenter(中心吸到0.5)供画布拖拽用(Inspector 不直接用,但属同模型)。
- 【拆分时关键帧处理(被 Inspector 行为间接依赖)】splitClip 在 atFrame 处:左 trimEnd+=右侧源帧、fadeOut=0;右 trimStart+=左侧源帧、fadeIn=0;源帧换算=round(offset*speed)。每条 track 用 splitKeyframeTrack:在切点采样插入边界关键帧保证两侧曲线连续,右侧帧整体减 splitOffset 重基。clamp/rescale 关键帧:改时长后丢弃 frame<0 或 >duration 的帧;变速 rescaleKeyframes 按 scale 缩放帧号(round)。
- 【ScrubbableNumberField 数值解析】拖动:next=clamp(dragStartValue + dx*sens/mult, range),sens 受 shift(*10)/command(*0.1) 调节,mult=displayMultiplier(0当1)。键入提交:去尾随单位后缀、去空白、逗号→点,Double 解析失败放弃;raw=clamp(parsed/mult, range)。mixed(多选值不一致,value==nil)显示‘—’且禁拖。sharedClipValue:多 clip 取值,全相等返回该值否则 nil(驱动 mixed)。
- 【AI 标签可用性 EditAction.availability(纯本地判断,不联网)】upscale:仅 video/image;video 需 sourceHeight>0 且 <2160(否则‘已4K’);已是放大结果/正在生成→禁用。edit:video 需有效时长且<=10s(editMaxDurationSeconds),image 无时长限制,audio/text/lottie 不支持。createVideo:仅 image。rerun:需 isGenerated 且模型仍存在于 ModelRegistry。generateMusic/SFX:仅 video,经对应模型 validate(spanSeconds)。effectiveDuration 优先 AVAsset 时长,回退到记录的生成时长。AIEditTab 用 trimmedSource.durationSeconds 覆盖时长做可用性判断。
- 【裁剪段时长(TrimmedSource)】durationSeconds = sourceFramesConsumed/max(1,fps);sourceFramesConsumed = round(durationFrames*speed)。‘仅用裁剪段’开关开时,可用性与放大成本均用此覆盖。视频配乐放到时间线的 PendingAudioPlacement.spanSeconds = 裁剪段时长 ?? (asset.duration>0?asset.duration: durationFrames/fps),且不小于 1/fps。

**苹果框架使用**:
- SwiftUI [high] — 整个检查器的视图层:VStack/HStack/ScrollView/Canvas/Menu/Picker/Toggle/Button、@Environment/@State/@Bindable、GeometryReader、DragGesture、contextMenu、alert 等。
- AppKit [medium] — ScrubbableNumberField 的 NSView 鼠标跟踪(mouseDown/Dragged/Up + resizeLeftRight 光标);ColorField 的 NSColorPanel + colorDidChangeNotification 桥;FontPickerField 的 NSMenu/NSMenuItem 弹出与字体预览;TextContentField 的 NSScrollView+NSTextView;关键帧 ruler 用 NSViewRepresentable 复用 TimelineRuler;NSPasteboard(复制 prompt);ByteCountFormatter(文件大小);NSHapticFeedbackManager(吸附触感)。
- CoreText / CoreGraphics [medium] — NSFont 解析字体与 familyName、字体菜单预览;NSColor sRGB 分量读写(颜色面板/RGBA 互转);CGPath/Canvas 绘制菱形关键帧、playhead 三角、吸附虚线;TextStyle 提供 NSParagraphStyle/CATextLayerAlignmentMode(文字渲染在别处)。
- Combine [low] — AccountService 用 AnyCancellable 订阅 Convex 账户/套餐流;Inspector 仅读取其派生布尔(aiAllowed/isMisconfigured),不直接用 Combine。
- AVFoundation [low] — 仅经 MediaAsset.duration / sourceWidth/Height 与 TrimmedSource 间接出现(放大/编辑时长、裁剪段秒数),Inspector 本身不调 AV API。

**闭源云**:Inspector 不直接发起网络请求,但 AI Edit 标签是闭源云的入口:1) 标签可见性由 AccountService.shared.isMisconfigured / aiAllowed 门控,而 AccountService 通过 ClerkKit(登录)+ ConvexMobile(account:get / billing:* / users:upsertFromAuth)访问闭源后端;2) 所有放大/编辑/重跑/图生视频/视频配乐动作经 EditSubmitter → editor.generationService.generate 或 editor.seedGenerationPanel,最终走闭源生成式 AI 云(上传素材并调用各模型);3) 资产详情页用 ModelRegistry.displayName,模型目录由 ModelCatalog 经 ConvexMobile 拉取;CostEstimator 仅本地按拉取到的费率算 credits。变换/速度/音量/淡变/文字/关键帧等纯编辑功能完全本地、无云。

**移植策略**:Inspector 是 UI 外壳,用 React/TS 重建面板与交互,真正的算法下沉到 Rust core 一比一复刻。分层建议:(A) Rust core 复刻领域逻辑——Clip/Track/Timeline/Transform/Crop、KeyframeTrack 的 upsert/remove/move/sample(linear/smooth=smoothstep/hold)、VolumeScale(dB↔线性, floor-60→0, ceil+15)、fadeMultiplier、writeScale/Position/Rotation/Opacity/Volume/Crop 的‘有关键帧则写帧否则写静态值’规则、cropFittingAspect、fitTextClipToContent(配合下述文字测量)、split/clamp/rescale 关键帧、SnapEngine(sticky1.5x/playhead1.5x, 阈值 lane4px/timeline8px);全部用整数帧,秒=帧/fps,所有提交走命令模式 undo 栈(before/after 快照),拖动期不入栈。(B) 前端 React 组件:ScrubbableNumberField(指针拖拽改值、shift*10/cmd*0.1、点击转输入、多选‘—’)、ColorField(改用浏览器取色或自绘 HSV 面板,拖动实时回调,无 NSColorPanel)、FontPickerField(查询系统/打包字体列表,Tauri 端枚举字体)、KeyframesPanel(用 Canvas/SVG 画菱形/playhead/吸附虚线,frame↔x 用 KeyframesMetrics 公式)、TextContentField(<textarea>)。(C) 替换点与坑:NSColorPanel→自绘/原生 input[type=color](注意它只在提交时回调,需自绘才能拿到拖动中变化以复刻 debounced commit);NSFont familyName/字体测量→Tauri 调用系统(Rust 端 font-kit/cosmic-text)做 TextLayout.naturalSize(maxWidth=画布宽*0.9),否则文字自适应包围盒会与上游不一致;NSHapticFeedbackManager→桌面无触感,吸附改为视觉虚线+可选音效;ByteCountFormatter→Rust humansize;TimelineRuler 复用→需在 Rust/Canvas 统一刻度算法。(D) AI Edit 标签属闭源云,单独按 cloud-rebuild 处理:可保留‘可用性判断(EditAction 规则,纯本地)+成本估算’逻辑,但把 Clerk/Convex/生成服务替换为自有后端或开放模型 API;若 OpenTake 不接生成式云,可整块 drop 仅保留放大可选。媒体时长/源尺寸由 FFmpeg 探测填充 MediaAsset(替代 AVAsset)。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Inspector/InspectorView.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Inspector/Keyframes/KeyframesLane.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Inspector/AIEditTab.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Inspector/TextTab.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Inspector/Components/ScrubbableNumberField.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Inspector/Components/ColorField.swift

## Account  ·  `cloud-client` → **cloud-rebuild**

**职责**:
- 鉴权:通过 Clerk 发起 Google OAuth 登录/登出,并通过 ClerkConvexAuthProvider 把 Clerk 会话桥接给 Convex,监听 AuthState(loading/authenticated/unauthenticated)
- 账户配置 provision:登录后调用 Convex mutation users:upsertFromAuth,把 Clerk 用户的 email/name/image 写入后端(带 3 次重试)
- 账户数据实时同步:订阅 Convex 的 account:get(账户+套餐)与 billing:listPlans(可购套餐),实时推送到 @Observable 状态
- 积分账本计算:budget = 套餐月度积分 + 已购积分;remaining = max(0, budget - 本期已花);并据此派生 hasCredits/remainingCredits
- 计费动作:发起订阅结账(billing:createCheckoutSession)、积分充值(billing:createTopOffCheckoutSession)、管理订阅门户(billing:createPortalSession),并用 NSWorkspace 打开 Stripe URL
- URL 安全校验:只允许打开 https 且 host 属于 checkout.stripe.com / billing.stripe.com 的链接,否则拒绝
- 对外开关:isSignedIn / aiAllowed / isPaid / hasCredits,被全 App 用来 gate AI 生成与 Agent 聊天
- 反馈上报:feedback:send action,携带消息/邮箱/可否联系/截图 base64/App 版本/OS 版本
- UI 呈现:头像(UserAvatar)、身份条(IdentityStrip)、账户气泡卡(AccountPopoverCard)、积分摘要(CreditSummaryView)、充值输入(TopOffField)、设置页账户面板(AccountPane)
- 错误与日志:统一 lastError 字符串供 UI 展示,关键事件经 Log/Telemetry 上报 Sentry

**核心类型**:
- `AccountService` (class) — 核心单例(@Observable @MainActor,static shared)。持有 Convex 客户端、所有订阅与鉴权任务,集中管理登录态/账户数据/积分/计费动作,是整个模块对外的唯一状态与行为入口。
- `AccountTier` (enum) — 套餐档位 none/pro/max(String 可解码)。提供 isPaid、planLabel('Free'/'Pro plan'/'Max plan')、upgradeLabel('' /'Pro'/'Max')。
- `AccountUser` (struct) — 后端返回的用户主体:email/name/image/tier/currentPeriodEnd(毫秒时间戳)/cancelAtPeriodEnd/spentCreditsThisPeriod/purchasedCredits。派生 displayName(trim 后非空)、firstName(按空格取首段)。
- `AccountPlan` (struct) — 当前账户所处套餐:tier/monthlyPriceUsd/monthlyBudgetCredits(可空)。用于积分预算计算。
- `AvailablePlan` (struct) — 可购买套餐(Identifiable,id=tier.rawValue):tier/monthlyPriceUsd/discountedMonthlyPriceUsd/monthlyBudgetCredits。派生 hasDiscount(折扣价 < 原价)与 effectiveMonthlyPriceUsd(有折扣取折扣价)。
- `AccountResponse` (struct) — account:get 订阅的载荷:{user: AccountUser, plan: AccountPlan?}。
- `TopOffLimits` (enum) — 充值金额边界常量:minDollars=5,maxDollars=1000(纯命名空间常量)。
- `AuthState<String>` (enum) — 来自 ConvexMobile 的泛型鉴权状态枚举,三态 .loading/.authenticated/.unauthenticated;泛型参数(此处 String)为身份/令牌类型。AccountService 据此驱动整个登录流程。
- `AccountPopoverCard` (struct) — SwiftUI 视图。点头像弹出的紧凑账户卡:身份块+套餐块(积分进度条/升级按钮)+底部(设置/反馈/登录登出)。
- `CreditSummaryView` (struct) — SwiftUI 视图,两种样式:.full(设置页大进度条)与 .compact(生成面板胶囊小芯片,点开 CreditActionsPopover 充值/升级)。
- `TopOffField` (struct) — SwiftUI 泛型视图(带 Trailing slot)。美元输入框,实时换算 credits=美元*100,做 5–1000 校验,触发 buyCredits。
- `UserAvatar / IdentityStrip / UserAvatarButton` (struct) — SwiftUI 头像与身份条组件:登录显示首字母圆/远程头像,未登录显示占位符号;Strip 显示主/次文本(名/邮箱)。
- `BackendConfig` (enum) — 从 Info.plist 读取后端配置:PalmierClerkPublishableKey / PalmierConvexDeploymentURL / PalmierConvexHttpURL;isConfigured 判断是否齐全。

**核心算法/逻辑(供 Rust 复刻)**:
- [积分预算核心算法] budgetCredits:若无 user 返回 nil;否则 tierBudget = account.plan.monthlyBudgetCredits ?? 0,再加 user.purchasedCredits ?? 0(套餐月额度 + 已购充值额度)。spentCredits = user.spentCreditsThisPeriod ?? 0。remainingCredits = max(0, (budgetCredits ?? 0) - spentCredits)。hasCredits = remainingCredits > 0。注意:只有当 budgetCredits 非 nil(即已有 user)时,UI 才显示积分块;Rust 复刻需保留‘nil 表示未知/不展示’与‘0 表示已耗尽’的区分。
- [美元→积分换算] credits = max(0, dollars) * 100(每 1 美元 = 100 积分,整数运算)。充值合法区间 [5,1000] 美元(闭区间,含端点)。TopOffField 中 isValid = (5...1000).contains(dollars);按钮文案合法时为 'Buy $<n>' 否则 'Buy';换算文案 1 时显示 '= 1 credit' 否则 '= <n> credits'。
- [充值动作 buyCredits] 入参 dollars:Int。先校验 (5...1000).contains(dollars),越界则设 lastError='Amount must be $5–$1000.' 并直接返回。若已有 isBuyingCredits 在跑则忽略(去重/防重复点击)。置 isBuyingCredits=true,起一个 @MainActor Task,defer 里复位 isBuyingCredits=false 并清空 buyCreditsTask。调用 Convex action 'billing:createTopOffCheckoutSession',参数 {dollars: Double(dollars)}(注意后端要 Double),拿到 {url} 后走 openInBrowser。异常写 lastError。
- [订阅动作 subscribe] 入参 tier。先清 lastError;若 tier 非付费(none)或无 convex 直接返回。调用 action 'billing:createCheckoutSession' 参数 {tier: tier.rawValue},得 {url} 后 openInBrowser。异常写 lastError。
- [管理订阅 manageSubscription] 调用 action 'billing:createPortalSession'(无参),得 {url} 后 openInBrowser。
- [URL 安全闸门 openInBrowser] 解析 URL,必须满足:scheme=='https' 且 host 非空 且 host ∈ {checkout.stripe.com, billing.stripe.com}(白名单 Set)。任一不满足则 lastError='Refused to open untrusted URL.' 且不打开。通过则 NSWorkspace.shared.open。Rust 端复刻需保留同样的协议+host 白名单校验后再交给系统浏览器打开。
- [鉴权观察主循环 startAuthObservation] 先自旋等待 Clerk 恢复缓存会话:最多 50 次、每次 sleep 100ms(即最长 5 秒),条件 while !Clerk.shared.isLoaded。随后 for await 监听 convex.authState.values:loading→isLoading=true;authenticated→先 await provisionAndSubscribe() 再 isLoading=false;unauthenticated→clearAccount() 且 isLoading = (Clerk.shared.session != nil)(即仍有本地会话时保持 loading,等待 Convex token 就绪,避免登录瞬间闪现登出 UI)。这是一个易错的边界条件,需精确复刻。
- [provision 重试策略 provisionAndSubscribe] 组装 name = [firstName,lastName] 去 nil 后空格拼接,空串则传 nil;args = {email, name, image}。循环 attempt 0..<3 调用 mutation 'users:upsertFromAuth':成功 break;失败写 lastError,若 attempt==2(最后一次)记录失败并 return(不再订阅);否则 sleep 500ms 重试。成功后调用 startAccountSubscription()。即:最多 3 次、每次失败间隔 0.5 秒。
- [实时订阅模型] 用 Combine。startPlansSubscription:订阅 'billing:listPlans' yielding [AvailablePlan],receive(on: main),sink 把值写 availablePlans,失败写 lastError。startAccountSubscription:订阅 'account:get' yielding AccountResponse,收到值写 account 并清空 lastError,失败写 lastError。两条订阅互相独立;plans 订阅在 configure 阶段(尚未登录)就启动,account 订阅在 provision 成功后才启动。
- [登录态派生] isSignedIn = (!isMisconfigured && authState==.authenticated)。aiAllowed = isSignedIn && !isMisconfigured。tier = account?.user.tier ?? .none。isPaid = tier.isPaid。这些是被全 App gate AI 功能的真值来源(Agent.canStream 还要求 hasCredits;ToolExecutor 生成前 guard isSignedIn 再 guard hasCredits)。
- [clearAccount 清理] 取消并置空 accountSubscription、取消并置空 buyCreditsTask,account=nil,isBuyingCredits=false(注意不动 plansSubscription,登出后仍可看套餐)。
- [configure 幂等] 用 didConfigure 布尔保证只配置一次。若 BackendConfig 缺 clerkPublishableKey 或 convexDeploymentURL,则 isMisconfigured=true、isLoading=false 并告警返回(此后全 App 把 AI 功能直接当作不可用而非‘需登录’)。配置成功则 Clerk.configure(redirectUrl='palmier://callback', scheme='palmier'),创建 ConvexClientWithAuth(deploymentUrl, ClerkConvexAuthProvider()),启动 plans 订阅与鉴权观察。
- [周期结束时间换算] currentPeriodEnd 是毫秒级 Unix 时间戳(Double)。展示用 Date(timeIntervalSince1970: endMs/1000) 再 abbreviated 日期。cancelAtPeriodEnd==true 时显示 'Cancels <date>' 否则积分块显示 'Resets <date>'。
- [积分进度条颜色阈值] remaining = budget>0 ? min(1.0, left/budget) : 0(left=max(0,budget-spent))。颜色:<0.05 红;<0.25 橙;否则品牌主色。CreditSummaryView 与 AccountPopoverCard 用同一套阈值。
- [套餐积分简写] creditsShortLabel:若 credits>=1000 且能被 1000 整除,显示 '<n/1000>k credits';否则 '<千分位格式> credits'。
- [反馈上报 sendFeedback] 无 convex 抛 NSError(domain 'Palmier.Feedback', code -1)。args 必含 message/mayContact/appVersion/osVersion;email 与 screenshotPngBase64 可选(非 nil 才加入)。调用 action 'feedback:send' 期望 {ok}。截图以 PNG 的 base64 字符串上传。

**苹果框架使用**:
- AppKit [low] — NSWorkspace.shared.open 打开 Stripe 结账/门户链接;构造 NSError 作为反馈失败错误
- SwiftUI [medium] — 全部账户相关界面(头像、身份条、账户气泡卡、积分摘要、充值输入、设置账户面板),使用 @Observable/@Bindable 双向绑定、AsyncImage 加载远程头像、ProgressView 画积分条、popover
- Observation [low] — @Observable + @ObservationIgnored 标注 AccountService,驱动 UI 自动刷新
- Combine [low] — 对 Convex 的 account:get / billing:listPlans 订阅用 AnyCancellable.sink 接收并切回主线程
- Foundation [none] — URL/URLSession(参考文件上传见 GenerationBackend)、Bundle 读 Info.plist 配置、Date 周期时间换算、JSONDecoder 解码、Task/async 并发
- os.Logger [none] — 经 Log/CategoryLog('account') 输出分类日志并镜像到 stderr

**闭源云**:是,且本模块的存在本质即为访问闭源云:1) Clerk(闭源鉴权 SaaS)做 Google OAuth 登录与会话;2) Convex(闭源 BaaS,convex-swift + ConvexClientWithAuth)做实时数据库订阅(account:get、billing:listPlans、generations:*)与 serverless mutation/action(users:upsertFromAuth、billing:createCheckoutSession/createTopOffCheckoutSession/createPortalSession、feedback:send、uploads:*);3) Stripe(经 Convex 返回 checkout.stripe.com / billing.stripe.com 链接)做支付;4) Sentry 做遥测上报。注意:本模块本身不直接调用生成式 AI,但它是 AI 云能力的‘鉴权+计费闸门’——下游 GenerationBackend/PalmierClient 通过 AccountService.convex 与积分开关访问后端的图像/视频/音频/Claude 聊天生成。closedCloudTouch = 全链路闭源云依赖。

**移植策略**:这是闭源云客户端层,不能直接 port,需在 OpenTake 里整体重建为自有后端方案。具体替换:1) 鉴权:用开源方案替代 Clerk——Tauri 端用系统浏览器走 OAuth2 PKCE / 设备码,Rust core 用 oauth2 crate + 自定义 deep-link(opentake://callback,对应原 palmier://callback)接回 token,令牌存 OS keychain(keyring crate);或自托管 authentik/Keycloak/Supabase Auth。2) 后端实时数据与函数:用 Convex 的开源自托管版,或换 Supabase(Postgres + Realtime + Edge Functions)/ 自建 axum 服务 + WebSocket/SSE 订阅,复刻 account:get、billing:listPlans 的‘服务端推送即时刷新’语义(前端用 TanStack Query + WS 失效)。3) 计费:Stripe 仍可用——Rust 后端用 stripe-rust 创建 checkout/portal session,Tauri 用 tauri-plugin-shell/opener 打开返回 URL;务必复刻 openInBrowser 的‘https + host 白名单(checkout/billing.stripe.com)’安全闸门(在 Rust 侧用 url crate 校验 scheme/host 后再 open)。4) 积分账本算法是纯整数运算,可在 Rust 端 1:1 直译(budget = 套餐额度 + 已购;remaining = max(0, budget-spent);dollars*100=credits;5..=1000 充值区间;阈值 0.05/0.25 上色)——这部分属 direct-port。5) 状态机(loading/authenticated/unauthenticated、登出时 isLoading 取决于是否仍有本地会话、provision 3 次×0.5s 重试、Clerk 恢复会话最多 5s 自旋)建议在 Rust 用 async + tokio 复刻,前端只读状态。6) UI(头像/身份条/积分条/充值框/设置账户面板)全部 ui-rebuild 为 React/TS 组件,沿用 AppTheme 令牌映射成 CSS 变量;AsyncImage→<img> 懒加载,ProgressView→进度条组件。7) 遥测:Sentry 有官方 Rust/JS SDK,可平移。整体工作量集中在‘自有后端 + 鉴权 + 计费’的重建,而非编辑逻辑。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/AccountService.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/BackendConfig.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/AccountPopoverCard.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/CreditSummaryView.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/TopOffField.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Account/IdentityViews.swift

## Search  ·  `engine` → **needs-replacement**

**职责**:
- 模型生命周期管理:启动时探测已安装模型、按需从 HuggingFace 下载/SHA256 校验/编译 SigLIP2 编码器、加载到内存并做一次 warm-up(encode 文本 'warm up')
- 逐素材索引调度:把需要索引的素材(视频/图片)排队,后台 utility 优先级 worker 串行处理;导出期间暂停;支持取消与失败重试(失败仅在单批次内去重)
- 视频抽帧采样:按时间间隔抽候选帧,用 8x8 luma 网格做场景切换检测划分镜头 (shot),并用覆盖率下限保证长静态镜头也被采样
- 图像/文本编码:把 CGImage 预处理成方形 BGRA pixel buffer 喂图像编码器;把文本 tokenize+padding 后喂文本编码器,输出 768 维 embedding
- embedding 持久化:自定义二进制格式(magic + JSON header + Float64 时间元组 + Float16 向量)按文件身份(路径|mtime|size 的 SHA256 取前 32 字符)做幂等缓存
- 查询与排名:文本向量与各素材帧矩阵做 GEMV 点积,每个镜头只保留最佳帧,按分数排序并用绝对下限(0.05)+ 相对截断(top×0.85)过滤,limit 截断
- 跨窗口协调:用弱引用注册表对所有打开的项目协调器做 sweep/cancel/reset/clearGlobally 的 app 级 fan-out;导出暂停用引用计数跨窗口共享
- 工程/数据集成:embedding 缓存在 cachesDirectory,模型装在 applicationSupportDirectory;通过 assetsProvider 闭包从 EditorViewModel 拉取当前素材列表

**核心类型**:
- `SearchIndexCoordinator` (class) — @MainActor @Observable 的每项目索引队列 + 查询入口。维护待索引队列、失败集合、后台 worker、已加载索引缓存(loadedIndexes);暴露 indexingProgress/indexingActive 进度、search() 查询。用静态弱引用表 NSHashTable 对所有实例做 app 级广播,用静态引用计数器 ExportPauseCounter 实现导出期间暂停索引。workerGeneration 防止过期 worker 退出时误清新 worker 引用。
- `VisualModelLoader` (class) — @MainActor @Observable 单例 (shared)。SigLIP 模型的 app 级加载器,状态机 State(unknown/notInstalled/downloading(progress)/preparing/ready/failed)。prepare() 只加载不下载且幂等;download() 触发下载安装;setEnabled/remove 控制开关与删除;持有唯一的 VisualEmbedder。isReady == (state==.ready)。
- `VisualEmbedder` (class) — @unchecked Sendable。封装两个 CoreML MLModel(图像编码器 + 文本编码器)与 TextTokenizer。提供 encode(image:)->[Float] 和 encode(text:)->[Float],含 CGImage→CVPixelBuffer 预处理(squash 到正方形、黑底、sRGB、premultipliedFirst+byteOrder32Little)与输出 'embedding' 特征向量提取。内嵌 Spec(model/version/embeddingDim=768/imageSize=256/contextLength=64)。
- `EmbeddingStore` (struct) — per-asset 帧 embedding 的磁盘缓存。定义二进制格式(Header 可 Codable / Row / AssetIndex),提供 key(基于文件身份的 SHA256)、load/save/header/isCurrent/clearAll。向量在内存中是 Float32 扁平数组(供 BLAS),落盘是 Float16。
- `FrameSampler` (enum) — 视频抽帧器。用 AsyncThrowingStream 流式产出'视觉上不同'的帧:luma 场景突变开启新镜头,覆盖率下限保证长镜头有代表帧。内含 LumaGrid(8x8 luma 网格指纹 + meanDiff)。samplerVersion=1 参与缓存失效判定。Options 含 candidateInterval/coverageFloor/promoteDiff/maxSize/highResEdge。
- `VisualIndexer` (enum) — 单素材索引器:抽样帧→编码→写 EmbeddingStore,按 (file, model, sampler) 幂等。index() 处理视频(逐帧累积 + shot 边界计算),indexImage() 处理静图(单 embedding,零长 shot)。needsIndex() 判断是否需要重新索引。
- `ModelDownloader` (class) — @unchecked Sendable。下载/校验/编译/安装 SigLIP 编码器到 Application Support。Manifest 描述文件名+SHA256+字节数;install() 幂等下载三个 zip、SHA256 校验、ditto 解压、MLModel.compileModel 编译 .mlpackage→.mlmodelc、原子安装并写 spec.json。
- `VisualSearch` (enum) — 纯排名算法(无状态)。search() 用 cblas_sgemv 算 query 与每素材帧矩阵的点积,best-per-shot 去重,全局按分数排序,minScore 绝对下限 + relativeCutoff 相对截断 + limit 截断。返回 Hit(assetID/time/shotStart/shotEnd/score)。
- `TextTokenizer` (class) — @unchecked Sendable。包 swift-transformers 的 AutoTokenizer,从 tokenizer 文件夹加载。tokenize() 编码后截断到 contextLength 并用 padToken=0 右填充到定长(匹配 SigLIP 训练时无 attention mask 的 max_length padding)。
- `SearchIndexConfig` (enum) — 配置常量:enabled(UserDefaults,默认 true)、visualMatchCosineFloor=0.05、HuggingFace hostedURL、以及完整 Manifest(siglip2-base-patch16-256, v1, dim=768, imageSize=256, contextLength=64,三个文件的 SHA256 与字节数)。

**核心算法/逻辑(供 Rust 复刻)**:
- 【单位与时间基准】全模块时间单位是秒 (Double),非帧。AVFoundation 时间用 CMTime(seconds:, preferredTimescale: 600)。帧/秒换算在调用方完成(MediaTab+Search.swift: secondsToFrame(seconds:, fps:),即 frame = round(seconds*fps));Search 内部永远用秒。Rust 复刻时索引与 Hit 全部存秒,timeline 帧换算放在调用边界。
- 【抽帧采样算法 FrameSampler.sample,samplerVersion=1】(1) 取视频第一条 video track;若 naturalSize 的 max(|w|,|h|) >= highResEdge(3000)则把候选间隔 interval 翻倍(2.0→4.0)。(2) 候选时间点 = stride(from: interval/2, to: duration, by: interval),即 {interval/2, interval/2+interval, ...} 严格小于 duration;若为空(duration<=interval/2)则取 [duration/2]。(3) AVAssetImageGenerator: appliesPreferredTrackTransform=true(应用旋转),maximumSize=512x512,requestedTimeToleranceBefore/After = max(interval/2, 1.0) 秒(允许解码器取最近关键帧)。(4) 对每个成功解码帧:用实际时间 actualTime.seconds=t,丢弃 t<=lastTime 的重复帧(去重);计算 8x8 luma 网格 grid;若有上一帧 grid 则 isNewShot = LumaGrid.meanDiff(grid,last) > promoteDiff(12),否则首帧 isNewShot=true;更新 lastGrid=grid。(5) 保留条件:isNewShot 或 (t - lastKeptTime >= coverageFloor=8.0);满足则 lastKeptTime=t 并 emit(Frame{time:t, image, isNewShot})。注意:luma 比较用所有解码帧更新,但只在被保留时才推进 lastKeptTime。
- 【LumaGrid 指纹】cells=8。把 CGImage 用 CGContext 高质量插值绘制到 8x8 RGBA(premultipliedLast)缓冲;每格 luma = R*0.299 + G*0.587 + B*0.114(Rec.601 系数,作用于 sRGB 字节值,未做 gamma 线性化)。meanDiff(a,b) = Σ|a[i]-b[i]| / 64(L1 平均差)。Rust 复刻:FFmpeg 缩放到 8x8 后用同系数算 luma,L1 平均差,阈值 12。
- 【视频索引帧累积 VisualIndexer.index】遍历 FrameSampler 帧流:维护 shotStarts:[Double]。每遇 isNewShot 帧:shotStarts.append(shotStarts.isEmpty ? 0 : frame.time)——即第一个镜头起点强制为 0(不管首帧实际时间),其余镜头起点为该帧时间。每帧:vectors += encode(image);times.append(frame.time);shotIndices.append(shotStarts.count-1)。最后构造 Row:time=帧时间,shotStart=shotStarts[shot],shotEnd = (shot+1 < shotStarts.count ? shotStarts[shot+1] : duration)——即镜头结束=下一镜头起点,最后一个镜头结束=视频总时长。每帧报进度 min(frame.time/duration, 1)。
- 【静图索引 VisualIndexer.indexImage】不走采样器:decodeImage 用 CGImageSource 生成 thumbnail(kCGImageSourceThumbnailMaxPixelSize=512, CreateThumbnailFromImageAlways=true, WithTransform=true 应用 EXIF 方向);单条 Row{time:0, shotStart:0, shotEnd:0}(零长镜头),单个 embedding。若解码失败则 rows/vectors 为空但仍写一个 count=0 的索引文件(标记已处理,避免反复重试)。
- 【图像预处理 VisualEmbedder.pixelBuffer】CVPixelBuffer 格式 kCVPixelFormatType_32BGRA,尺寸 imageSize×imageSize(256×256)。CGContext bitmapInfo = premultipliedFirst | byteOrder32Little,色彩空间优先 sRGB。关键:缓冲内存是复用未清零的,所以先用黑色 (gray:0, alpha:1) 填满整张,再绘制(保证带 alpha 的源在黑底上混合);SigLIP 预处理是 squash-resize 到正方形(直接拉伸,不裁剪不保持宽高比)。Rust 复刻:FFmpeg/swscale 直接 scale 到 256x256(忽略宽高比),BGRA,带 alpha 的先合成到黑底。
- 【文本 tokenize TextTokenizer】用 SigLIP 的 tokenizer。encode 文本→id 数组(Int32);若 > contextLength(64)截断取前 64;再用 padToken=0 右填充到正好 64 长。SigLIP 训练时是 max_length padding 且无 attention mask,所以填充必须与 Python 参考严格一致(定长 64、pad=0)。文本编码器输入是 shape [1,64] 的 int32 MLMultiArray,key='tokens'。
- 【embedding 提取 VisualEmbedder.vector】从模型输出取 featureValue('embedding').multiArrayValue,断言 count==dim(768);若 dataType==.float32 直接 buffer 拷贝,否则逐元素 floatValue。注意:模型输出未在本模块做 L2 归一化——是否归一化取决于导出的 CoreML 图。排名用裸点积 cblas_sgemv,若模型已归一化则等价余弦相似度;Rust 复刻必须用同一导出模型/确认是否归一化以保证分数一致。
- 【磁盘格式 EmbeddingStore,二进制 little-endian】布局: magic 'PALMEMB1'(8 字节 UTF8) + UInt32 headerLen(4 字节,小端) + JSON(Header) + count 行;每行 = Float64 time(8B) + Float64 shotStart(8B) + Float64 shotEnd(8B) + dim 个 Float16(每个 2B)。行字节数 rowBytes = 3*8 + dim*2 = 24 + 768*2 = 1560。Header{model,modelVersion,samplerVersion,dim,count}。读时严格校验:总字节必须 == offset + count*rowBytes,否则抛 corrupt。写时 .atomic。Rust 复刻:用 byteorder LE,Float64 直读,Float16 用 half crate 转 f32。
- 【缓存键与失效 EmbeddingStore.key / isCurrent】key = SHA256('path|mtime.timeIntervalSince1970|size') 的十六进制取前 32 字符。文件路径或修改时间或大小任一变化都使缓存失效(注意 mtime 用浮点秒,精度敏感)。needsIndex = !isCurrent(model 名 && modelVersion && samplerVersion 全等)。任何一项版本不匹配都重新索引。Rust 复刻:同样拼 'path|mtime_unix_f64|size' 再 SHA256 截前 32 hex;注意 timeIntervalSince1970 是相对 1970 的浮点秒。
- 【查询排名 VisualSearch.search,核心检索】对每个 (assetID, AssetIndex):若 index.header.dim != query.count 或 count==0 则跳过。scores = vectors(count×dim 行主序) · query,用 cblas_sgemv(RowMajor, NoTrans, M=count, N=dim, alpha=1, A=vectors, lda=dim, x=query, beta=0, y=scores)。然后 best-per-shot:对每帧按其 row.shotStart 分组,只留分数最高的帧(existing.score >= score 则跳过,即同分保留先出现的)。把每个 shot 的最佳帧加入 hits(assetID, time, shotStart, shotEnd, score)。全局 hits 按 score 降序排序。若有 minScore(=visualMatchCosineFloor 0.05)先过滤掉 < 0.05 的。再取 top=最高分;若 top<=0 返回空。floor = top * relativeCutoff(0.85);返回 hits.prefix(limit) 里 score>=floor 的(注意顺序:先 prefix(limit) 再 filter floor,所以最终条数 <= limit)。
- 【索引调度与并发 SearchIndexCoordinator】schedule(asset):enabled 且有 embedder 且非 isGenerating;asset.id 不在 queue 也不在 failedIds;needsVisual(视频/图片且 VisualIndexer.needsIndex) 或 needsTranscript(音频或带音轨视频且 TranscriptCache 无磁盘缓存)成立才入队,batchTotal+1,ensureWorker。worker(单个,utility 优先级):循环 dequeue,导出活跃时每 2 秒轮询等待,indexOne。dequeue 跳过已不存在的 id(batchCompleted+1),队列空则 resetBatch 返回 nil。indexOne:defer batchCompleted+1;若需转写则视觉占进度 0.5 否则 1.0;并发 async let 跑转写(TranscriptCache.transcript)与视觉索引;视觉完成后 currentAssetFraction=visualShare 再 await 转写。失败(非取消)记入 failedIds 并 warning。
- 【进度计算】indexingProgress = min(1, (batchCompleted + clamp(currentAssetFraction,0,1)) / batchTotal),batchTotal==0 时为 0。currentAssetFraction 在 indexOne 内由视觉索引进度回调 * visualShare 更新(0.5 或 1.0 权重)。
- 【导出暂停 ExportPauseCounter】静态引用计数,exportDidBegin/End 跨所有窗口共享。waitWhileExportActive():while exportActive { sleep 2s }。索引 worker 与 VisualIndexer 每处理一帧前、转写前都调用它,保证导出占用编解码资源时索引让路。
- 【查询的 actor 切换】search() 在 @MainActor 上 snapshot 候选素材 (id,url) 与 loadedIndexes 缓存,然后 Task.detached(userInitiated) 里做:stat/SHA256 算 key、命中内存缓存(key 相等)则复用否则 EmbeddingStore.load 磁盘、model.encode(text) 编码 query、VisualSearch.search 排名;回到 main 把新加载的索引 merge 进 loadedIndexes。空 query(trim 后)返回 []。
- 【模型下载 ModelDownloader.install】幂等(已安装直接返回)。三个文件 imageEncoder/textEncoder/tokenizer 顺序下载到临时 staging:URLSession.shared.download + 进度委托(按累计字节/总字节);SHA256 流式校验(1MB 块)不符抛 checksumMismatch;ditto(-x -k)解压,要求 zip 内恰好一个顶层非点开头条目;若是 .mlpackage 则 MLModel.compileModel 编译成 .mlmodelc 否则原样(tokenizer 文件夹)。最后原子地 moveItem 到 installDir(model-vN)/{ImageEncoder.mlmodelc, TextEncoder.mlmodelc, tokenizer/} 并写 spec.json。installed() 通过三个路径存在性(含 tokenizer/tokenizer.json)判定。

**苹果框架使用**:
- CoreML [blocker] — 加载/运行 SigLIP2 图像编码器与文本编码器(.mlmodelc),下载后用 MLModel.compileModel 把 .mlpackage 编译成 .mlmodelc;MLMultiArray 喂文本 token、取 768 维 embedding 输出;computeUnits=.all 走 ANE/GPU/CPU。这是整个模块的推理核心。
- CoreVideo [low] — 创建 32BGRA CVPixelBuffer 作为图像编码器输入,锁定基址用 CGContext 绘制。
- CoreGraphics [low] — CGImage→pixel buffer 的 squash-resize 预处理(黑底、sRGB、premultipliedFirst);8x8 luma 网格 CGContext 下采样做场景切换指纹。
- ImageIO [low] — CGImageSourceCreateThumbnailAtIndex 解码静态图片并应用 EXIF 方向,最大边 512。
- AVFoundation [medium] — AVURLAsset 读视频轨道(naturalSize/preferredTransform);AVAssetImageGenerator.images(for:) 批量按时间抽帧,maximumSize 512、容差 max(interval/2,1)s、应用 preferredTrackTransform。
- Accelerate [none] — cblas_sgemv 计算 query 向量与素材帧矩阵(count×dim)的点积得分,是排名的唯一数值核心。
- CryptoKit [none] — SHA256 计算文件身份缓存键(path|mtime|size)与下载文件完整性校验(1MB 分块流式)。

**闭源云**:无生成式 AI 云接触。整个 Search 模块唯一的网络请求是 ModelDownloader 从 HuggingFace(https://huggingface.co/palmier-io/siglip2-base-coreml/resolve/main,DEBUG 下可经 UserDefaults 'searchIndexModelBaseURL' 覆盖)下载 SigLIP2 模型权重 zip(ImageEncoder/TextEncoder/tokenizer),用 URLSession.shared.download + SHA256 校验,属于一次性静态模型分发,不是闭源生成式云。索引与查询全部本地 CoreML 推理。不经过 Convex/ConvexMobile/Clerk/ClerkKit(这些依赖在 Package.swift 中存在但本模块不 import、不调用)。唯一外发遥测是 Log.search.notice(..., telemetry:) 经 Sentry 发送的面包屑,内容仅为计数/状态(assets 数、queueDepth、generation、enabled、耗时秒数、错误描述),不含媒体内容或 embedding。

**移植策略**:整体可在 Rust core 中忠实复刻,但 CoreML 推理是硬替换点。分模块方案: (1) 检索/排名 VisualSearch:direct-port,用 ndarray + BLAS(openblas/accelerate-src)或手写 SIMD 点积复刻 cblas_sgemv;best-per-shot 去重(HashMap<OrderedFloat<f64>,(row,score)>,同分保留先出现)、降序排序、minScore 0.05 绝对下限 + relativeCutoff 0.85(先 take(limit) 再 filter)逻辑一比一照搬。(2) EmbeddingStore 二进制格式:direct-port,用 byteorder(LE)读写 magic 'PALMEMB1' + u32 headerLen + JSON header(serde) + 每行 f64*3 + dim 个 f16(half crate);rowBytes=24+dim*2;严格长度校验。缓存键 SHA256('path|mtime_unix_f64|size') 取前 32 hex(sha2 crate)。(3) FrameSampler + LumaGrid:需用 FFmpeg 替换 AVAssetImageGenerator——按 stride(interval/2..duration step interval)定位时间戳、seek 到最近关键帧解码,scale 到 8x8 算 Rec.601 luma(系数 .299/.587/.114 作用于 sRGB 字节)、L1 平均差阈值 12 判镜头,coverageFloor 8s;注意复刻'应用旋转(preferredTransform/EXIF)'、'实际帧时间去重 t>lastTime'、'高分辨率(max 边>=3000)间隔翻倍'。(4) VisualEmbedder 图像/文本编码:CoreML 不可移植——换成 ort(ONNX Runtime)或 candle 加载 SigLIP2 的 ONNX/safetensors;图像预处理用 swscale squash-resize 到 256x256 BGRA、带 alpha 合成黑底;文本用 tokenizers crate(HF Rust 原生,与 swift-transformers 同源)做 SigLIP tokenize、截断 64、pad=0 右填充定长 64。务必确认模型是否在图内做 L2 归一化以保证裸点积分数与上游一致;建议直接复用上游同一份导出权重(转 ONNX)。(5) ModelDownloader:needs-replacement 但简单——reqwest 下载 + sha2 校验 + zip 解压(zip crate,替代 ditto)+ ONNX/candle 无需编译步骤(去掉 MLModel.compileModel)。(6) 调度/并发 SearchIndexCoordinator:用 tokio 任务 + 单 worker 队列复刻;导出暂停用 AtomicUsize 引用计数 + 轮询/Notify;@MainActor/@Observable 的 UI 状态换成 Tauri 事件向前端推进度。MediaTab 的秒↔帧换算(round(seconds*fps))保留在前端/调用边界。整体算法确定性强,无 Apple 专有算法,除推理引擎外都是直接可移植的数值/IO 逻辑。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Search/SearchIndexCoordinator.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Search/Indexing/FrameSampler.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Search/Indexing/EmbeddingStore.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Search/Indexing/VisualIndexer.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Search/Query/VisualSearch.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Search/Models/VisualEmbedder.swift

## Settings  ·  `ui` → **ui-rebuild**

**职责**:
- 渲染设置窗口的整体布局:侧边栏(IdentityStrip + 分页按钮)+ 详情区(标题 + ScrollView),用 AppKit NSWindowController 托管一个独立暗色磨砂窗口(SettingsWindowController.shared)。
- 分页可见性逻辑:当账户后端未配置(isMisconfigured)时隐藏 Account 分页;若当前选中分页不可见则回退到第一个可见分页或 General。
- Account 分页:根据 isLoading / isSignedIn / isPaid 状态机切换 UI;展示订阅计划卡片(价格/折扣价/月度积分额度)、剩余积分进度条、Top-off 充值输入,触发登录/登出/订阅/管理订阅/购买积分等账户动作(全部转发给 AccountService)。
- General 分页:通知开关(写 AppNotifications.isEnabled 并按需 requestAuthorization)与隐私遥测开关(写 Telemetry.isEnabled,提示需重启);二者都是 UserDefaults 布尔偏好。
- Models 分页:从 ModelCatalog(image/video/audio 三类)拉取模型列表,提供搜索过滤与每个模型的启用/禁用开关(写 ModelPreferences,本质是一个 disabledModelIds 集合)。
- Agent 分页:管理用户自带的 Anthropic API Key(SecureField + 掩码显示 + 存/删 macOS Keychain),以及本地 MCP HTTP 服务器(127.0.0.1:19789)的运行状态指示与开关。
- Storage 分页:显示并清理磁盘缓存(预览/波形/缩略图)、显示并清理设备端媒体搜索索引(embeddings)、显示并移除已下载的本地 SigLIP 模型;字节大小用 ByteCountFormatter 展示,清理操作在后台 Task 中跑。
- 提供可复用子组件 SettingsToggleRow(标题+副标题+开关)供各分页统一样式。

**核心类型**:
- `SettingsTab` (enum) — 设置分页枚举(account/general/models/agent/storage),提供 label 与 SF Symbol 名;CaseIterable 驱动侧边栏。Rust/前端可直接复刻为字符串枚举。
- `SettingsView` (struct) — 设置窗口根视图:HStack(侧边栏 220pt + 详情区);含分页可见性过滤逻辑与初始分页注入。
- `SettingsWindowController` (class) — AppKit 单例窗口控制器(@MainActor NSWindowController.shared),创建并托管设置窗口(暗色、磨砂、无标题栏、可拖拽、frameAutosaveName=PalmierProSettings-v2),show(tab:) 可定位到指定分页并强制刷新(.id(UUID()))。纯 macOS 窗口管理,Tauri 下用独立 WebviewWindow 替代。
- `SettingsToggleRow` (struct) — 通用开关行(标题+副标题+右侧 switch),被通知/隐私分页复用。纯展示组件。
- `AccountPane` (struct) — 账户分页视图;消费 AccountService.shared,渲染订阅/积分/计划卡片,触发计费动作。本地状态仅 topOffDollars(默认 20)。
- `AgentPane` (struct) — Agent 分页;管理 Anthropic Key(经 AnthropicKeychain 存取 Keychain,掩码=36 个圆点+末 4 位)与 MCP 服务器开关(经 AppState.setMCPEnabled)。
- `ModelsPane` (struct) — 模型分页;从 ModelCatalog 取 image/video/audio 列表,按 displayName 做不区分大小写的子串搜索,每行开关读写 ModelPreferences。
- `StoragePane` (struct) — 存储分页;聚合三处磁盘占用(预览缓存、搜索索引、本地模型),提供清理/移除按钮,后台计算字节数并刷新。
- `NotificationsPane / PrivacyPane` (struct) — General 分页的两块:系统通知开关、匿名崩溃遥测开关(改后提示重启)。仅读写 UserDefaults 并联动 AppNotifications/Telemetry。

**核心算法/逻辑(供 Rust 复刻)**:
- 【订阅计划与积分的纯数值规则,务必一比一复刻】预算积分 budgetCredits = (plan.monthlyBudgetCredits ?? 0) + (user.purchasedCredits ?? 0);已花费 spentCredits = user.spentCreditsThisPeriod ?? 0;剩余 remainingCredits = max(0, budgetCredits - spentCredits);hasCredits = remaining > 0。进度比例 remaining = budget>0 ? min(1.0, left/budget) : 0,其中 left = max(0, budget - spent)。
- 【积分进度条配色阈值】按剩余比例 r:r < 0.05 → 红色;0.05 ≤ r < 0.25 → 橙色;否则 → 主题强调色。这是 CreditSummaryView.barColor 的精确分段。
- 【计划卡片价格显示】有效月价 effectiveMonthlyPriceUsd = hasDiscount ? discountedMonthlyPriceUsd! : monthlyPriceUsd;hasDiscount 当且仅当 discountedMonthlyPriceUsd 存在且 < monthlyPriceUsd(此时原价加删除线展示)。所有价格是整数美元。
- 【Top-off 充值换算与校验(关键)】积分 = max(0, dollars) * 100(即 1 美元=100 积分);合法区间 isValid = dollars ∈ [TopOffLimits.minDollars, TopOffLimits.maxDollars]。注意源码中两处常量不一致:TopOffLimits 定义为 min=5/max=1000,但 AccountPane 文案硬编码显示 "$\(min)–$\(max)"。校验在 UI(isValid)与 AccountService.buyCredits 双重进行;buyCredits 还有重入保护(isBuyingCredits 为真时直接返回)。复刻时以常量 5..1000 为准。
- 【账户状态机(AccountService)】isSignedIn = (!isMisconfigured && authState==.authenticated);tier = account.user.tier ?? .none;isPaid = tier != .none。UI 分三态:isLoading→"Loading…";isSignedIn 且 isPaid→订阅区+积分区;isSignedIn 未付费→订阅引导(有计划则显示 Pro/Max 卡片,否则显示两个升级按钮);未登录→"Sign in with Google"。lastError 非空时底部红字展示。
- 【订阅周期日期格式化】currentPeriodEnd 是以毫秒计的 Unix 时间戳(Double);转 Date = Date(timeIntervalSince1970: endMs/1000),再以 abbreviated 日期、省略时间格式化。cancelAtPeriodEnd==true 时额外橙字提示 "Cancels <date>";积分卡显示 "Resets <date>"。
- 【账户/计费云调用映射(需在 Rust 后端重建为对自有服务的调用)】登录=Clerk OAuth(provider .google,redirect palmier://callback);provision=convex.mutation "users:upsertFromAuth"(失败重试 3 次,间隔 500ms);账户数据=convex.subscribe "account:get";计划列表=convex.subscribe "billing:listPlans";订阅结账=convex.action "billing:createCheckoutSession"{tier};充值结账=convex.action "billing:createTopOffCheckoutSession"{dollars: Double};管理订阅=convex.action "billing:createPortalSession";反馈=convex.action "feedback:send"。
- 【打开计费 URL 的安全白名单(务必复刻)】openInBrowser 仅允许 https 且 host ∈ {checkout.stripe.com, billing.stripe.com},否则置 lastError="Refused to open untrusted URL." 不打开。这是防 open-redirect 的硬校验。
- 【Anthropic API Key 的存取与掩码】保存:trim 后非空才存,存入 Keychain(service=bundleId,account="anthropic-api-key",可访问性 kSecAttrAccessibleAfterFirstUnlock,先 SecItemUpdate,errSecItemNotFound 时 SecItemAdd);读取:DEBUG 下优先读环境变量 ANTHROPIC_API_KEY(trim 非空),否则读 Keychain(读出后 trim,空则视为无)。掩码 mask(key):若 key.count>4 → 36 个 U+2022 圆点 + 末尾 4 位明文;否则 32 个圆点。存/删后都发 NotificationCenter 通知 .anthropicAPIKeyChanged 让 AgentService 重建客户端。UI 中只要草稿 trim 非空就显示 Save 按钮,否则若已有 key 显示删除(垃圾桶)按钮。
- 【模型启用偏好(ModelPreferences)】数据结构=一个 disabledModelIds: Set<String>,持久化到 UserDefaults 键 "disabledModelIds"(存为字符串数组)。isEnabled(id)= !contains(id);setEnabled(id,true)=remove,(id,false)=insert,每次改动立即 persist。即默认全部启用,只记录被关掉的。
- 【模型搜索过滤】query trim+lowercase 后,对每类(image/video/audio)的 displayName.lowercased().contains(q) 过滤;q 为空则不过滤;过滤后空的 section 整段隐藏。模型目录来自 convex.subscribe "models:list"(ModelCatalog),未加载时显示 "Loading models…"。
- 【通知开关(AppNotifications)】偏好键 "io.palmier.pro.notifications.enabled",缺省视为 true(object==nil→true)。开启时调用 configure():仅当运行于 .app 包内(bundleURL 后缀==app 且 bundleId 含".")才 requestAuthorization([.alert,.sound]) 并设代理。生成完成通知文案:count>1→"<count> <type>s are ready in Palmier Pro.";否则有名字→"<name> is ready." 无名字→"Your <type> is ready."。点击通知回调会 reveal 对应资产。
- 【隐私遥测开关(Telemetry)】偏好键 "io.palmier.pro.telemetry.enabled",缺省 true。enabledForCurrentLaunch 在启动时快照一次;PrivacyPane.didChange = (当前开关值 != enabledForCurrentLaunch),为真时显示"需重启"提示(因为 Sentry 只在启动时按快照初始化一次)。Telemetry.start() 仅当本次启动启用且 DSN 非空才 SentrySDK.start(sendDefaultPii=false, tracesSampleRate=0.1, appHangTimeout=8s 等)。
- 【MCP 服务器开关(MCPService/AppState)】偏好键 "io.palmier.pro.mcp.enabled",缺省 true。固定端口 19789,绑 127.0.0.1。setMCPEnabled(true)→若未运行则 startMCPService(创建 MCPHTTPServer,注册 ToolDefinitions.all 工具与两个资源 palmier://models/{video,image});false→stop。UI 圆点:运行=绿,停=灰;运行时显示 "Running on 127.0.0.1:19789"。
- 【存储:磁盘缓存清理】缓存目录根=~/Library/Caches/PalmierPro;StoragePane 聚合两个 DiskCache 实例 [ImageVideoGenerator.cache("ImageVideos"), MediaVisualCache.diskCache("MediaVisualCache")]。size()=递归累加目录下所有文件 .fileSize;clear()=删除目录内所有顶层条目(保留目录本身)。显示路径把 $HOME 替换为 "~"。清理在 Task.detached 中执行,期间显示 "Clearing…",完成后刷新。Clear 按钮在清理中或字节为 0 时禁用。
- 【存储:搜索索引(EmbeddingStore)二进制格式 — 需精确复刻】文件后缀 .embed,路径 ~/Library/Caches/<subsystem>/Embeddings/<key>.embed。布局:magic(8 字节 ASCII "PALMEMB1")+ UInt32 小端 headerLen + header(JSON: model, modelVersion, samplerVersion, dim, count)+ count 行,每行 = 3 个 Float64(time, shotStart, shotEnd,共 24 字节)紧跟 dim 个 Float16 向量值(每行字节数 rowBytes = 3*8 + dim*2)。读出时把 Float16 转 Float32 存入扁平 count×dim 数组。文件总长必须恰好 = 8+4+headerLen+count*rowBytes,否则视为 corrupt。整数全部小端、无对齐(loadUnaligned)。写入用 .atomic。
- 【存储:缓存键(EmbeddingStore.key)】身份串 = "<文件路径>|<修改时间 timeIntervalSince1970(Double)>|<文件字节数>",对其 UTF8 取 SHA256,十六进制小写,取前 32 字符作为 key。索引时效性 isCurrent = header 的 model+modelVersion+samplerVersion 三者全部匹配当前值。
- 【存储:清理索引/移除模型的级联】Clear index = SearchIndexCoordinator.clearIndexGlobally():先 resetAll(取消所有项目正在进行的索引、清空内存中 loadedIndexes/failedIds),再 EmbeddingStore.clearAll()(删整个 Embeddings 目录),再 sweepAll()(重新入队)。Remove model = VisualModelLoader.remove():resetAll + 删除 ~/Application Support/PalmierPro/Models 整个目录 + 状态置 notInstalled。媒体搜索开关 setEnabled 写 SearchIndexConfig.enabled(键 "searchIndexEnabled",缺省 true),开→prepare()+sweepAll,关→cancelAll+释放 embedder。
- 【存储:本地模型下载与校验(ModelDownloader)】安装目录 ~/Application Support/PalmierPro/Models/<model>-v<version>/{ImageEncoder.mlmodelc, TextEncoder.mlmodelc, tokenizer/, spec.json}。下载 3 个 zip(image/text 编码器 + tokenizer),逐个用流式 SHA256(1MiB 分块)对比 manifest 中的期望 sha256,不符抛 checksumMismatch;用 /usr/bin/ditto -x -k 解压,每个 zip 必须恰好一个顶层条目;.mlpackage 用 CoreML 编译成 .mlmodelc,tokenizer zip 直接搬。进度=按各文件字节数加权的 0…1。manifest 固定:siglip2-base-patch16-256,version 1,embeddingDim 768,imageSize 256,contextLength 64;托管地址 huggingface.co/palmier-io/siglip2-base-coreml。视觉匹配余弦下限 visualMatchCosineFloor=0.05。

**苹果框架使用**:
- SwiftUI [high] — 全部 5 个设置分页与子组件的声明式 UI、@Observable 状态绑定、Toggle/SecureField/TextField/ProgressView 等控件、ScrollView 与磨砂材质 .ultraThinMaterial。
- AppKit [medium] — SettingsWindowController 用 NSWindow + NSHostingController 托管一个独立暗色无标题栏可拖拽窗口(frameAutosaveName、darkAqua、fullSizeContentView);NSWorkspace.shared.open 打开外部 URL(Anthropic 控制台、Stripe 结账)。
- Security (Keychain Services) [medium] — KeychainStore 用 kSecClassGenericPassword + kSecAttrAccessibleAfterFirstUnlock 存取用户自带的 Anthropic API Key。
- UserNotifications [low] — AppNotifications 申请通知权限并在生成完成时投递本地系统通知,点击回调跳转到对应资产。
- Foundation (UserDefaults/ByteCountFormatter/FileManager) [none] — 各类布尔偏好持久化(通知/遥测/MCP/搜索索引启用、禁用模型 ID 列表)、缓存目录字节统计与清理、字节大小本地化展示。
- CryptoKit (SHA256) [none] — 经 EmbeddingStore(缓存键)与 ModelDownloader(下载校验)间接关联,Storage 分页通过它们工作。
- CoreML [high] — 经 ModelDownloader.MLModel.compileModel 与 VisualEmbedder 间接关联(本地 SigLIP 推理),Storage 分页只负责展示大小与删除文件。

**闭源云**:是,且程度较深(但集中在 Account/Models 两块)。Account 分页经 AccountService 通过 ClerkKit(身份认证,Google OAuth)+ ConvexMobile(实时后端)访问闭源云:provision(users:upsertFromAuth)、account:get、billing:listPlans、billing:createCheckoutSession/createTopOffCheckoutSession/createPortalSession、feedback:send,结账最终跳转 Stripe(checkout/billing.stripe.com,有 host 白名单)。Models 分页的 ModelCatalog 也经 Convex 订阅 models:list 拉取可用生成模型清单。Privacy 分页的遥测经 Sentry(第三方崩溃云)。Agent 分页保存的是用户自带的 Anthropic API Key,Key 本身只存本地 Keychain,但其用途是直连 api.anthropic.com 做生成式 AI 聊天(由 AgentService/AnthropicClient 使用,不在本模块发起网络请求)。Storage 分页本身不触云,但其管理的 SigLIP 模型下载自 huggingface.co(公开权重,非生成式云)。General 的通知、Models 的开关、Storage 的清理逻辑均为纯本地。

**移植策略**:整个 Settings 模块的 UI 必须在 React/TypeScript 中重建(SwiftUI/AppKit 无法移植);把它作为 Tauri 的一个独立设置窗口(WebviewWindow)。其中可直接移到 Rust core 的是少量纯逻辑与持久化:(1) 偏好读写——把 UserDefaults 的各布尔键(notifications/telemetry/mcp/searchIndex enabled,缺省全为 true)与 disabledModelIds 集合,统一落到一个 Rust 端 settings.json 或 Tauri Store;模型启用规则保持'默认启用、只记录禁用集'语义。(2) 积分/计费纯数值规则(budget=plan额度+已购、remaining=max(0,budget-spent)、进度比 min(1,left/budget)、配色阈值 0.05/0.25、Top-off 美元×100=积分、合法区间 5..1000、有效价=折扣价优先)——这些是可单测的纯函数,直接在 Rust 复刻并暴露给前端。(3) 字节统计/缓存清理——用 std::fs 递归累加文件大小、删除目录内顶层条目即可一比一复刻 DiskCache;路径根改为跨平台缓存目录(Tauri app_cache_dir)。(4) EmbeddingStore 二进制格式(magic PALMEMB1 + u32-LE headerLen + JSON header + 行:3×f64 + dim×f16,小端无对齐,总长校验,Float16 转 f32)与缓存键(path|mtime|size 的 SHA256 取前 32 hex)——可在 Rust 用 byteorder + half + sha2 精确重写,Storage 分页只读其大小、整体删除目录。(5) ModelDownloader 的下载-SHA256校验-解压-安装流程可在 Rust 重写(reqwest + sha2 流式 + zip 解压),但 .mlpackage→.mlmodelc 的 CoreML 编译与设备端 SigLIP 推理是 Apple 专有,跨平台需改用 ONNX Runtime/candle 加载等价 SigLIP 权重,属另一模块的工作。需要 cloud-rebuild 的部分:账户/登录/订阅/积分购买/反馈——Clerk+Convex+Stripe 的闭源栈要替换为 OpenTake 自有的后端(自建 auth + 计费 + 模型目录接口),前端只改调用端点,UI 状态机(三态:加载/已登录付费/已登录未付费/未登录,以及 lastError 红字)与 Stripe host 白名单等行为照搬。Anthropic Key 的本地存储用 keyring crate(跨平台 Keychain/Credential Manager/libsecret)替代 Security.framework,掩码规则(>4 位时 36 点+末4位,否则 32 点)与变更后发事件通知 Agent 重连的行为照搬。MCP 服务器开关在 Tauri 下指向 Rust 实现的 MCP server(端口/绑 127.0.0.1 行为照搬,端口 19789 可沿用)。通知改用 tauri-plugin-notification;遥测可选 Sentry Rust SDK 或置空,'改后需重启'的提示因为是启动时快照初始化,可照搬该 UX。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/SettingsView.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/AccountPane.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/AgentPane.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/StoragePane.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/ModelsPane.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Settings/PrivacyPane.swift

## Help  ·  `ui` → **ui-rebuild**

**职责**:
- 渲染快捷键速查表:把硬编码的 7 组(Playback/Tools/Editing/Timeline/File/Edit/View)快捷键按两列布局展示,左列前 4 组、右列其余 3 组
- 渲染 MCP 接入向导:动态拼接服务器地址 http://127.0.0.1:19789/mcp,为 4 种 MCP 客户端生成对应的 CLI 命令 / JSON 配置 / 深链接,并提供复制按钮与可折叠的手动配置说明
- Cursor 一键安装:把 {type:http,url} 配置做 JSON 序列化→Base64→URL 百分号编码,拼成 cursor:// 深链接并用 NSWorkspace 打开
- Claude Desktop 一键安装:打开 App bundle 内置的 palmier-pro.mcpb 文件
- 渲染反馈表单:多行描述(上限 10000 字符)、未登录时的可选邮箱、是否允许回访的勾选、截图缩略图预览与开关、环境信息提示;提交成功后切换为致谢视图
- 捕获主窗口截图作为反馈附件:用 AppKit 把当前主窗口 contentView 离屏渲染为 PNG,超过 1920px 时按比例缩小
- 通过 NSWindowController 单例管理 Help 窗口与 Feedback 窗口的创建、定位、深色外观与显示
- 把剪贴板复制(NSPasteboard)、提交反馈(经 AccountService→Convex 云)等副作用封装在小型按钮/表单组件里

**核心类型**:
- `HelpTab` (enum) — Help 窗口的 Tab 枚举,仅两个 case:shortcuts(图标 keyboard)与 mcp(图标 network)。CaseIterable+Identifiable,驱动侧边栏列表
- `HelpView` (struct) — Help 主界面 SwiftUI View:左侧 220pt 固定宽侧边栏 + 右侧 detail 区,detail 根据 selectedTab 切换 ShortcutsPane 或 MCPInstructionsPane。最小尺寸 820x520
- `HelpWindowController` (class) — @MainActor NSWindowController 单例(shared),用 NSHostingController 承载 HelpView,管理无边框深色玻璃窗口;show(tab:) 用 .id(UUID()) 强制重建视图以切到指定 Tab
- `ShortcutsPane` (struct) — 快捷键速查表 View,核心是 static let allShortcuts 这份硬编码数据(7 组,每组若干 (按键, 描述) 元组),以及把它切成左右两列的 leftColumn/rightColumn
- `ShortcutGroup` (struct) — 快捷键分组数据模型:title:String + shortcuts:[(String,String)]
- `MCPInstructionsPane` (struct) — MCP 接入向导 View:所有连接字符串/JSON/深链接都是基于 MCPService.port 计算的 computed property;含 Overview/Server URL/Cursor/Claude Desktop/Claude Code/Codex 六个 section
- `FeedbackView` (struct) — 反馈表单 View:管理 message/email/includeScreenshot/mayContact/isSending/errorText/didSend 等本地 @State;canSubmit 校验(非空且≤10000 字符);submit() 异步调用 AccountService.sendFeedback
- `FeedbackWindowController` (class) — @MainActor NSWindowController 单例,show(prefill:) 时先于窗口成为 key 之前捕获主窗口截图(避免把反馈窗自身拍进去),再构建 FeedbackView
- `FeedbackScreenshot` (enum) — @MainActor 无实例的工具命名空间,captureMainWindow() 把主窗口离屏渲染为 PNG 并按需缩小;依赖 AppKit 的 cacheDisplay/NSBitmapImageRep/CGContext

**核心算法/逻辑(供 Rust 复刻)**:
- 【快捷键数据是静态硬编码,不是从配置/实时绑定读取的】ShortcutsPane.allShortcuts 共 7 组,顺序固定:1) Playback: Space=Play/Pause, ←=Step Backward, →=Step Forward, Shift+←=Skip Backward, Shift+→=Skip Forward;2) Tools: V=Selection Tool, C=Razor Tool;3) Editing: Cmd+K=Split at Playhead, [或Q=Trim Start to Playhead, ]或W=Trim End to Playhead, Backspace=Delete, Shift+Backspace=Ripple Delete, Opt+Drag=Duplicate Clip;4) Timeline: Shift+Drag Ruler=Select Range, Drag Range Edge=Adjust Range, I=Mark Range Start, O=Mark Range End, Opt+Scroll=Zoom to Cursor, Pinch=Zoom to Cursor, Cmd+Scroll=Scroll Horizontally;5) File: Cmd+N=New, Cmd+O=Open, Cmd+S=Save, Cmd+Shift+S=Save As, Cmd+I=Import Media, Cmd+E=Export;6) Edit: Cmd+Z=Undo, Cmd+Shift+Z=Redo, Cmd+X=Cut, Cmd+C=Copy, Cmd+V=Paste, Cmd+A=Select All;7) View: Cmd+F=Full Screen, `=Maximize Focused Panel, Cmd+Scroll=Zoom Preview to Cursor, Esc=Deselect & Reset Tool。注意:这只是一份给人看的速查表,真正的快捷键行为实现不在本模块——它是上游编辑器键位逻辑的权威清单,复刻时应据此核对 Rust/前端实际键绑定。
- 【快捷键两列分配规则】leftColumn = allShortcuts.prefix(4)(即前 4 组 Playback/Tools/Editing/Timeline),rightColumn = allShortcuts.dropFirst(4)(即后 3 组 File/Edit/View)。两列各自 VStack,组间距 20,组内行距 6,按键列固定宽 118pt 左对齐等宽字体,描述列 fixedSize 不换行。
- 【MCP 端点拼接规则,单位/常量精确】serverURL = "http://127.0.0.1:\(MCPService.port)",其中 MCPService.port = 19789(UInt16 常量);mcpEndpoint = serverURL + "/mcp"。即固定本地回环地址 http://127.0.0.1:19789/mcp。所有客户端配置都从这一个端点派生。
- 【Claude Code 命令】claude mcp add --transport http palmier-pro {mcpEndpoint}。【Codex 命令】codex mcp add palmier-pro --url {mcpEndpoint}。【Cursor JSON】mcpServers.palmier-pro = {type:http, url:mcpEndpoint},说明放进 ~/.cursor/mcp.json。【Claude Desktop JSON】用 npx -y mcp-remote {mcpEndpoint} --allow-http --transport http-only 作为 command/args(因 Claude Desktop 不支持直连 http transport,需 mcp-remote 桥接)。
- 【Cursor 深链接生成算法,需一比一复刻】config = ["type":"http", "url": mcpEndpoint];步骤:(1) JSONSerialization.data(config, options:[.sortedKeys]) 得到键名按字典序排序的 JSON 字节;(2) data.base64EncodedString();(3) 对 base64 字符串再做 addingPercentEncoding(withAllowedCharacters:.urlQueryAllowed) 百分号编码;(4) 拼成 URL: cursor://anysphere.cursor-deeplink/mcp/install?name=palmier-pro&config={encoded}。任一步失败返回 nil(按钮点击则什么都不做)。复刻要点:必须 sortedKeys 保证确定性,且 base64 之后还要再 URL-encode 一层。
- 【Claude Desktop 一键安装】openClaudeDesktopBundle():取 Bundle.main.resourceURL,拼接子路径 "palmier-pro.mcpb";仅当 FileManager 确认该文件存在时,用 NSWorkspace.shared.open 打开(交给系统注册的 .mcpb 处理器/Claude Desktop)。文件不存在则静默不动作。
- 【反馈表单提交校验规则】maxMessageLen = 10000;trimmedMessage = message 去首尾空白与换行;canSubmit = (!isSending) && (!trimmedMessage.isEmpty) && (message.count <= 10000)。注意非空判断用 trim 后的文本,但长度上限判断用未 trim 的原始 message.count(边界细节:纯空白也算长度)。
- 【是否有回信邮箱 hasReplyEmail】若已登录(account.isSignedIn)则取决于 account.account?.user.email 是否非 nil;若未登录则取决于 trimmedEmail 非空。该值决定『允许回访』勾选框是否可用(disabled 当无邮箱),且无邮箱时即使勾选,提交时 mayContact 也被强制改为 false(submit 里 mayContact: hasReplyEmail ? mayContact : false)。
- 【提交流程 submit()】guard canSubmit;清空 errorText;isSending=true;启动 @MainActor Task,defer 里复位 isSending;计算 attachedScreenshot = (includeScreenshot ? screenshot : nil)?.base64EncodedString()(即用户关掉开关就不带截图);await AccountService.shared.sendFeedback(message: trimmedMessage, email: trimmedEmail 为空则 nil, mayContact: 上述强制规则, screenshotPngBase64: attachedScreenshot, appVersion, osVersion);成功 didSend=true(切到致谢视图),失败把 error.localizedDescription 写入 errorText 显示为红字。
- 【环境信息采集】appVersion = "{CFBundleShortVersionString} ({CFBundleVersion})"(任一缺失用 "?");osVersion = ProcessInfo.processInfo.operatingSystemVersion 拼成 "major.minor.patch"。这两个值随每次反馈一并上报。
- 【致谢文案分支 successDetailText】replyAddr = 登录邮箱 ?? (trimmedEmail 为空则 nil 否则 trimmedEmail);若 replyAddr 存在且 mayContact:『...may reach out at {replyAddr}』;若 replyAddr 存在但不允许:『...won't email you, as requested』;若无邮箱:『...Add an email next time...』。
- 【主窗口截图算法 captureMainWindow(),需用 FFmpeg 体系外的方案复刻】候选窗口选择顺序:NSApp.keyWindow ?? NSApp.mainWindow ?? 第一个 (isVisible 且 title 不以 "Send feedback" 开头) 的窗口;取其 contentView;用 view.bitmapImageRepForCachingDisplay(in: bounds) + view.cacheDisplay(in:to:) 做离屏位图缓存;representation(using:.png) 得 PNG。关键时序:FeedbackWindowController.show() 必须在反馈窗口成为 key 之前先截图,否则会把反馈窗自身拍进去。
- 【截图缩放规则 downscaledIfNeeded】maxDimension = 1920;若宽和高都 ≤1920 直接返回原 PNG;否则 scale = min(1920/width, 1920/height),newW=Int(width*scale)、newH=Int(height*scale);用 CGContext(8 位/分量, premultipliedLast, sRGB 或源色彩空间, interpolationQuality=.high)把 cgImage 重绘到新尺寸,再转回 PNG。任一步失败均回退返回原始 PNG(降级而非报错)。
- 【窗口外观】Help 与 Feedback 窗口都是:深色 darkAqua 外观、半透明(backgroundColor = base.withAlphaComponent(0.4)、isOpaque=false)、标题隐藏、titlebar 透明、可拖背景移动、fullSizeContentView。Feedback 额外 isReleasedWhenClosed=false。show 时 .id(UUID()) 强制重建保证状态重置。

**苹果框架使用**:
- SwiftUI [high] — 全部三个面板与反馈表单的视图层(View/ScrollView/VStack/HStack/Toggle/TextEditor/TextField/ProgressView、@State/@Bindable、withAnimation 等)
- AppKit [high] — NSWindowController/NSWindow/NSHostingController 管理无边框深色窗口;NSWorkspace.shared.open 打开 cursor:// 深链接与 .mcpb 文件;NSPasteboard 复制文本;NSImage 展示截图缩略图;NSAppearance(darkAqua);NSApp.activate/keyWindow/mainWindow
- AppKit(离屏渲染子集) [medium] — FeedbackScreenshot 截图:NSView.bitmapImageRepForCachingDisplay + cacheDisplay、NSBitmapImageRep、CGContext/CGImage/CGColorSpace 做 PNG 编码与缩放
- Foundation [none] — JSONSerialization 生成 Cursor 深链接配置;Base64/百分号编码;Bundle.main info plist 读取版本号与资源路径;ProcessInfo 取 OS 版本;FileManager 检查 .mcpb;Task.sleep 控制复制按钮提示
- Combine [low] — 经 AccountService.shared 的 @Observable/@Bindable 间接订阅登录状态(isSignedIn、邮箱)

**闭源云**:是。反馈提交是闭源云触点:FeedbackView.submit() → AccountService.sendFeedback(...) → convex.action(\"feedback:send\", with: args)(args 含 message/mayContact/appVersion/osVersion 以及可选 email、screenshotPngBase64)。底层走 ConvexMobile 客户端连接 Convex 部署(BackendConfig.convexDeploymentURL),身份用 ClerkKit/ClerkConvex。若 convex 未配置则抛错『Backend not configured.』。注意:这里的 Convex 仅作反馈收集后端,本调用本身不触达生成式 AI 云(gener AI 模型调用在 Generation/* 模块)。另一处网络相关是 MCP server,但那是本机 127.0.0.1:19789 的本地 HTTP server,不是闭源云。截图/快捷键/MCP 向导本身都无网络请求。

**移植策略**:整体在 OpenTake 用 React/TypeScript 重建为帮助面板(可做成应用内 Modal/独立窗口,Tauri 多窗口或单窗口路由皆可),Rust 侧基本不需要承载逻辑。分块方案:(1) 快捷键面板——把这份硬编码清单直接搬成 TS 常量(建议做成 i18n 资源),按相同两列分配(前4组/后3组)渲染;务必让它与前端真实键位绑定保持一致,它是上游键位的权威来源。(2) MCP 向导——端口常量改由 Rust core 暴露(本地 MCP/HTTP server 端口,可仍用 19789 或改为可配置),前端用同样算法拼 4 种客户端配置;Cursor 深链接生成需一比一复刻『JSON(sortedKeys)→base64→urlQueryAllowed 百分号编码→cursor://anysphere.cursor-deeplink/mcp/install?name=palmier-pro&config=』,可在 TS 用 JSON.stringify(按 key 排序)+btoa+encodeURIComponent 实现;打开深链接/.mcpb 用 Tauri 的 shell/opener 插件替代 NSWorkspace;Claude Desktop 的 .mcpb 包需在新打包流程里产出或改为纯 JSON 手动配置指引。(3) 复制按钮用 navigator.clipboard 或 Tauri clipboard 插件,1.4s 后复位提示的行为照搬。(4) 反馈表单——UI 用 React 重写,校验规则(≤10000 字符、trim 非空、无邮箱强制 mayContact=false、关开关不带截图)逐条复刻;提交后端要在 OpenTake 自建(自有 HTTP 端点/邮件/工单),不要复用 Convex『feedback:send』这个闭源云 action——属 cloud-rebuild 的子项。(5) 截图附件——用 Tauri 的窗口/屏幕截图能力或前端 canvas 抓取替代 AppKit cacheDisplay;保留『提交前先截图、排除反馈窗自身、>1920px 等比缩小、失败降级返回原图』的规则。窗口外观(深色半透明无边框)用前端样式 + Tauri 窗口装饰配置近似还原。

**关键文件**:Sources/PalmierPro/Help/HelpView.swift、Sources/PalmierPro/Help/ShortcutsPane.swift、Sources/PalmierPro/Help/MCPInstructionsPane.swift、Sources/PalmierPro/Help/FeedbackView.swift、Sources/PalmierPro/Help/FeedbackScreenshot.swift

## App  ·  `infra` → **needs-replacement**

**职责**:
- 进程入口与启动序列编排(main.swift):按固定顺序 Log.bootstrap → Telemetry.start(Sentry) → BundledFonts.register → AccountService.shared.configure(Clerk+Convex) → ModelCatalog.shared.configure;设置 NSInitialToolTipDelay=10;创建 NSApplication 并安装 AppDelegate 与主菜单后 app.run()
- 应用代理(AppDelegate):applicationDidFinishLaunching 中设激活策略=.regular、激活 App、初始化 Sparkle Updater、显示主页窗口、配置通知、启动 MCP;禁止打开无标题文件;无可见窗口时重新打开则回主页;响应链上提供 showSettings/showKeyboardShortcuts/showMCPInstructions/showFeedback/showTutorial 五个菜单动作
- 全局应用状态与工程生命周期(AppState,@Observable @MainActor 单例):持有当前 activeProject 与 mcpService;新建工程(NSSavePanel→VideoProject→保存→注册)、从面板/路径/样例打开工程、主页与编辑器窗口互斥切换(切主页前若文档已改则先 autosave)
- MCP 服务开关(AppState + MCPService):依据 UserDefaults 偏好启用/停用本地 MCP HTTP 服务(端口 19789),把当前工程的 EditorViewModel 以弱引用闭包注入 ToolExecutor
- 系统通知(AppNotifications):仅在 .app bundle 内启用,请求 alert+sound 授权;在生成完成时投递本地通知;用户点击通知后,通过 assetId/projectPath 在已打开工程中定位资产并在媒体面板高亮显示
- 主菜单与编辑快捷键(MainMenuBuilder):构建 App/File/Edit/View/Help 五个菜单,定义全部键盘快捷键(剪辑/裁剪/删除/布局/面板/播放/导出/导入),通过 @objc 协议 EditorActions 经响应链派发到当前 EditorViewModel
- 自动更新(Updater + UpdateBadgeView):封装 Sparkle SPUStandardUpdaterController,发现更新时在 UI 上显示玻璃质感更新徽章,支持检查更新与忽略
- 更新日志(ChangelogStore):从 bundle 内 JSON 读取 changelog,仅在真正的版本号变化(非全新安装)时弹出 What's New 浮层

**核心类型**:
- `AppState` (class) — @Observable @MainActor 全局单例。工程生命周期与窗口编排的中枢:activeProject、mcpService 状态机;createNewProject/openProject/openSample/openProjectFromPanel;showHome/showEditor/revealGeneratedAssetFromNotification;startMCPService/stopMCPService/setMCPEnabled。是 App 层对外的主要状态对象。
- `AppDelegate` (class) — NSApplicationDelegate。负责启动后激活、初始化 Updater、显示主页、配置通知、启动 MCP;并承载从主菜单经响应链触发的 Settings/Shortcuts/MCP/Feedback/Tutorial 动作。
- `MainMenuBuilder` (enum) — 无实例的菜单工厂。buildMenu() 构建整套 NSMenu 及全部快捷键绑定,是 macOS 原生菜单与编辑命令的唯一定义处。
- `EditorActions` (protocol) — @MainActor @objc 协议,声明所有经 AppKit 响应链派发到 EditorViewModel 的编辑命令(splitAtPlayhead/trimStartToPlayhead/trimEndToPlayhead/deleteSelectedClips/importMedia/playPause/step/skip/showExport/toggle*Panel/toggleMaximizePanel/setLayout*)。
- `AppNotifications` (enum) — @MainActor。UserNotifications 封装:授权、投递'生成完成'通知、点击回调定位资产。内部 AppNotificationDelegate 实现 UNUserNotificationCenterDelegate。
- `Updater` (class) — @Observable @MainActor。Sparkle 适配器,持有 SPUStandardUpdaterController,暴露 updateAvailable/updateVersion 供 UI 绑定,实现 SPUUpdaterDelegate 回调。
- `ChangelogStore` (class) — @Observable @MainActor 单例。读取 bundle 内 changelog.json,基于 lastSeenVersion 与当前版本对比决定是否展示 What's New。包含 ChangelogFeed/ChangelogEntry/ChangelogSection 三个 Decodable 数据模型。
- `MCPService` (class) — (被 App 层引用)@Observable @MainActor。本地 MCP HTTP 服务器(端口 19789),把 EditorViewModel 注入 ToolExecutor,注册 tools 与 resources(视频/图片模型目录)。是 Agent/MCP 对外编辑能力的入口。
- `VideoProject` (class) — (被 App 层引用)NSDocument 子类,定义 .palmier 工程包的读写格式与窗口装配。是工程持久化与编辑器视图模型的宿主。

**核心算法/逻辑(供 Rust 复刻)**:
- 【启动序列(main.swift,顺序敏感,Rust 中需保持等价拓扑顺序)】1) Log.bootstrap();2) Telemetry.start()(Sentry,可被偏好关闭且 DSN 为空时跳过);3) BundledFonts.register();4) AccountService.shared.configure()(初始化 Clerk+Convex,异步等待恢复缓存会话);5) ModelCatalog.shared.configure();6) UserDefaults 'NSInitialToolTipDelay'=10;7) 创建 NSApplication,装 AppDelegate 与 MainMenuBuilder.buildMenu(),app.run()。注意:第10行注释写'2s→0.01s'但实际写入值为 10(疑似 bug/单位含混,复刻时按实际值或重新定义为用户可调的 tooltip 延迟常量)。
- 【启动后(AppDelegate.applicationDidFinishLaunching)】设置 setActivationPolicy(.regular) 并 activate(ignoringOtherApps:true)(从 CLI 启动而非 .app 时必需,Rust/Tauri 下等价为前台化主窗口);_ = Updater.shared 触发懒加载启动 Sparkle;HomeWindowController 显示主页;AppNotifications.configure();AppState.shared.startMCPService()。applicationShouldOpenUntitledFile=false(从不自动建空文档);applicationShouldHandleReopen:无可见窗口(flag==false)时调用 showHome()。
- 【MCP 服务开关状态机(AppState)】startMCPService:若 mcpService 已存在则直接返回(幂等);若 MCPService.isEnabledPreference 为 false 则记录日志并不启动;否则用 editorProvider 闭包(弱引用 self,返回 activeProject?.editorViewModel)构造 MCPService 并 start()。stopMCPService:stop() 后置 nil。setMCPEnabled(enabled):先写 UserDefaults 偏好,再据此 start 或 stop。偏好键 'io.palmier.pro.mcp.enabled',缺省视为 true(首次默认开启)。
- 【主页/编辑器窗口互斥切换(AppState.showHome)】若无 activeProject:直接显示主页窗口并返回。否则定义 presentHome 闭包:有 fileURL 时 ProjectRegistry.register(url)→把该工程所有 windowControllers 的窗口 orderOut(隐藏)→若 activeProject 仍是该工程则置 nil→显示主页。关键边界:若 project.isDocumentEdited 为真,则先 project.autosave(withImplicitCancellability:false){完成后回主线程 presentHome()};否则直接 presentHome()。即'切回主页前必先保存脏文档'。showEditor(for:):设 activeProject、隐藏主页窗口、project.showWindows()。
- 【从通知定位生成资产(AppState.revealGeneratedAssetFromNotification(assetId, projectURL))】1) NSApp.activate;2) notificationTargetProject 解析目标工程:优先用 projectURL 在已打开文档中按'标准化路径字符串相等'(sameFile:比较 standardizedFileURL.path)匹配;否则用 assetId 在各工程的 mediaAssets 中查找包含该 id 的工程;再否则回退到当前 activeProject。3) 若解析不到工程:当 activeProject==nil 时显示主页,然后返回。4) 命中工程后:设 activeProject、隐藏主页、showWindows、首个窗口 makeKeyAndOrderFront。5) 若给了 assetId 且能在 editorViewModel.mediaAssets 找到对应资产:打开媒体面板(mediaPanelVisible=true)、清除最大化面板、焦点设为 .media、selectMediaAsset(asset)、并把 mediaPanelRevealAssetId 设为该 id(供 UI 滚动高亮)。
- 【新建工程(AppState.createNewProject)】用 NSSavePanel:allowedContentTypes=[io.palmier.project 的 UTType]、默认名 'Untitled Project'、目录 ~/Documents/Palmier Pro、标题 'New Project'。用户确认后:创建 VideoProject()→设 fileURL/fileType→makeWindowControllers→showWindows→NSDocumentController.addDocument→save(to:url, ofType:typeIdentifier, for:.saveOperation){成功后 ProjectRegistry.register(url)}。打开工程(openProject)用 VideoProject(contentsOf:ofType:),失败弹 NSAlert;openSample 先查 SampleProjectService 缓存,缺失则 materialize(带进度回调)再打开,且 register=false。ProjectOpenOptions.startTutorial 为真时下一 runloop 调 editor.tour.start。
- 【工程文件格式(VideoProject,Rust 工程格式需一比一复刻)】.palmier 是一个文件包(UTType io.palmier.project,符合 .package;扩展名 'palmier')。包内固定子文件:project.json=Timeline(JSON,缺失则抛 fileReadCorruptFile),media.json=MediaManifest(JSON,解码失败抛 fileReadCorruptFile),generation-log.json=GenerationLog(JSON,可选,解码失败仅忽略用 try?),thumbnail.jpg=JPEG 缩略图,media/=媒体资源目录(以 .immediate 整目录打包),以及聊天会话目录(ChatSessionStore.dirName,内含每会话一个 <UUID>.json,仅保存非空消息会话)。全部用 Foundation JSONEncoder/JSONDecoder 默认设置(无自定义键策略/日期策略)。autosavesInPlace=true。
- 【保存快照机制(VideoProject.captureSaveSnapshot)】save() 先记录 url 的 contentModificationDate 到 fileModificationDate,再 captureSaveSnapshot() 把 editorViewModel 的 timeline/mediaManifest/generationLog 各自 JSONEncode 成 Data、生成 thumbnail、编码聊天会话文件,置 snapshotPreparedForFileWrapper=true,然后 super.save。fileWrapper(ofType:) 若快照未准备且非主线程则报错(快照必须在主线程预先采集,避免非 Sendable 数据跨线程);随后用 replaceChild 把各 Data 写入 packageWrapper 的对应子文件并返回。replaceChild:若同名子文件已存在先 removeFileWrapper 再 addFileWrapper,并设 preferredFilename。
- 【缩略图生成(VideoProject.captureThumbnail)】带 cachedThumbnail 缓存。遍历所有 video 轨道的 clip,解析 mediaRef→URL:若为 image,用 ImageEncoder 生成 maxPixelSize=640 缩略图并以 JPEG quality=0.7 编码;若为 video,用 AVURLAsset+AVAssetImageGenerator(maximumSize 320x180、appliesPreferredTrackTransform=true),取帧时间 CMTime(value=clip.trimStartFrame, timescale=timeline.fps)(即帧号/帧率=秒),异步取帧并以 DispatchSemaphore 等待最长 5 秒,超时则取消继续下一个;成功则 NSBitmapImageRep→JPEG(compressionFactor 0.7)。取首个成功的帧即返回。Rust 中改用 FFmpeg 按帧号 seek 取一帧编码 JPEG。
- 【打开后媒体恢复(VideoProject.restoreAssetsFromManifest)】遍历 mediaManifest.entries:用 resolver.expectedURL(for:id) 解析期望路径(解析不到记 missing 并跳过);构造 MediaAsset(entry, resolvedURL) 并加入 mediaAssets;若文件实际不存在记 missing 并跳过;存在则 restored++,并据资产类型预生成:audio/video→波形,video→视频缩略图,image→图片缩略图,且异步 loadMetadata。最后记录 restored/missing 统计。无 generationLog 时调 seedGenerationLogFromAssets() 从资产回填生成日志。
- 【工程注册表(ProjectRegistry,最近工程列表)】@Observable @MainActor 单例,持久化为 ~/Documents/Palmier Pro/project-registry.json([ProjectEntry] 的 JSON 数组,原子写)。ProjectEntry{id:UUID,url,createdDate,lastOpenedDate;name=去扩展名文件名;isAccessible=文件是否存在}。sortedEntries 按 lastOpenedDate 降序。register(url):URL 标准化后,若已存在则更新 lastOpenedDate=now,否则追加新条目。remove 按标准化 URL 删除。delete:异步 trashIfPresent(移入废纸篓,文件不存在视为成功)成功后再 remove。updateURL(from,to):VideoProject 改名(fileURL setter)时同步更新条目 url 与 lastOpenedDate。加载用 actor 异步读盘,加载期间 mutate 入队 pendingMutations,加载完成后回放。
- 【菜单与编辑快捷键绑定(MainMenuBuilder,Rust/TS 前端需复刻同套快捷键语义)】App 菜单:About;Check for Updates…(target=Updater.shared,#selector checkForUpdates);Settings… ⌘,;Quit ⌘Q。File:New ⌘N;Open… ⌘O;Save ⌘S;Save As… ⇧⌘S;Import Media… ⌘I;Export… ⌘E。Edit:Undo ⌘Z;Redo ⇧⌘Z(用字符串 selector 'undo:'/'redo:' 走 UndoManager 响应链);Cut/Copy/Paste/Select All;Split at Playhead ⌘K;Trim Start to Playhead = 单键 'q'(无修饰键);Trim End to Playhead = 单键 'w'(无修饰键);Delete = Backspace(\u{8},无修饰)。View:Media Panel ⌘0;Inspector ⌥⌘0;Agent Panel ⌥⌘A;Maximize Focused Panel = 反引号 '`'(无修饰);Layout 子菜单 Default ⌘1 / Media ⌘2 / Vertical ⌘3;Enter Full Screen ⌘F。Help:Tutorial;Keyboard Shortcuts ⌘?(即 ⇧⌘/);MCP Instructions;Send Feedback…。所有自定义动作通过 @objc 协议 EditorActions 经响应链派发,首响应者(EditorViewModel/控制器)实现具体逻辑。
- 【时间线核心常量(Constants.swift,Rust 复刻编辑算法的关键数值,单位敏感)】Defaults:pixelsPerFrame=4.0;图片默认时长 5.0s、TTS 音频 10.0s、音乐 60.0s、文字 3.0s(秒→帧需乘 fps);aspectTolerance=0.02。Snap(吸附):thresholdPixels=8.0,stickyMultiplier=1.5,playheadMultiplier=1.5(吸附判定基于像素阈值,黏滞与播放头吸附各有 1.5 倍系数)。Zoom:min=0.05,floor=0.0001,max=40.0,scrollSensitivity=0.04,magnifySensitivity=1.5,panSpeed=5.0,fitAllBuffer=3.0。Layout:trackHeight=50,rulerHeight=24,trackHeaderWidth=100,insertThreshold=10,dragThreshold=3。TimelineAutoScroll:edgeZoneWidth=56,maxZoneFraction=0.5,minStep=4,maxStep=28,interval=1/60s。TrackSize:min32/max200/handle6。Trim:handleWidth=4,clipCornerRadius=3。另含 gcd(a,b) 用于宽高比化简。这些常量定义了拖拽阈值、吸附距离、缩放范围、默认片段时长等编辑行为,需在 Rust core 中原样保留。
- 【更新检查(Updater)】仅当运行于 .app bundle(bundleURL.pathExtension=='app')且 Info.plist 含 SUFeedURL 时才创建 SPUStandardUpdaterController(startingUpdater:true)并 checkForUpdateInformation。SPUUpdaterDelegate 回调:didFindValidUpdate→updateAvailable=true、updateVersion=item.displayVersionString;updaterDidNotFindUpdate→清空。dismissUpdate 清空可用状态(仅 UI 层忽略,不影响后台)。
- 【更新日志判定(ChangelogStore.checkForWhatsNew)】从 bundle resourceURL 下两个候选路径(Changelog/changelog.json 或 PalmierPro_PalmierPro.bundle/Changelog/changelog.json)读取并解码 ChangelogFeed。读取 lastSeenVersion 偏好,随即写入当前版本(CFBundleShortVersionString)。仅当 lastSeen 非空且不等于当前版本时,pending=feed.entries 中 version==当前版本者(即:全新安装不弹、版本无变化不弹,只有真正升级才弹)。
- 【系统通知(AppNotifications)】仅在 canUseUserNotifications(bundleURL.pathExtension=='app' 且 bundleIdentifier 含 '.')时生效。configure:设 delegate、若偏好启用则请求 [.alert,.sound] 授权(偏好键 'io.palmier.pro.notifications.enabled' 缺省 true)。generationComplete(assetId,projectURL,assetName,assetType:ClipType,count):title='Generation complete',body 规则:count>1 时 'N <type>s are ready in Palmier Pro.';否则 assetName 去空白后为空则 'Your <type> is ready.' 否则 '<name> is ready.';userInfo 放 assetId 与可选 projectPath。前台展示策略:启用则 [.banner,.sound] 否则空。点击回调读取 userInfo 调 AppState.revealGeneratedAssetFromNotification。

**苹果框架使用**:
- AppKit [high] — 整个应用外壳:NSApplication 运行循环与激活策略、NSApplicationDelegate 生命周期、NSDocument/NSDocumentController 工程文档与最近文档、NSMenu 主菜单与快捷键、NSSavePanel/NSOpenPanel 新建/打开面板、NSAlert 错误弹窗、NSWindow 配置(暗色外观、透明标题栏、标题栏 SwiftUI 配件)、NSHostingController 桥接 SwiftUI、NSBitmapImageRep 编码缩略图。
- SwiftUI [high] — UpdateBadgeView 更新徽章(glassEffect 玻璃质感、transition)、@Observable/@Bindable 状态绑定、EditorView/ExportView/TourOverlay 等以 sheet/overlay 组合、tint 主题色。App 层主要做装配与少量徽章 UI。
- AVFoundation [medium] — VideoProject.captureThumbnail 用 AVURLAsset 打开视频、AVAssetImageGenerator 在指定帧时间异步取一帧作为工程缩略图(maximumSize 320x180、appliesPreferredTrackTransform)。
- CoreMedia [low] — 缩略图取帧的时间表示:CMTime(value: trimStartFrame, timescale: fps) 实现帧号→时间换算(秒=帧/帧率)。
- UserNotifications [medium] — AppNotifications 本地通知:请求 alert+sound 授权、投递'生成完成'通知、前台展示策略、点击回调携带 assetId/projectPath 定位资产。仅在 .app bundle 内启用。
- Foundation [none] — UserDefaults 偏好(MCP/通知/遥测开关、lastSeenVersion、tooltip 延迟)、JSON 编解码工程文件、FileWrapper 组织 .palmier 文件包、FileManager(目录创建/废纸篓/存在性)、DispatchSemaphore 同步等待取帧。
- UniformTypeIdentifiers [low] — 以 UTType('io.palmier.project') 定义工程包类型,用于保存/打开面板的 allowedContentTypes 与文档类型识别(符合 .package)。

**闭源云**:是。App 层在 main.swift 启动序列中直接调用 AccountService.shared.configure() 与 ModelCatalog.shared.configure(),并用 Telemetry.start() 启动 Sentry。AccountService 依赖闭源云栈 ClerkKit(身份认证,含 Google OAuth signInWithOAuth)、ClerkConvex/ConvexMobile(Convex 后端,ConvexClientWithAuth + ClerkConvexAuthProvider),后端地址来自 Info.plist 的 PalmierClerkPublishableKey/PalmierConvexDeploymentURL/PalmierConvexHttpURL;生成式 AI(视频/图片/音频生成)经 Convex 后端进行。Updater 通过 Sparkle 从 SUFeedURL 拉取更新 appcast(网络)。Sentry 上报遥测到其 SaaS。App 层本身只是触发与编排这些云访问;具体网络请求在 AccountService/PalmierClient/ModelCatalog/GenerationBackend 中。

**移植策略**:App 层是装配/编排层,需在 Tauri2 中按职责拆分重建,而非逐行直译:
1) 启动序列(main.swift)→ 映射为 Rust main() + tauri::Builder.setup():按等价顺序初始化 日志(tracing,替 Log)、遥测(可选 sentry-rust 或自建,替 Sentry)、字体(前端 CSS/Tauri 资源,替 BundledFonts)、账户(见第6点)、模型目录;NSInitialToolTipDelay 改为前端 tooltip 延迟常量(注意上游写的是 10 而非注释所称 0.01s,按需重定义)。
2) AppDelegate/NSApplication 生命周期 → Tauri 应用事件(RunEvent::Ready/Reopen)与窗口管理;'无可见窗口时回主页'对应 Tauri 多窗口显隐逻辑。
3) AppState 工程生命周期与主页↔编辑器互斥切换 → 放入 Rust core 的应用状态 + Tauri 命令;'切主页前若文档脏则先 autosave'的规则必须保留为显式状态机。新建/打开用 Tauri dialog 插件替 NSSavePanel/NSOpenPanel。
4) 工程文件格式(VideoProject/.palmier 包)→ 这是必须一比一复刻的核心:在 Rust 中以同名子文件(project.json=Timeline、media.json=MediaManifest、generation-log.json、thumbnail.jpg、media/ 目录、chat 会话目录)用 serde_json(默认无键/日期转换策略)读写;NSDocument 的 autosave/快照在主线程采集的约束改为 Rust 同步序列化即可,无线程隔离问题。缩略图改用 FFmpeg 按帧 seek(seconds=frame/fps)取一帧编码 JPEG,替 AVAssetImageGenerator。
5) ProjectRegistry(最近工程)→ 直接移植为 Rust struct + serde_json 原子写到 ~/Documents/Palmier Pro/project-registry.json;标准化路径比较用 std::fs::canonicalize;删除走系统回收站可用 trash crate 替 trashItem。
6) 账户/生成云(Clerk+Convex)→ cloud-rebuild:Clerk/ConvexMobile 无 Rust SDK,需用 reqwest 直连 Clerk REST/OAuth 与 Convex HTTP API(或自建后端代理),OAuth 走系统浏览器回调;若产品决定保留同一闭源后端,需自行实现 ClerkConvexAuthProvider 等价的 token 注入。
7) 主菜单与快捷键(MainMenuBuilder/EditorActions)→ Tauri 原生菜单(tauri::menu)+ 前端/Rust 命令分发替代 AppKit 响应链;务必原样保留全部快捷键语义(Split ⌘K、Trim Start=Q、Trim End=W、Delete=⌫、Layout ⌘1/2/3、Media ⌘0、Inspector ⌥⌘0、Agent ⌥⌘A、Maximize=`、Undo/Redo 等)。
8) 通知(AppNotifications)→ tauri notification 插件 + 自定义 deep-link/事件携带 assetId/projectPath 复刻'点击定位资产'流程;'生成完成'文案规则(count>1 复数、空名回退)按原样实现。
9) Sparkle 自动更新 → tauri-plugin-updater(从 updater endpoint 拉取 manifest);UpdateBadgeView 在前端用 React 复刻。
10) ChangelogStore 'What's New' → 纯逻辑可直译:读 bundle 内 changelog.json,lastSeenVersion 比较,仅版本真正变化时弹窗(serde + Tauri 存储)。
11) Constants.swift 全部时间线/吸附/缩放/默认时长常量 → 放入 Rust core 原值保留,前端读取同一份,避免数值漂移。
MCPService 端口 19789 与工具注册属 Agent/MCP 子系统,App 层只做开关(偏好键缺省 true),Rust 中用 axum/hyper + rmcp(MCP Rust SDK)重建,保留'已运行则幂等、偏好关闭不启动'语义。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/App/AppState.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/App/MainMenu.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/App/AppNotifications.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/App/main.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Project/VideoProject.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Utilities/Constants.swift

## Utilities  ·  `infra` → **needs-replacement**

**职责**:
- 有界并发控制:AsyncSemaphore 以 actor 实现公平 FIFO 信号量,带取消支持,用于限制波形/缩略图解码的并发数(波形=2,图片缩略图=4)
- 磁盘缓存目录:DiskCache 在 ~/Library/Caches/PalmierPro/<name> 下创建命名目录,提供递归求大小与清空
- 图片编码/下采样:ImageEncoder 将图片压到最长边<=1568px、<=3.5MB 的 JPEG,供 Agent 节省 token;带 path+size+mtime 三元组内存缓存(上限 32 条);也提供 CGImage->JPEG、元数据读取、缩略图生成
- JSON 数字规整:roundJSONFloatingPointNumbers 递归把任意 JSON 结构里的浮点数四舍五入到指定小数位,非有限值转 null,布尔不动
- 凭据存储:KeychainStore 用 Security 框架在 Keychain(kSecClassGenericPassword)读写删字符串(如 Anthropic API key)
- 日志与崩溃处理:Log 提供 12 个分类 os.Logger;同时镜像到 stderr 并转发到 Sentry 遥测;CrashHandler 安装未捕获异常处理器与信号处理器,把崩溃栈写入 crash.log
- 时间换算:帧<->秒<->时间码(HH:MM:SS:FF)的纯函数换算,以及 Double 四舍五入到小数位扩展
- 全局常量:LayoutPreset 枚举与 Layout/Defaults/Snap/Zoom/TimelineAutoScroll/Trim/Project/TrackSize 常量组,定义 UI 尺寸、吸附/缩放参数、默认时长、工程文件名与存储路径,以及 gcd 函数
- 字体注册:BundledFonts 用 CoreText 注册随包字体并提供系统字体选择列表(过滤符号字体)

**核心类型**:
- `AsyncSemaphore` (actor) — 有界并发信号量。以 actor 序列化访问 permits 与 waiters 队列,实现带取消语义的 FIFO 异步信号量,用于限制共享系统资源(音视频解码器)的并发 fan-out。
- `ImageEncoder` (enum) — 图片下采样+JPEG 重编码器(命名空间式 enum)。把图片压缩到最长边1568px/3.5MB 以内供 Agent 使用,内置基于文件指纹的内存缓存。
- `ImageEncoder.FileStamp` (struct) — 图片缓存键:由 path+size+mtime 组成的 Hashable 三元组,用于在 Agent 每轮循环里避免重复读取与重编码同一图片。
- `DiskCache` (struct) — 命名磁盘缓存目录的轻量封装(Sendable)。负责在统一根目录下创建子目录、递归统计字节数、清空目录内容。
- `KeychainStore` (enum) — macOS Keychain 的字符串读写封装,用 service=bundleId、account 为键存取敏感凭据(如 API key),可见性 kSecAttrAccessibleAfterFirstUnlock。
- `Log` (enum) — 分类日志门面+崩溃处理入口。聚合 12 个 CategoryLog 分类、错误链格式化(detail)、crash.log URL,bootstrap() 在启动时安装崩溃处理器。
- `CategoryLog` (struct) — 单分类日志器。封装 os.Logger,提供 debug/info/notice/warning/error/fault 六级;同时镜像到 stderr 并把 warning 及以上转发到 Sentry 遥测。
- `CrashHandler` (enum) — 私有崩溃处理器。安装 NSSetUncaughtExceptionHandler 与 6 个致命信号(SEGV/ABRT/BUS/ILL/FPE/TRAP)的 async-signal-safe 处理器,把回溯写入 crash.log。
- `BundledFonts` (enum) — @MainActor 字体注册器。用 CoreText 把 Resources/Fonts 下的 ttf/otf 注册进进程,并提供供字体选择器使用的系统字体列表(过滤不可预览的符号字体)。
- `LayoutPreset` (enum) — 编辑器布局预设枚举(default/media/vertical),带 label 与 SF Symbol 图标名,用于驱动三种面板布局。
- `Layout / Defaults / Snap / Zoom / TimelineAutoScroll / Trim / Project / TrackSize` (enum) — 一组常量命名空间。定义 UI 尺寸、吸附阈值、缩放范围、默认媒体时长、工程文件名/扩展名/存储路径等魔法数字的单一来源。

**核心算法/逻辑(供 Rust 复刻)**:
- [AsyncSemaphore 公平信号量算法] 字段:permits:Int(初始化时 max(0,value)),nextWaiterID 自增计数器,waiters 为 (id, continuation) 数组充当 FIFO 队列。wait():若 permits>0 则 permits-=1 立即返回;否则先 Task.checkCancellation(),再分配自增 id,进入 withTaskCancellationHandler 包裹的 withCheckedThrowingContinuation——若此时已取消则直接 resume(throwing: CancellationError),否则把 (id,continuation) 入队等待;onCancel 闭包里 Task{ await cancelWaiter(id) }。signal():若队列非空,取出队首(removeFirst)并 resume()(唤醒最早等待者,permits 不变);否则 permits+=1。cancelWaiter(id):按 id 在队列中查找并移除该等待者,对其 continuation resume(throwing: CancellationError)。复刻要点:严格 FIFO;permits 永不为负;signal 唤醒等待者时不增 permits(把许可直接转交给被唤醒者);取消必须能从队列中精确摘除对应 waiter。Rust 可用 tokio::sync::Semaphore(已是公平 FIFO,acquire 支持 cancel-safe)直接替代,或用 Mutex<状态>+VecDeque<oneshot::Sender> 自实现。
- [图片编码主流程 ImageEncoder.encode] 输入文件 URL。先算 fileStamp(path+size+mtime);若 stamp 命中内存缓存直接返回。否则走两条路径取其一:(1) passthrough 直通:要求能 sniff 出 MIME(仅 png/jpeg/gif/webp)且 文件字节<=3_500_000 且 像素 max(w,h)<=1568,满足则把原文件字节(mmap 读取)原样返回、保留原 MIME;(2) 否则 downscaled:用 ImageIO 生成最长边=1568 的缩略图(自动应用 EXIF 方向),再依次以 JPEG 质量 [0.85,0.7,0.55,0.4] 编码,取第一个字节数<=3_500_000 的结果,MIME 固定 image/jpeg;四档都超限则返回 nil。成功结果写回缓存;写前若缓存条目数>=32 则整表清空(粗暴全清,非 LRU)。复刻要点:1568 与 3.5MB 两个阈值、质量四档序列、缓存满清空策略、直通仅限四种格式、像素判定用 max(宽,高)。Rust 用 image crate 解码+缩放(Lanczos3),mozjpeg/jpeg-encoder 编码并循环试质量;EXIF 方向需用 kamadak-exif 手动旋转(ImageIO 的 CreateThumbnailWithTransform 在 Rust 无等价,必须显式处理)。
- [图片元数据/缩略图 ImageEncoder] metadata(url, thumbnailMaxPixelSize?):用 CGImageSourceCopyPropertiesAtIndex 读 kCGImagePropertyPixelWidth/Height;若给了 thumbnailMaxPixelSize 则同时生成缩略图。makeThumbnail 选项:CreateThumbnailFromImageAlways=true、WithTransform=true(应用方向)、ShouldCacheImmediately=true、ThumbnailMaxPixelSize=给定值;imageSource 创建时 ShouldCache=false。encodeJPEG(CGImage,quality):用 CGImageDestination 以 LossyCompressionQuality 编码为 JPEG Data。
- [时间码格式化 formatTimecode] 输入 frame:Int、fps:Int,输出 'HH:MM:SS:FF'(冒号分隔,帧号而非毫秒)。fps<=0 时返回 '00:00:00:00'。取 absFrame=abs(frame);ff=absFrame%fps;totalSeconds=absFrame/fps(整除);ss=totalSeconds%60;mm=(totalSeconds/60)%60;hh=totalSeconds/3600;负 frame 前缀 '-'。twoDigit:仅当 0<=value<10 时补前导零,>=10 或负数原样输出(即只保证个位数补零,不截断大于两位的值如小时)。复刻要点:这是非 drop-frame 时间码,帧字段用整数取模 fps;小时不会被两位截断。
- [帧秒换算] frameToSeconds(frame,fps)= fps>0 ? Double(frame)/Double(fps) : 0。secondsToFrame(seconds,fps)= Int(seconds*Double(fps)),注意是向零截断(Int() 截断小数,不是四舍五入)——复刻必须用 truncate 而非 round,否则边界帧会差一。Double.rounded(toPlaces:)= 乘 10^places 后 .rounded()(就近,.5 向偶/远取决于平台默认 schoolbook)再除回。
- [JSON 浮点规整 roundJSONFloatingPointNumbers] 递归遍历 Any:字典则 mapValues 递归;数组则 map 递归;若是 NSNumber 且非布尔(用 CFGetTypeID==CFBooleanGetTypeID 判定布尔)且 objCType 为 'd' 或 'f'(double/float),取 doubleValue:有限值用 NSDecimalNumber 以 .plain 模式、scale=places 四舍五入返回 NSDecimalNumber;非有限(NaN/Inf)返回 NSNull()。其它类型原样返回。复刻要点:只规整浮点,整数 NSNumber 不动;布尔不能被当数字;NaN/Inf -> null。Rust 中对 serde_json::Value 递归,仅处理 Number 里的 f64,用 (x*10^p).round()/10^p,非有限置 Null。
- [Keychain 存取 KeychainStore] service 固定为 Bundle.main.bundleIdentifier(回退 'io.palmier.pro')。save:先构造 query(class=GenericPassword,service,account),attrs 含 ValueData=utf8 数据 + Accessible=AfterFirstUnlock;先 SecItemUpdate,若返回 errSecItemNotFound 则把 attrs 合并进 query 后 SecItemAdd(典型 upsert 模式)。load:query 加 ReturnData=true、MatchLimit=One;成功后把 Data 转 utf8 字符串并 trim 空白换行,空串视为 nil。delete:按 service+account 删除。复刻要点:upsert 语义、AfterFirstUnlock 可见性、读出后 trim 且空串当无。Rust 用 keyring crate(macOS 后端即 Security framework),跨平台时 Windows=Credential Manager、Linux=Secret Service。
- [崩溃处理 CrashHandler] install():创建 crash.log 父目录;以 O_WRONLY|O_CREAT|O_APPEND 模式 open 文件得 fd(失败为 -1);NSSetUncaughtExceptionHandler 注册 C 函数指针;对 SIGSEGV/SIGABRT/SIGBUS/SIGILL/SIGFPE/SIGTRAP 六个信号注册 signalHandler。未捕获异常处理器:写入时间戳、异常名、reason、callStackSymbols 到 fd。信号处理器(必须 async-signal-safe,只用 write/backtrace/backtrace_symbols_fd/fsync/raise):写 '*** FATAL SIGNAL ***' 头,用 backtrace 抓最多 64 帧并 backtrace_symbols_fd 写出,fsync,然后 signal(sig,SIG_DFL) 复位并 raise(sig) 重新触发以产生系统崩溃报告。复刻要点:信号处理里绝不能分配堆/加锁/用非 async-signal-safe 调用。Rust 用 libc::sigaction + backtrace crate(注意 backtrace 在信号处理中不完全 async-signal-safe,生产可改用第三方如 sentry-rust 的 native crash 或 minidump),异常对应到 Rust panic hook。
- [日志转发 CategoryLog] 六级方法 debug/info/notice/warning/error/fault。所有 notice 及以上都先 mirror 到 stderr(格式 '[category] LEVEL: msg\n'),再写 os.Logger(privacy:.public)。遥测转发规则:notice 仅当显式传入 telemetry 参数时发面包屑;warning 总是发 Telemetry.logWarning(默认用消息本身);error->logError;fault->logFault。复刻在 Rust 用 tracing + tracing-subscriber 分 target(category)分级别,遥测转发对接 sentry crate(见 closedCloudTouch)。
- [字体注册 BundledFonts.register] 幂等(registered 标志)。手动定位 Fonts 目录(不用 Bundle.module 以免资源缺失时 fatalError):DEBUG 下为 resourceURL/PalmierPro_PalmierPro.bundle/Fonts,RELEASE 为 resourceURL/Fonts;不存在则警告并跳过。递归枚举目录收集所有 .ttf/.otf;对每个文件用 CTFontManagerCreateFontDescriptorsFromURL 取家族名去重(变体字体一文件多 descriptor,用 Set 去重)。注册用 CTFontManagerRegisterFontURLs(基于 URL 而非 descriptor,避免变体字体被当重复)、scope=.process。families 排序缓存。systemFamiliesForPicker:用 NSFontManager.availableFontFamilies 过滤掉已随包家族,并标注每个是否 canPreviewText(family)。canPreviewText:取 12pt 字体的 coveredCharacterSet,要求覆盖 'A''a''1' 三个字符全部命中才算可预览(否则是符号/emoji/dingbat 字体)。复刻:Rust 用 fontdb/font-kit 加载随包字体目录;'能否预览' 用字符覆盖判断 'Aa1'。
- [常量数值规则(上层编辑行为依据)] Defaults.pixelsPerFrame=4.0(时间线每帧像素基准);imageDurationSeconds=5.0、audioTTSDurationSeconds=10.0、audioMusicDurationSeconds=60.0、textDurationSeconds=3.0(各类素材落到时间线的默认时长,单位秒,需 *fps 转帧);aspectTolerance=0.02(宽高比匹配容差)。Snap.thresholdPixels=8.0、stickyMultiplier=1.5、playheadMultiplier=1.5(吸附像素阈值及粘滞/播放头吸附的放大系数)。Zoom: min=0.05、floor=0.0001、max=40.0、scrollSensitivity=0.04、magnifySensitivity=1.5、panSpeed=5.0、fitAllBuffer=3.0(缩放范围与手势灵敏度)。TimelineAutoScroll: edgeZoneWidth=56、maxZoneFraction=0.5、minStep=4、maxStep=28、interval=1/60(拖拽到边缘自动滚动:边缘区宽、最大占比、步长范围、60fps 节拍)。Layout.insertThreshold=10、dragThreshold=3(插入判定与拖拽起始阈值,像素)。这些数值定义了吸附/缩放/自动滚动/拖拽的精确行为,Rust/前端复刻须逐一保留。
- [工程与存储常量 Project] fileExtension='palmier';typeIdentifier='io.palmier.project';registryFilename='project-registry.json';timelineFilename='project.json';manifestFilename='media.json';generationLogFilename='generation-log.json';thumbnailFilename='thumbnail.jpg';mediaDirectoryName='media';defaultProjectName='Untitled Project'。storageDirectory= ~/Documents/Palmier Pro(惰性创建)。这是工程包的目录结构与文件命名约定,复刻工程格式时必须沿用(或显式迁移)。gcd(a,b) 为欧几里得递归最大公约数,用于宽高比化简。

**苹果框架使用**:
- Foundation [low] — FileManager(目录创建/枚举/属性/缓存目录定位)、URL、Data(mmap 读取)、Date、NSNumber/NSDecimalNumber(JSON 数字规整)、NSError(错误链)、UserDefaults(经 Telemetry)、ProcessInfo
- ImageIO [medium] — ImageEncoder 全部图片能力:CGImageSourceCreateWithURL 解码、CopyPropertiesAtIndex 读宽高、CreateThumbnailAtIndex 生成带方向校正的缩略图、CGImageDestination 以指定质量编码 JPEG、CGImageSourceGetType 探测容器 UTI
- UniformTypeIdentifiers [low] — ImageEncoder 用 UTType.png/jpeg/gif/webp.identifier 把容器 UTI 映射到 MIME 字符串
- CoreText [medium] — BundledFonts 注册随包字体:CTFontManagerCreateFontDescriptorsFromURL 取家族名、CTFontManagerRegisterFontURLs(.process) 进程内注册、CTFontDescriptorCopyAttribute 读家族名
- AppKit [medium] — BundledFonts 系统字体列表:NSFontManager.availableFontFamilies、NSFont.coveredCharacterSet 判定可预览;LayoutPreset.icon 返回 SF Symbol 名(UI 层用)
- Security [low] — KeychainStore 用 SecItemAdd/Update/CopyMatching/Delete 在 macOS Keychain 存取字符串凭据(kSecClassGenericPassword,AfterFirstUnlock)
- os(os.Logger) [none] — CategoryLog 用 Logger(subsystem:category:) 做结构化分级日志,privacy:.public
- Darwin/POSIX [medium] — CrashHandler 用 open/write/fsync/strlen、backtrace/backtrace_symbols_fd、signal/raise 实现 async-signal-safe 崩溃落盘;NSSetUncaughtExceptionHandler 捕获 ObjC 异常

**闭源云**:无生成式 AI 云访问。但存在第三方遥测/崩溃上报云:Log 的 warning/error/fault 会转发到内部 Telemetry,而 Telemetry 直接 import Sentry 并通过 SentrySDK 把面包屑、消息、错误、性能 trace 以 0.1 采样率上报到由 Info.plist 中 SentryDSN 配置的 Sentry 服务端(可经 io.palmier.pro.telemetry.enabled 用户开关关闭,sendDefaultPii=false)。这是网络请求型外发,但目标是 Sentry(错误监控),非 Convex/Clerk/生成式 AI 云。本目录内其余文件(信号量/缓存/图片/Keychain/时间换算/常量/字体)均为纯本地,无任何网络访问。

**移植策略**:整体为基础设施,大部分可直接重写为 Rust。逐项:AsyncSemaphore->tokio::sync::Semaphore(已是公平 FIFO 且 acquire 取消安全)直接替代,无需自实现。DiskCache->std::fs + dirs crate 取缓存目录,递归求大小用 walkdir,direct-port。TimeFormatting(帧/秒/时间码、Double 取整)->纯算术 direct-port,务必保留 secondsToFrame 的截断(as i64/trunc 而非 round)与时间码小时不截断、非 drop-frame 取模规则。JSONNumberFormatting->对 serde_json::Value 递归,仅处理 f64,非有限置 Null,direct-port。Constants->全部常量搬到 Rust 常量或前端 TS 常量(吸附/缩放/自动滚动/像素每帧/默认时长这些定义编辑行为的数值必须逐一保真);gcd 用 num-integer 或自写。KeychainStore->keyring crate(macOS=Security、Windows=Credential Manager、Linux=Secret Service);保留 upsert 与读出 trim+空串当无的语义。Log->tracing+tracing-subscriber 分 target/level;stderr 镜像与文件落盘用 tracing layer。ImageEncoder->image crate 解码+Lanczos3 缩放、mozjpeg/jpeg-encoder 编码循环试质量[0.85,0.7,0.55,0.4]、阈值 1568px/3.5MB 保真;关键坑:ImageIO 自动应用 EXIF 方向(ThumbnailWithTransform),Rust 必须用 kamadak-exif 手动读方向并旋转,否则竖拍图会倒。BundledFonts->字体随包注册在 Tauri 里多由前端 CSS @font-face/系统处理,Rust 侧若需可用 fontdb/font-kit;'Aa1' 可预览判定可保留。CrashHandler->Rust 用 std::panic hook + sentry-rust(原生崩溃/minidump)替代;不要在 signal handler 里手写 backtrace(Rust 的 backtrace 在信号上下文不完全 async-signal-safe)。Telemetry 转发->对接 sentry crate(同一 DSN 体系),保留用户开关与 PII 关闭。判定 needs-replacement 而非 direct-port,因 ImageIO/CoreText/Keychain/Sentry/崩溃处理都需换成对应 Rust 生态库,且 EXIF 方向与信号安全两处有真实坑。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Utilities/Constants.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Utilities/ImageEncoder.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Utilities/TimeFormatting.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Utilities/AsyncSemaphore.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Utilities/Log.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Utilities/KeychainStore.swift

## UI  ·  `ui` → **ui-rebuild**

**职责**:
- 集中托管全部设计令牌:背景/边框/边框宽度/强调色/玻璃拟态/状态色/文本色/不透明度阶梯/轨道类型色/圆角/间距/字号/字重/字距/图标尺寸/组件尺寸/窗口尺寸/字幕约束/生成面板尺寸/媒体面板尺寸/阴影/动画时长(全部以 enum 命名空间 + static let 形式暴露,既给 SwiftUI 的 Color 也给 AppKit 的 NSColor)
- 提供 ClipType -> NSColor 的主题色映射扩展(themeColor),供时间线轨道/片段/缩略图按媒体类型着色
- 提供 View 扩展:.shadow(ShadowStyle) 阴影便捷修饰、panelHeaderBar() 统一面板头(高 28pt、raised 背景、底部 1pt 主边框)
- 定义 CapsuleButtonStyle 胶囊按钮样式(secondary/prominent 两种外观 + small/regular 两种尺寸 + 可选自定义填充),含悬停叠白与按下降透明的交互态
- 定义 GeneratingOverlay 生成中遮罩:标题文字(AI 银色渐变)+ 确定式假进度条,并叠加斜向白色微光扫光动画;支持 thumbnail/preview 两种尺寸与 reduce-motion 降级
- 定义 HoverHighlight 悬停高亮修饰符:为小图标按钮按 (isActive, isHovered) 四象限叠加不同强度的白色圆角背景,并用 contentShape 扩大命中区
- 定义 SidebarRowButton 侧栏行按钮:图标+文字+弹性占位的横向行,复用 hoverHighlight 表达选中/悬停
- 定义 MouseWheelHorizontalScroll 修饰符:鼠标悬停期间安装本地 NSEvent 滚轮监听,把非精确(普通滚轮)的纵向滚动量改写到横轴,使普通鼠标也能驱动横向 ScrollView;触控板精确事件原样放行

**核心类型**:
- `AppTheme` (enum) — 全局设计令牌根命名空间(纯静态、不可实例化)。内含嵌套枚举:Background/Border/BorderWidth/Accent/Glass/Status/Text/Opacity/TrackColor/Radius/Spacing/FontSize/FontWeight/Tracking/IconSize/ComponentSize/Window/Caption/GenerationPanel/MediaPanel/Anim,以及顶层 aiGradient/aiGradientDark 渐变与 ShadowStyle 结构和 Shadow 枚举。所有 UI 数值都必须从这里取(项目硬性规定),是整个前端的视觉单一真相源。
- `AppTheme.Caption` (enum) — 字幕相关的约束常量(本目录唯一带编辑业务语义的部分):defaultFontSize=48、minFontSize=12、maxFontSize=300、minPosition=0、maxPosition=1、centerSnapValue=0.5、centerSnapThreshold=0.02、defaultCenterY=0.9、defaultCenter=(0.5,0.9)、minDisplayDuration=0.7。被 CaptionTab、ToolExecutor+Captions、EditorViewModel+Captions 消费,决定字幕默认样式、归一化坐标钳制与吸附、最短分句时长。
- `AppTheme.ShadowStyle` (struct) — 阴影参数值类型(color/radius/x/y),配合 Shadow.sm/md/lg 预设与 View.shadow(_:) 扩展使用。
- `CapsuleButtonStyle` (struct) — 实现 SwiftUI ButtonStyle 的胶囊按钮样式。含 Variant(secondary/prominent)与 Size(small/regular)枚举及可选 fill。内部私有 Chrome 视图持有 @State hovered,负责字号/内边距/前景背景色推导与悬停/按下交互态;通过 ButtonStyle 扩展暴露 .capsule 便捷构造器。
- `GeneratingOverlay` (struct) — 生成中遮罩视图。含 Size(thumbnail/preview)枚举决定字号/间距/进度条宽高;持有 @State progress 驱动确定式假进度;读取 accessibilityReduceMotion 做降级。
- `ShimmerModifier` (struct) — 私有 ViewModifier,实现斜向白色高光带从左到右无限平移的微光(shimmer)效果,经 .screen 混合并用内容自身做 mask;支持 active 开关。通过私有 View.shimmering(active:) 暴露给 GeneratingOverlay。
- `HoverHighlight` (struct) — ViewModifier,按 (isActive, isHovered) 状态机给视图叠加不同不透明度的白色圆角背景并扩大命中区;通过 View.hoverHighlight(cornerRadius:isActive:) 暴露。
- `MouseWheelHorizontalScroll` (struct) — ViewModifier,管理一个本地 NSEvent 滚轮事件监听的生命周期(悬停安装/移出移除/消失移除),把普通鼠标的纵向滚轮事件改写为横向。通过 View.mouseWheelScrollsHorizontally() 暴露。
- `SidebarRowButton` (struct) — 侧栏导航行按钮视图(label/systemImage/isSelected/action),用 .plain 按钮样式 + hoverHighlight 表达选中态,被 SettingsView、HomeView 使用。

**核心算法/逻辑(供 Rust 复刻)**:
- 【设计令牌数值表(需在 Rust/前端原样复刻为 CSS 变量或常量)】背景:base=rgb(10,10,10)/surface=rgb(22,22,22)/raised=rgb(30,30,30)/prominent=rgb(44,44,44),placeholder=raised,previewCanvas=纯黑。边框(白色叠加 alpha):primary=0.16/subtle=0.12/divider=0.44。边框宽度:hairline=0.5/thin=1/medium=1.5/thick=2。强调:timecode=rgb(242,153,51)即(0.95,0.6,0.2),primary 暖白=(0.961,0.937,0.894),spotlight=(1.0,0.27,0.27),spotlightGradient 为三色左上→右下线性渐变[(1.0,0.34,0.30),(0.95,0.15,0.28),(1.0,0.48,0.22)]。状态错误色 error=rgb(229,79,79)。文本(白叠加 alpha):primary=1.0/secondary=0.80/tertiary=0.62/muted=0.34。不透明度阶梯:opaque=1,subtle=0.04,hint=0.06,faint=0.08,soft=0.10,muted=0.15,moderate=0.25,medium=0.35,strong=0.55,prominent=0.80。轨道色:video=rgb(0,145,194)/audio=rgb(88,168,34)/image=rgb(183,45,210)/text=rgb(183,45,210,与image相同)/lottie=rgb(224,168,0)。圆角:xs=3/xsSm=4/sm=6/md=10/mdLg=12/lg=14/xl=20。间距:xxs=2/xs=4/sm=6/smMd=8/md=10/mdLg=12/lg=14/lgXl=16/xl=20/xlXxl=24/xxl=28。字号:micro=8/xxs=9/xs=10/sm=11/smMd=12/md=13/mdLg=14/lg=15/xl=18/title1=22/title2=28/display=36。字距:tight=-0.5/normal=0/wide=1.5。图标尺寸:xxs=12/xs=14/sm=18/smMd=20/md=22/mdLg=24/lg=26/lgXl=28/xl=30。阴影:sm=(黑0.3,r=1,x=0,y=0.5)/md=(黑0.3,r=4,x=0,y=2)/lg=(黑0.25,r=24,x=0,y=8)。动画:hover=0.15s/transition=0.2s。
- 【派生尺寸公式(复刻时按公式而非写死)】Radius.concentric(outer,padding)=max(outer-padding,0)(同心圆角:子层圆角=外层圆角-内边距,下限 0)。MediaPanel.tabRailWidth=IconSize.lg + Spacing.sm*2 = 26 + 6*2 = 38。MediaPanel.contextRowHeight=IconSize.md=22。其余组件/窗口尺寸为常量:captionPreviewMaxHeight=150,captionPreviewMaxTextWidthRatio=0.9,toolImagePreviewMaxHeight=50,projectCardWidth=150,projectCardHeight=120,updateOverlayWidth=640;窗口 homeDefault=1200x1200/homeMin=760x480/projectDefault=1600x1000/projectMin=960x600/projectTitlebarTrailingWidth=280;GenerationPanel: mediaAreaMinHeight=120/loadingHeight=180/promptMinHeight=40/referenceTileWidth=80/referenceTileHeight=56。
- 【字幕约束与中心吸附规则(本目录唯一编辑语义,必须忠实复刻)】坐标系为归一化 [0,1] 的画面相对坐标。默认字号 48,允许范围 [12,300];默认中心 (0.5,0.9)(水平居中、底部上方一点);位置范围 [0,1]。中心吸附:对某一轴坐标 v(Double),若 abs(v - 0.5) < 0.02 则吸附为 0.5,否则保持 v;吸附后再钳制到 [0,1]。UI 中当 center.x==0.5 或 center.y==0.5 时分别绘制竖/横中心参考线(颜色用 timecode 橙 0.95,0.6,0.2 叠 0.80 不透明度)。最短字幕显示时长 minDisplayDuration=0.7 秒,作为 CaptionBuilder 分句的下限传入(分句不得短于 0.7s)。注意:这些是 UI 目录暴露的常量,真正的分句/换行/合并算法在 EditorViewModel+Captions 与 CaptionBuilder,不在本目录。
- 【GeneratingOverlay 确定式假进度算法】常量 progressDuration=45(秒),progressTarget=0.9。onAppear 时:若 reduceMotion 为真,直接令 progress=0.9(无动画);否则用 SwiftUI withAnimation(.easeOut(duration:45)) 把 progress 从 0 动画到 0.9。进度条用 GeometryReader 取容器宽 W,前景胶囊宽=W*progress。即:这是一个与真实进度无关的、单次 ease-out 缓动、45 秒内逼近 90% 后停住的伪进度条,纯视觉占位(真实完成由外部移除遮罩)。Size: preview -> 字号18/间距14/条宽160/条高4;thumbnail -> 字号10/间距8/条宽60/条高3。标题文字填充用 aiGradient(银色)。
- 【Shimmer 微光扫光算法】常量 duration=1.35s。仅当 active(=非 reduce-motion)时启用:在内容上叠加一个 LinearGradient(竖直方向,stops:[clear@0, white(0.42)@0.48, clear@1]),其宽度=容器宽*0.45,旋转 18 度,水平 offset = 容器宽 * phase。phase 初值 -1,onAppear 以 .linear(duration:1.35).repeatForever(autoreverses:false) 从 -1 动画到 2(即高光带从左侧画面外平移到右侧画面外,循环不反向)。整层用 .screen 混合模式叠加并以内容自身作为 mask(只在不透明像素处显示扫光)。
- 【CapsuleButtonStyle 外观/交互推导】尺寸:small -> fontSize=FontSize.xs(10)/水平内边距=Spacing.smMd(8)/垂直内边距=Spacing.xs(4);regular -> fontSize=FontSize.smMd(12)/水平=Spacing.lgXl(16)/垂直=Spacing.smMd(8)。前景:prominent -> 背景 base 色(深底字,即用 Background.base 当前景做反白胶囊上的深字),secondary -> Text.secondary。背景:prominent -> fill 优先,否则 Accent.primary 暖白;secondary -> Background.prominent(rgb44)。字重固定 .medium。交互态:悬停时叠加一层白色 opacity=faint(0.08)的胶囊;按下时整体 opacity=strong(0.55),未按下=1。命中形状为连续圆角胶囊;悬停态用 .easeOut(0.15) 过渡。
- 【HoverHighlight 四象限填充状态机】按 (isActive, isHovered):(true,true)=白 opacity muted(0.15);(true,false)=白 soft(0.10);(false,true)=白 faint(0.08);(false,false)=透明。背景与命中区都用连续圆角矩形(默认 cornerRadius=Radius.sm=6);isHovered 与 isActive 变化均以 .easeOut(0.15) 动画。用途:小图标按钮的可点性反馈与命中区扩大。
- 【MouseWheelHorizontalScroll 轴改写算法(平台相关,复刻见 portability)】生命周期:onHover(true) 安装监听(若已存在则跳过),onHover(false) 与 onDisappear 移除监听。监听 .scrollWheel 本地事件:若 event.hasPreciseScrollingDeltas 为真(触控板)则原样返回 event(放行);否则取 event.cgEvent.copy(),读取轴1(纵向)的三个字段——整型 scrollWheelEventDeltaAxis1(行数)、双精度 scrollWheelEventPointDeltaAxis1(点)、scrollWheelEventFixedPtDeltaAxis1(定点);把轴1 三字段清零,把这三个值分别写入轴2(横向)对应字段(DeltaAxis2/PointDeltaAxis2/FixedPtDeltaAxis2);用改写后的 cgEvent 重建 NSEvent 返回。效果:普通鼠标的纵向滚轮被整体搬到横轴,从而驱动 .horizontal ScrollView;触控板精确两指滚动不受影响。
- 【ClipType.themeColor 映射】video->TrackColor.video、audio->audio、image->image、text->text(与 image 同为紫 183,45,210)、lottie->lottie(金 224,168,0)。被时间线轨道头、片段渲染(ClipRenderer 会对该色做 blended(0.3 of white) 提亮再叠 0.85 alpha 作描边)、缩略图角标、预览角标统一使用——即媒体类型的视觉编码须全局一致。
- 【panelHeaderBar 复刻】统一面板头:宽度撑满,高度=Layout.panelHeaderHeight=28(该常量在 Utilities/Constants.swift,不在本目录),背景=Background.raised(rgb30),底部叠加一条高=BorderWidth.thin(1)、色=Border.primary(白 0.16)的分隔线。

**苹果框架使用**:
- SwiftUI [medium] — 全目录的声明式 UI 基座:定义 Color/LinearGradient 令牌、ButtonStyle(CapsuleButtonStyle)、ViewModifier(HoverHighlight/ShimmerModifier/MouseWheelHorizontalScroll)、GeneratingOverlay/SidebarRowButton 视图;使用 GeometryReader 测容器尺寸、withAnimation/.easeOut/.linear.repeatForever 做动画、accessibilityReduceMotion 做无障碍降级、blendMode(.screen)/mask 做微光合成、Capsule/RoundedRectangle(style:.continuous) 做形状。
- AppKit [medium] — AppTheme 用 NSColor 定义颜色令牌(同时桥接为 SwiftUI Color)、用 NSSize 定义窗口尺寸;MouseWheelScroll 用 NSEvent.addLocalMonitorForEvents(matching:.scrollWheel) 安装/移除全局本地滚轮事件监听,并用 NSEvent(cgEvent:) 重建事件。
- CoreGraphics [low] — MouseWheelScroll 通过 NSEvent.cgEvent 拿到底层 CGEvent,getInteger/getDoubleValueField 与 setInteger/setDoubleValueField 读写 CGEventField(scrollWheelEventDeltaAxis1/2、scrollWheelEventPointDeltaAxis1/2、scrollWheelEventFixedPtDeltaAxis1/2)以把纵向滚轮量改写到横轴;颜色令牌的下游用 .cgColor 给 CoreGraphics 绘制。

**闭源云**:无。本目录不引用 Convex/ConvexMobile/Clerk/ClerkKit,也不发起任何网络请求或访问生成式 AI 云。GeneratingOverlay 仅是一个与真实进度无关的本地伪进度+微光动画占位 UI;真实的生成任务调度与云访问在 Generation/Agent 等其它模块。

**移植策略**:本目录是 SwiftUI/AppKit 表现层,无法直接移植,需在 OpenTake 的 React/TypeScript 前端用等价方式重建;但其中的设计令牌与少量编辑常量必须忠实搬运。分项方案:(1) AppTheme 全套令牌 —— 直接转写为 CSS 自定义属性(:root 变量)+ 一个 TS 常量模块(tokens.ts)。颜色按上方精确 RGB/alpha 写死;派生公式(concentric=max(outer-padding,0)、tabRailWidth=26+6*2 等)在 TS 里以函数/计算保留,不要写死结果,便于令牌改动时联动。轨道类型色映射(ClipType->color)放进共享常量,前端时间线与 Rust 渲染若都需要可在 Rust 端再镜像一份同值常量,保证媒体类型视觉编码全局一致;ClipRenderer 对该色 blended(0.3 white)+0.85 alpha 的描边提亮规则一并复刻。(2) AppTheme.Caption 是真正的编辑语义,务必移到 Rust 核心的领域常量(字幕默认字号48、范围[12,300]、默认中心(0.5,0.9)、中心吸附阈值0.02→吸附到0.5再钳制[0,1]、最短显示0.7s),前端只做展示与滑块范围,吸附/钳制以 Rust 为准。(3) CapsuleButtonStyle/HoverHighlight/SidebarRowButton 是纯样式与 hover/press/active 状态机,用 React 组件 + CSS :hover/:active/[data-active] + 上述令牌重建,过渡统一用 --duration-hover:150ms ease-out;胶囊用 border-radius:9999px。(4) GeneratingOverlay:伪进度用单次 ease-out 在 45s 内 width 从 0→90% 的 CSS transition/JS 动画;微光扫光用一个 45% 宽、旋转18°、mix-blend-mode:screen、被内容 mask 的线性渐变条,1.35s linear infinite 从 translateX(-100%)→translateX(200%);prefers-reduced-motion 时关闭扫光并直接定格到 90%。注意按 web 规则优先动画 transform/opacity(扫光用 transform:translateX,避免动画 left)。(5) MouseWheelHorizontalScroll:Web 上更简单且更安全 —— 监听容器 wheel 事件,若 e.deltaMode!==0(非像素,近似普通鼠标“行”模式)或可用 e.shiftKey 约定,则 scrollLeft += deltaY 并 preventDefault;触控板的像素级 deltaX 原样放行。完全不需要 CGEvent/NSEvent 那套 macOS 私有事件改写,属于按平台重写而非移植底层 API。整体风险点仅在:连续圆角(SwiftUI .continuous superellipse)在 CSS 中只能近似为普通圆角;.screen 混合与 mask 行为需在目标浏览器核对。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/UI/AppTheme.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/UI/CapsuleButton.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/UI/GeneratingOverlay.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/UI/HoverHighlight.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/UI/MouseWheelScroll.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/UI/SidebarRowButton.swift

## Transcription  ·  `engine` → **needs-replacement**

**职责**:
- 从视频/音频文件用 AVAssetReader 解码出 16kHz/单声道/16-bit 小端 PCM 临时 .caf 文件(可选只截取 sourceRange 时间段)
- 选择最佳识别语言：优先用户传入 locale，否则按系统 preferredLanguages + 当前 locale 与设备支持列表做 语言码优先、地区码次之 的匹配
- 用 SpeechTranscriber + SpeechAnalyzer 做设备端流式识别，按需下载安装对应语种离线模型(AssetInventory)
- 把识别结果(每个 Result=一个端点切分出的语句段)解码为 TranscriptionResult：全文 text、逐段 segments(文本+起止秒)、逐词 words(去空白 token + 可空起止秒)
- 当只识别了某个 sourceRange 时，把所有时间戳整体加回 range.lowerBound 偏移，还原到源媒体时间轴
- 可选脏词过滤(censorProfanity → etiquetteReplacements)
- TranscriptCache：以文件身份哈希为键缓存『整文件』文字稿(内存上限4条、磁盘 JSON)，窗口请求通过过滤整文件结果服务；编辑文件会因 mtime/size 变化自然失效
- TranscriptSearch：对磁盘缓存的文字稿做大小写/变音符号不敏感、要求所有查询词全部命中的逐段精确搜索

**核心类型**:
- `Transcription` (enum) — 无实例的命名空间(纯静态方法集合)。核心入口：transcribeVideoAudio / transcribe / supportedLocales / matchLocale / bestSupportedLocale，以及私有的 extractAudioTrack(抽音频)与 decodeResults(解码)。
- `TranscriptionResult` (struct) — Codable/Sendable 的识别结果聚合：text(去首尾空白全文)、language(BCP-47)、words[]、segments[]。带 offsetting(by:) 方法对所有时间戳整体平移。是缓存与下游消费的标准数据契约。
- `TranscriptionSegment` (struct) — 一个『端点切分』出来的自然语句段：text(带模型给的标点与大小写)+ start/end(秒，非可空)。是字幕短语切分和口语搜索的基本单位。
- `TranscriptionWord` (struct) — 单个 token：text(已去首尾空白)+ start/end(秒，可空——某些 token 无音频时间范围)。用于词级时间戳、主讲轨判定的词计数。
- `TranscriptionError` (enum) — LocalizedError 错误枚举：unsupportedLocale / modelInstallFailed / decodeFailed(声明但未实际抛出) / audioExtractionFailed / analysisFailed，errorDescription 给英文用户文案。
- `TranscriptCache` (actor) — 单例 actor(shared)。整文件文字稿的内存(LRU-ish，上限4)+磁盘(Caches/io.palmier.pro/Transcripts/<key>.json)缓存。key 由 路径|mtime|size 做 SHA256 取前32位 hex。提供 nonisolated 的 cachedOnDisk / hasCachedOnDisk 供同步路径(搜索/索引判定)用。
- `TranscriptSearch` (enum) — 无状态命名空间。对一组 (assetID,url) 资产读取磁盘缓存文字稿，按段做关键词匹配，产出 Hit(assetID/start/end/text)。包含 terms(分词去边缘标点) 与 matches(全词命中、忽略大小写与变音符号)。
- `TranscriptSearch.Hit` (struct) — 一条口语搜索命中：assetID + start/end(源秒)+ 命中段全文 text。Equatable。

**核心算法/逻辑(供 Rust 复刻)**:
- 【时间单位总则】本模块内部一律用『源媒体秒(Double)』。帧/秒换算不在本模块发生，全部由调用方(EditorViewModel+Captions)用 timeline.fps 完成：seconds = frames / fps，frames = seconds * fps。复刻时务必保持本模块纯秒、不引入帧概念。
- 【音频抽取 extractAudioTrack】1) AVURLAsset 载入，loadTracks(.audio).first 取第一条音频轨；无音频轨→audioExtractionFailed。2) AVAssetReaderTrackOutput 强制输出格式：LinearPCM / 采样率 16000 / 单声道 / 16-bit / 非浮点 / 小端 / 交错。3) 若给了 range，设置 reader.timeRange = [CMTime(range.lowerBound, timescale 600), CMTime(range.upperBound, timescale 600)](注意是 端到端 的绝对时间区间，不是 start+duration)。4) 循环 copyNextSampleBuffer：对每个 sampleBuffer 取 FormatDescription→ASBD→AVAudioFormat，numSamples 作为帧数,frames==0 跳过；用 CMSampleBufferCopyPCMDataIntoAudioBufferList 拷进 AVAudioPCMBuffer 再写入 AVAudioFile。5) 输出文件首个 buffer 到来时才惰性创建,路径 = 临时目录/palmier-stt-<UUID>.caf,settings/commonFormat/interleaved 取自实际 buffer 的 format。6) reader.status==.failed→报错；一个样本都没写出(audioFile 仍为 nil)→audioExtractionFailed("No audio samples")。7) Rust+FFmpeg 等价：ffmpeg 解码音频轨为 PCM s16le、单声道、16000Hz；带 range 时用 -ss(lowerBound) -to(upperBound)(绝对秒，等价于 timeRange 端到端语义)。
- 【调用入口语义】transcribeVideoAudio(videoURL,...) = 先 extractAudioTrack(可带 range) → transcribe(临时caf) → 对结果 offsetting(by: range.lowerBound ?? 0) 还原源时间。transcribe(fileURL,...,sourceRange) 若带 sourceRange 则同样先抽取该段再识别再 offset；不带 range 时直接对整文件识别。注意：抽取阶段已经裁掉了 range 之前的音频，所以识别出的时间是从0开始的相对时间，必须 +lowerBound 偏移回去——这是 offsetting 的唯一用途。
- 【offsetting 规则】offset==0 直接返回自身(短路)。否则 words 的 start/end(可空,用 map 保持空值)、segments 的 start/end 全部 +offset；text 与 language 不变。
- 【语言匹配 matchLocale(candidates, supported)】按 candidates 顺序遍历：取 candidate 的 languageCode(如 "en"),在 supported 里筛出同语言码的集合 sameLang；若空则看下一个 candidate；否则在 sameLang 里优先返回 region 完全相同者(如 en-US 对 en-US),找不到就返回 sameLang.first。全部 candidate 都不匹配→nil。bestSupportedLocale = 用 (系统 preferredLanguages 映射成 Locale) ++ [当前 Locale] 作为 candidates 调 matchLocale。transcribe 选语言优先级：① preferredLocale 能匹配则用匹配结果(不是原样 preferredLocale) ② 否则 bestSupportedLocale ③ 都不行→unsupportedLocale(抛错)。
- 【识别流程 transcribe 主体】1) SpeechTranscriber(locale, transcriptionOptions: censorProfanity?[.etiquetteReplacements]:[], reportingOptions:[], attributeOptions:[.audioTimeRange])——必须开启 audioTimeRange 否则拿不到词级时间。2) AssetInventory.assetInstallationRequest(supporting:[transcriber]) 非 nil 则 downloadAndInstall(),失败→modelInstallFailed。3) AVAudioFile(forReading: 上一步 caf)。4) SpeechAnalyzer(modules:[transcriber])。5) 起一个 Task 持续 for-await transcriber.results 收集成数组(消费侧)。6) analyzer.analyzeSequence(from: audioFile) 返回 lastSampleTime:非 nil→finalizeAndFinish(through: lastSampleTime);nil→cancelAndFinishNow()。7) 分析抛错→取消收集 Task 并 analysisFailed。8) 等收集 Task.value 拿到 [SpeechTranscriber.Result]→decodeResults。
- 【结果解码 decodeResults】遍历每个 Result(=一个端点切分语句段):① fullText 累加 String(result.text.characters)(不加分隔符,直接拼接,模型自带空格/标点)。② segmentText = 该段字符去首尾空白;非空才 append 一个 segment,start=result.range.start.seconds,end=result.range.end.seconds。③ 遍历该段 AttributedString 的 runs:runText=该 run 子串,trimmed 去首尾空白,空则跳过;range=run.audioTimeRange,start=range.start.seconds(可空),end=(range.start+range.duration).seconds(可空,即 起点+时长 而非直接取 end);append 一个 word(text=trimmed)。④ 最终 TranscriptionResult.text = fullText 去首尾空白,language=locale.identifier(.bcp47)。关键点:segment 的时间来自 result.range,word 的时间来自 run.audioTimeRange(可能为 nil),两者口径不同;word.text 已 trim 而 segment.text 也 trim 但保留内部标点大小写。
- 【缓存键 TranscriptCache.key】identity 字符串 = "\(url.path)|\(mtime.timeIntervalSince1970)|\(size)";SHA256(utf8(identity)) → 每字节 %02x 拼成64位 hex → 取前32位(.prefix(32))作为键。取不到文件属性(size/mtime)→返回 nil(此时不缓存,直接现算)。文件被编辑(mtime 或 size 变)→键变→旧缓存自然失效(不主动删除旧文件)。Rust 复刻:同样 stat 拿 mtime(秒,浮点 Unix 时间)与字节数,字符串拼接后 sha256 取前16字节(32 hex)。
- 【缓存读写策略 transcript(for:isVideo:range:)】1) 算 key;若 key 有效且命中缓存(先内存后磁盘):有 range 则对整文件结果调 filter(到 range) 返回,否则返回整文件。2) 未命中且有 range:直接现算该 range(video 走 transcribeVideoAudio 否则 transcribe),且 不写缓存(只缓存整文件)。3) 未命中且无 range:现算整文件,key 有效则 store(写内存+磁盘 JSON)。内存 remember:count>=4 时先 removeAll() 再放入(粗暴清空,不是逐条 LRU)。磁盘 store:确保目录存在,JSONEncoder 编码后写 <key>.json;读 cached 时先查内存,再尝试读磁盘 JSON 解码,成功则 remember 进内存。所有磁盘 IO 用 try?,失败静默(不抛 decodeFailed,该错误枚举实际未被使用)。
- 【窗口过滤 filter(r, to range)】segments 保留满足 end>range.lowerBound && start<range.upperBound 的(半开重叠判定,边界相等不算重叠);words 保留 start/end 都非空 且 e>lo && s<hi 的(空时间戳的词被丢弃)。过滤后 text = 各保留段 text 用单个空格 join(注意:这与整文件 decodeResults 的『直接拼接无分隔符』不同——窗口结果的 text 是空格连接的段文本)。language 沿用,words/segments 用过滤后的。
- 【口语搜索 TranscriptSearch.search】terms(query):按空白分词,每个词 trim 掉 .punctuationCharacters(去边缘标点,如 "budget,"→"budget"),过滤空串;无 term→空结果。遍历 assets:读 cachedOnDisk(无缓存跳过),对每个 segment 若 matches 则加一条 Hit(assetID, segment.start, segment.end, segment.text);命中数达到 limit(默认20)立即返回(短路,不再扫后续)。matches:terms.allSatisfy { text.range(of: term, options:[.caseInsensitive,.diacriticInsensitive]) != nil }——所有词都必须作为子串出现(子串匹配,非整词;忽略大小写与变音符号;AND 语义)。Rust 复刻:Unicode 大小写折叠 + NFD 去组合标记(变音符号不敏感)后做 contains,所有词 AND。
- 【下游消费约定(供复刻字幕/搜索时对齐)】调用方(EditorViewModel+Captions)用 视频可见源区间 visibleSource(clip)=(trimStartFrame, trimStartFrame+durationFrames*max(speed,0.0001)) 帧 来界定要哪段;visibleSourceUnion 把同一 mediaRef 多个 clip 的可见源跨度取 min/max 再 /fps 转秒、左右各 pad 1.0 秒(下限 clamp 到 0)作为 sourceRange 传入缓存。带 censorProfanity 或 指定 locale 时『绕过缓存』直接现算(因为这些选项会产出不同文字稿)。词级命中归属某 clip 用 取词中点((start+end)/2*fps) 落在 visibleSource 区间[start,end)内 计数;短语归属 clip 用 重叠时长最大且重叠>=短语半长 判定。这些规则在 Captions 模块而非本模块,但复刻时需保证本模块输出的 segment/word 秒值精确,否则下游帧映射会偏。

**苹果框架使用**:
- Speech (SpeechTranscriber / SpeechAnalyzer / AssetInventory) [blocker] — 核心:设备端离线语音识别。SpeechTranscriber 配 locale + etiquetteReplacements(脏词替换) + audioTimeRange(词级时间);SpeechAnalyzer.analyzeSequence/finalizeAndFinish 驱动分析;AssetInventory 按需下载安装语种模型。结果是带 AttributedString(含 audioTimeRange run 属性) 的 Result 流,每个 Result=一个端点切分语句。
- AVFoundation [low] — 用 AVAssetReader+AVAssetReaderTrackOutput 把视频/音频解码成 16kHz/单声道/16-bit 小端 PCM,经 AVAudioPCMBuffer 写出 .caf 临时文件;AVURLAsset.loadTracks 取音频轨;带 range 时设 reader.timeRange 只读该段。
- CoreMedia [low] — CMTime/CMTimeRange 表达 reader 截取区间与识别结果的时间(.seconds 转 Double);CMSampleBuffer / CMAudioFormatDescription 系列从样本缓冲取 PCM 数据与格式描述。
- CryptoKit [none] — SHA256 对 『文件路径|mtime|size』 做哈希取前32 hex 当缓存键,实现『文件被编辑即缓存失效』。
- Foundation [none] — Locale 语言匹配(languageCode/region/identifier(.bcp47)/preferredLanguages)、FileManager(临时目录/缓存目录/文件属性 size+mtime)、JSON 编解码缓存、URL/UUID。

**闭源云**:无生成式 AI 云访问。语音识别完全在设备端(Apple Speech 框架离线模型)进行,不发送音频或文本到任何网络服务。唯一的网络相关项是经 Log→Telemetry 转发的 Sentry 面包屑/告警(崩溃与诊断遥测),它不传输音频/文字稿内容、与识别逻辑解耦,且非 Convex/Clerk/任何生成式 AI 云;复刻时可直接省略或替换为本地日志。模型下载(AssetInventory.downloadAndInstall)是 Apple 系统级的语种包下载,非本应用的闭源云。

**移植策略**:整体可在 Rust 端忠实复刻,但语音识别引擎必须整体替换(Apple Speech 在 macOS 专有,Windows/Linux 不可用)。分三块:【1 数据模型与纯逻辑——direct-port】TranscriptionResult/Segment/Word 用 serde struct;offsetting、TranscriptCache.filter(半开重叠判定 + 段文本空格 join)、TranscriptSearch(分词去边缘标点 + 全词 AND 子串、大小写/变音符不敏感)、matchLocale(语言码优先地区次之) 全部是确定性算法,按 coreLogic 逐条 1:1 实现。变音符不敏感建议用 unicode-normalization 做 NFD 去组合标记 + 大小写折叠后 contains。【2 音频抽取——needs-replacement,低风险】用 FFmpeg(命令行或 ffmpeg-next 绑定)把媒体解码为 PCM s16le/mono/16000Hz;带 range 用 -ss <lowerBound> -to <upperBound>(绝对秒,等价 CMTimeRange 端到端语义),输出临时 wav/caf 供识别器吃。【3 识别引擎——needs-replacement,blocker 级工作量】Apple Speech 无跨平台对应,改用 whisper.cpp(本地、可跨平台、支持词级时间戳与语言自动检测,最贴近设备端离线 + 多语种 + word timestamps 的需求)或 vosk。映射:locale 选择→whisper language 参数(可保留 matchLocale 的优先级思路从用户/系统语言推断,或用 whisper 自动检测后回填 language);segment→whisper segment(start/end 秒,文本含标点);word→whisper token-level timestamps(注意 whisper 词级时间需开启 token timestamps,且其 token 边界与 Apple 的 run 不同,复刻下游字幕『词中点落区间计数』『短语重叠归属』时要保证秒值精度即可,不必逐 token 对齐 Apple);脏词过滤 Apple 用 etiquetteReplacements,whisper 无内建,需自建脏词表后处理(可作为可选项)。【缓存】键算法(path|mtime|size 的 sha256 前16字节 hex)与磁盘 JSON、内存上限4『满则清空』策略原样照搬,放在 Rust core,nonisolated 同步读路径对应普通同步函数。【遥测】丢弃 Sentry,改 tracing 本地日志。【时间单位】本模块保持纯秒,帧/秒换算仍由上层(对应 OpenTake 的领域模型/编辑层)用 timeline fps 完成。

**关键文件**:/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Transcription/Transcription.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Transcription/TranscriptCache.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Transcription/TranscriptSearch.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Editor/ViewModel/EditorViewModel+Captions.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Agent/Tools/ToolExecutor+Search.swift、/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Search/SearchIndexCoordinator.swift

## Telemetry  ·  `infra` → **needs-replacement**

**职责**:
- 读取 Info.plist 中的 SentryDSN 与版本号,在 App 启动时初始化 Sentry SDK(start())
- 管理遥测启用开关:存储在 UserDefaults(key=io.palmier.pro.telemetry.enabled),默认开启;并缓存一份'本次启动时的值'供 UI 检测是否需要重启
- 添加面包屑日志(breadcrumb):带 message/category/level/data 字典
- 捕获消息(captureMessage)与捕获错误对象(captureError)上报到 Sentry
- 分级日志辅助:logWarning(走面包屑)/logError(走 capture message, level=error)/logFault(走 capture message, level=fatal),给每条挂 log_category tag 与 log extra
- 设置 scope 的 extra 自定义上下文键值(setExtra),用于附加当前工程快照等信息
- 包裹同步/异步闭包为 Sentry 性能事务(trace),成功 finish、抛错则以 .internalError 状态 finish 并重抛
- 提供 shortId 工具:取字符串前 8 字符,用于在遥测里脱敏/缩短 UUID 类 id

**核心类型**:
- `Telemetry` (enum) — 无 case 的命名空间式 enum,全部为 static 成员。承载 DSN 读取、isEnabled 开关、start/breadcrumb/captureMessage/captureError/setExtra/logWarning/logError/logFault/trace/shortId 等所有遥测 API。是本模块唯一对外类型。
- `Telemetry.Payload` (other) — typealias = [String: Any],遥测附加数据字典的别名。被 Log.swift 等调用方广泛复用作为参数类型。

**核心算法/逻辑(供 Rust 复刻)**:
- 【启用开关三态默认逻辑】isEnabled.get:读 UserDefaults.standard;若 object(forKey:enabledKey) == nil(即用户从未设置过)返回 true(默认开启遥测);否则返回 bool(forKey:)。set:写入 UserDefaults。Rust 复刻:用持久化偏好(如 JSON/SQLite 设置表)存布尔,键名固定为 'io.palmier.pro.telemetry.enabled';'缺省即 true' 这一三态语义必须保留(键不存在=开启,而非关闭)。
- 【启动门控】enabledForCurrentLaunch = isEnabled 在类型加载时求值一次并缓存为 static let,代表'本次进程启动时刻'的开关值。UI(PrivacyPane)用它与当前 isEnabled 比较来判断是否需要提示重启。didStart 为 nonisolated(unsafe) static var 布尔标志,初值 false。
- 【start() 初始化序列与门控】先 guard enabledForCurrentLaunch(关则直接 return,本次进程不上报);再 guard !dsn.isEmpty(无 DSN 则 return)。然后 SentrySDK.start 配置:sendDefaultPii=false(不发个人信息);environment 按编译条件 DEBUG→'development' 否则 'production';tracesSampleRate=0.1(10% 性能采样);appHangTimeoutInterval=8.0 秒(应用卡顿判定阈值);attachStacktrace=true;enableCaptureFailedRequests=false(不自动上报失败网络请求);enableUncaughtNSExceptionReporting=true;releaseName 由 Info.plist 的 CFBundleShortVersionString 与 CFBundleVersion 拼成 'palmier-pro@<version>+<build>'(两者都存在时才设置)。成功后置 didStart=true。所有其它 API 都以 'guard didStart else return'(trace 例外,见下)为前置,确保未初始化时静默 no-op。
- 【DSN 来源】dsn = Bundle.main Info.plist 的 'SentryDSN' 字符串,缺失则空串。Rust 复刻:从打包配置/环境变量读取 DSN,空则不初始化。
- 【breadcrumb】didStart 才执行;构造 Breadcrumb(level, category),设置 message 与 data,调用 SentrySDK.addBreadcrumb。category 默认 'app',level 默认 .info。
- 【分级日志映射规则(重要)】logWarning → breadcrumb(level=.warning)(注意:warning 只进面包屑,不单独上报为事件);logError → captureLogMessage(level=.error);logFault → captureLogMessage(level=.fatal)。captureLogMessage 内部:guard didStart;SentrySDK.capture(message:){ scope.setLevel(level); scope.setTag(value:category, key:'log_category'); 若 data 非空则 scope.setExtra(value:data, key:'log') }。即:错误/致命会产生独立 Sentry 事件并打 log_category 标签与 log 附加数据,而警告仅作为后续事件的上下文面包屑。
- 【captureMessage / captureError】captureMessage(message, level 默认 .warning):didStart 才执行,SentrySDK.capture(message:){ scope.setLevel(level) }。captureError(error):didStart 才执行,SentrySDK.capture(error:)。
- 【setExtra】didStart 才执行;SentrySDK.configureScope{ scope.setExtra(value:, key:) }。用于把当前工程快照等挂到全局 scope(调用方 EditorViewModel 用 key='project')。
- 【trace 性能事务(同步与异步两个重载)】若 !didStart 则直接执行 work() 并返回(不创建事务,保证零开销直通);否则 txn = SentrySDK.startTransaction(name, operation);do{ result=work(); txn.finish(); return result } catch { txn.finish(status:.internalError); throw }。operation 默认 'task'。同步版用 rethrows + 闭包,异步版用 async rethrows + async 闭包,语义一致。
- 【shortId 脱敏工具】String(id.prefix(8)),取前 8 个字符。调用方对 assetId/资源 id 用它做缩短与轻度脱敏后再放入遥测 payload。Rust 复刻:取字符串前 8 个 Unicode scalar/char(需注意是字符前缀而非字节,避免截断多字节字符)。
- 【调用方耦合点(便于复刻接线)】(1) App/main.swift:Log.bootstrap() 后立即 Telemetry.start()。(2) Utilities/Log.swift 的 CategoryLog 是主要门面:notice(telemetry:) 在提供 telemetry 文案时调 breadcrumb;warning/error/fault 分别调 logWarning/logError/logFault,并把 telemetry 文案兜底为普通消息 m。(3) Settings/PrivacyPane.swift:开关 UI 写 Telemetry.isEnabled,并用 didChange=telemetryEnabled != enabledForCurrentLaunch 提示用户重启生效。(4) Agent/ToolExecutor 与 EditorViewModel 通过 Log 门面写带 Payload 的遥测(工具耗时、timelineChanged 等)。

**苹果框架使用**:
- Foundation [none] — UserDefaults.standard 读写遥测开关;Bundle.main.object(forInfoDictionaryKey:) 读取 SentryDSN、CFBundleShortVersionString、CFBundleVersion;String.prefix 做 shortId。均为标准跨平台基础设施,无音视频或图形相关用途。

**闭源云**:不触达 Convex/Clerk/任何闭源生成式 AI 云。唯一的网络出口是第三方崩溃监控服务 Sentry(自托管或 sentry.io 均可,DSN 从 Info.plist 注入)。上报内容刻意脱敏:sendDefaultPii=false、enableCaptureFailedRequests=false,且产品文案明确声明"媒体与工程内容绝不收集",仅发送崩溃/错误/面包屑与脱敏后的 id(shortId 取前 8 位)。此为可选诊断遥测,与生成式 AI 业务云无关,且受用户开关控制(默认开启,可在隐私设置关闭后重启生效)。

**移植策略**:逻辑本身简单可直接移植,但 Sentry Cocoa SDK 必须替换为 Rust 生态对应物。方案:在 Rust core 用官方 `sentry` crate(sentry-rust,支持 native 崩溃/panic 捕获、breadcrumb、scope、performance transaction,概念一一对应)。映射:SentrySDK.start→sentry::init(ClientOptions{ dsn, environment, traces_sample_rate:0.1, release:Some('palmier-pro@<ver>+<build>'.into()), send_default_pii:false, .. });breadcrumb→sentry::add_breadcrumb(Breadcrumb{ message, category, level, data });captureMessage/Error→sentry::capture_message / capture_error(或 anyhow/Error 经 sentry-anyhow);setExtra→sentry::configure_scope(|s| s.set_extra(...));logWarning/Error/Fault 的'warning 只进面包屑、error/fatal 才成事件并打 log_category tag' 这套分级语义需在 Rust 侧用同样的分支显式复刻;trace→sentry 的 start_transaction + finish(失败时 set status=internal_error)。isEnabled 三态默认(键缺省=开启)用 Tauri 的设置存储或自管 JSON/SQLite 复刻,键名沿用 'io.palmier.pro.telemetry.enabled' 或改 OpenTake 命名空间。appHangTimeoutInterval(8s 卡顿)、enableUncaughtNSExceptionReporting 这类 macOS 特有项无对应,可丢弃或用 sentry panic handler 近似。前端(React)若也要采集可用 @sentry/browser/@sentry/tauri,但核心崩溃上报建议放 Rust 端。隐私/合规:保留默认脱敏(不发 PII、不收媒体/工程内容)、保留用户可关开关、上报前对 id 做 shortId(前 8 char)截断。整体属诊断基础设施,非编辑算法,移植优先级低,可后置。

**关键文件**:Sources/PalmierPro/Telemetry/Telemetry.swift、Sources/PalmierPro/Utilities/Log.swift、Sources/PalmierPro/Settings/PrivacyPane.swift、Sources/PalmierPro/App/main.swift

## Toolbar  ·  `ui` → **ui-rebuild**

**职责**:
- 渲染工具栏布局：撤销/重做组、工具模式组、分割/裁剪组、新增文字、右侧缩放滑块，用 Divider 与 Spacer 分隔
- 把 7 个按钮的点击事件分别转发到 EditorViewModel 的方法（splitAtPlayhead / trimStartToPlayhead / trimEndToPlayhead / addTextClip）或通过 NSApp.sendAction 触发响应链的 undo:/redo:
- 维护并展示当前 ToolMode（pointer/razor）的高亮选中态，点击切换 editor.toolMode
- 通过对数映射的双向 Binding 驱动缩放滑块，写回 editor.zoomScale，使滑块行程在每个缩放倍数上视觉均匀
- 用本地辅助函数（toolbarButton/toolModeButton/textGlyphButton/bracketButton）统一按钮样式：24x24 图标框 + hoverHighlight 悬停高亮 + .help 工具提示(含快捷键)
- 所有尺寸/颜色/字号严格走 AppTheme 设计令牌，不硬编码

**核心类型**:
- `ToolbarView` (struct) — 唯一的本模块类型。SwiftUI View，通过 @Environment(EditorViewModel.self) 取得编辑器视图模型。body 是一个 HStack 布局，包含 5 组按钮 + 缩放滑块。内部私有方法仅生成样式化按钮，无业务状态。
- `EditorViewModel` (class) — 外部依赖（@Observable @MainActor）。工具栏读取它的 toolMode、zoomScale、minZoomScale，并调用 splitAtPlayhead/trimStartToPlayhead/trimEndToPlayhead/addTextClip。是所有编辑逻辑的真正落点。
- `ToolMode` (enum) — 外部依赖。两个 case：pointer(V 键，选择/移动/裁剪)、razor(C 键，点击分割)。工具栏的两个工具模式按钮就是切换它。
- `Zoom` (enum) — 外部依赖（Constants.swift 中的常量命名空间）。提供 min=0.05 / floor=0.0001 / max=40.0 等缩放边界，工具栏滑块上限用 Zoom.max。
- `HoverHighlight` (struct) — 外部依赖。ViewModifier，给小图标按钮加圆角悬停背景并用 contentShape 扩大点击命中区；支持 isActive 态(选中工具更亮)。工具栏每个按钮都 .hoverHighlight()。

**核心算法/逻辑(供 Rust 复刻)**:
- 【缩放滑块的对数映射 — 必须一比一复刻】滑块的 value 不是直接绑定 zoomScale，而是对数空间：get 返回 log(editor.zoomScale)，set 执行 editor.zoomScale = exp(newValue)。滑块区间为 log(editor.minZoomScale) ... log(Zoom.max)，Zoom.max=40.0。这样滑块每移动等长一段，缩放倍数乘以固定比例（指数缩放），视觉均匀。Rust/前端复刻：滑块内部存 ln(scale)，区间 [ln(minZoom), ln(40)]，回写时 scale=e^value。注意 log 是自然对数 ln。
- 【minZoomScale 计算 — 适配全部内容的最小缩放】定义在 EditorViewModel+PreviewTabs：totalFrames=timeline.totalFrames(所有轨道 clip.endFrame 的最大值)。若 totalFrames<=0 或 timelineVisibleWidth<=0 返回 Zoom.min(0.05)。否则 availableWidth = timelineVisibleWidth - Layout.trackHeaderWidth(100)；若 availableWidth<=0 返回 0.05。fitAll = availableWidth / (totalFrames * Zoom.fitAllBuffer)，fitAllBuffer=3.0(留 3 倍尾部余量)。最终 return min(Zoom.max=40, max(Zoom.floor=0.0001, fitAll))。单位：zoomScale 即 pixelsPerFrame(每帧像素数)，默认 Defaults.pixelsPerFrame=4.0。
- 【撤销/重做触发方式】工具栏不直接调用 undoManager。undo() 执行 NSApp.sendAction(Selector("undo:"), to:nil, from:nil)，redo() 同理用 "redo:"。to:nil 表示沿 AppKit 响应链向上找第一个能处理该 selector 的对象——即第一响应者→...→EditorWindowController 上的 NSDocument/NSUndoManager。复刻要点：这是标准的'第一响应者派发'，Tauri/前端需建立一个全局命令派发，把 Cmd+Z/Shift+Cmd+Z 与工具栏按钮都路由到当前活动文档的 UndoManager（撤销栈）。MainMenu 中 Edit 菜单也用同样的 "undo:"/"redo:" selector + 快捷键 z / Z(即 Shift+Cmd+Z)。
- 【splitAtPlayhead 算法】遍历 selectedClipIds，对每个 id 调 splitClip(clipId, atFrame: currentFrame)。currentFrame 是播放头时间线帧。
- 【splitClip 与 splitSingleClip — 分割的精确规则】(1) 找到 clip；若有 linkGroupId 则取 [clipId]+linkedPartnerIds 一并分割(A/V 联动)，否则只分自己。(2) 在 undo group 内对每个 id 调 splitSingleClip。(3) 守卫：仅当 atFrame > clip.startFrame && atFrame < clip.endFrame 才分割(端点不分)。(4) splitOffset = atFrame - startFrame(时间线帧)。(5) 源帧换算：leftSource = round(splitOffset * speed)；rightSource = round((durationFrames - splitOffset) * speed)。(6) 左半 left=clip 复制：durationFrames=splitOffset；trimEndFrame=原 trimEndFrame + rightSource；fadeOutFrames=0；clampFadesToDuration()。(7) 右半 right=clip 复制：新 id=UUID；startFrame=atFrame；durationFrames=原 durationFrames - splitOffset；trimStartFrame=原 trimStartFrame + leftSource；fadeInFrames=0；clampFadesToDuration()。(即左半保留头部淡入、清掉淡出；右半保留尾部淡出、清掉淡入)。(8) 6 条关键帧轨(opacity/volume/position/scale/rotation/crop)各自调 splitKeyframeTrack 在切点切分。(9) 把 left 写回原位、append right、按 startFrame 排序。(10) 注册撤销：撤销时删除 right、并把 left 还原成原始 clip。(11) 若是联动分割(groupIds.count>1)，给所有右半生成新的 linkGroupId 重新成组，使分割后左右各自成对。
- 【splitKeyframeTrack — 关键帧轨分割保持曲线连续】输入 track、splitOffset(clip 相对帧)、fallback 默认值。若 track 为空/未激活返回(track,track)。boundary = track.sample(at: splitOffset, fallback)(在切点采样插值出边界值)。左轨：保留 frame<=splitOffset 的关键帧，若末尾不是恰好 splitOffset 则补一个 (splitOffset, boundary)。右轨：取 frame>=splitOffset 的关键帧，每个 frame 减去 splitOffset 重新基准化(rebase 到 0)，并保留各自 interpolationOut；若首帧不是 0 则在 0 处插入 (0, boundary)。空则返回 nil。要点：是切断+插边界帧，而不是把整条轨复制给两半(避免越界/未 rebase 帧)。
- 【trimStartToPlayhead — 入点裁到播放头】遍历 selectedClipIds：守卫 currentFrame>startFrame && currentFrame<endFrame。delta = currentFrame - startFrame(时间线帧)。sourceDelta = round(delta * speed)(转源帧)。调 trimClips([(id, trimStartFrame: 原 trimStartFrame + sourceDelta, trimEndFrame: 不变)])。
- 【trimEndToPlayhead — 出点裁到播放头】守卫同上。delta = endFrame - currentFrame。sourceDelta = round(delta * speed)。调 trimClips([(id, trimStartFrame: 不变, trimEndFrame: 原 trimEndFrame + sourceDelta)])。
- 【trimClips / trimClipInternal — overwrite 式裁剪(就地缩放，不波纹)】对每条 edit：传入的 trimStartFrame/trimEndFrame 是源帧。deltaStartSource = 新 trimStart - 旧 trimStart；deltaEndSource = 新 trimEnd - 旧 trimEnd。换算回时间线帧：deltaStartTimeline = round(deltaStartSource / speed)；deltaEndTimeline = round(deltaEndSource / speed)。newDuration = 旧 durationFrames - deltaStartTimeline - deltaEndTimeline；newStartFrame = 旧 startFrame + deltaStartTimeline。写回 trimStartFrame/trimEndFrame/startFrame/setDuration(newDuration)，再按 startFrame 排序。注册反向撤销(用旧 trim 值再调一次)。注意：这是'同轨不推挤相邻 clip、不向其它轨同步'的覆盖式裁剪(注释明确写 overwrite-style)。
- 【setDuration 副作用】setDuration(n){ durationFrames=n; clampKeyframesToDuration(); clampFadesToDuration() }。clampFadesToDuration: fadeInFrames=clamp(0..durationFrames)；fadeOutFrames=clamp(0..(durationFrames-fadeInFrames))(头+尾淡变不得超过总时长)。
- 【addTextClip — 新增文字 clip】durationFrames = max(1, secondsToFrame(Defaults.textDurationSeconds=3.0, fps))。trackIdx = insertTrack(at:0, type:.video)(索引 0 是 UI 最顶层；insertTrack 会把视觉轨钳制在第一条音频轨之前)。用 TextLayout.naturalSize 算文字自然尺寸，归一化为画布占比 w=natural.width/canvasW、h=natural.height/canvasH，Transform 居中(topLeft=((1-w)/2,(1-h)/2), width:w, height:h)。新建 Clip(mediaRef:"", mediaType:.text, sourceClipType:.text, startFrame:max(0,currentFrame), durationFrames, transform)，设 textContent/textStyle。append+排序。注册撤销(删除该 clip)。selectedClipIds={新id}。最后 videoEngine?.syncTextLayers()(文字走 CATextLayer 叠加层渲染，不走合成管线)。
- 【时间换算口径】secondsToFrame(s,fps)=Int(s*fps)(截断取整，非四舍五入)；frameToSeconds(f,fps)=f/fps。clip 内部所有 trim/split 的源帧换算用 round()(四舍五入到最近整数)。speed 是源帧/时间线帧的比率：durationFrames(时间线) * speed = 消耗的源帧 sourceFramesConsumed。endFrame=startFrame+durationFrames。totalFrames=所有轨道 max(clip.endFrame)。
- 【工具模式切换】toolModeButton：isActive=(editor.toolMode==mode)；点击 editor.toolMode=mode。pointer 默认选择/移动/裁剪；razor 在时间线点击即分割(实际分割行为在 Timeline 输入控制器里依据 toolMode 决定，工具栏只负责切换状态与高亮)。
- 【按钮命中与样式规则】所有按钮 buttonStyle(.plain)+24x24 frame+hoverHighlight()。hoverHighlight 用 contentShape(圆角矩形)扩大点击区到整个 24x24 框(而非仅图标像素)，悬停时填充白色低透明度背景；选中态(isActive)更亮。.help 提供原生 tooltip，文本含快捷键如 'Undo (⌘Z)'、'Pointer (V)'、'Split at Playhead (⌘K)'、'Trim Start to Playhead (Q)'。

**苹果框架使用**:
- SwiftUI [high] — 整个 ToolbarView 用 SwiftUI 构建：HStack/Divider/Spacer/Slider/Button/Image(systemName:)/Text、@Environment 依赖注入、Binding 做缩放滑块对数映射、.help/.tint/.controlSize/.buttonStyle(.plain) 等修饰器。
- AppKit [medium] — 仅用于 undo/redo：NSApp.sendAction(Selector("undo:"/"redo:"), to:nil, from:nil) 沿响应链派发；import AppKit。SF Symbols 图标名(cursorarrow/scissors/square.split.2x1/minus.magnifyingglass 等)经 SwiftUI Image(systemName:) 渲染，本质也是 Apple 符号字体资源。

**闭源云**:无。Toolbar 目录下无任何 Convex/ConvexMobile/Clerk/ClerkKit/URLSession/网络请求；grep 确认 0 命中。工具栏只触发本地编辑(分割/裁剪/撤销/缩放/新增文字)与本地渲染刷新，不触达任何生成式 AI 云。

**移植策略**:ToolbarView 是纯 SwiftUI 表现层，需在 React/TS 前端整体重建为一个工具栏组件，不可直接移植。但它本身几乎没有逻辑——真正要忠实复刻的是它调用的 EditorViewModel 编辑算法(split/trim/addText)，那些应放到 Rust core 实现，前端按钮只发命令。具体替换方案：(1) 布局：用 flex 行 + 分隔符/弹簧重建；图标用任意图标库(如 lucide)替换 SF Symbols(cursorarrow→鼠标、scissors→剪刀、square.split.2x1→分割、放大镜→zoom)。(2) 悬停高亮 hoverHighlight→CSS :hover + 圆角背景，命中区用 padding/伪元素扩大。(3) 撤销/重做：在 Rust core 维护撤销栈(对应 NSUndoManager 的 bidirectional swap 模式——每个 mutation 记录 before/after Timeline 或 per-clip 快照，撤销时回写并重新注册逆向 swap)；前端按钮与 Cmd+Z/Shift+Cmd+Z 都调用 core 的 undo()/redo() 命令(invoke)。(4) 工具模式：toolMode 作为前端/或 core 的 UI 状态枚举(Pointer/Razor)，razor 模式下时间线点击发 split 命令。(5) 分割/裁剪/新增文字算法严格按 coreLogic 在 Rust 实现：注意 round() 取整、源帧=时间线帧*speed、fade 在分割时左清淡出右清淡入、关键帧轨切点插边界帧并 rebase、trim 是 overwrite 式(不波纹)。secondsToFrame 用截断(Int(s*fps))而非四舍五入，务必一致。(6) 缩放滑块：前端 input[type=range] 存 ln(scale)，区间 [ln(minZoom), ln(40)]，回写 scale=Math.exp(v)；minZoom 计算(availableWidth/(totalFrames*3)，钳制到[0.0001,40])可放前端或 core。(7) 文字渲染独立于媒体合成(syncTextLayers→在 FFmpeg/前端 overlay 层单独绘制文字)，新增文字时把文字 clip 放到顶层视频轨、按 currentFrame 起点、默认 3 秒。无 Apple 私有框架阻塞，无 blocker。

**关键文件**:Sources/PalmierPro/Toolbar/ToolbarView.swift、Sources/PalmierPro/Editor/ViewModel/EditorViewModel+ClipMutations.swift、Sources/PalmierPro/Editor/ViewModel/EditorViewModel+Ripple.swift、Sources/PalmierPro/Editor/OverwriteEngine.swift、Sources/PalmierPro/Models/Timeline.swift、Sources/PalmierPro/Utilities/Constants.swift

