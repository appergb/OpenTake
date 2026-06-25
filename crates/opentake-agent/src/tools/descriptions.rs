//! Tool descriptions + input schemas, 1:1 with upstream `ToolDefinitions.swift`
//! (`agent-SPEC.md` §2.2). The description strings are copied VERBATIM — they
//! carry the behavior contract that drives the LLM (ARCHITECTURE §7). The ONLY
//! edits are product-name substitutions: `Palmier`/`palmier` → `OpenTake`/
//! `opentake`, and the `palmier://models/*` resource URI → `opentake://`.
//!
//! Schemas are the upstream `inputSchema` JSON, used directly as the MCP tool
//! schema. `additionalProperties` is enforced at runtime by the unknown-key
//! guard in `tools::errors` (which also covers nested `entries[]`, something
//! JSON Schema `additionalProperties:false` can't reach per upstream).

use serde_json::{json, Value};

use crate::tools::names::ToolName;

/// The description string for a tool (verbatim from upstream, product-name
/// adjusted).
pub fn description(tool: ToolName) -> &'static str {
    match tool {
        ToolName::GetTimeline => "Always call at the start of a session. Returns project settings (fps, resolution, totalFrames), track list with types and order, all clips with their frames and properties, and canGenerate (if false, generation/upscale tools will fail — tell the user to sign in to OpenTake and subscribe before attempting them). The clipId/trackId values here are what every other tool accepts.\n\nClip and track fields equal to their defaults are omitted: mediaType 'video', sourceClipType = mediaType, speed 1, volume 1, opacity 1, trims/fades 0, identity transform/crop, default textStyle, track muted/hidden false. Text clips never report trims (no source media).\n\nCaption clips (sharing a captionGroupId) come back per track as captionGroups instead of clips entries: properties common to the group are hoisted into 'shared' and each clip is a [clipId, startFrame, durationFrames, text] row (caption box width/height are auto-fit per text and omitted). Rows are capped at 200 per group — when clipCount exceeds the rows shown, page with startFrame/endFrame. Caption clips whose properties deviate from the group appear individually in clips.",

        ToolName::GetMedia => "Call before referencing any asset. Every mediaRef/reference ID in other tools comes from the IDs returned here. Also exposes generationStatus (generating | downloading | failed | none) for async-generated and -imported assets.",

        ToolName::InspectMedia => "Look at a media asset before referencing or editing it. Images: the image plus dimensions and EXIF. Video: sample frames plus a transcription of the audio track. Audio: transcription. Lottie: frames sampled evenly across the animation (over gray), plus framerate and duration — use this to verify a Lottie you wrote looks and moves right. Transcription is sentence-level segments — [text, start, end] tuples, capped at 400 — in source seconds, or project frames when clipId is set. When capped, pass the returned nextStartSeconds as startSeconds for the next page.\n\nLong media: pass overview=true for a one-image storyboard, read the segments, then re-call with startSeconds/endSeconds to zoom — windowed calls only transcribe that span, so they are fast.",

        ToolName::GetTranscript => "Returns the spoken transcript of the CURRENT timeline in project frames — the post-edit caption track in one call. Unlike inspect_media (which transcribes one source asset in isolation, in source seconds), this walks every audio/video clip on the timeline, maps each word through that clip's trim/speed/position, and concatenates in timeline order. Deleted ranges are gone by construction, so after cuts this always reflects what's actually audible — no stale results, no per-clip frame math.\n\nReturns clips in timeline order, each with its words nested as compact [text, startFrame, endFrame] rows (the field order is given once in wordFormat) — clipId and trackIndex are stated once per clip, not repeated per word. Words are monotonic and non-overlapping; each is attributed to one clip, so a word split across a clip seam is emitted once, not re-emitted per clip. Pass a clip's clipId and a word's frames straight to ripple_delete_ranges. Capped at 10000 words total; page with startFrame/endFrame using nextStartFrame. Pass clipId to scope to a single clip (\"what does this clip say?\"). Transcription runs on-device.\n\nUse for transcript-driven edits (filler-word / dead-air removal, locating a quote, take selection) and to verify what remains after cutting.",

        ToolName::InspectTimeline => "See the composited timeline — what the user actually sees in the preview at a given frame: all video tracks stacked with their transforms, opacity, crop, and keyframes applied, plus text and caption overlays baked in. Use this to verify your edits landed (a PIP's position, a title's placement, layer order) — inspect_media shows the raw source asset, not the cut.\n\nFrames are project frames (from get_timeline). Pass a single startFrame for one composited frame; add endFrame to sample maxFrames evenly across [startFrame, endFrame) for a transition or sequence. Frames past content render black. Returns frames downscaled for token efficiency, with the frameNumbers sampled.",

        ToolName::SearchMedia => "Search the media library by content: what's on screen (visual) and what's said (spoken). Visual matching is semantic and on-device — phrase the query like an image caption ('a wide shot of a harbor at sunset'), not keywords; covers videos and stills. Spoken matching layers exact keywords over on-device semantic matching of transcript segments — quote the words said, or paraphrase them; transcripts are created automatically while indexing (and by inspect_media and add_captions), so coverage grows as indexing completes. The two groups rank independently and are never blended. Scores are uncalibrated — use them for ordering only.\n\nHits are source-second ranges. To place exactly that moment, multiply by fps and pass as trimStartFrame/trimEndFrame with a matching durationFrames to add_clips or set_clip_properties. Image hits have no time range.\n\nstatus reports the visual index: ready | indexing | modelNotInstalled | downloadingModel | preparing | disabled | failed. When not ready, moments may be empty or incomplete (compare indexedAssets to indexableAssets) — report that instead of concluding the footage doesn't exist, and don't poll in a loop. Spoken results work regardless of status.",

        ToolName::AddClips => "Places one or more media assets on the timeline as a single undoable action. Each entry's asset type must be compatible with its target track (video/image are interchangeable across video/image tracks; audio requires an audio track). When a video asset with audio is placed on a video track, a linked audio clip is automatically created on an audio track (an existing one if available, otherwise a new one). The whole batch is one undo step.\n\ntrackIndex is optional. Omit it on all entries and the tool auto-creates the needed tracks — one shared video track for visual entries and one shared audio track for audio entries (matches the captioning pattern in add_texts). To target existing tracks, set trackIndex on every entry. Mixing (some entries specify, others omit) is rejected — split into two calls.\n\nTracks work as layers: clips on the SAME track are sequential — if a new clip's range overlaps an existing clip on that track, the existing clip is trimmed/split/removed to make room, matching the UI's drag-onto-track overwrite behavior.",

        ToolName::InsertClips => "Inserts one or more media assets at a single point and RIPPLES: every clip at or after atFrame is pushed right to open a gap, so nothing is overwritten. This is the non-destructive counterpart to add_clips (which clears the landing region, trimming/splitting/removing whatever's there). Use insert_clips to splice footage in without losing existing clips; use add_clips to fill empty space or deliberately overwrite.\n\nEntries are laid end-to-end starting at atFrame on the target track (entry[0] at atFrame, entry[1] immediately after, ...). The push equals the sum of the entries' durations and is applied to the target track, every sync-locked track, AND the audio track any auto-created linked audio lands on — so a clip and its linked audio stay aligned. As in add_clips, a video asset with audio spawns a linked audio clip. One undoable action; one bad entry rejects the whole call with no partial state.\n\ntrackIndex is required — ripple needs an existing track to push. For placement into empty space, use add_clips.",

        ToolName::RemoveClips => "Removes one or more clips by ID as a single undoable action. Any clip that belongs to a link group (e.g. a video with its paired audio) takes its whole group with it, matching the UI's linked-delete behavior.",

        ToolName::RemoveTracks => "Removes whole tracks and every clip on them in one undoable action. Linked partners on OTHER tracks are not removed. Remaining track indexes shift down after removal.",

        ToolName::MoveClips => "Moves one or more clips to a new track and/or frame position. Single undoable action. Each move specifies the clip ID and at least one of toTrack (must be compatible with the clip's media type) and toFrame. Overlap on the destination is resolved as in add_clips (existing clips on the destination track are trimmed/split/removed). Linked partners follow the named clip: startFrame propagates as a delta to preserve l-cut / j-cut offsets; tracks stay with the named clip.",

        ToolName::SetClipProperties => "Apply the same property values to one or more clips in a single undoable action. Pass any combination of durationFrames, trimStartFrame, trimEndFrame, speed, volume, opacity, transform, or — for text clips only — content, fontName, fontSize, color, alignment. All values are applied to every clip in clipIds; for per-clip differences, make separate calls. trimStartFrame/trimEndFrame are offsets from the source media, not the timeline. speed 1.0 is normal, <1.0 slows (clip gets longer on the timeline), >1.0 speeds up. volume and opacity are 0.0–1.0. transform uses 0–1 normalized canvas coords, partial merge (pass only centerY to reposition vertically); flipHorizontal/flipVertical mirror the clip across the corresponding axis (no effect on text clips). When a text clip's content or font changes without an explicit transform, the bounding box auto-refits. Text-only fields with any non-text clip in clipIds are rejected.\n\nFor moves and start-frame changes, use move_clips. For animated values (keyframes), use set_keyframes — setting volume or opacity here clears any existing keyframe track on that property.\n\nTiming changes (durationFrames, trimStartFrame, trimEndFrame, speed) on a linked clip carry over to its linked partner so audio/video stay in sync — same as the timeline UI. Per-clip fields (volume, opacity, transform, text*) don't propagate. trim and speed are skipped for text partners.",

        ToolName::SetKeyframes => "Set animated keyframes on one property of one clip. Replaces the existing keyframe track for that property (pass an empty array to clear). Frames are CLIP-RELATIVE offsets (0 = first frame of the clip), so keyframes follow the clip when it moves. Rows are sorted by frame internally and the LAST row for any duplicate frame wins. Values must be finite numbers. Each row is `[frame, ...values, interp?]` where interp ∈ {linear, hold, smooth} (default smooth).\n\nProperties and their value layouts:\n  • volume `[frame, value]` — value 0.0–1.0\n  • opacity `[frame, value]` — value 0.0–1.0\n  • rotation `[frame, degrees]` — clockwise degrees\n  • position `[frame, topLeftX, topLeftY]` — TOP-LEFT corner in 0–1 normalized canvas coords. NOT the center. (Default static transform centers a full-canvas clip, so top-left of the static is (0, 0); a centered half-size clip has top-left (0.25, 0.25).)\n  • scale `[frame, width, height]` — clip's normalized width and height in 0–1 canvas coords (1.0 = fills the canvas axis). NOT a scale factor.\n  • crop `[frame, top, right, bottom, left]` — side insets in 0–1 of the source media.\n\nMotion keyframes (position/scale/rotation) override the static `transform` value when active.",

        ToolName::SplitClip => "Splits a clip into two at atFrame. The frame must be strictly between the clip's start and end — use get_timeline to confirm the range.",

        ToolName::RippleDeleteRanges => "Cuts one or more ranges out and closes the gaps in one undoable action — the fast path for filler-word/dead-air removal. Replaces hand-cranked split_clip → split_clip → remove_clips → move_clips loops: pass every range at once.\n\nTwo modes — pass exactly one of clipId or trackIndex:\n• trackIndex (preferred for transcript-driven cuts): ranges are PROJECT frames and may span any number of clips on that track. get_transcript returns a clips array with nested words in project frames — collect every cut across the whole timeline and pass them in ONE call, no per-clip splitting and no re-reading the timeline between cuts. units must be 'frames'.\n• clipId: ranges are cut within that single clip only, clamped to its visible span. Allows units 'seconds' (source-media seconds, e.g. inspect_media WITHOUT a clipId or search_media hits); 'frames' = project frames. Use when you already have one clip's per-word timestamps.\n\nOverlapping ranges merge. Linked audio/video partners of every touched clip are cut on the same span so A/V stays in sync. Remaining clips shift left to close every gap; sync-locked tracks shift along to preserve alignment (their content isn't cut). Refuses without changing anything if a sync-locked track can't absorb the shift (e.g. it would move past frame 0). Returns the anchor track's post-cut layout (clip ids/frames) so you don't need to re-read.",

        ToolName::Undo => "Reverts the assistant's most recent timeline edit (a cut, move, trim, split, or clip/text/caption add) as one step. The recovery path when an edit went too far — e.g. a ripple_delete_ranges removed more than intended. Verify a cut first (get_transcript reflects the post-cut audio), then undo if it overshot, then retry with corrected ranges.\n\nUndoes only edits the assistant made this session, most-recent-first — it never touches the user's own manual edits, and refuses if the latest change wasn't the assistant's. After undoing, the timeline is restored to its state before that edit; the ids/frames the edit returned are no longer valid, so re-read with get_timeline or get_transcript if you'll edit again. Takes no arguments.",

        ToolName::AddTexts => "Adds one or more text clips (titles, captions, lower-thirds) in a single undoable action. Text renders as an overlay on top of visual media. Transform uses 0–1 normalized canvas coords: (0.5,0.5) is center, (0.5,0.1) top-center, (0.5,0.9) bottom-center. Omit transform to center + auto-fit. Pass only centerX/centerY to reposition with auto-fit size (common for lower-thirds). Pass all four fields to override the box entirely. Colors are hex '#RRGGBB' or '#RRGGBBAA'.\n\ntrackIndex is optional. Omit it on all entries and the tool auto-creates one new video track at the top and places all text clips there — the common case for captions. To target existing tracks, set trackIndex on every entry (audio tracks rejected). Mixing (some entries specify, others omit) is rejected — split into two calls.\n\nTracks work as layers: clips on the SAME track are sequential — if a new clip's range overlaps an existing (or earlier-batch) clip on that track, the existing clip is trimmed/split/removed to make room, matching the UI's drag-onto-track overwrite behavior. To show multiple text clips at the same time (stacked titles, simultaneous labels), put each on a DIFFERENT trackIndex so they layer instead of trimming each other.\n\nFor captioning spoken audio, prefer add_captions — it transcribes and places styled caption clips in one call. Use add_texts only for bespoke text (titles, lower-thirds) or captioning a custom range by hand. Unknown fields are rejected.",

        ToolName::AddCaptions => "Auto-caption spoken audio: transcribes on-device and places styled caption clips on a new track — the same pipeline as the editor's Captions tab. This is the reliable path for 'caption this'; prefer it over hand-placing add_texts from a transcript. Omit clipIds to auto-pick the track with the most speech; pass clipIds to caption specific clips (e.g. only the interview).",

        ToolName::GenerateVideo => "Starts an async AI video generation. Returns a placeholder asset ID immediately; generation runs in the background and the asset becomes usable in add_clips once ready. Costs real money and is not undoable.",

        ToolName::GenerateImage => "Starts an async AI image generation. Returns a placeholder asset ID immediately; generation runs in the background. Costs real money and is not undoable.",

        ToolName::GenerateAudio => "Starts an async AI audio generation: text-to-speech, text-to-music, or video-to-music (scoring a video). Returns a placeholder asset ID immediately; the asset appears in get_media and becomes usable in add_clips once ready. TTS models (elevenlabs-tts-v3, gemini-3.1-flash-tts) convert the prompt into speech and accept a 'voice'. Music models (lyria3-pro, minimax-music-v2.6, elevenlabs-music, sonilo-v1.1-video-to-music) generate tracks from a prompt; include lyrics/tempo/vocal style in the prompt for Lyria 3 Pro, pass 'lyrics' for MiniMax vocals, or set 'instrumental' true when the selected model supports it. Video-to-audio models (inputs include 'video' — see list_models, e.g. sonilo-v1.1-video-to-music, mirelo-sfx-v1.5-video-to-audio) generate audio that matches a VIDEO: provide a timeline span via videoSourceStartFrame+videoSourceEndFrame (e.g. to score the timeline), or a video asset via videoSourceMediaRef; the prompt is then an optional style guide. PLACEMENT: when you pass a timeline span, the result is placed on the timeline automatically at that span (no add_clips needed); for a media-asset source or a plain text-to-speech/music result, the asset lands in the library and you place it with add_clips. Use list_models with type='audio' to see each model's 'inputs', category, and voices. Costs real money and is not undoable.",

        ToolName::UpscaleMedia => "Upscales an existing video or image asset to higher resolution using an AI upscaler. Returns a placeholder asset ID immediately; the upscaled asset appears in get_media once ready. Use list_models with type='upscale' to pick a model that supports the asset's type. Costs real money and is not undoable.",

        ToolName::ImportMedia => "Imports external media into the project's library — the bridge for assets coming from other MCP servers (stock libraries, music services, web search) or local files the user already has. The 'source' object must set exactly one of: url (HTTPS only — downloaded in the background, the dominant case; max 1 GB), path (absolute local file path — referenced in place; may also be a directory, which is imported recursively, mirroring its subfolder structure as media folders), or bytes (base64-encoded inline data — max ~15 MB of base64 ≈ 11 MB binary; use url/path for anything larger). For url, type is inferred from the URL path's file extension unless source.mimeType is set as an override (needed for signed URLs whose path has no usable extension). For bytes, source.mimeType is required.\n\nSupported types and extensions: video (mov, mp4, m4v), audio (mp3, wav, aac, m4a), image (png, jpg, jpeg, tiff, heic). Anything else is rejected — the caller must transcode externally.\n\nReturns a placeholder asset id immediately; URL imports run in the background and the asset becomes usable in add_clips once ready (same async pattern as generate_*). Path and bytes imports finalize synchronously. Costs nothing.",

        ToolName::ListFolders => "Lists every folder in the media panel as {id, name, parentFolderId}. Folders are nested (parentFolderId is nil for top-level). Use to find an existing folder by name before generating new media.",

        ToolName::CreateFolder => "Creates folders in the media panel. Pass either name/parentFolderId for one folder or entries for multiple folders, not both. Direct form returns one folder; entries returns { folders }. Undoable. Use to organize related generations (e.g. 'Hero shot variations'). Don't create folders for unrelated concepts.",

        ToolName::MoveToFolder => "Moves media assets to folders. Pass either assetIds/folderId for one destination or entries for multiple destinations, not both. Omit folderId to move to root. Undoable.",

        ToolName::RenameMedia => "Renames media assets in the library. Pass either mediaRef/name for one asset or entries for multiple assets, not both. Undoable.",

        ToolName::RenameFolder => "Renames folders in the media panel. Pass either folderId/name for one folder or entries for multiple folders, not both. Undoable.",

        ToolName::DeleteMedia => "Deletes media assets from the library. Any clips referencing them are removed from the timeline in the same undoable action.",

        ToolName::DeleteFolder => "Deletes folders and everything inside them (subfolders and assets). Clips referencing any deleted asset are removed from the timeline in the same undoable action.",

        ToolName::ListModels => "Lists AI models with their capabilities (durations, aspect ratios, resolutions, first/last frame support, reference support, voices/category for audio, upscaler speed). Always call before generate_video, generate_image, generate_audio, or upscale_media so the model you pick actually supports the constraints you need. Returns { models, loaded } — if loaded=false the catalog hasn't synced yet (e.g. user not signed in); the models array may be empty even when models exist, so do not conclude no models are available. Retry after the user signs in.",

        // --- OpenTake workflow-plugin tools (agent-SPEC §7.4; OpenTake-authored, styled to match upstream) ---
        ToolName::ActivateWorkflow => "Activates a workflow plugin for the current project. A workflow plugin packages editing conventions for one video type (talking-head, vlog, montage, interview, review, wedding, ...): it injects type-specific guidance into your instructions and adds rule checks to your edits. Call list_workflows first to see installed plugins and their ids. Activating replaces any previously active workflow. The plugin's track-role mapping and declared video_type override auto-detection.",

        ToolName::ListWorkflows => "Lists installed workflow plugins as {id, name, description, videoType, active}. A workflow plugin packages editing conventions for one video type and, once activated with activate_workflow, injects guidance into your instructions and adds rule checks to your edits.",

        ToolName::DeactivateWorkflow => "Deactivates the currently active workflow plugin, if any. Removes its injected instructions and rule checks; auto-detection of video type and track roles resumes. Takes no arguments.",

        // --- OpenTake A-tier shader effects (docs/ADVANCED-FEATURES.md A-layer) ---
        ToolName::SetColorGrade => "Applies a high-end floating-point color grade to one or more clips in a single undoable action. The grade runs in linear light in a fixed order: exposure -> white balance -> lift/gamma/gain -> contrast -> saturation. All fields are optional and default to a no-op, so pass only what you want to change. exposure is in stops (0 = unchanged, +1 doubles brightness). temperature/tint are -1..1 white-balance trims (warm/magenta positive). lift/gamma/gain are per-channel {r,g,b} color wheels (lift offsets shadows, gamma is a midtone power, gain scales highlights). contrast pivots around mid-grey (0 = unchanged). saturation is a multiplier (1 = unchanged, 0 = greyscale). Pass clear:true to remove an existing grade. Applies to every clip in clipIds.",

        ToolName::ChromaKey => "Keys out a solid-color background (green/blue screen) on one or more clips in one undoable action — pure shader math, no model. keyColor is the background color to remove as hex '#RRGGBB' (default green '#00FF00'). similarity sets how close a pixel's chroma must be to the key to become transparent; smoothness feathers the matte edge above that threshold; spill (0..1) suppresses color spill of the key hue on the retained subject. The matte is luma-independent, so shadows and highlights on the subject survive. Pass clear:true to remove keying. Applies to every clip in clipIds.",

        ToolName::SetMask => "Sets the vector mask(s) on one or more clips in one undoable action — the masks generate a per-pixel alpha that hides everything outside them (intersection of all masks). Each mask is one of: a linear/gradient split (a line through a point with a normal), a circle/ellipse (center + per-axis radius), or a polygon/pen shape (a list of points). feather softens the edge in normalized canvas units; invert flips inside/outside. Coordinates are 0–1 normalized canvas space. Pass an empty masks array to clear all masks. Applies to every clip in clipIds.",

        ToolName::ApplyEffect => "Sets the effect chain on one or more clips in one undoable action — an ordered list of named pixel effects, each a shader pass with named numeric parameters. Each effect is { name, params } where name selects the effect (e.g. 'gaussianBlur') and params are its scalar inputs (e.g. { radius: 4 }); pass enabled:false to keep a disabled effect in the chain. The list replaces the clip's current effects; pass an empty array to clear them. Applies to every clip in clipIds.",

        // --- OpenTake Web motion graphics (docs/MOTION-GRAPHICS-PLUGIN.md, Issue #14) ---
        ToolName::AddMotionGraphic => "Adds a Web motion graphic (animated title, lower-third, data callout, transition card) to the timeline as a single undoable action, and returns its clipId. You author the animation with Web technology and OpenTake renders it deterministically — every frame reproducible, preview == export — to a transparent RGBA overlay that composites over your other layers like any other clip.\n\nThe 'source' object is exactly one of:\n  • { code: \"<html/css/js>\" } — a self-contained HTML document (it may embed <style> and <script>; CSS keyframes, the Web Animations API, SVG, or libraries like GSAP are all fine). Animate against the injected deterministic clock: OpenTake.seek(seconds) is called once per frame, and OpenTake.onSeek(t => …) lets you drive state from it; do NOT rely on Date.now()/real timers, which are frozen for reproducibility. Keep the body background transparent so the result is an overlay.\n  • { templateId, params } — instantiate a registered motion template by id with typed params (string/number/bool/color; colors are hex '#RRGGBB'/'#RRGGBBAA'). The template declares which params it accepts.\n\nstartFrame/durationFrames are project frames (from get_timeline). transparent defaults true (overlay); set false for an opaque clip. trackIndex is optional — omit to auto-create a new video track at the top for the graphic; set it to target an existing non-audio track (same overwrite-on-overlap behavior as add_texts). Rendering runs in a network-isolated sandbox with a time and size budget, so reference only inline/allowlisted assets.",

        ToolName::EditMotionGraphic => "Edits an existing motion graphic clip's source and re-renders it as a single undoable action. Pass the clipId (from add_motion_graphic or get_timeline) and at least one of:\n  • code — replace the inline HTML/CSS/JS of a code-authored graphic.\n  • params — override template params (merged over the current bindings) of a template-authored graphic.\n\nRe-rendering is content-hash cached: if your change produces identical pixels (or you pass values that resolve to the same source) the cached frames are reused; otherwise only the changed graphic is recomputed. Use this to iterate on a graphic — tweak the text, timing, or colors — without removing and re-adding the clip. Editing code on a template clip (or params on a code clip) is rejected; it must match how the clip was authored.",
    }
}

