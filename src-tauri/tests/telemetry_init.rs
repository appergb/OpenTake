use std::cell::RefCell;

use opentake_tauri_lib::telemetry::{
    init_telemetry_with, redact_sensitive_text, TelemetryInitStatus, TelemetrySource,
};

#[test]
fn starts_only_with_explicit_packaged_or_environment_dsn() {
    let starts = RefCell::new(Vec::new());
    let start = |options: &opentake_tauri_lib::telemetry::TelemetryOptions| {
        starts.borrow_mut().push((
            options.source,
            options.dsn.to_string(),
            options.send_default_pii,
            options.capture_failed_requests,
            options.traces_sample_rate,
        ));
        Ok(())
    };

    assert_eq!(
        init_telemetry_with(None, None, start),
        TelemetryInitStatus::Disabled
    );
    assert_eq!(
        init_telemetry_with(Some("  "), Some(""), start),
        TelemetryInitStatus::Disabled
    );
    assert!(starts.borrow().is_empty());

    assert_eq!(
        init_telemetry_with(Some("https://public@example.com/1"), None, start),
        TelemetryInitStatus::Started(TelemetrySource::Packaged)
    );
    assert_eq!(
        init_telemetry_with(
            Some("https://packaged@example.com/1"),
            Some("https://environment@example.com/2"),
            start,
        ),
        TelemetryInitStatus::Started(TelemetrySource::Environment)
    );
    assert_eq!(starts.borrow().len(), 2);
    assert_eq!(starts.borrow()[0].0, TelemetrySource::Packaged);
    assert_eq!(starts.borrow()[1].0, TelemetrySource::Environment);
    assert_eq!(starts.borrow()[1].1, "https://environment:@example.com/2");
    assert!(!starts.borrow()[1].2);
    assert!(!starts.borrow()[1].3);
    assert_eq!(starts.borrow()[1].4, 0.1);

    assert_eq!(
        init_telemetry_with(None, Some("not a dsn"), start),
        TelemetryInitStatus::InvalidConfiguration
    );
    assert_eq!(starts.borrow().len(), 2);

    let redacted = redact_sensitive_text(
        "failed /Users/alice/private.otproj api_key=sk-secret token=abc123 Bearer xyz",
    );
    assert!(!redacted.contains("/Users/alice"));
    assert!(!redacted.contains("sk-secret"));
    assert!(!redacted.contains("abc123"));
    assert!(!redacted.contains("xyz"));
}
