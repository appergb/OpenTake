//! The transcription backend's supported language set + validation.
//!
//! Upstream lists `SpeechTranscriber.supportedLocales` and validates a requested
//! language against it with `matchLocale` (`Transcription.swift:72-90`,
//! `add_captions` in `ToolExecutor+Captions.swift:20-26`). OpenTake's backend is
//! whisper.cpp, whose supported set is the fixed language table baked into the
//! multilingual models (99 base languages + Cantonese). We mirror that table here
//! as pure static data so the
//! Captions tab and the `add_captions` tool can validate a language and surface a
//! clear error *before* transcribing — without linking the native whisper lib
//! (the agent crate is pure). The whisper backend itself still receives the code
//! and is the final authority; this list is the pre-flight check.
//!
//! Codes are ISO-639-1 where one exists (whisper's own `whisper_lang_str` values),
//! e.g. `"en"`, `"zh"`, `"yue"` (Cantonese has no 2-letter code). Region/script
//! subtags are matched leniently by [`match_language`] via
//! [`crate::transcribe::locale::match_locale`], so `"en-GB"` resolves to `"en"`.

use super::locale::match_locale;

/// whisper.cpp's supported language codes (the multilingual models' full set).
/// Kept in the canonical order whisper emits them. This is the OpenTake analog of
/// upstream `SpeechTranscriber.supportedLocales`.
pub const WHISPER_LANGUAGES: &[&str] = &[
    "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv", "it",
    "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur",
    "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn",
    "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si",
    "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo",
    "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln",
    "ha", "ba", "jw", "su", "yue",
];

/// Resolve a requested language identifier (BCP-47-ish, e.g. `"es"`, `"en-GB"`,
/// `"zh-Hans-CN"`) to a supported whisper code, or `None` when the language isn't
/// supported. 1:1 with upstream's `Transcription.matchLocale(candidates:supported:)`
/// call in `add_captions`: matches on the language subtag, tolerating region and
/// script subtags. Returns the *supported* code (what the backend wants).
pub fn match_language(requested: &str) -> Option<String> {
    match_locale(&[requested], WHISPER_LANGUAGES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_code_matches_itself() {
        assert_eq!(match_language("es").as_deref(), Some("es"));
        assert_eq!(match_language("ja").as_deref(), Some("ja"));
    }

    #[test]
    fn region_and_script_subtags_are_tolerated() {
        assert_eq!(match_language("en-GB").as_deref(), Some("en"));
        assert_eq!(match_language("zh-Hans-CN").as_deref(), Some("zh"));
        assert_eq!(match_language("pt-BR").as_deref(), Some("pt"));
    }

    #[test]
    fn unsupported_language_is_none() {
        // A made-up / unsupported code returns None so the tool can error clearly.
        assert_eq!(match_language("xx"), None);
        assert_eq!(match_language("klingon"), None);
    }

    #[test]
    fn table_has_no_duplicates_and_expected_size() {
        let mut sorted = WHISPER_LANGUAGES.to_vec();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(before, sorted.len(), "duplicate language code in table");
        // whisper.cpp's multilingual set is 99 base languages + Cantonese (`yue`).
        assert_eq!(WHISPER_LANGUAGES.len(), 100);
        assert!(WHISPER_LANGUAGES.contains(&"yue"));
    }
}
