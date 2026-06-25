//! Editing skeleton (type → approach + flow) and editing-stage inference with
//! stage guidance (`agent-SPEC.md` §6.3/§6.5). Skeleton flow text is VERBATIM
//! from the doc.

use opentake_domain::{
    ClipType, EditingSkeleton, EditingStage, StageGuidance, Timeline, VideoType,
};

/// The editing skeleton for a video type (`AGENT-CONTEXT-SIGNAL.md` §2.3; flow
/// text verbatim).
pub fn editing_skeleton(video_type: VideoType) -> EditingSkeleton {
    match video_type {
        VideoType::TalkingHead => EditingSkeleton {
            approach: "audio_driven".into(),
            flow: vec![
                "提取主音轨".into(),
                "转写为字幕".into(),
                "识别气口/断点".into(),
                "精剪 A-roll".into(),
                "语义匹配 B-roll".into(),
                "贴画面上层".into(),
                "BGM 卡点".into(),
                "调色导出".into(),
            ],
            rules: vec![],
        },
        VideoType::Montage => EditingSkeleton {
            approach: "montage_beat".into(),
            flow: vec![
                "铺主音乐".into(),
                "检测节拍/重音".into(),
                "素材按景别分类(远/中/特)".into(),
                "景别递进匹配镜头".into(),
                "在节拍点切镜".into(),
                "调色导出".into(),
            ],
            rules: vec![],
        },
        VideoType::Vlog => EditingSkeleton {
            approach: "vlog_segment".into(),
            flow: vec![
                "乱序思维导图".into(),
                "提炼主线".into(),
                "分段式独立剪辑".into(),
                "旁白/节奏点串联".into(),
                "时钟理论布置爆点".into(),
                "调色导出".into(),
            ],
            rules: vec![],
        },
        VideoType::Interview => EditingSkeleton {
            approach: "interview_multicam".into(),
            flow: vec![
                "按音频波形对齐合板".into(),
                "导播式粗剪(谁说切谁)".into(),
                "加人名条".into(),
                "提取金句".into(),
                "BGM 铺底".into(),
                "导出".into(),
            ],
            rules: vec![],
        },
        VideoType::ShortForm => EditingSkeleton {
            approach: "audio_driven".into(),
            flow: vec![
                "提取主音轨".into(),
                "转写为字幕".into(),
                "精剪节奏".into(),
                "贴大字幕".into(),
                "卡点配乐".into(),
                "竖屏安全区检查".into(),
                "导出".into(),
            ],
            rules: vec![],
        },
        VideoType::LongForm => EditingSkeleton {
            approach: "audio_driven".into(),
            flow: vec![
                "章节切分".into(),
                "逐章精剪口播".into(),
                "贴 B-roll 与图示".into(),
                "章节转场".into(),
                "BGM 铺底".into(),
                "调色导出".into(),
            ],
            rules: vec![],
        },
    }
}

/// Infer the current editing stage from the timeline shape (a coarse heuristic
/// over `EditingStage`):
/// - no tracks → Importing
/// - tracks but no clips → Classifying
/// - clips, no caption/text overlays → RoughCut
/// - caption/text present, no separate B-roll track → AudioPolish
/// - a B-roll overlay present → BRollOverlay
pub fn infer_stage(timeline: &Timeline) -> EditingStage {
    if timeline.tracks.is_empty() {
        return EditingStage::Importing;
    }
    let total_clips: usize = timeline.tracks.iter().map(|t| t.clips.len()).sum();
    if total_clips == 0 {
        return EditingStage::Classifying;
    }
    let has_captions = timeline
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .any(|c| c.caption_group_id.is_some() || c.media_type == ClipType::Text);
    let video_tracks_with_clips = timeline
        .tracks
        .iter()
        .filter(|t| t.kind == ClipType::Video && !t.clips.is_empty())
        .count();
    if video_tracks_with_clips >= 2 {
        EditingStage::BRollOverlay
    } else if has_captions {
        EditingStage::AudioPolish
    } else {
        EditingStage::RoughCut
    }
}

