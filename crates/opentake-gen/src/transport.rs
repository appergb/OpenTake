//! HTTP transport abstraction. Every network call in this crate flows through
//! `HttpTransport`, never `reqwest` directly. Production uses `ReqwestTransport`;
//! tests use `MockTransport`, which serves canned responses keyed by request and
//! makes the whole suite fully offline — no socket is ever opened.

use crate::error::GenError;
use async_trait::async_trait;
use std::collections::HashMap;

/// HTTP method, kept minimal to what the adapters and client need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
        }
    }
}

/// Request body: JSON, raw bytes (with content-type), or none.
#[derive(Debug, Clone)]
pub enum Body {
    Empty,
    Json(serde_json::Value),
    Bytes { content_type: String, data: Vec<u8> },
}

/// A transport-agnostic HTTP request.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl HttpRequest {
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: Body::Empty,
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new(Method::Get, url)
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self::new(Method::Post, url)
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn json(mut self, value: serde_json::Value) -> Self {
        self.body = Body::Json(value);
        self
    }

    pub fn bytes(mut self, content_type: impl Into<String>, data: Vec<u8>) -> Self {
        self.body = Body::Bytes {
            content_type: content_type.into(),
            data,
        };
        self
    }
}

/// A transport-agnostic HTTP response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body,
        }
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Parse the body as JSON into `T`.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, GenError> {
        serde_json::from_slice(&self.body).map_err(GenError::from)
    }

    /// Find a response header value (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// The transport contract. Implementors perform a single request/response.
#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, GenError>;
}

/// Production transport backed by `reqwest`.
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for ReqwestTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpTransport for ReqwestTransport {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, GenError> {
        let method = match req.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
        };
        let mut builder = self.client.request(method, &req.url);
        for (k, v) in &req.headers {
            builder = builder.header(k, v);
        }
        builder = match req.body {
            Body::Empty => builder,
            Body::Json(v) => builder.json(&v),
            Body::Bytes { content_type, data } => {
                builder.header("Content-Type", content_type).body(data)
            }
        };

        let resp = builder
            .send()
            .await
            .map_err(|e| GenError::Transport(e.to_string()))?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|s| (k.as_str().to_string(), s.to_string()))
            })
            .collect();
        let body = resp
            .bytes()
            .await
            .map_err(|e| GenError::Transport(e.to_string()))?
            .to_vec();
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

/// One canned reply in a `MockTransport` script.
#[derive(Clone)]
struct Canned {
    response: HttpResponse,
}

/// Offline transport for tests. Two modes (composable):
///
/// - **Keyed map**: exact `"METHOD url"` -> response. Good for stable endpoints.
/// - **Sequence per key**: pops responses in order so `submit` then repeated
///   `poll` can return queued -> running -> succeeded.
///
/// Every sent request is recorded in `calls` for assertions. No I/O ever occurs.
#[derive(Clone, Default)]
pub struct MockTransport {
    inner: std::sync::Arc<MockInner>,
}

#[derive(Default)]
struct MockInner {
    routes: std::sync::Mutex<HashMap<String, Vec<Canned>>>,
    calls: std::sync::Mutex<Vec<HttpRequest>>,
    fallback: std::sync::Mutex<Option<HttpResponse>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(method: Method, url: &str) -> String {
        format!("{} {}", method.as_str(), url)
    }

    /// Register a single response for `method url`. Repeated registration on the
    /// same key appends to that key's sequence.
    pub fn on(
        &self,
        method: Method,
        url: impl AsRef<str>,
        status: u16,
        body: serde_json::Value,
    ) -> &Self {
        self.on_raw(
            method,
            url,
            HttpResponse::new(status, serde_json::to_vec(&body).unwrap()),
        )
    }

    /// Register a raw response (arbitrary body/headers).
    pub fn on_raw(&self, method: Method, url: impl AsRef<str>, response: HttpResponse) -> &Self {
        let key = Self::key(method, url.as_ref());
        self.inner
            .routes
            .lock()
            .unwrap()
            .entry(key)
            .or_default()
            .push(Canned { response });
        self
    }

    /// Register a sequence of JSON responses for `method url`, served in order.
    pub fn on_sequence(
        &self,
        method: Method,
        url: impl AsRef<str>,
        steps: Vec<(u16, serde_json::Value)>,
    ) -> &Self {
        for (status, body) in steps {
            self.on(method, url.as_ref(), status, body);
        }
        self
    }

