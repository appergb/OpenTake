const EVIDENCE: &str = include_str!(
    "../../../docs/audit/2026-07-14/runtime-artifacts/automated/hdr-proxy-account-real-device-2026-08-01.md"
);

#[test]
fn hdr_proxy_account_children_close_one_composite_acceptance() {
    for child in [
        "HDR child result: **PASS**",
        "Proxy child result: **PASS**",
        "Account child result: **PASS**",
    ] {
        assert!(EVIDENCE.contains(child), "missing child evidence: {child}");
    }
    assert!(EVIDENCE.contains("`HDR child PASS + proxy child PASS + account child PASS`"));
    assert!(EVIDENCE.contains("closes one composite\nacceptance"));
    assert!(EVIDENCE.contains("codesign --verify --deep --strict"));
    assert!(EVIDENCE.contains("This is not\nan HDR-passthrough claim."));
    assert!(EVIDENCE.contains("Export therefore used the original source, not the enabled proxy."));
    assert!(EVIDENCE.contains("Local editing remains the default"));
}