/// The JSON Schema (`inputSchema`) for a tool. Verbatim from upstream
/// `ToolDefinitions.swift`, transcribed to `serde_json`.
pub fn input_schema(tool: ToolName) -> Value {
    match tool {
        ToolName::GetTimeline => object(
            json!({
                "startFrame": {"type": "integer", "description": "Optional. Window start (inclusive); only clips intersecting [startFrame, endFrame) are returned. Tracks report totalClips when the window hides some."},
                "endFrame": {"type": "integer", "description": "Optional. Window end (exclusive)."}
            }),
            &[],
        ),

        ToolName::GetMedia => object(json!({}), &[]),

        ToolName::InspectMedia => object(
            json!({
                "mediaRef": {"type": "string", "description": "Asset ID from get_media."},
                "clipId": {"type": "string", "description": "Optional. A clip referencing this mediaRef; transcript times come back as project frames for that clip (out-of-range entries dropped)."},
                "maxFrames": {"type": "integer", "description": "Video and Lottie. Sample frame count (default 6, max 12)."},
                "startSeconds": {"type": "number", "description": "Video/audio. Source-time window start; scopes frames and transcription."},
                "endSeconds": {"type": "number", "description": "Video/audio. Window end (default: asset duration)."},
                "wordTimestamps": {"type": "boolean", "description": "Video/audio. Add word-level [text, start, end] tuples (capped at 10000 — most clips return all words at once; narrow with startSeconds/endSeconds only for very long media). Use for word-boundary edits like filler-word removal."},
                "overview": {"type": "boolean", "description": "Video only. One storyboard grid of visually distinct, timestamped moments instead of frames — far more coverage per token; few tiles means static footage. maxFrames ignored."}
            }),
            &["mediaRef"],
        ),

        ToolName::GetTranscript => object(
            json!({
                "startFrame": {"type": "integer", "description": "Optional. Only return words ending after this project frame. Use with the returned nextStartFrame to page a long timeline."},
                "endFrame": {"type": "integer", "description": "Optional. Only return words starting before this project frame."},
                "clipId": {"type": "string", "description": "Scope the transcript to a single clip — returns only what that clip says, in project frames. Answers \"what's in clip X?\" without scanning the whole timeline."}
            }),
            &[],
        ),

        ToolName::InspectTimeline => object(
            json!({
                "startFrame": {"type": "integer", "description": "Project frame to render (default 0). With no endFrame, a single frame is returned."},
                "endFrame": {"type": "integer", "description": "Optional. Sample maxFrames evenly across [startFrame, endFrame) instead of one frame."},
                "maxFrames": {"type": "integer", "description": "Frames to sample when endFrame is set (default 6, max 12)."}
            }),
            &[],
        ),

        ToolName::SearchMedia => object(
            json!({
                "query": {"type": "string", "description": "What to find. Visual: a caption-style scene description. Spoken: the words to match."},
                "scope": {"type": "string", "enum": ["visual", "spoken", "both"], "description": "Optional. Default both."},
                "mediaRef": {"type": "string", "description": "Optional. Restrict the search to one asset from get_media."},
                "limit": {"type": "integer", "description": "Optional. Max hits per group (default 10, max 50)."}
            }),
            &["query"],
        ),

        ToolName::AddClips => object(
            json!({
                "entries": {
                    "type": "array",
                    "description": "Clips to add. Each entry is validated up front; one bad entry rejects the whole call with no partial state.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "mediaRef": {"type": "string", "description": "ID of the media asset from get_media"},
                            "trackIndex": {"type": "integer", "description": "Optional. Track index (0-based). Omit on every entry to auto-create one shared track per asset zone (video/audio)."},
                            "startFrame": {"type": "integer", "description": "Timeline frame position to place the clip (project frames)."},
                            "durationFrames": {"type": "integer", "description": "Clip length on the timeline, in project frames."},
                            "trimStartFrame": {"type": "integer", "description": "Optional. Frames skipped from the START of the source media before the clip begins — a SOURCE offset, NOT a timeline position, but measured in PROJECT frames (the timeline's fps, same units as startFrame/durationFrames — never the source's own fps). 0 (default) starts at the source's first frame. Set this to trim on placement instead of a follow-up set_clip_properties call; semantics are identical to set_clip_properties."},
                            "trimEndFrame": {"type": "integer", "description": "Optional. Frames trimmed off the END of the source media, in PROJECT frames — same units as trimStartFrame. 0 (default) trims nothing off the end."}
                        },
                        "required": ["mediaRef", "startFrame", "durationFrames"]
                    }
                }
            }),
            &["entries"],
        ),

        ToolName::InsertClips => object(
            json!({
                "trackIndex": {"type": "integer", "description": "Track index (0-based, from get_timeline) to insert into and ripple."},
                "atFrame": {"type": "integer", "description": "Timeline frame (project frames) where insertion begins. Every clip at or after this frame on rippled tracks shifts right by the total inserted duration."},
                "entries": {
                    "type": "array",
                    "description": "Clips to insert, placed sequentially from atFrame. Validated up front; one bad entry rejects the whole call.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "mediaRef": {"type": "string", "description": "ID of the media asset from get_media."},
                            "durationFrames": {"type": "integer", "description": "Optional. Timeline length in project frames. Omit to use the asset's full source duration."},
                            "trimStartFrame": {"type": "integer", "description": "Optional. Frames skipped from the START of the source media — a SOURCE offset in PROJECT frames (same units as atFrame/durationFrames, never the source's own fps). 0 (default) starts at the source's first frame."},
                            "trimEndFrame": {"type": "integer", "description": "Optional. Frames trimmed off the END of the source media, in PROJECT frames. 0 (default) trims nothing."}
                        },
                        "required": ["mediaRef"]
                    }
                }
            }),
            &["trackIndex", "atFrame", "entries"],
        ),

        ToolName::RemoveClips => object(
            json!({
                "clipIds": {"type": "array", "description": "Clip IDs to remove.", "items": {"type": "string"}}
            }),
            &["clipIds"],
        ),

        ToolName::RemoveTracks => object(
            json!({
                "trackIndexes": {"type": "array", "items": {"type": "integer"}, "description": "Track indexes (0-based, from get_timeline) to remove."}
            }),
            &["trackIndexes"],
        ),

        ToolName::MoveClips => object(
            json!({
                "moves": {
                    "type": "array",
                    "description": "Per-clip move requests. At least one of toTrack or toFrame is required per entry.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "clipId": {"type": "string", "description": "The clip ID to move."},
                            "toTrack": {"type": "integer", "description": "Destination track index (0-based). Omit to keep the clip on its current track."},
                            "toFrame": {"type": "integer", "description": "Destination start frame. Omit to keep the clip at its current start."}
                        },
                        "required": ["clipId"]
                    }
                }
            }),
            &["moves"],
        ),

        ToolName::SetClipProperties => object(
            json!({
                "clipIds": {"type": "array", "description": "Clip IDs to update. The property values below apply to every clip in this list.", "items": {"type": "string"}},
                "durationFrames": {"type": "integer", "description": "New duration in frames."},
                "trimStartFrame": {"type": "integer", "description": "SOURCE-media offset, NOT a timeline frame: frames trimmed off the start of the source — measured in PROJECT frames (the timeline's fps, same units as startFrame/durationFrames; never the source's own fps). To turn a get_transcript project frame P into this clip's source offset, use trimStartFrame + (P − startFrame) × speed; setting trimStartFrame to that value makes the clip begin at P's source content."},
                "trimEndFrame": {"type": "integer", "description": "SOURCE-media offset, NOT a timeline frame: frames trimmed off the end of the source, in PROJECT frames. Maps the same way as trimStartFrame via startFrame/speed."},
                "speed": {"type": "number", "description": "Playback speed multiplier (default 1.0). >1 speeds up, <1 slows down. The clip's timeline length is rescaled to keep the same source content (2x speed → half the frames), unless you also pass durationFrames to set the length explicitly."},
                "volume": {"type": "number", "description": "Volume 0.0-1.0. Clears any existing volume keyframes."},
                "opacity": {"type": "number", "description": "Opacity 0.0-1.0. Clears any existing opacity keyframes."},
                "transform": {
                    "type": "object",
                    "description": "Partial transform. Any combination of centerX, centerY, width, height, flipHorizontal, flipVertical; omitted fields keep their current value.",
                    "properties": {
                        "centerX": {"type": "number"},
                        "centerY": {"type": "number"},
                        "width": {"type": "number"},
                        "height": {"type": "number"},
                        "flipHorizontal": {"type": "boolean", "description": "Mirror across the vertical axis."},
                        "flipVertical": {"type": "boolean", "description": "Mirror across the horizontal axis."}
                    }
                },
                "content": {"type": "string", "description": "Text clips only. New text content."},
                "fontName": {"type": "string", "description": "Text clips only. Font PostScript or family name."},
                "fontSize": {"type": "number", "description": "Text clips only. Font size in canvas points."},
                "color": {"type": "string", "description": "Text clips only. Hex '#RRGGBB' or '#RRGGBBAA'."},
                "alignment": {"type": "string", "enum": ["left", "center", "right"], "description": "Text clips only."}
            }),
            &["clipIds"],
        ),

        ToolName::SetKeyframes => object(
            json!({
                "clipId": {"type": "string", "description": "The clip ID."},
                "property": {"type": "string", "enum": ["volume", "opacity", "rotation", "position", "scale", "crop"], "description": "Which property's keyframe track to set."},
                "keyframes": {"type": "array", "description": "Replacement keyframe rows. Empty array clears the track. Row shape depends on property — see tool description.", "items": {"type": "array"}}
            }),
            &["clipId", "property", "keyframes"],
        ),

        ToolName::SplitClip => object(
            json!({
                "clipId": {"type": "string", "description": "The clip ID to split"},
                "atFrame": {"type": "integer", "description": "Frame position to split at (must be between clip start and end)"}
            }),
            &["clipId", "atFrame"],
        ),

        ToolName::RippleDeleteRanges => object(
            json!({
                "trackIndex": {"type": "integer", "description": "Cut project-frame ranges spanning every clip they cross on this track, in one call. From get_transcript's clips array. Mutually exclusive with clipId; requires units 'frames'."},
                "clipId": {"type": "string", "description": "Cut ranges within this single clip only, clamped to its visible span. Mutually exclusive with trackIndex."},
                "ranges": {"type": "array", "description": "Ranges to remove, each a [start, end] pair (end > start). In the unit given by 'units'.", "items": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2}},
                "units": {"type": "string", "enum": ["seconds", "frames"], "description": "Interpretation of range values. 'frames' (default) = project/timeline frames, matching get_transcript and inspect_media-with-clipId. 'seconds' = source-media seconds (clipId mode only)."}
            }),
            &["ranges"],
        ),

        ToolName::Undo => object(json!({}), &[]),

        ToolName::AddTexts => object(
            json!({
                "entries": {
                    "type": "array",
                    "description": "Text clips to add. Each entry is independent.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "trackIndex": {"type": "integer", "description": "Optional. Track index (0-based) for an existing non-audio track. Omit on every entry to auto-create one new track for the batch."},
                            "startFrame": {"type": "integer", "description": "Frame position to place the clip"},
                            "durationFrames": {"type": "integer", "description": "Duration in frames (>= 1)"},
                            "content": {"type": "string", "description": "Text to display. Supports \\n for line breaks."},
                            "transform": {
                                "type": "object",
                                "description": "Optional position/size. Omit for center + auto-fit. Pass centerX+centerY only for a specific position with auto-fit size. Pass all four for full override.",
                                "properties": {
                                    "centerX": {"type": "number", "description": "Horizontal center 0–1 (0=left edge, 1=right edge)"},
                                    "centerY": {"type": "number", "description": "Vertical center 0–1 (0=top, 1=bottom)"},
                                    "width": {"type": "number", "description": "Width 0–1 (optional; omit for auto-fit)"},
                                    "height": {"type": "number", "description": "Height 0–1 (optional; omit for auto-fit)"}
                                }
                            },
                            "fontName": {"type": "string", "description": "Font PostScript or family name, e.g. 'Helvetica-Bold', 'Georgia-Bold'. Default 'Helvetica-Bold'. Falls back to bold system font if not found."},
                            "fontSize": {"type": "number", "description": "Font size in canvas points (default 96). On a 1080p canvas ~50 is a caption, ~120 is a title."},
                            "color": {"type": "string", "description": "Hex '#RRGGBB' or '#RRGGBBAA' (default '#FFFFFF')"},
                            "alignment": {"type": "string", "enum": ["left", "center", "right"], "description": "Text alignment (default 'center')"}
                        },
                        "required": ["startFrame", "durationFrames", "content"]
                    }
                }
            }),
            &["entries"],
        ),

        ToolName::AddCaptions => object(
            json!({
                "clipIds": {"type": "array", "items": {"type": "string"}, "description": "Optional. Audio/video clips to caption. Omit to auto-detect the primary spoken track."},
                "language": {"type": "string", "description": "Optional BCP-47 language of the speech (e.g. 'es', 'ja', 'en-GB'). Defaults to the system language — set this when the footage is in another language, or transcription will be garbage."},
                "fontName": {"type": "string", "description": "Optional font PostScript or family name (default 'Helvetica-Bold'). Falls back to bold system font if not found."},
                "fontSize": {"type": "number", "description": "Optional font size in canvas points (default 48)."},
                "color": {"type": "string", "description": "Optional hex '#RRGGBB' or '#RRGGBBAA' (default white)."},
                "centerX": {"type": "number", "description": "Optional horizontal center 0–1 (default 0.5)."},
                "centerY": {"type": "number", "description": "Optional vertical center 0–1 (default 0.9, near the bottom)."},
                "textCase": {"type": "string", "enum": ["auto", "upper", "lower"], "description": "Optional letter case (default auto)."},
                "censorProfanity": {"type": "boolean", "description": "Optional. Mask profanity (default false)."}
            }),
            &[],
        ),

        ToolName::GenerateVideo => object(
            json!({
                "prompt": {"type": "string", "description": "Text description of the video to generate"},
                "name": {"type": "string", "description": "Display name for the asset in the media library. Defaults to first 30 chars of prompt."},
                "model": {"type": "string", "description": "Model ID (e.g. 'veo3.1-fast'). Use list_models to see options. Defaults to first available model."},
                "duration": {"type": "integer", "description": "Duration in seconds. Valid values depend on model."},
                "aspectRatio": {"type": "string", "description": "Aspect ratio (e.g. '16:9', '9:16', '1:1')"},
                "resolution": {"type": "string", "description": "Resolution (e.g. '720p', '1080p', '4k')"},
                "startFrameMediaRef": {"type": "string", "description": "Media asset ID to use as the first frame (image-to-video)"},
                "endFrameMediaRef": {"type": "string", "description": "Media asset ID to use as the last frame (supported by some models)"},
                "sourceVideoMediaRef": {"type": "string", "description": "Media asset ID of a source video (required by video-to-video edit models; ignores duration/aspectRatio/resolution)"},
                "sourceClipId": {"type": "string", "description": "Optional. Clip id (from get_timeline) referencing sourceVideoMediaRef. When set and the clip is trimmed, only the clip's visible range is sent to the model, not the full source — matches the UI's 'Use trimmed portion only'."},
                "referenceImageMediaRefs": {"type": "array", "items": {"type": "string"}, "description": "Media asset IDs of image references. Covers both reference-to-video generation (Seedance, Kling V3/O3 elements, Grok — refer as @Image1/@Element1 in prompt) and the single-image ref used by video-to-video edit models (Kling V3 Motion Control). See list_models maxReferenceImages for per-model cap."},
                "referenceVideoMediaRefs": {"type": "array", "items": {"type": "string"}, "description": "Media asset IDs of video references (Seedance only). Refer to them as @Video1, @Video2. See maxReferenceVideos and maxCombinedVideoRefSeconds."},
                "referenceAudioMediaRefs": {"type": "array", "items": {"type": "string"}, "description": "Media asset IDs of audio references (Seedance only). Refer to them as @Audio1, @Audio2. See maxReferenceAudios and maxCombinedAudioRefSeconds."},
                "folderId": {"type": "string", "description": "Optional. Folder id (from list_folders or create_folder) to place the result in. Omit for the project root."}
            }),
            &["prompt"],
        ),

        ToolName::GenerateImage => object(
            json!({
                "prompt": {"type": "string", "description": "Text description of the image to generate"},
                "name": {"type": "string", "description": "Display name for the asset in the media library. Defaults to first 30 chars of prompt."},
                "model": {"type": "string", "description": "Model ID (e.g. 'nano-banana-pro'). Use list_models to see options. Defaults to first available model."},
                "aspectRatio": {"type": "string", "description": "Aspect ratio (e.g. '16:9', '9:16')"},
                "resolution": {"type": "string", "description": "Resolution (e.g. '2K', '4K')"},
                "quality": {"type": "string", "description": "Image quality (e.g. 'low', 'medium', 'high'). Only supported by some models — see list_models."},
                "referenceMediaRefs": {"type": "array", "items": {"type": "string"}, "description": "Media asset IDs to use as reference images"},
                "folderId": {"type": "string", "description": "Optional. Folder id (from list_folders or create_folder) to place the result in. Omit for the project root."}
            }),
            &["prompt"],
        ),

        ToolName::GenerateAudio => object(
            json!({
                "prompt": {"type": "string", "description": "Required for TTS (the text to speak) and text-to-music (style/mood/genre; MiniMax needs ≥10 chars). For Lyria 3 Pro, include lyrics, tempo, language, and vocal style directly in the prompt. Optional style guide for video-to-music models."},
                "name": {"type": "string", "description": "Display name for the asset in the media library. Defaults to first 30 chars of prompt."},
                "model": {"type": "string", "description": "Model ID. Use list_models with type='audio' to see options and their 'inputs'. Defaults to the first model."},
                "voice": {"type": "string", "description": "TTS only. Voice preset name. list_models shows voicesSample (first 3) + voiceCount; any voice supported by the model is accepted. Defaults to the model's defaultVoice. Ignored by music models."},
                "lyrics": {"type": "string", "description": "MiniMax Music only. Lyrics with optional [Verse]/[Chorus] section tags. If omitted and instrumental=false, MiniMax auto-writes lyrics from the prompt."},
                "styleInstructions": {"type": "string", "description": "Gemini TTS only. Optional delivery instructions (e.g. 'warm and slow', 'British accent')."},
                "instrumental": {"type": "boolean", "description": "Music models only. true = no vocals when the selected model supports it. Defaults to false."},
                "duration": {"type": "integer", "description": "Length in seconds. ElevenLabs Music: 3–600. Sonilo text-to-music: up to 600. For a video source, defaults to the span/clip length. Ignored by TTS, MiniMax, and Lyria 3 Pro."},
                "videoSourceStartFrame": {"type": "integer", "description": "Video-to-audio models only. Start frame (timeline) of a span to render and score — pair with videoSourceEndFrame. Use get_timeline for frame numbers; for the whole timeline use 0 to the timeline's end frame."},
                "videoSourceEndFrame": {"type": "integer", "description": "Video-to-audio models only. End frame (exclusive) of the span to score. Must be > videoSourceStartFrame."},
                "videoSourceMediaRef": {"type": "string", "description": "Video-to-audio models only. Score this existing video asset instead of a timeline span. Mutually exclusive with the videoSource frames."},
                "folderId": {"type": "string", "description": "Optional. Folder id (from list_folders or create_folder) to place the result in. Omit for the project root."}
            }),
            &[],
        ),

        ToolName::UpscaleMedia => object(
            json!({
                "mediaRef": {"type": "string", "description": "ID of the video or image asset to upscale"},
                "model": {"type": "string", "description": "Upscaler model ID (e.g. 'bytedance-upscaler', 'seedvr-image-upscaler'). Defaults to the first model that supports the asset's type."},
                "sourceClipId": {"type": "string", "description": "Optional. Video clip id (from get_timeline) referencing mediaRef. When set and the clip is trimmed, only the clip's visible range is upscaled, not the full source."}
            }),
            &["mediaRef"],
        ),

        ToolName::ImportMedia => object(
            json!({
                "source": {
                    "type": "object",
                    "description": "Exactly one of url, path, or bytes must be set. mimeType is required when bytes is set; for url it acts as a type-inference override.",
                    "properties": {
                        "url": {"type": "string", "description": "HTTPS URL. Pre-signed URLs are fine but must not expire mid-download."},
                        "path": {"type": "string", "description": "Absolute local file or directory path, readable by the OpenTake process. A directory is imported recursively — every openable file is pulled in and the folder structure is replicated as media folders."},
                        "bytes": {"type": "string", "description": "Base64-encoded media data. Prefer url or path for anything over ~10MB."},
                        "mimeType": {"type": "string", "description": "Required when bytes is set. Optional override for url when its path has no usable extension (e.g. signed URLs). Accepted: video/mp4, video/quicktime, audio/mpeg, audio/wav, audio/aac, audio/mp4, image/png, image/jpeg, image/tiff, image/heic."}
                    }
                },
                "name": {"type": "string", "description": "Display name in the library. Defaults to the filename derived from url/path, or 'Imported asset' for bytes."},
                "folderId": {"type": "string", "description": "Optional. Folder id (from list_folders or create_folder) to place the result in. Omit for the project root."}
            }),
            &["source"],
        ),

        ToolName::ListFolders => object(json!({}), &[]),

        ToolName::CreateFolder => object(
            json!({
                "name": {"type": "string", "description": "Folder name."},
                "parentFolderId": {"type": "string", "description": "Optional parent folder id; omit for top level."},
                "entries": {
                    "type": "array",
                    "description": "Folders to create in one undoable action.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string", "description": "Folder name."},
                            "parentFolderId": {"type": "string", "description": "Optional parent folder id; omit for top level."}
                        },
                        "required": ["name"]
                    }
                }
            }),
            &[],
        ),

        ToolName::MoveToFolder => object(
            json!({
                "assetIds": {"type": "array", "items": {"type": "string"}, "description": "Media asset ids to move."},
                "folderId": {"type": "string", "description": "Destination folder id. Omit to move to the project root."},
                "entries": {
                    "type": "array",
                    "description": "Move operations to apply in one undoable action. Each entry can target a different folder.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "assetIds": {"type": "array", "items": {"type": "string"}, "description": "Media asset ids to move."},
                            "folderId": {"type": "string", "description": "Destination folder id. Omit to move to the project root."}
                        },
                        "required": ["assetIds"]
                    }
                }
            }),
            &[],
        ),

        ToolName::RenameMedia => object(
            json!({
                "mediaRef": {"type": "string", "description": "Media asset id from get_media."},
                "name": {"type": "string", "description": "New display name."},
                "entries": {
                    "type": "array",
                    "description": "Media assets to rename in one undoable action.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "mediaRef": {"type": "string", "description": "Media asset id from get_media."},
                            "name": {"type": "string", "description": "New display name."}
                        },
                        "required": ["mediaRef", "name"]
                    }
                }
            }),
            &[],
        ),

        ToolName::RenameFolder => object(
            json!({
                "folderId": {"type": "string", "description": "Folder id from list_folders."},
                "name": {"type": "string", "description": "New folder name."},
                "entries": {
                    "type": "array",
                    "description": "Folders to rename in one undoable action.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "folderId": {"type": "string", "description": "Folder id from list_folders."},
                            "name": {"type": "string", "description": "New folder name."}
                        },
                        "required": ["folderId", "name"]
                    }
                }
            }),
            &[],
        ),

        ToolName::DeleteMedia => object(
            json!({
                "assetIds": {"type": "array", "items": {"type": "string"}, "description": "Media asset ids to delete."}
            }),
            &["assetIds"],
        ),

        ToolName::DeleteFolder => object(
            json!({
                "folderIds": {"type": "array", "items": {"type": "string"}, "description": "Folder ids to delete."}
            }),
            &["folderIds"],
        ),

        ToolName::ListModels => object(
            json!({
                "type": {"type": "string", "enum": ["video", "image", "audio", "upscale"], "description": "Filter by type. Omit to list all models."}
            }),
            &[],
        ),

        ToolName::ActivateWorkflow => object(
            json!({
                "workflowId": {"type": "string", "description": "Plugin id from list_workflows (e.g. 'opentake-workflow-popular-science')."}
            }),
            &["workflowId"],
        ),

        ToolName::ListWorkflows => object(json!({}), &[]),

        ToolName::DeactivateWorkflow => object(json!({}), &[]),

        // --- OpenTake A-tier shader effects ---
        ToolName::SetColorGrade => object(
            json!({
                "clipIds": {"type": "array", "items": {"type": "string"}, "description": "Clip IDs to grade. The grade applies to every clip in this list."},
                "exposure": {"type": "number", "description": "Exposure in stops (0 = unchanged, +1 doubles linear brightness)."},
                "temperature": {"type": "number", "description": "White-balance temperature -1..1 (warm positive)."},
                "tint": {"type": "number", "description": "White-balance tint -1..1 (magenta positive, green negative)."},
                "lift": rgb_schema("Per-channel shadow offset (additive; identity 0)."),
                "gamma": rgb_schema("Per-channel midtone power (identity 1)."),
                "gain": rgb_schema("Per-channel highlight gain (multiplicative; identity 1)."),
                "contrast": {"type": "number", "description": "Contrast around mid-grey (0 = unchanged, positive raises contrast)."},
                "saturation": {"type": "number", "description": "Saturation multiplier (1 = unchanged, 0 = greyscale)."},
                "clear": {"type": "boolean", "description": "If true, removes the existing color grade from the clips (other fields ignored)."}
            }),
            &["clipIds"],
        ),

        ToolName::ChromaKey => object(
            json!({
                "clipIds": {"type": "array", "items": {"type": "string"}, "description": "Clip IDs to key. The key applies to every clip in this list."},
                "keyColor": {"type": "string", "description": "Background color to remove, hex '#RRGGBB' (default green '#00FF00')."},
                "similarity": {"type": "number", "description": "Chroma distance below which pixels are fully keyed (transparent). Larger removes more."},
                "smoothness": {"type": "number", "description": "Feather width above similarity over which alpha ramps from keyed to opaque."},
                "spill": {"type": "number", "description": "Spill suppression strength 0..1 (desaturates the key hue on the retained subject)."},
                "clear": {"type": "boolean", "description": "If true, removes the existing chroma key from the clips (other fields ignored)."}
            }),
            &["clipIds"],
        ),

        ToolName::SetMask => object(
            json!({
                "clipIds": {"type": "array", "items": {"type": "string"}, "description": "Clip IDs to mask. The masks apply to every clip in this list."},
                "masks": {
                    "type": "array",
                    "description": "Vector masks (intersected). Empty array clears all masks. Coordinates are 0–1 normalized canvas space.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "kind": {"type": "string", "enum": ["linear", "circle", "poly"], "description": "Mask shape."},
                            "point": point_schema("Linear masks: a point the dividing line passes through."),
                            "normal": point_schema("Linear masks: the line's outward normal (covered side is +normal)."),
                            "center": point_schema("Circle masks: ellipse center."),
                            "radius": point_schema("Circle masks: per-axis radius (x, y)."),
                            "points": {"type": "array", "description": "Poly masks: ordered polygon vertices (>= 3).", "items": point_schema("A polygon vertex.")},
                            "feather": {"type": "number", "description": "Edge feather in normalized canvas units (0 = hard edge)."},
                            "invert": {"type": "boolean", "description": "Invert coverage (mask out the inside instead of the outside)."}
                        },
                        "required": ["kind"]
                    }
                }
            }),
            &["clipIds", "masks"],
        ),

        ToolName::ApplyEffect => object(
            json!({
                "clipIds": {"type": "array", "items": {"type": "string"}, "description": "Clip IDs to apply the effect chain to. Applies to every clip in this list."},
                "effects": {
                    "type": "array",
                    "description": "Ordered effect chain. Replaces the clips' current effects; empty array clears them.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string", "description": "Effect identifier, e.g. 'gaussianBlur'."},
                            "params": {"type": "object", "description": "Named numeric parameters for the effect, e.g. { \"radius\": 4 }.", "additionalProperties": {"type": "number"}},
                            "enabled": {"type": "boolean", "description": "Whether the effect is active (default true)."}
                        },
                        "required": ["name"]
                    }
                }
            }),
            &["clipIds", "effects"],
        ),

        // --- OpenTake Web motion graphics ---
        ToolName::AddMotionGraphic => object(
            json!({
                "source": {
                    "type": "object",
                    "description": "Exactly one of code or templateId must be set. code is a self-contained HTML/CSS/JS document; templateId instantiates a registered template with params.",
                    "properties": {
                        "code": {"type": "string", "description": "Self-contained HTML document (may embed <style>/<script>). Animate against the injected deterministic clock (OpenTake.seek / OpenTake.onSeek), not wall-clock time. Keep the body transparent for an overlay."},
                        "templateId": {"type": "string", "description": "Registered motion-template id (e.g. 'lower-third.glass'). Mutually exclusive with code."},
                        "params": {"type": "object", "description": "Template params: name -> value. Values are string, number, bool, or a hex color string '#RRGGBB'/'#RRGGBBAA'. Only valid with templateId."}
                    }
                },
                "startFrame": {"type": "integer", "description": "Timeline frame position to place the graphic (project frames)."},
                "durationFrames": {"type": "integer", "description": "Clip length on the timeline, in project frames (>= 1)."},
                "transparent": {"type": "boolean", "description": "Render a transparent RGBA overlay (default true). Set false for an opaque clip."},
                "trackIndex": {"type": "integer", "description": "Optional. Existing non-audio track index (0-based) to place the graphic on. Omit to auto-create a new video track at the top."}
            }),
            &["source", "startFrame", "durationFrames"],
        ),

        ToolName::EditMotionGraphic => object(
            json!({
                "clipId": {"type": "string", "description": "The motion-graphic clip id to edit (from add_motion_graphic or get_timeline)."},
                "code": {"type": "string", "description": "Replacement HTML/CSS/JS for a code-authored graphic. Only valid when the clip was authored with code."},
                "params": {"type": "object", "description": "Template param overrides (merged over current bindings) for a template-authored graphic. Values are string, number, bool, or a hex color string. Only valid when the clip was authored from a template."}
            }),
            &["clipId"],
        ),
    }
}

