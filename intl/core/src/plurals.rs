// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The plural rules behind Intl.PluralRules and the plural forms of
//! Intl.NumberFormat.
//!
//! Selection takes a value as a significand string and a power of ten and
//! builds the UTS 35 operands from its digits, exactly, with
//! RawPluralOperands. ICU4X's own conversions (Decimal, and
//! PluralOperands::from_significand_and_exponent) keep only 18 fraction
//! digits and take a u8 exponent, so they are not used.

use crate::locale::language_id;
use icu::locale::LanguageIdentifier;
use icu::plurals::{
    PluralCategory, PluralOperands, PluralRuleType, PluralRules, PluralRulesOptions,
    PluralRulesPreferences, PluralRulesWithRanges, RawPluralOperands,
};

/// 10^18. An operand of more than 18 digits keeps its last 18 digits plus
/// this, ICU4X's convention for `i`: tests modulo a power of ten up to 10^18
/// keep their results, and the operand never equals a small number.
const LIMIT_18_DIGITS: u64 = 1_000_000_000_000_000_000;

/// The locale ICU4X's plural data is looked up with for `id`. CLDR keys
/// plural rules by language and language-region only (its one key with a
/// script, kok-Latn, has kok's rules), but ICU4X's locale fallback takes a
/// locale whose CLDR parent is root, such as sr-Latn or bs-Cyrl, to root's
/// rules, which have only "other"; so does a region that implies another
/// script (ku-IQ). So the script and variants are dropped, and the region
/// is kept only when ICU4X has rules for the language and region
/// themselves (pt-PT).
fn plural_data_locale(id: &LanguageIdentifier, ordinal: bool) -> LanguageIdentifier {
    use icu::plurals::provider::{Baked, PluralsCardinalV1, PluralsOrdinalV1};
    use icu_provider::prelude::*;
    let mut key = LanguageIdentifier::from((id.language, None, id.region));
    if key.region.is_some() {
        let locale = DataLocale::from(&key);
        let mut metadata = DataRequestMetadata::default();
        metadata.silent = true;
        let request = DataRequest {
            id: DataIdentifierBorrowed::for_locale(&locale),
            metadata,
        };
        // The baked provider names the locale it fell back to, if any.
        let exact = if ordinal {
            DataProvider::<PluralsOrdinalV1>::load(&Baked, request)
                .is_ok_and(|r| r.metadata.locale.is_none())
        } else {
            DataProvider::<PluralsCardinalV1>::load(&Baked, request)
                .is_ok_and(|r| r.metadata.locale.is_none())
        };
        if !exact {
            key.region = None;
        }
    }
    key
}

/// The cardinal (`ordinal` false) or ordinal plural rules, with plural
/// ranges, for the language identifier of `locale` (a canonical tag), looked
/// up as `plural_data_locale` says. None if `locale` does not parse or
/// ICU4X has no data.
pub(crate) fn new_plural_rules(
    locale: &str,
    ordinal: bool,
) -> Option<PluralRulesWithRanges<PluralRules>> {
    let prefs = PluralRulesPreferences::from(&plural_data_locale(&language_id(locale)?, ordinal));
    let options = PluralRulesOptions::default().with_type(if ordinal {
        PluralRuleType::Ordinal
    } else {
        PluralRuleType::Cardinal
    });
    PluralRulesWithRanges::try_new(prefs, options).ok()
}

/// The CLDR name of `category`: "zero", "one", "two", "few", "many" or
/// "other".
pub(crate) fn category_name(category: PluralCategory) -> &'static str {
    match category {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}

/// The value of `digits` (ASCII digits, no leading zeros), with the
/// LIMIT_18_DIGITS convention for more than 18 digits.
fn operand_value(digits: &[u8]) -> u64 {
    let parse = |d: &[u8]| d.iter().fold(0u64, |n, &b| n * 10 + u64::from(b - b'0'));
    if digits.len() <= 18 {
        parse(digits)
    } else {
        parse(&digits[digits.len() - 18..]) + LIMIT_18_DIGITS
    }
}

