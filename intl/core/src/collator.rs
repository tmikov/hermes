// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The collator behind Intl.Collator and String.prototype.localeCompare.
//! JS validates and resolves the options (ECMA-402 Intl.Collator); this
//! module turns the resolved values into ICU4X collator settings.

use crate::locale::language_id;
use crate::supported::COLLATIONS;
use icu::collator::options::{AlternateHandling, CaseLevel, CollatorOptions, Strength};
use icu::collator::preferences::{CollationCaseFirst, CollationNumericOrdering, CollationType};
use icu::collator::{Collator, CollatorBorrowed, CollatorPreferences};
use icu::locale::extensions::unicode::Value;

/// A collator for the language identifier of `locale` (a canonical tag).
/// Each option is passed to ICU4X explicitly, not through "-u-" keywords:
/// - `collation` is "default" or a value of COLLATIONS;
/// - `sensitivity` is "base", "accent", "case" or "variant";
/// - `case_first` is "upper", "lower" or "false".
///
/// None for any other value, or if ICU4X has no data.
pub(crate) fn new_collator(
    locale: &str,
    collation: &str,
    sensitivity: &str,
    ignore_punctuation: bool,
    numeric: bool,
    case_first: &str,
) -> Option<CollatorBorrowed<'static>> {
    let mut prefs = CollatorPreferences::from(&language_id(locale)?);
    if collation != "default" {
        if !COLLATIONS.contains(&collation) {
            return None;
        }
        let value = Value::try_from_str(collation).ok()?;
        prefs.collation_type = Some(CollationType::try_from(&value).ok()?);
    }
    prefs.case_first = Some(match case_first {
        "upper" => CollationCaseFirst::Upper,
        "lower" => CollationCaseFirst::Lower,
        "false" => CollationCaseFirst::False,
        _ => return None,
    });
    prefs.numeric_ordering = Some(if numeric {
        CollationNumericOrdering::True
    } else {
        CollationNumericOrdering::False
    });

    let (strength, case_level) = match sensitivity {
        "base" => (Strength::Primary, CaseLevel::Off),
        "accent" => (Strength::Secondary, CaseLevel::Off),
        "case" => (Strength::Primary, CaseLevel::On),
        "variant" => (Strength::Tertiary, CaseLevel::Off),
        _ => return None,
    };
    let mut options = CollatorOptions::default();
    options.strength = Some(strength);
    options.case_level = Some(case_level);
    // Always set: th defaults to shifted.
    options.alternate_handling = Some(if ignore_punctuation {
        AlternateHandling::Shifted
    } else {
        AlternateHandling::NonIgnorable
    });
    Collator::try_new(prefs, options).ok()
}

