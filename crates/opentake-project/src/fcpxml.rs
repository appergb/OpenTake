//! Timeline export as XMEML 4 (Final Cut Pro 7 XML). 1:1 port of upstream
//! `Export/XMLExporter.swift`.
//!
//! 上游有两种时间线交换格式:XMEML 与 FCPXML。FCPXML 更新、支持更多特性,但
//! Premiere Pro 不原生支持它——选 FCPXML 就得让用户用 DaVinci 当桥或第三方工具把
//! `.fcpxml` 转成 `.xml`。Premiere Pro 是当前优先级,所以上游(以及本端口)选了
//! 已被弃用的 XMEML。命令/前端名沿用任务约定的 `export_fcpxml`/`exportFcpxml`,
//! 但**产物与本模块实现的是 XMEML 4(`.xml`)**,DaVinci/FCP 也能导入 FCP7 XML。
//!
//! 文档结构:整份文档建成一棵 [`XmlNode`] 树;`render` 独占所有缩进与转义,任何片段
//! 都不自带空白。自上而下读 [`Builder::build`] 即可看清格式:`<xmeml><sequence>
//! <media>` 外壳,再到 tracks → clipitems → files / filters / links。
//!
//! 会传输的内容:
//! - 片段位置与裁剪 → `<clipitem>` 的 `<start>`/`<end>`/`<in>`/`<out>`
//! - 速度 → Time Remap filter
//! - 音量(静态 + 关键帧)→ Audio Levels filter
//! - 透明度(静态 + 关键帧)→ 独立的 Opacity filter
//! - 变换——缩放 / 旋转 / 位置(静态 + 关键帧)→ Basic Motion filter
//! - 裁剪(静态 + 关键帧)→ Crop filter
//! - 淡入/淡出 → 单边 transition(视频 Cross Dissolve、音频 Cross Fade)
//! - 链接的 A/V 片段 → 互相的 `<link>` 块
//! - 源帧率 → per-file NTSC 标记(29.97/23.976/59.94 → ntsc TRUE)
//!
//! **不会**传输的内容(与上游一致):
//! - 文本叠加(FCPXML 支持,XMEML 不支持)
//! - 翻转(水平/垂直)
//! - 关键帧插值曲线(linear/hold/smooth):导入时用默认缓动
//!
//! 与上游的两点差异(都是跨平台降级,语义对齐):
//! - **源起始时间码**:上游用 AVFoundation 读 QuickTime `tmcd` 轨;Rust/Tauri 无等价
//!   实现,这里 1:1 降级为 startFrame=0 + `00:00:00:00`(正是上游读不到 tmcd 时的回退
//!   分支)。源时间码读取为后续(可用 ffprobe 补)。
//! - **文件存在性检查**:domain 的 [`MediaResolver`] 是零 IO 的(只算 expected_path),
//!   上游 `resolveURL` 会过滤解析不到的 clip。这里在本模块内用 `expected_path() +
//!   is_file()` 复刻过滤语义,不污染 domain 的零 IO 约束;过滤后 link 的
//!   trackindex/clipindex 仍与实际发出的 clip 一致(先 sort_emittable 再
//!   index_addresses,照搬上游顺序)。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use opentake_domain::{
    AnimatableProperty, Clip, ClipType, Crop, FadeEdge, MediaManifest, MediaResolver, Timeline,
    Track, Transform,
};

/// `seconds * fps` 截断取整。1:1 对应上游 `secondsToFrame`(`Int(seconds * fps)`)。
fn seconds_to_frame(seconds: f64, fps: i32) -> i32 {
    (seconds * fps as f64) as i32
}

/// 把 [`Timeline`] 导出为 XMEML 4(FCP7 XML)字符串。纯函数:输入时间线、媒体清单、
/// 以及解析 `Project` 相对路径所需的工程目录,输出完整 XML 文本。
pub fn export_xmeml(
    timeline: &Timeline,
    manifest: &MediaManifest,
    project_base: Option<&Path>,
) -> String {
    let resolver = MediaResolver::new(manifest, project_base);
    Builder::new(timeline, &resolver).build()
}

// MARK: - Builder

/// 哪一条边的淡变。
#[derive(Clone, Copy)]
enum TransitionEdge {
    Left,
    Right,
}

/// `(media_ref, is_audio)` 复合键:同一素材的视频/音频用不同的 `<file>` id。
type FileKey = (String, bool);

/// 片段在其媒体类型内的地址,用于发出 `<link>` 交叉引用。索引 1-based。
struct ClipAddress {
    track_index: i32,
    clip_index: i32,
    is_audio: bool,
}

struct Builder<'a> {
    timeline: &'a Timeline,
    resolver: &'a MediaResolver<'a>,
    fps: i32,
    seq_width: i32,
    seq_height: i32,

    /// 已完整发出的文件;重复引用折叠为自闭合 `<file id="..."/>`。
    emitted_files: HashSet<FileKey>,
    /// clip id → 其在媒体类型内的地址。
    clip_addresses: HashMap<String, ClipAddress>,
    /// link group id → 该组的片段(按出现顺序)。
    clips_by_link_group: HashMap<String, Vec<Clip>>,
}

impl<'a> Builder<'a> {
    fn new(timeline: &'a Timeline, resolver: &'a MediaResolver<'a>) -> Self {
        Builder {
            timeline,
            resolver,
            fps: timeline.fps,
            seq_width: timeline.width,
            seq_height: timeline.height,
            emitted_files: HashSet::new(),
            clip_addresses: HashMap::new(),
            clips_by_link_group: HashMap::new(),
        }
    }

    // MARK: - Document shell