/// `digits` without its leading zeros.
fn strip_leading_zeros(digits: &[u8]) -> &[u8] {
    let start = digits
        .iter()
        .position(|&b| b != b'0')
        .unwrap_or(digits.len());
    &digits[start..]
}

/// The UTS 35 operands of significand × 10^exponent, by decision 13 of the
/// Phase 3 plan: `i`, `v`, `w`, `f` and `t` from the digits of the value
/// (the significand's with its decimal point moved `exponent` places right),
/// and `c` the exponent when it is positive, else 0. `significand` is one
/// or more ASCII digits, optionally followed by "." and one or more digits;
/// None for anything else.
fn operands(significand: &str, exponent: i32) -> Option<RawPluralOperands> {
    let bytes = significand.as_bytes();
    let (int_part, frac_part) = match bytes.iter().position(|&b| b == b'.') {
        Some(dot) => (&bytes[..dot], &bytes[dot + 1..]),
        None => (bytes, &[][..]),
    };
    let well_formed = |d: &[u8]| d.iter().all(u8::is_ascii_digit);
    if int_part.is_empty()
        || !well_formed(int_part)
        || !well_formed(frac_part)
        || (frac_part.is_empty() && int_part.len() != bytes.len())
    {
        return None;
    }
    // The digits of the significand and of the value are the same, with the
    // decimal point at `point` from the start of `digits`: the value's
    // integer digits are digits[..point] followed by point - len zeros, and
    // its fraction digits -point zeros followed by digits[point..].
    let digits: Vec<u8> = int_part.iter().chain(frac_part).copied().collect();
    let len = digits.len() as i64;
    let point = int_part.len() as i64 + i64::from(exponent);

    // The integer digits other than the appended zeros, and how many zeros.
    let head = &digits[..point.clamp(0, len) as usize];
    let zeros = (point - len).max(0) as u64;
    let head = strip_leading_zeros(head);
    let i = if head.is_empty() {
        0
    } else if head.len() as u64 + zeros <= 18 {
        operand_value(head) * 10u64.pow(zeros as u32)
    } else if zeros >= 18 {
        LIMIT_18_DIGITS
    } else {
        let kept = 18 - zeros as usize;
        operand_value(&head[head.len().saturating_sub(kept)..]) * 10u64.pow(zeros as u32)
            + LIMIT_18_DIGITS
    };

    // The fraction digits other than the leading zeros, and how many zeros.
    let tail = &digits[point.clamp(0, len) as usize..];
    let leading = (-point).max(0) as u64;
    let v = if tail.is_empty() {
        0
    } else {
        leading + tail.len() as u64
    };
    let f = operand_value(strip_leading_zeros(tail));
    let (w, t) = match tail.iter().rposition(|&b| b != b'0') {
        Some(last) => (
            leading + last as u64 + 1,
            operand_value(strip_leading_zeros(&tail[..=last])),
        ),
        None => (0, 0),
    };
    Some(RawPluralOperands {
        i,
        v: usize::try_from(v).ok()?,
        w: usize::try_from(w).ok()?,
        f,
        t,
        c: usize::try_from(exponent.max(0)).ok()?,
    })
}

/// The plural category of significand × 10^exponent (see `operands`). None
/// if `significand` is not plain ASCII digits with an optional "." between
/// digits.
pub(crate) fn select(
    rules: &PluralRulesWithRanges<PluralRules>,
    significand: &str,
    exponent: i32,
) -> Option<PluralCategory> {
    let operands = PluralOperands::from(operands(significand, exponent)?);
    Some(rules.rules().category_for(operands))
}

/// The plural category of a range from the categories of its ends, given
/// by their CLDR names. None if either is not a category name.
pub(crate) fn select_range(
    rules: &PluralRulesWithRanges<PluralRules>,
    start: &str,
    end: &str,
) -> Option<PluralCategory> {
    Some(rules.resolve_range(
        PluralCategory::get_for_cldr_string(start)?,
        PluralCategory::get_for_cldr_string(end)?,
    ))
}