/// A `{r, g, b}` object schema for color-grade wheel fields.
fn rgb_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "description": description,
        "properties": {
            "r": {"type": "number"},
            "g": {"type": "number"},
            "b": {"type": "number"}
        }
    })
}

/// A `{x, y}` object schema for mask geometry points.
fn point_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "description": description,
        "properties": {
            "x": {"type": "number"},
            "y": {"type": "number"}
        }
    })
}

/// Assemble an object schema: `{type:object[, properties][, required]}`. Mirrors
/// upstream `objectSchema` (omits empty properties/required).
fn object(properties: Value, required: &[&str]) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("type".into(), Value::String("object".into()));
    if let Value::Object(props) = &properties {
        if !props.is_empty() {
            obj.insert("properties".into(), properties);
        }
    }
    if !required.is_empty() {
        obj.insert(
            "required".into(),
            Value::Array(
                required
                    .iter()
                    .map(|s| Value::String(s.to_string()))
                    .collect(),
            ),
        );
    }
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_nonempty_description() {
        for t in ToolName::ALL {
            assert!(
                !description(t).is_empty(),
                "{} has empty description",
                t.as_str()
            );
        }
    }

    #[test]
    fn product_name_substituted_not_palmier() {
        for t in ToolName::ALL {
            let d = description(t);
            assert!(!d.contains("Palmier"), "{} leaks Palmier", t.as_str());
            assert!(!d.contains("palmier"), "{} leaks palmier", t.as_str());
        }
    }

    #[test]
    fn get_timeline_mentions_opentake_signin() {
        // Upstream "sign in to Palmier and subscribe" -> OpenTake.
        assert!(description(ToolName::GetTimeline).contains("sign in to OpenTake and subscribe"));
    }

    #[test]
    fn import_media_mentions_opentake_process() {
        // The "readable by the OpenTake process" phrase is the product-name
        // substitution of upstream's "Palmier process" — and it lives in the
        // `source.path` SCHEMA field description (upstream ToolDefinitions.swift
        // :439), NOT in the top-level tool description (:431). Assert it against
        // its real source so the test tracks the actual string.
        let schema = input_schema(ToolName::ImportMedia);
        let path_desc = schema["properties"]["source"]["properties"]["path"]["description"]
            .as_str()
            .expect("source.path has a description");
        assert!(
            path_desc.contains("readable by the OpenTake process"),
            "{path_desc}"
        );
    }

    #[test]
    fn import_media_top_level_description_is_verbatim() {
        // Pin the top-level import_media description to the upstream text
        // (ToolDefinitions.swift:431) verbatim — it contains no product name, so
        // the substitution leaves it unchanged. This is the contract string the
        // LLM reads; it must match upstream byte-for-byte.
        let d = description(ToolName::ImportMedia);
        assert!(
            d.starts_with(
                "Imports external media into the project's library — the bridge for assets coming from other MCP servers"
            ),
            "{d}"
        );
        assert!(
            d.ends_with("Path and bytes imports finalize synchronously. Costs nothing."),
            "{d}"
        );
    }

    #[test]
    fn all_31_descriptions_match_upstream_anchors() {
        // Pins every upstream tool description (ToolDefinitions.swift) at both
        // ends to its verbatim text (after the only allowed edit, Palmier ->
        // OpenTake). The full byte-for-byte equivalence was verified against the
        // upstream Swift source out-of-band; these head/tail anchors are the
        // in-crate regression guard that catches any drift (truncation, edits,
        // or a wrong string source) the way the earlier single-phrase spot-check
        // could not. `agent-SPEC.md` §2 — descriptions are a behavior contract.
        //
        // (variant, head_anchor, tail_anchor)
        const ANCHORS: &[(ToolName, &str, &str)] = &[
            (
                ToolName::GetTimeline,
                "Always call at the start of a session. Returns project",
                "he group appear individually in clips.",
            ),
            (
                ToolName::GetMedia,
                "Call before referencing any asset. Every mediaRef/refer",
                " async-generated and -imported assets.",
            ),
            (
                ToolName::AddClips,
                "Places one or more media assets on the timeline as a si",
                "'s drag-onto-track overwrite behavior.",
            ),
            (
                ToolName::InsertClips,
                "Inserts one or more media assets at a single point and",
                "ement into empty space, use add_clips.",
            ),
            (
                ToolName::RemoveClips,
                "Removes one or more clips by ID as a single undoable ac",
                "ching the UI's linked-delete behavior.",
            ),
            (
                ToolName::RemoveTracks,
                "Removes whole tracks and every clip on them in one undo",
                "rack indexes shift down after removal.",
            ),
            (
                ToolName::MoveClips,
                "Moves one or more clips to a new track and/or frame pos",
                "sets; tracks stay with the named clip.",
            ),
            (
                ToolName::SetClipProperties,
                "Apply the same property values to one or more clips in",
                "d speed are skipped for text partners.",
            ),
            (
                ToolName::SetKeyframes,
                "Set animated keyframes on one property of one clip. Rep",
                " static `transform` value when active.",
            ),
            (
                ToolName::SplitClip,
                "Splits a clip into two at atFrame. The frame must be st",
                "use get_timeline to confirm the range.",
            ),
            (
                ToolName::RippleDeleteRanges,
                "Cuts one or more ranges out and closes the gaps in one",
                "/frames) so you don't need to re-read.",
            ),
            (
                ToolName::Undo,
                "Reverts the assistant's most recent timeline edit (a cu",
                "you'll edit again. Takes no arguments.",
            ),
            (
                ToolName::AddTexts,
                "Adds one or more text clips (titles, captions, lower-th",
                " by hand. Unknown fields are rejected.",
            ),
            (
                ToolName::AddCaptions,
                "Auto-caption spoken audio: transcribes on-device and pl",
                "cific clips (e.g. only the interview).",
            ),
            (
                ToolName::GenerateVideo,
                "Starts an async AI video generation. Returns a placehol",
                " Costs real money and is not undoable.",
            ),
            (
                ToolName::GenerateImage,
                "Starts an async AI image generation. Returns a placehol",
                " Costs real money and is not undoable.",
            ),
            (
                ToolName::GenerateAudio,
                "Starts an async AI audio generation: text-to-speech, te",
                " Costs real money and is not undoable.",
            ),
            (
                ToolName::UpscaleMedia,
                "Upscales an existing video or image asset to higher res",
                " Costs real money and is not undoable.",
            ),
            (
                ToolName::ImportMedia,
                "Imports external media into the project's library — the",
                "finalize synchronously. Costs nothing.",
            ),
            (
                ToolName::ListModels,
                "Lists AI models with their capabilities (durations, asp",
                "ilable. Retry after the user signs in.",
            ),
            (
                ToolName::InspectMedia,
                "Look at a media asset before referencing or editing it.",
                "ranscribe that span, so they are fast.",
            ),
            (
                ToolName::GetTranscript,
                "Returns the spoken transcript of the CURRENT timeline i",
                " to verify what remains after cutting.",
            ),
            (
                ToolName::InspectTimeline,
                "See the composited timeline — what the user actually se",
                "ciency, with the frameNumbers sampled.",
            ),
            (
                ToolName::SearchMedia,
                "Search the media library by content: what's on screen (",
                "ken results work regardless of status.",
            ),
            (
                ToolName::ListFolders,
                "Lists every folder in the media panel as {id, name, par",
                "r by name before generating new media.",
            ),
            (
                ToolName::CreateFolder,
                "Creates folders in the media panel. Pass either name/pa",
                "create folders for unrelated concepts.",
            ),
            (
                ToolName::MoveToFolder,
                "Moves media assets to folders. Pass either assetIds/fol",
                "it folderId to move to root. Undoable.",
            ),
            (
                ToolName::RenameMedia,
                "Renames media assets in the library. Pass either mediaR",
                "r multiple assets, not both. Undoable.",
            ),
            (
                ToolName::RenameFolder,
                "Renames folders in the media panel. Pass either folderI",
                " multiple folders, not both. Undoable.",
            ),
            (
                ToolName::DeleteMedia,
                "Deletes media assets from the library. Any clips refere",
                " timeline in the same undoable action.",
            ),
            (
                ToolName::DeleteFolder,
                "Deletes folders and everything inside them (subfolders",
                " timeline in the same undoable action.",
            ),
        ];
        assert_eq!(ANCHORS.len(), 31, "all 31 upstream tools must be anchored");
        for (tool, head, tail) in ANCHORS {
            let d = description(*tool);
            assert!(
                d.starts_with(head),
                "{} head drift:\n  want start: {head:?}\n  got:        {:?}",
                tool.as_str(),
                &d[..head.len().min(d.len())]
            );
            assert!(
                d.ends_with(tail),
                "{} tail drift:\n  want end: {tail:?}\n  got:      {:?}",
                tool.as_str(),
                &d[d.len().saturating_sub(tail.len())..]
            );
        }
    }

    #[test]
    fn schemas_are_objects_with_type() {
        for t in ToolName::ALL {
            let s = input_schema(t);
            assert_eq!(s["type"], serde_json::json!("object"), "{}", t.as_str());
        }
    }

    #[test]
    fn add_clips_schema_requires_entries() {
        let s = input_schema(ToolName::AddClips);
        assert_eq!(s["required"], serde_json::json!(["entries"]));
        // Nested entry required keys preserved.
        let item_required = &s["properties"]["entries"]["items"]["required"];
        assert_eq!(
            item_required,
            &serde_json::json!(["mediaRef", "startFrame", "durationFrames"])
        );
    }

    #[test]
    fn no_args_tools_omit_properties() {
        // get_media / undo / list_folders take no args.
        for t in [
            ToolName::GetMedia,
            ToolName::Undo,
            ToolName::ListFolders,
            ToolName::ListWorkflows,
        ] {
            let s = input_schema(t);
            assert!(
                s.get("properties").is_none(),
                "{} should omit properties",
                t.as_str()
            );
            assert!(
                s.get("required").is_none(),
                "{} should omit required",
                t.as_str()
            );
        }
    }
}
