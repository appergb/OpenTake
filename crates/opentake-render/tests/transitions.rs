//! Owning acceptance test for the first advertised editable transition.

use std::collections::HashMap;
use std::rc::Rc;

use opentake_domain::{Clip, ClipType, Timeline, Track, Transform, TransitionKind};
use opentake_ops::{apply, EditCommand, EditorState, SeqIdGen};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::source::DecodedFrame;
use opentake_render::wgpu;
use opentake_render::{
    build_render_plan, Compositor, GpuTexture, RenderDevice, RenderSize, SourceMetrics,
    TextureResolver, TextureSource,
};

const SIZE: RenderSize = RenderSize {
    width: 16,
    height: 16,
};

struct Metrics;

impl SourceMetrics for Metrics {
    fn natural_size(&self, _media_ref: &str) -> Option<(u32, u32)> {
        Some((16, 16))
    }
}

struct PairResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    cache: HashMap<String, Rc<GpuTexture>>,
}

impl TextureResolver for PairResolver<'_> {
    fn resolve(&mut self, source: &TextureSource, _frame: i64) -> Option<Rc<GpuTexture>> {
        let media_ref = match source {
            TextureSource::Decoded { media_ref }
            | TextureSource::Image { media_ref }
            | TextureSource::Lottie { media_ref } => media_ref,
            TextureSource::Text { .. } => return None,
        };
        if let Some(texture) = self.cache.get(media_ref) {
            return Some(texture.clone());
        }
        let color = match media_ref.as_str() {
            "red" => [255, 0, 0, 255],
            "blue" => [0, 0, 255, 255],
            other => panic!("unexpected transition source {other}"),
        };
        let mut rgba = vec![0; 16 * 16 * 4];
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.copy_from_slice(&color);
        }
        let frame = DecodedFrame::new(16, 16, rgba, true);
        let texture = Rc::new(upload_rgba(
            self.device,
            self.queue,
            &frame,
            false,
            Some("transition fixture"),
        ));
        self.cache.insert(media_ref.clone(), texture.clone());
        Some(texture)
    }
}

fn transition_timeline() -> Timeline {
    let mut timeline = Timeline::new();
    timeline.fps = 30;
    timeline.width = 16;
    timeline.height = 16;
    let mut outgoing = Clip::new("a", "red", 0, 12);
    outgoing.transform = Transform::default();
    let mut incoming = Clip::new("b", "blue", 12, 12);
    incoming.transform = Transform::default();
    let mut track = Track::new("v", ClipType::Video);
    track.clips = vec![outgoing, incoming];
    timeline.tracks.push(track);
    timeline
}

fn render_frame(device: &RenderDevice, timeline: &Timeline, frame: i32) -> DecodedFrame {
    let plan = build_render_plan(timeline, SIZE, &Metrics);
    let frame_plan = plan.frame(timeline, frame);
    let compositor = Compositor::new(&device.device);
    let mut resolver = PairResolver {
        device: &device.device,
        queue: &device.queue,
        cache: HashMap::new(),
    };
    compositor
        .render_to_rgba(
            &device.device,
            &device.queue,
            SIZE,
            &frame_plan,
            &mut resolver,
        )
        .expect("render transition frame")
}

fn center_pixel(frame: &DecodedFrame) -> [u8; 4] {
    let index = ((frame.height / 2 * frame.width + frame.width / 2) * 4) as usize;
    frame.rgba[index..index + 4].try_into().unwrap()
}

fn assert_pixel_near(actual: [u8; 4], expected: [u8; 4]) {
    for channel in 0..4 {
        assert!(
            (actual[channel] as i16 - expected[channel] as i16).abs() <= 3,
            "expected {expected:?}, got {actual:?}"
        );
    }
}

