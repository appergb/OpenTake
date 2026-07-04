//! opentake-gen — generative AI backend clients.
//!
//! Two modes with one call surface (axiom A7):
//! - **BYOK**: `GenClient` drives provider adapters (fal / Replicate / OpenAI /
//!   ElevenLabs) directly with user-supplied keys, using a built-in static
//!   catalog — no backend required.
//! - **Managed**: `GenClient` talks to a self-hosted proxy over a Bearer JWT.
//!
//! Every network call flows through the `HttpTransport` trait; tests inject
//! `MockTransport` so the suite is fully offline. Provider keys live in the OS
//! keychain via `KeyStore` (tests use `MemoryKeyStore`).
//!
//! Wire types (`GenerationParams`, `GenerationJob`, `CatalogEntry`) are 1:1 ports
//! of the upstream Palmier Pro contracts; see `docs/specs/gen-SPEC.md`.

pub mod build_params;
pub mod catalog;
pub mod client;
pub mod error;
pub mod job;
pub mod keys;
pub mod params;
pub mod provider;
pub mod transport;

// Public API surface.
pub use build_params::{
    build_audio_params, build_image_params, build_params, build_upscale_params,
    build_video_edit_params, build_video_params,
};
pub use catalog::{
    builtin_catalog, AudioCaps, AudioPricing, Catalog, CatalogEntry, ImageCaps, ModelKind,
    ResponseShape, UiCapabilities, UpscaleCaps, VideoCaps,
};
pub use client::{
    can_generate, filter_by_kind, AuthMode, GenClient, StaticToken, TokenProvider, UploadTicket,
};
pub use error::GenError;
pub use job::{GenerationJob, JobStatus};
pub use keys::{KeyStore, KeyringStore, MemoryKeyStore, ProviderKey};
pub use params::{AudioParams, GenerationParams, ImageParams, UpscaleParams, VideoParams};
pub use provider::{
    content_type_for, ElevenLabsAdapter, FalAdapter, ModelRoute, OpenAiAdapter, ProviderAdapter,
    ProviderRegistry, ReplicateAdapter,
};
pub use transport::{
    Body, HttpRequest, HttpResponse, HttpTransport, Method, MockTransport, ReqwestTransport,
};

// Re-export the domain input type assembled by `build_params` for downstream use.
pub use opentake_domain::GenerationInput;
