// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The unit patterns behind Intl.NumberFormat's `style: "unit"`: the six
//! plural patterns of a sanctioned unit or of a `X-per-Y` compound of two
//! sanctioned units, from the CLDR data of `cldr` (decisions 4 and 14 of the
//! Phase 3 plan). ICU4X's own unit names are not used: it lacks 21 of the 45
//! units and its long names are wrong.

use crate::cldr;
use crate::locale::language_id;

/// The plural categories, in the order of the patterns returned.
const CATEGORIES: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

/// The 45 sanctioned units of ECMA-402, in the order of `SANCTIONED_UNITS`
/// in `js/10-Intl.js` (a test checks that the two lists are the same).
const SANCTIONED_UNITS: [&str; 45] = [
    "acre",
    "bit",
    "byte",
    "celsius",
    "centimeter",
    "day",
    "degree",
    "fahrenheit",
    "fluid-ounce",
    "foot",
    "gallon",
    "gigabit",
    "gigabyte",
    "gram",
    "hectare",
    "hour",
    "inch",
    "kilobit",
    "kilobyte",
    "kilogram",
    "kilometer",
    "liter",
    "megabit",
    "megabyte",
    "meter",
    "microsecond",
    "mile",
    "mile-scandinavian",
    "milliliter",
    "millimeter",
    "millisecond",
    "minute",
    "month",
    "nanosecond",
    "ounce",
    "percent",
    "petabyte",
    "pound",
    "second",
    "stone",
    "terabit",
    "terabyte",
    "week",
    "yard",
    "year",
];

/// Whether `unit` is a sanctioned unit.
fn is_sanctioned(unit: &str) -> bool {
    SANCTIONED_UNITS.contains(&unit)
}

/// Whether `c` is ECMA-262 WhiteSpace or LineTerminator, which is the
/// whitespace JS treats as next to `{0}` (decision 10).
fn is_js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

/// The patterns of `unit` at `width` in the units.txt locale `tag`, for the
/// six plural categories, each as the file has it (through the parents) or,
/// if the locale has no pattern for the category, "other"'s. None if there
/// is no "other" pattern.
fn category_patterns(tag: &str, width: &str, unit: &str) -> Option<Vec<&'static str>> {
    let units = cldr::units();
    let other = units.pattern(tag, width, unit, "other")?;
    Some(
        CATEGORIES
            .iter()
            .map(|c| units.pattern(tag, width, unit, c).unwrap_or(other))
            .collect(),
    )
}

/// The unit patterns for the language identifier of `locale` (a canonical
/// tag), `unit` and `width` (`long`, `short` or `narrow`): the patterns for
/// the plural categories zero, one, two, few, many and other, in CLDR's
/// form (`"{0} miles"`), where a category the locale lacks has "other"'s.
/// `unit` is a sanctioned unit or `X-per-Y` of two sanctioned units, which
/// uses, in this order (decision 14; ICU4C's order):
///
/// 1. the dedicated compound unit, if the locale has one;
/// 2. Y's per-unit pattern with `{0}` replaced by X's pattern;
/// 3. the width's compound pattern with `{0}` replaced by X's pattern and
///    `{1}` by Y's "one" pattern without `{0}` and the space next to it.
///
/// A pattern may lack `{0}` (`ar` long meter "one" is `"متر"`, decision
/// 10), but "other" has it. None for any other unit or width, or a locale
/// that does not parse (the file has no pattern for such a width).
pub fn unit_patterns(locale: &str, unit: &str, width: &str) -> Option<Vec<String>> {
    let tag = cldr::units().locale_entry(&language_id(locale)?);
    let tag = tag.as_str();
    if is_sanctioned(unit) {
        return Some(strings(category_patterns(tag, width, unit)?));
    }
    let (x, y) = unit.split_once("-per-")?;
    if !is_sanctioned(x) || !is_sanctioned(y) {
        return None;
    }
    if let Some(dedicated) = category_patterns(tag, width, unit) {
        return Some(strings(dedicated));
    }
    let numerators = category_patterns(tag, width, x)?;
    if let Some(per) = cldr::units().pattern(tag, width, y, "per") {
        return Some(numerators.iter().map(|n| per.replace("{0}", n)).collect());
    }
    let compound = cldr::units().pattern(tag, width, "-", "per")?;
    let one = category_patterns(tag, width, y)?[1];
    // Y's "one" pattern without {0} and the whitespace next to it.
    let denominator = one.replace("{0}", "");
    let denominator = denominator.trim_matches(is_js_space);
    let compound = compound.replace("{1}", denominator);
    Some(
        numerators
            .iter()
            .map(|n| compound.replace("{0}", n))
            .collect(),
    )
}