#[test]
fn adjacent_clip_transition_is_editable_undoable_and_matches_preview_export() {
    let ids = SeqIdGen::default();
    let mut state = EditorState::from_timeline(transition_timeline());
    apply(
        &mut state,
        EditCommand::SetTransition {
            from_clip_id: "a".into(),
            to_clip_id: "b".into(),
            kind: Some(TransitionKind::CrossDissolve),
            duration_frames: 4,
        },
        &ids,
    )
    .expect("add advertised transition");

    let transition = state.timeline.tracks[0].clips[0]
        .transition_out
        .as_ref()
        .expect("transition persisted on outgoing clip");
    assert_eq!(transition.to_clip_id, "b");
    assert_eq!(transition.kind, TransitionKind::CrossDissolve);
    assert_eq!(transition.duration_frames, 4);

    // Pair identity must be explicit in the saved object, not inferred solely
    // from whichever clip happens to contain it after reopening.
    let saved = serde_json::to_string(&state.timeline).expect("save timeline JSON");
    assert!(saved.contains(r#""fromClipId":"a""#));
    assert!(saved.contains(r#""toClipId":"b""#));
    let reopened: Timeline = serde_json::from_str(&saved).expect("reopen timeline JSON");
    assert_eq!(reopened, state.timeline);

    apply(&mut state, EditCommand::Undo, &ids).expect("undo transition");
    assert!(state.timeline.tracks[0].clips[0].transition_out.is_none());
    apply(&mut state, EditCommand::Redo, &ids).expect("redo transition");
    assert_eq!(state.timeline, reopened);

    apply(
        &mut state,
        EditCommand::SetTransition {
            from_clip_id: "a".into(),
            to_clip_id: "b".into(),
            kind: Some(TransitionKind::CrossDissolve),
            duration_frames: 3,
        },
        &ids,
    )
    .expect("change transition duration");
    assert_eq!(
        state.timeline.tracks[0].clips[0]
            .transition_out
            .as_ref()
            .unwrap()
            .duration_frames,
        3
    );
    apply(
        &mut state,
        EditCommand::SetTransition {
            from_clip_id: "a".into(),
            to_clip_id: "b".into(),
            kind: None,
            duration_frames: 3,
        },
        &ids,
    )
    .expect("remove transition");
    assert!(state.timeline.tracks[0].clips[0].transition_out.is_none());
    apply(&mut state, EditCommand::Undo, &ids).expect("undo transition removal");
    assert_eq!(
        state.timeline.tracks[0].clips[0]
            .transition_out
            .as_ref()
            .unwrap()
            .duration_frames,
        3
    );

    // A 12-frame pair exposes a six-frame centered transition handle. An
    // oversized request is an explicit refusal and must not mutate history.
    let mut invalid = EditorState::from_timeline(transition_timeline());
    apply(
        &mut invalid,
        EditCommand::SetTransition {
            from_clip_id: "a".into(),
            to_clip_id: "b".into(),
            kind: Some(TransitionKind::CrossDissolve),
            duration_frames: 7,
        },
        &ids,
    )
    .expect_err("transition longer than either available handle must be rejected");
    assert_eq!(invalid.version(), 0);
    assert!(!invalid.can_undo());

    let device = match RenderDevice::try_new() {
        Ok(device) => device,
        Err(error) => {
            eprintln!("[skip] transition pixel fixture: no GPU device ({error})");
            return;
        }
    };

    // Duration four covers frames 8..12. Exercise the first transition frame,
    // midpoint, last blended frame, and the exact cut/end frame. Preview and
    // export are represented by fresh compositor/resolver executions.
    for (frame, golden) in [
        (8, [255, 0, 0, 255]),
        (10, [128, 0, 128, 255]),
        (11, [64, 0, 191, 255]),
        (12, [0, 0, 255, 255]),
    ] {
        let preview = render_frame(&device, &reopened, frame);
        let export = render_frame(&device, &reopened, frame);
        assert_eq!(
            preview.rgba, export.rgba,
            "preview/export drift at frame {frame}"
        );
        assert_pixel_near(center_pixel(&preview), golden);
    }
}
