use super::*;

pub(super) fn range_not_satisfiable(length: u64) -> Response<Vec<u8>> {
    secure_response_builder(StatusCode::RANGE_NOT_SATISFIABLE)
        .header(CONTENT_RANGE, format!("bytes */{length}"))
        .body(Vec::new())
        .expect("static response headers")
}

pub(super) fn secure_response_builder(status: StatusCode) -> tauri::http::response::Builder {
    Response::builder()
        .status(status)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, asset_origin())
        .header("x-content-type-options", "nosniff")
        .header("content-security-policy", "default-src 'none'; sandbox")
}

pub(super) fn error_response(
    status: StatusCode,
    message: &str,
    extra: Option<(tauri::http::HeaderName, &'static str)>,
) -> Response<Vec<u8>> {
    let mut builder =
        secure_response_builder(status).header(CONTENT_TYPE, "text/plain; charset=utf-8");
    if let Some((name, value)) = extra {
        builder = builder.header(name, value);
    }
    builder
        .body(message.as_bytes().to_vec())
        .expect("static response headers")
}

#[cfg(debug_assertions)]
pub(super) fn asset_origin() -> &'static str {
    "http://localhost:1420"
}

#[cfg(all(not(debug_assertions), target_os = "windows"))]
pub(super) fn asset_origin() -> &'static str {
    "http://tauri.localhost"
}

#[cfg(all(not(debug_assertions), not(target_os = "windows")))]
pub(super) fn asset_origin() -> &'static str {
    "tauri://localhost"
}
