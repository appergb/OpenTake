//! Export the production manifest for opt-in model qualification tooling.

#[cfg(feature = "model-download")]
fn main() {
    use opentake_media::search::config;
    println!(
        "{}",
        serde_json::json!({
            "base_url": config::MODEL_DOWNLOAD_BASE_URL,
            "manifest": config::manifest(),
        })
    );
}

#[cfg(not(feature = "model-download"))]
fn main() {
    eprintln!("search_manifest requires --features model-download");
    std::process::exit(2);
}
