// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! Case mapping behind String.prototype.toLocaleUpperCase and
//! toLocaleLowerCase (ECMA-402 TransformCase). JS strings are UTF-16 and may
//! hold unpaired surrogates, which ICU4X's UTF-8 API cannot represent, so the
//! text is split at each unpaired surrogate: every well-formed run is mapped
//! on its own, and the surrogate is copied through unchanged.

use icu::casemap::CaseMapper;
use icu::locale::LanguageIdentifier;

/// The full upper (`upper`) or lower case mapping of `text`.
/// `language` is "und" or a language tag whose language subtag selects the
/// tailoring (ICU4X tailors tr, az, lt, el, hy for these two mappings). None
/// if `language` does not parse. The empty string does not parse, so it
/// gives None.
///
/// Context-sensitive rules (the final sigma) do not look across an unpaired
/// surrogate.
pub(crate) fn case_map(text: &[u16], language: &str, upper: bool) -> Option<Vec<u16>> {
    let langid: LanguageIdentifier = language.parse().ok()?;
    let mapper = CaseMapper::new();
    let mut out: Vec<u16> = Vec::with_capacity(text.len());
    // The current well-formed run, as UTF-8.
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut Vec<u16>| {
        if run.is_empty() {
            return;
        }
        let mapped = if upper {
            mapper.uppercase_to_string(run, &langid)
        } else {
            mapper.lowercase_to_string(run, &langid)
        };
        out.extend(mapped.encode_utf16());
        run.clear();
    };
    for unit in char::decode_utf16(text.iter().copied()) {
        match unit {
            Ok(c) => run.push(c),
            Err(e) => {
                flush(&mut run, &mut out);
                out.push(e.unpaired_surrogate());
            }
        }
    }
    flush(&mut run, &mut out);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(text: &str, language: &str, upper: bool) -> String {
        let units: Vec<u16> = text.encode_utf16().collect();
        String::from_utf16(&case_map(&units, language, upper).unwrap()).unwrap()
    }

    #[test]
    fn default_mapping() {
        assert_eq!(map("ß", "und", true), "SS");
        assert_eq!(map("ﬃ", "und", true), "FFI");
        assert_eq!(map("İ", "und", false), "i\u{307}");
        assert_eq!(map("ΑΣ", "und", false), "ας");
        assert_eq!(map("\u{10428}", "und", true), "\u{10400}");
    }

    #[test]
    fn tailored_mapping() {
        for lang in ["tr", "az"] {
            assert_eq!(map("i", lang, true), "İ");
            assert_eq!(map("I", lang, false), "ı");
        }
        assert_eq!(map("Ì", "lt", false), "i\u{307}\u{300}");
        assert_eq!(map("άλφα", "el", true), "ΑΛΦΑ");
        assert_eq!(map("և", "hy", true), "ԵՎ");
    }

    #[test]
    fn unpaired_surrogates() {
        assert_eq!(
            case_map(&[0x61, 0xD800, 0x62], "und", true).unwrap(),
            [0x41, 0xD800, 0x42]
        );
        assert_eq!(
            case_map(&[0xDC00, 0xD800], "und", true).unwrap(),
            [0xDC00, 0xD800]
        );
        assert_eq!(
            case_map(&[0xD801, 0xDC28, 0xDC00], "und", true).unwrap(),
            [0xD801, 0xDC00, 0xDC00]
        );
        assert_eq!(case_map(&[], "und", true).unwrap(), []);
    }

    #[test]
    fn rejects_bad_language() {
        assert_eq!(case_map(&[0x61], "en_US", true), None);
    }

    #[test]
    fn sigma_before_an_unpaired_surrogate() {
        // Each run is mapped alone, so a lone surrogate ends the text for the
        // final-sigma rule: a sigma right before it is final when a letter
        // precedes it in the run, and a sigma alone is not final.
        assert_eq!(
            case_map(&[0x391, 0x3A3, 0xD800], "und", false).unwrap(),
            [0x3B1, 0x3C2, 0xD800]
        );
        assert_eq!(
            case_map(&[0x3A3, 0xD800], "und", false).unwrap(),
            [0x3C3, 0xD800]
        );
    }

    #[test]
    fn long_text() {
        let text = vec![0x61u16; 1_000_000];
        let out = case_map(&text, "und", true).unwrap();
        assert_eq!(out.len(), 1_000_000);
        assert!(out.iter().all(|&u| u == 0x41));
    }

    #[test]
    fn empty_language() {
        // "" is not a language tag; callers pass "und" for the default.
        assert_eq!(case_map(&[0xDF], "", true), None);
    }
}
