use opentake_tauri_lib::feedback::{FeedbackDraft, FeedbackSubmission};

#[test]
fn submission_includes_app_and_os_version() {
    let submission = FeedbackSubmission::from_draft_with_versions(
        FeedbackDraft {
            message: "  Playback stalls after a split.  ".into(),
            email: Some("editor@example.com".into()),
            may_contact: true,
            screenshot_png_base64: Some("private-image-data".into()),
        },
        Some("1.0.0"),
        Some("42"),
        Some("15.5.0"),
    )
    .expect("valid feedback should produce a submission");

    let value = serde_json::to_value(&submission).expect("submission should serialize");
    assert_eq!(value["message"], "Playback stalls after a split.");
    assert_eq!(value["email"], "editor@example.com");
    assert_eq!(value["mayContact"], true);
    assert_eq!(value["screenshotPngBase64"], "private-image-data");
    assert_eq!(value["appVersion"], "1.0.0 (42)");
    assert_eq!(value["osVersion"], "15.5.0");

    let fallback = FeedbackSubmission::from_draft_with_versions(
        FeedbackDraft {
            message: "Version metadata is unavailable".into(),
            email: None,
            may_contact: true,
            screenshot_png_base64: None,
        },
        None,
        Some(""),
        None,
    )
    .expect("missing version metadata must use safe fallbacks");
    let fallback_value =
        serde_json::to_value(&fallback).expect("fallback submission should serialize");
    assert_eq!(fallback_value["appVersion"], "? (?)");
    assert_eq!(fallback_value["osVersion"], "?.?.?");
    assert_eq!(fallback_value["mayContact"], false);
    assert!(fallback_value.get("email").is_none());
    assert!(fallback_value.get("screenshotPngBase64").is_none());

    let debug = format!("{submission:?}");
    assert!(!debug.contains("Playback stalls"));
    assert!(!debug.contains("editor@example.com"));
    assert!(!debug.contains("private-image-data"));
}