    /// Set a catch-all response for any unmatched request.
    pub fn fallback(&self, status: u16, body: serde_json::Value) -> &Self {
        *self.inner.fallback.lock().unwrap() = Some(HttpResponse::new(
            status,
            serde_json::to_vec(&body).unwrap(),
        ));
        self
    }

    /// Number of requests sent so far.
    pub fn call_count(&self) -> usize {
        self.inner.calls.lock().unwrap().len()
    }

    /// Snapshot of all recorded requests.
    pub fn calls(&self) -> Vec<HttpRequest> {
        self.inner.calls.lock().unwrap().clone()
    }

    /// The most recently recorded request, if any.
    pub fn last_call(&self) -> Option<HttpRequest> {
        self.inner.calls.lock().unwrap().last().cloned()
    }
}

#[async_trait]
impl HttpTransport for MockTransport {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, GenError> {
        self.inner.calls.lock().unwrap().push(req.clone());
        let key = Self::key(req.method, &req.url);
        let mut routes = self.inner.routes.lock().unwrap();
        if let Some(queue) = routes.get_mut(&key) {
            if queue.len() > 1 {
                // Sequence: pop the front, keep the rest.
                return Ok(queue.remove(0).response);
            }
            if let Some(c) = queue.first() {
                // Single entry: serve it repeatedly (sticky).
                return Ok(c.response.clone());
            }
        }
        drop(routes);
        if let Some(fb) = self.inner.fallback.lock().unwrap().clone() {
            return Ok(fb);
        }
        Err(GenError::Transport(format!("no mock route for {key}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn mock_serves_keyed_response() {
        let m = MockTransport::new();
        m.on(Method::Get, "https://x/a", 200, json!({"ok": true}));
        let resp = m.send(HttpRequest::get("https://x/a")).await.unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(
            resp.json::<serde_json::Value>().unwrap(),
            json!({"ok":true})
        );
        assert_eq!(m.call_count(), 1);
    }

    #[tokio::test]
    async fn mock_serves_sequence_then_sticks_on_last() {
        let m = MockTransport::new();
        m.on_sequence(
            Method::Get,
            "https://x/poll",
            vec![
                (200, json!({"status":"queued"})),
                (200, json!({"status":"running"})),
                (200, json!({"status":"succeeded"})),
            ],
        );
        let s1: serde_json::Value = m
            .send(HttpRequest::get("https://x/poll"))
            .await
            .unwrap()
            .json()
            .unwrap();
        let s2: serde_json::Value = m
            .send(HttpRequest::get("https://x/poll"))
            .await
            .unwrap()
            .json()
            .unwrap();
        let s3: serde_json::Value = m
            .send(HttpRequest::get("https://x/poll"))
            .await
            .unwrap()
            .json()
            .unwrap();
        let s4: serde_json::Value = m
            .send(HttpRequest::get("https://x/poll"))
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(s1["status"], "queued");
        assert_eq!(s2["status"], "running");
        assert_eq!(s3["status"], "succeeded");
        // Last entry is sticky.
        assert_eq!(s4["status"], "succeeded");
    }

    #[tokio::test]
    async fn mock_records_request_details() {
        let m = MockTransport::new();
        m.on(Method::Post, "https://x/submit", 200, json!({}));
        let req = HttpRequest::post("https://x/submit")
            .header("Authorization", "Key abc")
            .json(json!({"prompt": "hi"}));
        m.send(req).await.unwrap();
        let last = m.last_call().unwrap();
        assert_eq!(last.method, Method::Post);
        assert!(last
            .headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Key abc"));
        match last.body {
            Body::Json(v) => assert_eq!(v["prompt"], "hi"),
            _ => panic!("expected json body"),
        }
    }

    #[tokio::test]
    async fn mock_unmatched_route_errors() {
        let m = MockTransport::new();
        let r = m.send(HttpRequest::get("https://x/none")).await;
        assert!(matches!(r, Err(GenError::Transport(_))));
    }

    #[tokio::test]
    async fn mock_fallback_used_when_no_route() {
        let m = MockTransport::new();
        m.fallback(404, json!({"error":{"code":"not_found","message":"x"}}));
        let r = m.send(HttpRequest::get("https://x/none")).await.unwrap();
        assert_eq!(r.status, 404);
    }
}