/// Own the patterns.
fn strings(v: Vec<&str>) -> Vec<String> {
    v.into_iter().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(locale: &str, unit: &str, width: &str) -> Vec<String> {
        unit_patterns(locale, unit, width).unwrap()
    }
    fn cat(v: &[String], c: &str) -> String {
        v[CATEGORIES.iter().position(|x| *x == c).unwrap()].clone()
    }

    #[test]
    fn simple_units() {
        assert_eq!(cat(&p("en", "kilometer", "short"), "other"), "{0} km");
        let mile = p("en", "mile", "long");
        assert_eq!(cat(&mile, "one"), "{0} mile");
        assert_eq!(cat(&mile, "other"), "{0} miles");
        assert_eq!(cat(&p("de", "byte", "long"), "other"), "{0} Byte");
        assert_eq!(cat(&p("en", "percent", "short"), "other"), "{0}%");
        assert_eq!(cat(&p("en", "celsius", "narrow"), "other"), "{0}°C");
    }

    #[test]
    fn long_meter_is_long() {
        assert_eq!(cat(&p("en", "meter", "long"), "other"), "{0} meters");
    }

    #[test]
    fn dedicated_compound() {
        assert_eq!(
            cat(&p("en", "kilometer-per-hour", "short"), "other"),
            "{0} km/h"
        );
        assert_eq!(
            cat(&p("ja", "kilometer-per-hour", "long"), "other"),
            "時速 {0} キロメートル"
        );
    }

    #[test]
    fn per_unit_pattern() {
        assert_eq!(
            cat(&p("en", "kilobyte-per-hour", "short"), "other"),
            "{0} kB/h"
        );
        assert_eq!(
            cat(&p("en", "byte-per-hour", "long"), "one"),
            "{0} byte per hour"
        );
    }

    #[test]
    fn compound_pattern() {
        // acre has no per-unit pattern.
        let v = p("en", "byte-per-acre", "long");
        assert_eq!(cat(&v, "other"), "{0} bytes per acre");
        assert_eq!(cat(&v, "one"), "{0} byte per acre");
    }

    #[test]
    fn missing_categories_get_others() {
        let v = p("en", "mile", "long");
        assert_eq!(cat(&v, "zero"), "{0} miles");
        assert_eq!(cat(&v, "few"), "{0} miles");
        let ru = p("ru", "kilometer", "long");
        assert_eq!(cat(&ru, "many"), "{0} километров");
        assert_eq!(cat(&ru, "other"), "{0} километра");
    }

    #[test]
    fn inherits_from_parent() {
        let units = cldr::units();
        // en-GB has no meter patterns of its own (en-001 has).
        assert_eq!(units.own_pattern("en-GB", "long", "meter", "other"), None);
        assert_eq!(cat(&p("en-GB", "meter", "long"), "other"), "{0} metres");
        assert_eq!(p("en-GB", "meter", "long"), p("en-001", "meter", "long"));
        // A tag that is not a key of the file uses the entry ICU4X's
        // fallback reaches.
        assert_eq!(p("en-US", "mile", "long"), p("en", "mile", "long"));
    }

    #[test]
    fn number_less_patterns() {
        // CLDR's "one" pattern of ar long meter has no {0} (decision 10).
        assert_eq!(cat(&p("ar", "meter", "long"), "one"), "متر");
    }

    #[test]
    fn rejects_bad_units() {
        for unit in [
            "furlong",
            "meter-per-",
            "-per-meter",
            "meter-per-meter-per-meter",
            "meter-per-furlong",
            "Meter",
            "",
            "kilometer-per-hour-per",
        ] {
            assert_eq!(unit_patterns("en", unit, "long"), None, "{unit}");
        }
        assert_eq!(unit_patterns("en", "meter", "Long"), None);
        assert_eq!(unit_patterns("en", "meter", ""), None);
        assert_eq!(unit_patterns("en_US", "meter", "long"), None);
    }

    #[test]
    fn sanctioned_units_match_the_js_list() {
        let js = include_str!("../../js/10-Intl.js");
        let start = js.find("var SANCTIONED_UNITS = [").unwrap();
        let list = &js[start..][..js[start..].find("];").unwrap()];
        let from_js: Vec<&str> = list.split('\'').skip(1).step_by(2).collect();
        assert_eq!(from_js, SANCTIONED_UNITS);
    }

    #[test]
    fn every_locale_every_unit() {
        let mut units: Vec<&str> = SANCTIONED_UNITS.to_vec();
        units.extend(["kilometer-per-hour", "byte-per-second"]);
        assert_eq!(units.len(), 47);
        for tag in crate::locale::test_locales() {
            for unit in &units {
                for width in ["long", "short", "narrow"] {
                    let v = unit_patterns(tag, unit, width)
                        .unwrap_or_else(|| panic!("{tag} {unit} {width}"));
                    assert_eq!(v.len(), 6, "{tag} {unit} {width}");
                    for (i, pat) in v.iter().enumerate() {
                        assert!(
                            pat.matches("{0}").count() <= 1 && !pat.contains("{1}"),
                            "{tag} {unit} {width} {pat:?}"
                        );
                        assert!(!pat.contains('\0'));
                        if i == 5 {
                            assert!(pat.contains("{0}"), "{tag} {unit} {width} {pat:?}");
                        }
                    }
                }
            }
        }
    }
}