/// Built-in guidance for a stage (`description` + `next_actions` + `warnings`).
/// Plugin stages are appended on top by the plugin layer.
pub fn stage_guidance(stage: EditingStage) -> StageGuidance {
    match stage {
        EditingStage::Importing => StageGuidance {
            description: "导入阶段：工程还没有轨道。先导入素材。".into(),
            next_actions: vec![
                "导入口播、B-roll、音乐素材".into(),
                "调用 get_media 确认资源".into(),
            ],
            warnings: vec![],
        },
        EditingStage::Classifying => StageGuidance {
            description: "分类阶段：有轨道但还没有 clip。按景别/用途归类素材。".into(),
            next_actions: vec!["将素材放上对应轨道".into(), "先铺主音轨/主画面".into()],
            warnings: vec![],
        },
        EditingStage::RoughCut => StageGuidance {
            description: "粗剪阶段：主时间线已铺好，开始精剪口播。".into(),
            next_actions: vec![
                "识别并标记所有气口和句界断点".into(),
                "删除啰嗦/重复/卡顿段".into(),
            ],
            warnings: vec!["不要在词中间切分".into()],
        },
        EditingStage::BRollOverlay => StageGuidance {
            description: "贴 B-roll 阶段：在主画面上层补充镜头。".into(),
            next_actions: vec![
                "按口播语义匹配 B-roll".into(),
                "对齐口播时长，成组添加".into(),
            ],
            warnings: vec!["B-roll 不要重复，整轨静音".into()],
        },
        EditingStage::AudioPolish => StageGuidance {
            description: "音频精修阶段：处理气口、音量、BGM 让位。".into(),
            next_actions: vec![
                "BGM 在口播段压低让位人声".into(),
                "段落间做 J/L-cut 过渡".into(),
            ],
            warnings: vec!["不可整轨静音主声音轨".into()],
        },
        EditingStage::ColorGrade => StageGuidance {
            description: "调色阶段：统一画面色调。".into(),
            next_actions: vec!["统一白平衡与曝光".into(), "按风格做 LUT/曲线".into()],
            warnings: vec![],
        },
        EditingStage::ExportReady => StageGuidance {
            description: "导出就绪：检查画幅与导出参数。".into(),
            next_actions: vec!["按平台选择导出参数".into()],
            warnings: vec![],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, ClipType, Track};

    #[test]
    fn skeletons_have_verbatim_flow() {
        let s = editing_skeleton(VideoType::TalkingHead);
        assert_eq!(s.approach, "audio_driven");
        assert_eq!(s.flow.first().map(String::as_str), Some("提取主音轨"));
        assert!(s.flow.contains(&"识别气口/断点".to_string()));

        let m = editing_skeleton(VideoType::Montage);
        assert_eq!(m.approach, "montage_beat");
        assert!(m.flow.contains(&"在节拍点切镜".to_string()));

        let it = editing_skeleton(VideoType::Interview);
        assert_eq!(it.approach, "interview_multicam");
        assert!(it.flow.contains(&"加人名条".to_string()));
    }

    #[test]
    fn empty_timeline_is_importing() {
        assert_eq!(infer_stage(&Timeline::new()), EditingStage::Importing);
    }

    #[test]
    fn tracks_without_clips_is_classifying() {
        let mut tl = Timeline::new();
        tl.tracks.push(Track::new("v1", ClipType::Video));
        assert_eq!(infer_stage(&tl), EditingStage::Classifying);
    }

    #[test]
    fn single_video_clips_is_rough_cut() {
        let mut tl = Timeline::new();
        let mut v = Track::new("v1", ClipType::Video);
        v.clips.push(Clip::new("a", "asset", 0, 30));
        tl.tracks.push(v);
        assert_eq!(infer_stage(&tl), EditingStage::RoughCut);
    }

    #[test]
    fn two_video_tracks_with_clips_is_broll() {
        let mut tl = Timeline::new();
        let mut v1 = Track::new("v1", ClipType::Video);
        v1.clips.push(Clip::new("a", "asset", 0, 30));
        let mut v2 = Track::new("v2", ClipType::Video);
        v2.clips.push(Clip::new("b", "asset", 0, 30));
        tl.tracks.push(v1);
        tl.tracks.push(v2);
        assert_eq!(infer_stage(&tl), EditingStage::BRollOverlay);
    }

    #[test]
    fn rough_cut_guidance_warns_not_mid_word() {
        let g = stage_guidance(EditingStage::RoughCut);
        assert!(g.warnings.iter().any(|w| w.contains("不要在词中间切分")));
        assert!(!g.next_actions.is_empty());
    }
}