/// ICU4X's defaults for the language identifier of `locale`, from a
/// collator made with no options: "<caseFirst> <ignorePunctuation>", for
/// example "false false", "upper false" (da, mt) or "false true" (th).
pub(crate) fn locale_defaults(locale: &str) -> Option<String> {
    let prefs = CollatorPreferences::from(&language_id(locale)?);
    let collator = Collator::try_new(prefs, CollatorOptions::default()).ok()?;
    let resolved = collator.resolved_options();
    let case_first = match resolved.case_first {
        CollationCaseFirst::Upper => "upper",
        CollationCaseFirst::Lower => "lower",
        _ => "false",
    };
    let shifted = resolved.alternate_handling == AlternateHandling::Shifted;
    Some(format!("{case_first} {shifted}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    fn make(locale: &str, collation: &str, sensitivity: &str) -> CollatorBorrowed<'static> {
        new_collator(locale, collation, sensitivity, false, false, "false").unwrap()
    }

    fn cmp(c: &CollatorBorrowed, x: &str, y: &str) -> Ordering {
        let x: Vec<u16> = x.encode_utf16().collect();
        let y: Vec<u16> = y.encode_utf16().collect();
        c.compare_utf16(&x, &y)
    }

    #[test]
    fn option_mapping() {
        // The sensitivity table of test262 non-normative-sensitivity.js.
        let input = ["Aa", "bA", "E", "b", "aA", "fC", "áÁ", "Aã"];
        let table: [(&str, &[&str]); 4] = [
            ("base", &["Aa", "aA", "áÁ", "Aã"]),
            ("accent", &["Aa", "aA"]),
            ("case", &["Aa", "Aã"]),
            ("variant", &["Aa"]),
        ];
        for (sensitivity, expected) in table {
            let c = make("en", "default", sensitivity);
            let equal: Vec<&str> = input
                .iter()
                .copied()
                .filter(|s| cmp(&c, "Aa", s) == Ordering::Equal)
                .collect();
            assert_eq!(equal, expected, "{sensitivity}");
        }
    }

    #[test]
    fn ignore_punctuation() {
        for locale in ["en", "th"] {
            let on = new_collator(locale, "default", "variant", true, false, "false").unwrap();
            assert_eq!(cmp(&on, "", " "), Ordering::Equal, "{locale}");
            assert_eq!(cmp(&on, "", "*"), Ordering::Equal, "{locale}");
            let off = new_collator(locale, "default", "variant", false, false, "false").unwrap();
            assert_eq!(cmp(&off, "", " "), Ordering::Less, "{locale}");
            assert_eq!(cmp(&off, "", "*"), Ordering::Less, "{locale}");
        }
    }

    #[test]
    fn numeric_and_case_first() {
        let n = |numeric, case_first| {
            new_collator("en", "default", "variant", false, numeric, case_first).unwrap()
        };
        assert_eq!(cmp(&n(true, "false"), "2", "10"), Ordering::Less);
        assert_eq!(cmp(&n(false, "false"), "10", "2"), Ordering::Less);
        assert_eq!(cmp(&n(false, "upper"), "A", "a"), Ordering::Less);
        assert_eq!(cmp(&n(false, "lower"), "a", "A"), Ordering::Less);
    }

    #[test]
    fn collation_types() {
        // The values of test262 non-normative-phonebook.js.
        let c = make("de", "phonebk", "variant");
        let mut list = ["A", "b", "Af", "Ab", "od", "off", "Ä", "ö"];
        list.sort_by(|x, y| cmp(&c, x, y));
        assert_eq!(list, ["A", "Ab", "Ä", "Af", "b", "od", "ö", "off"]);
    }

    #[test]
    fn canonical_equivalence() {
        let c = make("en", "default", "variant");
        for (x, y) in [
            ("o\u{308}", "ö"),
            ("Å", "A\u{30A}"),
            ("\u{1111}\u{1171}\u{11B6}", "퓛"),
        ] {
            assert_eq!(cmp(&c, x, y), Ordering::Equal, "{x:?} {y:?}");
        }
    }

    #[test]
    fn unusual_text() {
        let c = make("en", "default", "variant");
        let big = vec![b'a' as u16; 1_000_000];
        let texts: [&[u16]; 5] = [
            &[0xD800],
            &[0x61, 0, 0x62],
            &[0x61, 0, 0x63],
            &[0xD801, 0xDC28],
            &big,
        ];
        for x in texts {
            assert_eq!(c.compare_utf16(x, x), Ordering::Equal);
            for y in texts {
                assert_eq!(c.compare_utf16(x, y), c.compare_utf16(y, x).reverse());
            }
        }
        assert_ne!(
            c.compare_utf16(texts[1], texts[2]),
            Ordering::Equal,
            "a\\0b and a\\0c"
        );
    }

    #[test]
    fn rejects_bad_values() {
        let f = |locale, co, sens, cf| new_collator(locale, co, sens, false, false, cf).is_none();
        assert!(!f("en", "default", "variant", "false"));
        assert!(f("en", "standard", "variant", "false"));
        assert!(f("en", "search", "variant", "false"));
        assert!(f("en", "xyz", "variant", "false"));
        assert!(f("en", "default", "Base", "false"));
        assert!(f("en", "default", "variant", "FALSE"));
        assert!(f("en_US", "default", "variant", "false"));
    }

    #[test]
    fn locale_defaults_of_locales() {
        for l in ["en", "ja", "de", "und"] {
            assert_eq!(locale_defaults(l).as_deref(), Some("false false"), "{l}");
        }
        for l in ["da", "mt"] {
            assert_eq!(locale_defaults(l).as_deref(), Some("upper false"), "{l}");
        }
        for l in ["th", "th-TH"] {
            assert_eq!(locale_defaults(l).as_deref(), Some("false true"), "{l}");
        }
        assert_eq!(locale_defaults("en_US"), None);
    }
}