/// The names of the categories `rules` has, in the order zero, one, two,
/// few, many, other. "other" is always last.
pub(crate) fn categories(rules: &PluralRulesWithRanges<PluralRules>) -> Vec<&'static str> {
    rules.rules().categories().map(category_name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(locale: &str, ordinal: bool) -> PluralRulesWithRanges<PluralRules> {
        new_plural_rules(locale, ordinal).unwrap()
    }

    /// The category name for significand × 10^exponent.
    fn sel(r: &PluralRulesWithRanges<PluralRules>, s: &str, e: i32) -> &'static str {
        category_name(select(r, s, e).unwrap())
    }

    #[test]
    fn cardinal_and_ordinal() {
        let en = rules("en", false);
        assert_eq!(sel(&en, "1", 0), "one");
        assert_eq!(sel(&en, "1.0", 0), "other");
        assert_eq!(sel(&en, "2", 0), "other");
        let en_ord = rules("en", true);
        assert_eq!(sel(&en_ord, "2", 0), "two");
        assert_eq!(sel(&en_ord, "3", 0), "few");
        assert_eq!(sel(&en_ord, "11", 0), "other");
        let ar = rules("ar", false);
        assert_eq!(sel(&ar, "0", 0), "zero");
        assert_eq!(sel(&ar, "2", 0), "two");
        assert_eq!(sel(&ar, "3", 0), "few");
        assert_eq!(sel(&ar, "11", 0), "many");
    }

    #[test]
    fn exponents() {
        let fr = rules("fr", false);
        assert_eq!(sel(&fr, "1", 6), "many");
        assert_eq!(sel(&fr, "1.5", 6), "many");
        assert_eq!(sel(&fr, "1", 0), "one");
        // i = 0, which fr's "one" rule (i = 0,1) matches.
        assert_eq!(sel(&fr, "1", -6), "one");
        // 0.001: v = 3, so none of ru's integer rules match.
        assert_eq!(sel(&rules("ru", false), "1", -3), "other");
    }

    #[test]
    fn long_operands() {
        // f = 1 with v = 20, past the 18 fraction digits ICU4X keeps.
        assert_eq!(sel(&rules("bs", false), "0.00000000000000000001", 0), "one");
        // 24 digits: i keeps its last 18 digits plus 10^18, so i % 10 = 1
        // and i % 100 = 1.
        assert_eq!(
            sel(&rules("ru", false), "100000000000000000000001", 0),
            "one"
        );
        // i is 10^18, not 1.
        let big = format!("1{}", "0".repeat(30));
        assert_eq!(sel(&rules("en", false), &big, 0), "other");
    }

    #[test]
    fn operand_values() {
        let ops = |s, e| {
            let o = operands(s, e).unwrap();
            (o.i, o.v, o.w, o.f, o.t, o.c)
        };
        assert_eq!(ops("1.50", 0), (1, 2, 1, 50, 5, 0));
        assert_eq!(ops("1.50", 1), (15, 1, 0, 0, 0, 1));
        assert_eq!(ops("1.50", 3), (1500, 0, 0, 0, 0, 3));
        assert_eq!(ops("1.50", -2), (0, 4, 3, 150, 15, 0));
        assert_eq!(ops("0.00", 0), (0, 2, 0, 0, 0, 0));
        assert_eq!(ops("007", 0), (7, 0, 0, 0, 0, 0));
        assert_eq!(ops("12", -1), (1, 1, 1, 2, 2, 0));
        // Digits past the 18th keep the last 18 plus 10^18.
        let one_e18 = 1_000_000_000_000_000_000;
        assert_eq!(ops("1", 18), (one_e18, 0, 0, 0, 0, 18));
        assert_eq!(
            ops("123", 17),
            (one_e18 + 300_000_000_000_000_000, 0, 0, 0, 0, 17)
        );
        assert_eq!(
            ops("0.1234567890123456789", 0),
            (
                0,
                19,
                19,
                one_e18 + 234_567_890_123_456_789,
                one_e18 + 234_567_890_123_456_789,
                0
            )
        );
        // Extreme exponents need no digits of their own.
        assert_eq!(ops("5", i32::MAX), (one_e18, 0, 0, 0, 0, i32::MAX as usize));
        assert_eq!(ops("5", i32::MIN), (0, 1usize << 31, 1usize << 31, 5, 5, 0));
    }

    #[test]
    fn ranges() {
        let r = |locale, start, end| {
            category_name(select_range(&rules(locale, false), start, end).unwrap())
        };
        assert_eq!(r("en", "one", "other"), "other");
        assert_eq!(r("fr", "one", "one"), "one");
        // Entries that are not the end's category.
        assert_eq!(r("ar", "one", "two"), "other");
        assert_eq!(r("sl", "one", "one"), "few");
        // CLDR has no ar (two, one) entry, so the end's category is used.
        assert_eq!(r("ar", "two", "one"), "one");
        assert!(select_range(&rules("en", false), "One", "other").is_none());
        assert!(select_range(&rules("en", false), "one", "").is_none());
    }

    #[test]
    fn categories_order() {
        assert_eq!(
            categories(&rules("en", true)),
            ["one", "two", "few", "other"]
        );
        assert_eq!(
            categories(&rules("ar", false)),
            ["zero", "one", "two", "few", "many", "other"]
        );
    }

    #[test]
    fn rejects_bad_input() {
        let en = rules("en", false);
        for s in [
            "-1", "1e3", "1.2.3", "", ".", ".5", "5.", "+1", " 1", "1 ", "١",
        ] {
            assert!(select(&en, s, 0).is_none(), "{s:?}");
        }
        assert!(new_plural_rules("en_US", false).is_none());
    }

    #[test]
    fn script_variants_use_the_language_rules() {
        // CLDR keys plural rules by language and language-region; ICU4X's
        // fallback takes these locales to root, which has only "other".
        assert_eq!(sel(&rules("sr-Latn", false), "2", 0), "few");
        assert_eq!(
            categories(&rules("bs-Cyrl", false)),
            ["one", "few", "other"]
        );
        assert_eq!(categories(&rules("uz-Cyrl", false)), ["one", "other"]);
        assert_eq!(sel(&rules("sr-Latn-BA", false), "21", 0), "one");
        // The region stays: pt-PT has its own rules (CLDR 49 plurals.json:
        // pt one is "i = 0..1", pt-PT one is "i = 1 and v = 0").
        assert_eq!(sel(&rules("pt", false), "0", 0), "one");
        assert_eq!(sel(&rules("pt-PT", false), "0", 0), "other");
        // A region that implies another script must not lose the rules.
        assert_eq!(categories(&rules("ku-Latn-IQ", false)), ["one", "other"]);
        assert_eq!(sel(&rules("en-US-posix", false), "1", 0), "one");
        // CLDR's one key with a script, kok-Latn, has kok's rules.
        for ordinal in [false, true] {
            assert_eq!(
                categories(&rules("kok-Latn", ordinal)),
                categories(&rules("kok", ordinal))
            );
        }
    }

    #[test]
    fn every_locale() {
        for locale in crate::locale::test_locales() {
            for ordinal in [false, true] {
                let r = new_plural_rules(locale, ordinal)
                    .unwrap_or_else(|| panic!("{locale} ordinal {ordinal}"));
                assert_eq!(
                    categories(&r).last(),
                    Some(&"other"),
                    "{locale} ordinal {ordinal}"
                );
                for s in ["0", "1", "2.5"] {
                    assert!(select(&r, s, 0).is_some(), "{locale} {ordinal} {s}");
                }
                // A locale whose language has plural forms has them too.
                let language = locale.split('-').next().unwrap();
                if categories(&rules(language, ordinal)).len() > 1 {
                    assert!(categories(&r).len() > 1, "{locale} ordinal {ordinal}");
                }
            }
        }
    }
}