    fn build(&mut self) -> String {
        // FCP XML 把视频轨从 bottom→top 排列;我方模型存的是 top→bottom,需 reversed。
        let video_tracks: Vec<&Track> = self
            .timeline
            .tracks
            .iter()
            .filter(|t| t.kind.is_visual())
            .rev()
            .collect();
        let audio_tracks: Vec<&Track> = self
            .timeline
            .tracks
            .iter()
            .filter(|t| t.kind == ClipType::Audio)
            .collect();
        let sorted_video: Vec<Vec<Clip>> = video_tracks
            .iter()
            .map(|t| self.sort_emittable(t))
            .collect();
        let sorted_audio: Vec<Vec<Clip>> = audio_tracks
            .iter()
            .map(|t| self.sort_emittable(t))
            .collect();

        self.index_addresses(&sorted_video, false);
        self.index_addresses(&sorted_audio, true);
        self.index_link_groups();

        let video_track_nodes: Vec<XmlNode> = video_tracks
            .iter()
            .zip(sorted_video.iter())
            .map(|(track, clips)| self.track_node(track, clips, false))
            .collect();
        let audio_track_nodes: Vec<XmlNode> = audio_tracks
            .iter()
            .zip(sorted_audio.iter())
            .map(|(track, clips)| self.track_node(track, clips, true))
            .collect();

        let mut video_children = vec![self.video_format_node()];
        video_children.extend(video_track_nodes);

        let mut audio_children = vec![
            leaf_i("numOutputChannels", 2),
            self.audio_format_node(),
            self.audio_outputs_node(),
        ];
        audio_children.extend(audio_track_nodes);

        let sequence = el_attrs(
            "sequence",
            vec![("id", "sequence-1")],
            vec![
                leaf("name", "Timeline Export"),
                leaf_i("duration", self.timeline.total_frames()),
                self.rate(self.fps, false),
                self.timecode_node(),
                el(
                    "media",
                    vec![el("video", video_children), el("audio", audio_children)],
                ),
            ],
        );
        let root = el_attrs("xmeml", vec![("version", "4")], vec![sequence]);
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n{}",
            render(&root, 0)
        )
    }

    fn timecode_node(&self) -> XmlNode {
        el(
            "timecode",
            vec![
                self.rate(self.fps, false),
                leaf("string", "00:00:00:00"),
                leaf_i("frame", 0),
                leaf("source", "source"),
                leaf("displayformat", "NDF"),
            ],
        )
    }

    fn video_format_node(&self) -> XmlNode {
        el(
            "format",
            vec![el(
                "samplecharacteristics",
                vec![
                    leaf_i("width", self.seq_width),
                    leaf_i("height", self.seq_height),
                    boolean("anamorphic", false),
                    leaf("pixelaspectratio", "square"),
                    leaf("fielddominance", "none"),
                    self.rate(self.fps, false),
                ],
            )],
        )
    }

    fn audio_format_node(&self) -> XmlNode {
        el(
            "format",
            vec![el(
                "samplecharacteristics",
                vec![leaf_i("samplerate", 48000), leaf_i("depth", 16)],
            )],
        )
    }

    fn audio_outputs_node(&self) -> XmlNode {
        el(
            "outputs",
            vec![el(
                "group",
                vec![
                    leaf_i("index", 1),
                    leaf_i("numchannels", 2),
                    leaf_i("downmix", 0),
                    el("channel", vec![leaf_i("index", 1)]),
                    el("channel", vec![leaf_i("index", 2)]),
                ],
            )],
        )
    }

    // MARK: - Tracks → clipitems

    fn track_node(&mut self, track: &Track, sorted_clips: &[Clip], is_audio: bool) -> XmlNode {
        let enabled = if is_audio {
            !track.muted
        } else {
            !track.hidden
        };
        let mut children = vec![boolean("enabled", enabled), boolean("locked", false)];
        for clip in sorted_clips {
            if let Some(fade_in) = self.fade_transition(clip, TransitionEdge::Left, is_audio) {
                children.push(fade_in);
            }
            children.push(self.clip_item_node(clip, is_audio));
            if let Some(fade_out) = self.fade_transition(clip, TransitionEdge::Right, is_audio) {
                children.push(fade_out);
            }
        }
        el("track", children)
    }

    fn clip_item_node(&mut self, clip: &Clip, is_audio: bool) -> XmlNode {
        let source_duration = self
            .source_duration_frames(&clip.media_ref)
            .unwrap_or_else(|| clip.source_duration_frames());
        // in/out 是源帧偏移,跨度 source_frames_consumed(速度由 Time Remap 处理)。
        let in_point = clip.trim_start_frame;
        let out_point = clip.trim_start_frame + clip.source_frames_consumed();

        let mut children = vec![
            leaf("masterclipid", &self.masterclip_id(clip, is_audio)),
            leaf("name", &self.resolver.display_name(&clip.media_ref)),
            boolean("enabled", true),
            leaf_i("duration", source_duration),
            self.rate(self.fps, false),
            leaf_i("start", clip.start_frame),
            leaf_i("end", clip.end_frame()),
            leaf_i("in", in_point),
            leaf_i("out", out_point),
            self.file_node(&clip.media_ref, is_audio),
        ];
        if let Some(remap) = self.time_remap_filter(clip.speed, is_audio) {
            children.push(remap);
        }
        if is_audio {
            children.extend(self.volume_filters(clip));
        } else {
            children.extend(self.video_filters(clip));
        }
        children.extend(self.link_nodes(clip));
        el("clipitem", children).with_attr("id", &format!("clipitem-{}", clip.id))
    }

    fn masterclip_id(&self, clip: &Clip, is_audio: bool) -> String {
        if let Some(group) = &clip.link_group_id {
            return format!("masterclip-{group}");
        }
        format!(
            "masterclip-{}-{}",
            clip.media_ref,
            if is_audio { "audio" } else { "video" }
        )
    }

    // MARK: - File elements

    /// 不同媒体类型用不同 id —— Premiere 拒绝 clipitem 指向类型不符的 `<file>`。
    /// 重复引用折叠为自闭合 `<file id="..."/>`。文件按 `(media_ref, is_audio)` 去重。
    fn file_node(&mut self, media_ref: &str, is_audio: bool) -> XmlNode {
        let file_id = format!(
            "file-{}-{}",
            media_ref,
            if is_audio { "audio" } else { "video" }
        );
        let key: FileKey = (media_ref.to_string(), is_audio);
        if self.emitted_files.contains(&key) {
            return el("file", vec![]).with_attr("id", &file_id);
        }
        self.emitted_files.insert(key);

        let entry = self.resolver.entry(media_ref);
        let path = self.existing_path(media_ref);
        // 文件名:优先解析到的真实文件名,否则 entry.name,否则 media_ref。
        let file_name = path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .or_else(|| entry.map(|e| e.name.clone()))
            .unwrap_or_else(|| media_ref.to_string());
        // Premiere 需要这种多斜杠的 host 形式;规范的单斜杠会解析失败。
        let path_url = path
            .as_ref()
            .map(|p| format!("file://localhost//{}", p.to_string_lossy()))
            .unwrap_or_else(|| format!("media/{media_ref}"));

        // 一张静图解码为恰好 1 帧。
        let is_image = entry.map(|e| e.kind == ClipType::Image).unwrap_or(false);
        let duration_frames = if is_image {
            1
        } else {
            entry
                .map(|e| seconds_to_frame(e.duration, self.fps).max(0))
                .unwrap_or(0)
        };
        let (timebase, ntsc) =
            rate_tags(entry.and_then(|e| e.source_fps).unwrap_or(self.fps as f64));

        let media = if is_audio {
            el(
                "media",
                vec![el(
                    "audio",
                    vec![
                        el(
                            "samplecharacteristics",
                            vec![leaf_i("samplerate", 48000), leaf_i("depth", 16)],
                        ),
                        leaf_i("channelcount", 2),
                    ],
                )],
            )
        } else {
            let mut video_children = Vec::new();
            if is_image {
                video_children.push(leaf_i("duration", 1));
            }
            video_children.push(el(
                "samplecharacteristics",
                vec![
                    leaf_i(
                        "width",
                        entry.and_then(|e| e.source_width).unwrap_or(self.seq_width),
                    ),
                    leaf_i(
                        "height",
                        entry
                            .and_then(|e| e.source_height)
                            .unwrap_or(self.seq_height),
                    ),
                    boolean("anamorphic", false),
                    leaf("pixelaspectratio", "square"),
                    leaf("fielddominance", "none"),
                    self.rate(timebase, ntsc),
                ],
            ));
            el("media", vec![el("video", video_children)])
        };

        // timecode 是 DaVinci Resolve 必需的。源时间码无跨平台读取实现,降级为 0。
        let drop_frame = ntsc && timebase % 30 == 0;
        let start_frame = 0; // 源起始时间码读取为后续(上游无 tmcd 时也是 0)。
        let timecode = el(
            "timecode",
            vec![
                self.rate(timebase, ntsc),
                leaf(
                    "string",
                    &format_timecode(start_frame, timebase, drop_frame),
                ),
                leaf_i("frame", start_frame),
                leaf("displayformat", if drop_frame { "DF" } else { "NDF" }),
            ],
        );
        el(
            "file",
            vec![
                leaf("name", &file_name),
                leaf("pathurl", &path_url),
                self.rate(timebase, ntsc),
                leaf_i("duration", duration_frames),
                timecode,
                media,
            ],
        )
        .with_attr("id", &file_id)
    }

    // MARK: - Links

    /// 链接片段为每个伙伴发一个 `<link>`,让 Premiere 重建 A/V 配对。
    fn link_nodes(&self, clip: &Clip) -> Vec<XmlNode> {
        let group = match &clip.link_group_id {
            Some(g) => g,
            None => return Vec::new(),
        };
        let partners = match self.clips_by_link_group.get(group) {
            Some(p) if p.len() > 1 => p,
            _ => return Vec::new(),
        };
        partners
            .iter()
            .filter_map(|partner| {
                let addr = self.clip_addresses.get(&partner.id)?;
                Some(el(
                    "link",
                    vec![
                        leaf("linkclipref", &format!("clipitem-{}", partner.id)),
                        leaf("mediatype", if addr.is_audio { "audio" } else { "video" }),
                        leaf_i("trackindex", addr.track_index),
                        leaf_i("clipindex", addr.clip_index),
                    ],
                ))
            })
            .collect()
    }

    // MARK: - Transitions (fades)

    /// 淡变导出为到黑/到静音的单边 dissolve(没有 clip-to-clip 模型)。
    fn fade_transition(
        &self,
        clip: &Clip,
        edge: TransitionEdge,
        is_audio: bool,
    ) -> Option<XmlNode> {
        let fade_edge = match edge {
            TransitionEdge::Left => FadeEdge::Left,
            TransitionEdge::Right => FadeEdge::Right,
        };
        let frames = clip.fade_frames(fade_edge);
        if frames <= 0 {
            return None;
        }

        let (start, end, alignment, cut_frames) = match edge {
            TransitionEdge::Left => (
                clip.start_frame,
                clip.start_frame + frames,
                "start-black",
                0,
            ),
            TransitionEdge::Right => (
                clip.end_frame() - frames,
                clip.end_frame(),
                "end-black",
                frames,
            ),
        };

        let mut children = vec![
            leaf_i("start", start),
            leaf_i("end", end),
            leaf("alignment", alignment),
        ];
        if is_audio {
            children.push(self.rate(self.fps, false));
            children.push(effect(
                "Cross Fade ( 0dB)",
                "KGAudioTransCrossFade0dB",
                "transition",
                "audio",
                None,
                vec![],
            ));
        } else {
            // Premiere 私有的 cut-point,单位 ticks(254016000000/秒):淡入为 0,
            // 淡出为整段长度。
            let cut_point_ticks = cut_frames as i64 * (254_016_000_000i64 / self.fps as i64);
            children.push(leaf("cutPointTicks", &cut_point_ticks.to_string()));
            children.push(self.rate(self.fps, false));
            children.push(effect(
                "Cross Dissolve",
                "Cross Dissolve",
                "transition",
                "video",
                Some("Dissolve"),
                vec![
                    leaf_i("wipecode", 0),
                    leaf_i("wipeaccuracy", 100),
                    leaf_i("startratio", 0),
                    leaf_i("endratio", 1),
                    boolean("reverse", false),
                ],
            ));
        }
        Some(el("transitionitem", children))
    }

    // MARK: - Filters

    /// Premiere 需要它来应用速度;它不会从 in/out 与 start/end 的比例反推。
    fn time_remap_filter(&self, speed: f64, is_audio: bool) -> Option<XmlNode> {
        if speed == 1.0 {
            return None;
        }
        let mediatype = if is_audio { "audio" } else { "video" };
        Some(filter(effect(
            "Time Remap",
            "timeremap",
            "motion",
            mediatype,
            None,
            vec![
                parameter(
                    "variablespeed",
                    "variablespeed",
                    Some("0"),
                    Some("1"),
                    leaf_i("value", 0),
                    vec![],
                ),
                parameter(
                    "speed",
                    "speed",
                    Some("-100000"),
                    Some("100000"),
                    leaf("value", &format!("{:.4}", speed * 100.0)),
                    vec![],
                ),
                parameter(
                    "reverse",
                    "reverse",
                    None,
                    None,
                    boolean("value", false),
                    vec![],
                ),
                parameter(
                    "frameblending",
                    "frameblending",
                    None,
                    None,
                    boolean("value", false),
                    vec![],
                ),
            ],
        )))
    }

    /// `level` 是线性值(1 = 0 dB,clamp 到 ~3.98)。用排除淡变的音量,因为淡变作为
    /// transition 单独导出。
    fn volume_filters(&self, clip: &Clip) -> Vec<XmlNode> {
        fn clamp_level(v: f64) -> f64 {
            v.clamp(0.0, 3.98)
        }
        let frames = clip.keyframe_frames(AnimatableProperty::Volume);
        let level = if frames.is_empty() {
            if clip.volume == 1.0 {
                return Vec::new();
            }
            scalar_param(
                "level",
                "Level",
                "0",
                "3.98107",
                clamp_level(clip.volume),
                &[],
                "%.4f",
            )
        } else {
            let kfs: Vec<(i32, f64)> = frames
                .iter()
                .map(|&f| (f - clip.start_frame, clamp_level(clip.raw_volume_at(f))))
                .collect();
            scalar_param("level", "Level", "0", "3.98107", kfs[0].1, &kfs, "%.4f")
        };
        vec![filter(effect(
            "Audio Levels",
            "audiolevels",
            "audio",
            "audio",
            None,
            vec![level],
        ))]
    }

    fn video_filters(&self, clip: &Clip) -> Vec<XmlNode> {
        [
            self.motion_filter(clip),
            self.crop_filter(clip),
            self.opacity_filter(clip),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// Basic Motion:缩放、旋转、中心——关键帧化,或静态(省略默认值)。
    fn motion_filter(&self, clip: &Clip) -> Option<XmlNode> {
        let source_width = self
            .resolver
            .entry(&clip.media_ref)
            .and_then(|e| e.source_width)
            .unwrap_or(0);
        let scale_pct = |width: f64| -> f64 {
            if source_width > 0 {
                (self.seq_width as f64 / source_width as f64) * width * 100.0
            } else {
                width * 100.0
            }
        };
        // FCP7 中心用归一化坐标(0 = 居中),不是像素。
        let center = |t: &Transform| -> (f64, f64) { (t.center_x - 0.5, t.center_y - 0.5) };

        // 中心依赖位置 + 缩放,因此在所有变换参数关键帧的并集上采样。
        let mut frame_set: Vec<i32> = clip
            .keyframe_frames(AnimatableProperty::Position)
            .into_iter()
            .chain(clip.keyframe_frames(AnimatableProperty::Scale))
            .chain(clip.keyframe_frames(AnimatableProperty::Rotation))
            .collect::<HashSet<i32>>()
            .into_iter()
            .collect();
        frame_set.sort_unstable();

        let mut params: Vec<XmlNode> = Vec::new();
        if frame_set.is_empty() {
            let t = &clip.transform;
            let c = center(t);
            let scaled = scale_pct(t.width);
            let rotated = -t.rotation;
            let needs_center = c.0.abs() > 0.001 || c.1.abs() > 0.001; // 归一化,用小 epsilon
            let needs_scale = (scaled - 100.0).abs() > 0.1;
            let needs_rotation = rotated.abs() > 0.05;
            if !(needs_center || needs_scale || needs_rotation) {
                return None;
            }
            if needs_scale {
                params.push(scalar_param(
                    "scale",
                    "Scale",
                    "0",
                    "1000",
                    scaled,
                    &[],
                    "%.2f",
                ));
            }
            if needs_rotation {
                params.push(scalar_param(
                    "rotation",
                    "Rotation",
                    "-100000",
                    "100000",
                    rotated,
                    &[],
                    "%.2f",
                ));
            }
            if needs_center {
                params.push(center_param(c, &[]));
            }
        } else {
            let scale_kfs: Vec<(i32, f64)> = frame_set
                .iter()
                .map(|&f| (f - clip.start_frame, scale_pct(clip.size_at(f).0)))
                .collect();
            let rotation_kfs: Vec<(i32, f64)> = frame_set
                .iter()
                .map(|&f| (f - clip.start_frame, -clip.rotation_at(f)))
                .collect();
            let center_kfs: Vec<(i32, f64, f64)> = frame_set
                .iter()
                .map(|&f| {
                    let c = center(&clip.transform_at(f));
                    (f - clip.start_frame, c.0, c.1)
                })
                .collect();
            params.push(scalar_param(
                "scale",
                "Scale",
                "0",
                "1000",
                scale_kfs[0].1,
                &scale_kfs,
                "%.2f",
            ));
            params.push(scalar_param(
                "rotation",
                "Rotation",
                "-100000",
                "100000",
                rotation_kfs[0].1,
                &rotation_kfs,
                "%.2f",
            ));
            params.push(center_param(
                (center_kfs[0].1, center_kfs[0].2),
                &center_kfs,
            ));
        }
        Some(filter(effect(
            "Basic Motion",
            "basic",
            "motion",
            "video",
            None,
            params,
        )))
    }

    /// Crop filter —— 各边内缩为 0–100 百分比(我方模型存的是 0–1 分数)。
    fn crop_filter(&self, clip: &Clip) -> Option<XmlNode> {
        let frames = clip.keyframe_frames(AnimatableProperty::Crop);
        if frames.is_empty() && clip.crop.is_identity() {
            return None;
        }

        let edge = |id: &str, get: fn(&Crop) -> f64| -> XmlNode {
            if frames.is_empty() {
                scalar_param(id, id, "0", "100", get(&clip.crop) * 100.0, &[], "%.2f")
            } else {
                let kfs: Vec<(i32, f64)> = frames
                    .iter()
                    .map(|&f| (f - clip.start_frame, get(&clip.crop_at(f)) * 100.0))
                    .collect();
                scalar_param(id, id, "0", "100", kfs[0].1, &kfs, "%.2f")
            }
        };
        let params = vec![
            edge("left", |c| c.left),
            edge("right", |c| c.right),
            edge("top", |c| c.top),
            edge("bottom", |c| c.bottom),
        ];
        Some(filter(effect(
            "Crop",
            "crop",
            "motion",
            "video",
            Some("motion"),
            params,
        )))
    }

    /// FCP7 把透明度放在独立的 Opacity effect(Basic Motion 没有 opacity 参数)。
    fn opacity_filter(&self, clip: &Clip) -> Option<XmlNode> {
        let frames = clip.keyframe_frames(AnimatableProperty::Opacity);
        let opacity = if frames.is_empty() {
            if clip.opacity == 1.0 {
                return None;
            }
            scalar_param(
                "opacity",
                "Opacity",
                "0",
                "100",
                clip.opacity * 100.0,
                &[],
                "%.1f",
            )
        } else {
            let kfs: Vec<(i32, f64)> = frames
                .iter()
                .map(|&f| (f - clip.start_frame, clip.raw_opacity_at(f) * 100.0))
                .collect();
            scalar_param("opacity", "Opacity", "0", "100", kfs[0].1, &kfs, "%.1f")
        };
        Some(filter(effect(
            "Opacity",
            "opacity",
            "motion",
            "video",
            None,
            vec![opacity],
        )))
    }

    // MARK: - Indexing helpers

    /// 丢弃解析不到的片段,让 track 构造与 `<link>` 索引一致。
    fn sort_emittable(&self, track: &Track) -> Vec<Clip> {
        let mut clips: Vec<Clip> = track
            .clips
            .iter()
            .filter(|c| self.existing_path(&c.media_ref).is_some())
            .cloned()
            .collect();
        clips.sort_by_key(|c| c.start_frame);
        clips
    }

    fn index_addresses(&mut self, sorted_tracks: &[Vec<Clip>], is_audio: bool) {
        for (ti, clips) in sorted_tracks.iter().enumerate() {
            for (ci, clip) in clips.iter().enumerate() {
                self.clip_addresses.insert(
                    clip.id.clone(),
                    ClipAddress {
                        track_index: ti as i32 + 1,
                        clip_index: ci as i32 + 1,
                        is_audio,
                    },
                );
            }
        }
    }

    fn index_link_groups(&mut self) {
        for track in &self.timeline.tracks {
            for clip in &track.clips {
                if let Some(group) = &clip.link_group_id {
                    self.clips_by_link_group
                        .entry(group.clone())
                        .or_default()
                        .push(clip.clone());
                }
            }
        }
    }

    fn source_duration_frames(&self, media_ref: &str) -> Option<i32> {
        let seconds = self.resolver.entry(media_ref)?.duration;
        Some(seconds_to_frame(seconds, self.fps).max(0))
    }

    /// 解析到的、且在磁盘上存在的绝对路径。复刻上游 `resolveURL` 的过滤语义,但把
    /// 文件存在性检查留在本模块,不污染 domain 的零 IO 约束。
    fn existing_path(&self, media_ref: &str) -> Option<std::path::PathBuf> {
        let path = self.resolver.expected_path(media_ref)?;
        if path.is_file() {
            Some(path)
        } else {
            None
        }
    }

    fn rate(&self, timebase: i32, ntsc: bool) -> XmlNode {
        el(
            "rate",
            vec![leaf_i("timebase", timebase), boolean("ntsc", ntsc)],
        )
    }
}

// MARK: - Effect & parameter builders

fn filter(effect: XmlNode) -> XmlNode {
    el("filter", vec![effect])
}

fn effect(
    name: &str,
    id: &str,
    type_: &str,
    mediatype: &str,
    category: Option<&str>,
    body: Vec<XmlNode>,
) -> XmlNode {
    let mut children = vec![leaf("name", name), leaf("effectid", id)];
    if let Some(category) = category {
        children.push(leaf("effectcategory", category));
    }
    children.push(leaf("effecttype", type_));
    children.push(leaf("mediatype", mediatype));
    children.extend(body);
    el("effect", children)
}

/// 一个 `<parameter>`;`value` 是它的 `<value>` 节点,可由 `keyframes` 动画化。
fn parameter(
    id: &str,
    name: &str,
    min: Option<&str>,
    max: Option<&str>,
    value: XmlNode,
    keyframes: Vec<(i32, XmlNode)>,
) -> XmlNode {
    let mut children = vec![leaf("parameterid", id), leaf("name", name)];
    if let Some(min) = min {
        children.push(leaf("valuemin", min));
    }
    if let Some(max) = max {
        children.push(leaf("valuemax", max));
    }
    children.push(value);
    for (when, kf_value) in keyframes {
        children.push(el("keyframe", vec![leaf_i("when", when), kf_value]));
    }
    el("parameter", children)
}

/// 标量 `<parameter>`,其值(及关键帧)是用 `spec` 格式化的数字。
fn scalar_param(
    id: &str,
    name: &str,
    min: &str,
    max: &str,
    base: f64,
    keyframes: &[(i32, f64)],
    spec: &str,
) -> XmlNode {
    let kf_nodes: Vec<(i32, XmlNode)> = keyframes
        .iter()
        .map(|&(when, v)| (when, leaf("value", &format_spec(spec, v))))
        .collect();
    parameter(
        id,
        name,
        Some(min),
        Some(max),
        leaf("value", &format_spec(spec, base)),
        kf_nodes,
    )
}

/// 双分量 Center `<parameter>`,其值是一对 `<horiz>`/`<vert>`。
fn center_param(base: (f64, f64), keyframes: &[(i32, f64, f64)]) -> XmlNode {
    fn vec_node(x: f64, y: f64) -> XmlNode {
        el(
            "value",
            vec![
                leaf("horiz", &format!("{x:.5}")),
                leaf("vert", &format!("{y:.5}")),
            ],
        )
    }
    let kf_nodes: Vec<(i32, XmlNode)> = keyframes
        .iter()
        .map(|&(when, x, y)| (when, vec_node(x, y)))
        .collect();
    parameter(
        "center",
        "Center",
        None,
        None,
        vec_node(base.0, base.1),
        kf_nodes,
    )
}

/// 实 fps → FCP7 (timebase, ntsc)。NTSC 速率(timebase×1000/1001:29.97、23.976…)
/// 置 ntsc TRUE。1:1 对应上游 `rateTags`。
fn rate_tags(raw_fps: f64) -> (i32, bool) {
    let timebase = (raw_fps.round() as i32).max(1);
    let ntsc_rate = timebase as f64 * 1000.0 / 1001.0;
    let ntsc = (raw_fps - ntsc_rate).abs() < (raw_fps - timebase as f64).abs();
    (timebase, ntsc)
}

/// 帧数 → SMPTE 字符串;drop-frame(29.97/59.94)用 `;` 分隔并跳过被丢弃的帧。
/// 1:1 对应上游 `formatTimecode`。
fn format_timecode(frame: i32, fps: i32, drop_frame: bool) -> String {
    let mut f = frame;
    if drop_frame {
        let drop = (fps as f64 * 0.066666).round() as i32; // 30 → 2,60 → 4
        let d = f / (fps * 600);
        let m = f % (fps * 600);
        f += drop * 9 * d
            + if m > drop {
                drop * ((m - drop) / (fps * 60))
            } else {
                0
            };
    }
    let sep = if drop_frame { ";" } else { ":" };
    let ff = f % fps;
    let ss = (f / fps) % 60;
    let mm = (f / (fps * 60)) % 60;
    let hh = f / (fps * 3600);
    format!("{hh:02}{sep}{mm:02}{sep}{ss:02}{sep}{ff:02}")
}

/// 按 `spec`(支持 `%.1f`/`%.2f`/`%.4f`/`%.5f`)格式化浮点。手写而非依赖外部 crate,
/// 与上游 `String(format:)` 的小数位语义对齐。
fn format_spec(spec: &str, value: f64) -> String {
    match spec {
        "%.1f" => format!("{value:.1}"),
        "%.2f" => format!("{value:.2}"),
        "%.4f" => format!("{value:.4}"),
        "%.5f" => format!("{value:.5}"),
        _ => format!("{value}"),
    }
}

// MARK: - XML rendering

/// 一棵极简 XML 树。上面的构造器只描述文档*结构*;`render` 独占所有空白与转义,
/// 任何片段都不自带缩进。1:1 对应上游 `XMLNode` + `render`/`escapeXML`。
struct XmlNode {
    name: String,
    attributes: Vec<(String, String)>,
    text: Option<String>,   // 叶子值 → `<name>text</name>`
    children: Vec<XmlNode>, // 无子且无文本 → 自闭合 `<name/>`
}

impl XmlNode {
    /// 追加一个属性并返回自身(链式)。
    fn with_attr(mut self, name: &str, value: &str) -> Self {
        self.attributes.push((name.to_string(), value.to_string()));
        self
    }
}

fn el(name: &str, children: Vec<XmlNode>) -> XmlNode {
    XmlNode {
        name: name.to_string(),
        attributes: Vec::new(),
        text: None,
        children,
    }
}

fn el_attrs(name: &str, attrs: Vec<(&str, &str)>, children: Vec<XmlNode>) -> XmlNode {
    XmlNode {
        name: name.to_string(),
        attributes: attrs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        text: None,
        children,
    }
}

fn leaf(name: &str, value: &str) -> XmlNode {
    XmlNode {
        name: name.to_string(),
        attributes: Vec::new(),
        text: Some(value.to_string()),
        children: Vec::new(),
    }
}

fn leaf_i(name: &str, value: i32) -> XmlNode {
    leaf(name, &value.to_string())
}

fn boolean(name: &str, value: bool) -> XmlNode {
    leaf(name, if value { "TRUE" } else { "FALSE" })
}

fn render(node: &XmlNode, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let attrs: String = node
        .attributes
        .iter()
        .map(|(k, v)| format!(" {k}=\"{}\"", escape_xml(v)))
        .collect();
    if let Some(text) = &node.text {
        return format!(
            "{pad}<{}{attrs}>{}</{}>",
            node.name,
            escape_xml(text),
            node.name
        );
    }
    if node.children.is_empty() {
        return format!("{pad}<{}{attrs}/>", node.name);
    }
    let inner: Vec<String> = node
        .children
        .iter()
        .map(|c| render(c, indent + 2))
        .collect();
    format!(
        "{pad}<{}{attrs}>\n{}\n{pad}</{}>",
        node.name,
        inner.join("\n"),
        node.name
    )
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{
        AnimPair, Keyframe, KeyframeTrack, MediaManifestEntry, MediaSource, Transform,
    };
    use std::fs;
    use std::path::PathBuf;

    // --- XML 渲染器小测 ---

    #[test]
    fn render_single_element() {
        let n = leaf("name", "hi");
        assert_eq!(render(&n, 0), "<name>hi</name>");
    }

    #[test]
    fn render_self_closing_empty_element() {
        let n = el("file", vec![]).with_attr("id", "file-1");
        assert_eq!(render(&n, 0), "<file id=\"file-1\"/>");
    }

    #[test]
    fn render_nested_indents_two_spaces_per_level() {
        let n = el("a", vec![el("b", vec![leaf("c", "x")])]);
        assert_eq!(render(&n, 0), "<a>\n  <b>\n    <c>x</c>\n  </b>\n</a>");
    }

    #[test]
    fn render_escapes_text_and_attributes() {
        let n = leaf("name", "a&b<c>\"d'").with_attr("k", "<v>");
        assert_eq!(
            render(&n, 0),
            "<name k=\"&lt;v&gt;\">a&amp;b&lt;c&gt;&quot;d&apos;</name>"
        );
    }

    // --- 导出 golden 测试 ---

    /// 在临时目录建一个真实的占位文件,让 `existing_path` 的 `is_file()` 过滤通过。
    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, b"x").unwrap();
        p
    }

    fn ext_entry(
        id: &str,
        name: &str,
        kind: ClipType,
        path: &Path,
        duration: f64,
    ) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.into(),
            name: name.into(),
            kind,
            source: MediaSource::External {
                absolute_path: path.to_string_lossy().into_owned(),
            },
            duration,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        }
    }

    #[test]
    fn export_basic_video_audio_structure() {
        let dir = std::env::temp_dir().join(format!("opentake-xmeml-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let vpath = touch(&dir, "shot.mp4");
        let apath = touch(&dir, "song.mp3");

        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(ext_entry("v1", "shot.mp4", ClipType::Video, &vpath, 4.0));
        manifest
            .entries
            .push(ext_entry("a1", "song.mp3", ClipType::Audio, &apath, 10.0));

        let mut tl = Timeline::new(); // fps 30, 1920x1080
        let mut vtrack = Track::new("vt", ClipType::Video);
        vtrack.clips.push(Clip::new("c-vid", "v1", 0, 60));
        let mut atrack = Track::new("at", ClipType::Audio);
        let mut aclip = Clip::new("c-aud", "a1", 0, 90);
        aclip.media_type = ClipType::Audio;
        atrack.clips.push(aclip);
        tl.tracks.push(vtrack);
        tl.tracks.push(atrack);

        let xml = export_xmeml(&tl, &manifest, None);

        // 文档外壳。
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n"));
        assert!(xml.contains("<xmeml version=\"4\">"));
        assert!(xml.contains("<sequence id=\"sequence-1\">"));
        assert!(xml.contains("<name>Timeline Export</name>"));
        // total_frames = max end = 90。
        assert!(xml.contains("<duration>90</duration>"));
        assert!(xml.contains("<width>1920</width>"));
        assert!(xml.contains("<height>1080</height>"));

        // 视频 clipitem:start/end/in/out + 源时长(4s*30=120)。
        assert!(xml.contains("<clipitem id=\"clipitem-c-vid\">"));
        assert!(xml.contains("<start>0</start>"));
        assert!(xml.contains("<end>60</end>"));
        assert!(xml.contains("<in>0</in>"));
        assert!(xml.contains("<out>60</out>"));
        assert!(xml.contains("<duration>120</duration>")); // 源时长帧
        assert!(xml.contains("<file id=\"file-v1-video\">"));
        assert!(xml.contains("<pathurl>file://localhost//"));

        // 音频 clipitem + lane。
        assert!(xml.contains("<clipitem id=\"clipitem-c-aud\">"));
        assert!(xml.contains("<file id=\"file-a1-audio\">"));
        assert!(xml.contains("<numOutputChannels>2</numOutputChannels>"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_skips_clips_with_missing_media() {
        // 媒体清单里有条目,但磁盘上没有文件 → 该 clip 被跳过。
        let mut manifest = MediaManifest::new();
        manifest.entries.push(MediaManifestEntry {
            id: "ghost".into(),
            name: "ghost.mp4".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/nonexistent/ghost.mp4".into(),
            },
            duration: 2.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        let mut tl = Timeline::new();
        let mut vtrack = Track::new("vt", ClipType::Video);
        vtrack.clips.push(Clip::new("c-ghost", "ghost", 0, 30));
        tl.tracks.push(vtrack);

        let xml = export_xmeml(&tl, &manifest, None);
        assert!(!xml.contains("clipitem-c-ghost"));
        assert!(!xml.contains("file-ghost-video"));
    }

    #[test]
    fn export_speed_emits_time_remap() {
        let dir = std::env::temp_dir().join(format!("opentake-xmeml-speed-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let vpath = touch(&dir, "fast.mp4");
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(ext_entry("v1", "fast.mp4", ClipType::Video, &vpath, 4.0));
        let mut tl = Timeline::new();
        let mut vtrack = Track::new("vt", ClipType::Video);
        let mut clip = Clip::new("c1", "v1", 0, 30);
        clip.speed = 2.0;
        vtrack.clips.push(clip);
        tl.tracks.push(vtrack);

        let xml = export_xmeml(&tl, &manifest, None);
        assert!(xml.contains("<effectid>timeremap</effectid>"));
        // speed*100 = 200,%.4f。
        assert!(xml.contains("<value>200.0000</value>"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_static_transform_crop_opacity_filters() {
        let dir = std::env::temp_dir().join(format!("opentake-xmeml-tf-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let vpath = touch(&dir, "pic.png");
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(ext_entry("v1", "pic.png", ClipType::Image, &vpath, 5.0));
        let mut tl = Timeline::new();
        let mut vtrack = Track::new("vt", ClipType::Video);
        let mut clip = Clip::new("c1", "v1", 0, 30);
        clip.transform = Transform {
            center_x: 0.6, // -0.5 -> 0.1 居中偏移
            center_y: 0.5,
            width: 0.5, // source_width=1920 -> scale = 1*0.5*100 = 50
            height: 0.5,
            rotation: 10.0,
            flip_horizontal: false,
            flip_vertical: false,
        };
        clip.crop = Crop {
            left: 0.1,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        };
        clip.opacity = 0.5;
        vtrack.clips.push(clip);
        tl.tracks.push(vtrack);

        let xml = export_xmeml(&tl, &manifest, None);
        // Basic Motion(scale/rotation/center)。
        assert!(xml.contains("<name>Basic Motion</name>"));
        assert!(xml.contains("<name>Scale</name>"));
        assert!(xml.contains("<name>Rotation</name>"));
        assert!(xml.contains("<horiz>0.10000</horiz>"));
        // 旋转取负:-10。
        assert!(xml.contains("<value>-10.00</value>"));
        // Crop filter(left=0.1*100=10)。
        assert!(xml.contains("<name>Crop</name>"));
        // Opacity filter(0.5*100=50.0,%.1f)。
        assert!(xml.contains("<name>Opacity</name>"));
        assert!(xml.contains("<value>50.0</value>"));
        // 图片 file duration = 1。
        assert!(xml.contains("<file id=\"file-v1-video\">"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_volume_keyframes_audio_levels() {
        let dir = std::env::temp_dir().join(format!("opentake-xmeml-vol-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let apath = touch(&dir, "voice.wav");
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(ext_entry("a1", "voice.wav", ClipType::Audio, &apath, 5.0));
        let mut tl = Timeline::new();
        let mut atrack = Track::new("at", ClipType::Audio);
        let mut clip = Clip::new("c1", "a1", 0, 30);
        clip.media_type = ClipType::Audio;
        // 静态音量 1.0 + 关键帧轨(dB)。
        clip.volume_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::new(0, 0.0),
            Keyframe::new(30, 0.0),
        ]));
        atrack.clips.push(clip);
        tl.tracks.push(atrack);

        let xml = export_xmeml(&tl, &manifest, None);
        assert!(xml.contains("<name>Audio Levels</name>"));
        assert!(xml.contains("<name>Level</name>"));
        // 关键帧用 clip-relative when。
        assert!(xml.contains("<when>0</when>"));
        assert!(xml.contains("<when>30</when>"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_fade_transitions() {
        let dir = std::env::temp_dir().join(format!("opentake-xmeml-fade-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let vpath = touch(&dir, "clip.mp4");
        let apath = touch(&dir, "clip.wav");
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(ext_entry("v1", "clip.mp4", ClipType::Video, &vpath, 4.0));
        manifest
            .entries
            .push(ext_entry("a1", "clip.wav", ClipType::Audio, &apath, 4.0));
        let mut tl = Timeline::new();
        let mut vtrack = Track::new("vt", ClipType::Video);
        let mut vclip = Clip::new("cv", "v1", 0, 60);
        vclip.fade_in_frames = 10;
        vclip.fade_out_frames = 10;
        vtrack.clips.push(vclip);
        let mut atrack = Track::new("at", ClipType::Audio);
        let mut aclip = Clip::new("ca", "a1", 0, 60);
        aclip.media_type = ClipType::Audio;
        aclip.fade_in_frames = 10;
        atrack.clips.push(aclip);
        tl.tracks.push(vtrack);
        tl.tracks.push(atrack);

        let xml = export_xmeml(&tl, &manifest, None);
        // 视频淡变 Cross Dissolve + cutPointTicks。
        assert!(xml.contains("<name>Cross Dissolve</name>"));
        assert!(xml.contains("<alignment>start-black</alignment>"));
        assert!(xml.contains("<alignment>end-black</alignment>"));
        // 淡出 cutPointTicks = 10 * (254016000000/30)。
        let expected_ticks = 10i64 * (254_016_000_000i64 / 30);
        assert!(xml.contains(&format!("<cutPointTicks>{expected_ticks}</cutPointTicks>")));
        // 音频淡入 Cross Fade。
        assert!(xml.contains("<name>Cross Fade ( 0dB)</name>"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_linked_av_emits_reciprocal_links() {
        let dir = std::env::temp_dir().join(format!("opentake-xmeml-link-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let vpath = touch(&dir, "av.mp4");
        let apath = touch(&dir, "av.wav");
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(ext_entry("v1", "av.mp4", ClipType::Video, &vpath, 4.0));
        manifest
            .entries
            .push(ext_entry("a1", "av.wav", ClipType::Audio, &apath, 4.0));
        let mut tl = Timeline::new();
        let mut vtrack = Track::new("vt", ClipType::Video);
        let mut vclip = Clip::new("cv", "v1", 0, 60);
        vclip.link_group_id = Some("g1".into());
        vtrack.clips.push(vclip);
        let mut atrack = Track::new("at", ClipType::Audio);
        let mut aclip = Clip::new("ca", "a1", 0, 60);
        aclip.media_type = ClipType::Audio;
        aclip.link_group_id = Some("g1".into());
        atrack.clips.push(aclip);
        tl.tracks.push(vtrack);
        tl.tracks.push(atrack);

        let xml = export_xmeml(&tl, &manifest, None);
        // 互相 link:视频 clipitem 引用音频伙伴,反之亦然。
        assert!(xml.contains("<linkclipref>clipitem-cv</linkclipref>"));
        assert!(xml.contains("<linkclipref>clipitem-ca</linkclipref>"));
        assert!(xml.contains("<masterclipid>masterclip-g1</masterclipid>"));
        // 1-based 索引。
        assert!(xml.contains("<trackindex>1</trackindex>"));
        assert!(xml.contains("<clipindex>1</clipindex>"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_duplicate_file_reference_collapses() {
        let dir = std::env::temp_dir().join(format!("opentake-xmeml-dup-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let vpath = touch(&dir, "same.mp4");
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(ext_entry("v1", "same.mp4", ClipType::Video, &vpath, 4.0));
        let mut tl = Timeline::new();
        let mut vtrack = Track::new("vt", ClipType::Video);
        vtrack.clips.push(Clip::new("c1", "v1", 0, 30));
        vtrack.clips.push(Clip::new("c2", "v1", 30, 30));
        tl.tracks.push(vtrack);

        let xml = export_xmeml(&tl, &manifest, None);
        // 第一处完整发出(带 pathurl);第二处折叠为自闭合。
        assert!(xml.contains("<file id=\"file-v1-video\">\n"));
        assert!(xml.contains("<file id=\"file-v1-video\"/>"));
        // 完整 file 节点(含 pathurl)只发出一次,即使两个 clipitem 引用同一素材。
        assert_eq!(xml.matches("<pathurl>").count(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_position_scale_keyframes_motion() {
        let dir = std::env::temp_dir().join(format!("opentake-xmeml-mkf-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let vpath = touch(&dir, "anim.mp4");
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(ext_entry("v1", "anim.mp4", ClipType::Video, &vpath, 4.0));
        let mut tl = Timeline::new();
        let mut vtrack = Track::new("vt", ClipType::Video);
        let mut clip = Clip::new("c1", "v1", 0, 30);
        clip.scale_track = Some(KeyframeTrack::from_keyframes(vec![
            Keyframe::new(0, AnimPair::new(1.0, 1.0)),
            Keyframe::new(15, AnimPair::new(0.5, 0.5)),
        ]));
        vtrack.clips.push(clip);
        tl.tracks.push(vtrack);

        let xml = export_xmeml(&tl, &manifest, None);
        assert!(xml.contains("<name>Basic Motion</name>"));
        // 关键帧并集采样 → scale/rotation/center 都带 <keyframe>。
        assert!(xml.contains("<keyframe>"));
        assert!(xml.contains("<when>0</when>"));
        assert!(xml.contains("<when>15</when>"));
        fs::remove_dir_all(&dir).ok();
    }
}
