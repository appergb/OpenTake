//! Locale matching — pure logic, verbatim port of `Transcription.matchLocale` /
//! `bestSupportedLocale` (`Transcription.swift:72-90`).
//!
//! Locale identifiers are BCP-47-ish strings like `"en"`, `"en-US"`,
//! `"zh-Hans-CN"`. We compare on the **language subtag** first (the part before
//! the first `-`/`_`), then prefer a matching **region subtag** (an uppercase
//! 2-letter or 3-digit subtag), falling back to the first same-language entry.

/// Language subtag (lowercased) of a locale identifier.
fn language_of(id: &str) -> String {
    id.split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Region subtag of a locale identifier, if present: a 2-letter alphabetic or
/// 3-digit subtag. Returned uppercased for alphabetic regions.
fn region_of(id: &str) -> Option<String> {
    id.split(['-', '_']).skip(1).find_map(|part| {
        let is_alpha2 = part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic());
        let is_digit3 = part.len() == 3 && part.chars().all(|c| c.is_ascii_digit());
        if is_alpha2 {
            Some(part.to_ascii_uppercase())
        } else if is_digit3 {
            Some(part.to_string())
        } else {
            None
        }
    })
}

/// First candidate whose language is supported; within that language, prefer the
/// entry with the same region, else the first same-language entry. Returns the
/// *supported* identifier (not the candidate). Port of `matchLocale`.
pub fn match_locale(candidates: &[&str], supported: &[&str]) -> Option<String> {
    for cand in candidates {
        let lang = language_of(cand);
        if lang.is_empty() {
            continue;
        }
        let same_lang: Vec<&&str> = supported
            .iter()
            .filter(|s| language_of(s) == lang)
            .collect();
        if same_lang.is_empty() {
            continue;
        }
        let region = region_of(cand);
        let chosen = same_lang
            .iter()
            .find(|s| region_of(s) == region)
            .or_else(|| same_lang.first())
            .map(|s| s.to_string());
        return chosen;
    }
    None
}

/// Pick the best supported locale from the host's preferred list followed by a
/// `current` fallback. Port of `bestSupportedLocale`. `preferred` is the ordered
/// preferred-language list; `current` is the system current locale.
pub fn best_supported_locale(
    preferred: &[&str],
    current: &str,
    supported: &[&str],
) -> Option<String> {
    let mut candidates: Vec<&str> = preferred.to_vec();
    candidates.push(current);
    match_locale(&candidates, supported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_same_language_same_region() {
        let supported = ["en-GB", "en-US", "fr-FR"];
        assert_eq!(
            match_locale(&["en-US"], &supported),
            Some("en-US".to_string())
        );
    }

    #[test]
    fn falls_back_to_first_same_language_when_region_absent() {
        let supported = ["en-GB", "en-US"];
        // candidate has no region → first same-language entry (en-GB).
        assert_eq!(match_locale(&["en"], &supported), Some("en-GB".to_string()));
    }

    #[test]
    fn falls_back_to_first_same_language_when_region_unmatched() {
        let supported = ["en-GB", "en-US"];
        // candidate region AU not present → first same-language (en-GB).
        assert_eq!(
            match_locale(&["en-AU"], &supported),
            Some("en-GB".to_string())
        );
    }

    #[test]
    fn tries_candidates_in_order() {
        let supported = ["fr-FR", "de-DE"];
        // zh unsupported, then fr supported.
        assert_eq!(
            match_locale(&["zh-CN", "fr-FR"], &supported),
            Some("fr-FR".to_string())
        );
    }

    #[test]
    fn no_language_match_is_none() {
        let supported = ["fr-FR", "de-DE"];
        assert_eq!(match_locale(&["ja-JP"], &supported), None);
    }

    #[test]
    fn handles_underscore_separator() {
        let supported = ["en_US", "en_GB"];
        assert_eq!(
            match_locale(&["en_GB"], &supported),
            Some("en_GB".to_string())
        );
    }

    #[test]
    fn language_only_supported_entry() {
        let supported = ["en", "fr"];
        assert_eq!(match_locale(&["en-US"], &supported), Some("en".to_string()));
    }

    #[test]
    fn best_supported_uses_preferred_then_current() {
        let supported = ["es-ES", "en-US"];
        // preferred ja (unsupported), current en-US (supported).
        assert_eq!(
            best_supported_locale(&["ja-JP"], "en-US", &supported),
            Some("en-US".to_string())
        );
    }

    #[test]
    fn region_parsing_ignores_script_subtag() {
        // zh-Hans-CN: language zh, script Hans (ignored), region CN.
        assert_eq!(language_of("zh-Hans-CN"), "zh");
        assert_eq!(region_of("zh-Hans-CN"), Some("CN".to_string()));
    }

    #[test]
    fn digit3_region_supported() {
        // UN M.49 numeric region like 419 (Latin America).
        assert_eq!(region_of("es-419"), Some("419".to_string()));
    }
}
