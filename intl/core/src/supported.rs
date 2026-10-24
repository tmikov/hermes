// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The lists behind Intl.supportedValuesOf. Each value must also be accepted
//! by the constructor that uses it once that constructor exists (test262
//! checks this): calendars by DateTimeFormat, collations by Collator, and so
//! on. The "unit" list is the ECMA-402 table and lives in JS.
//!
//! The numbering systems are CLDR's numeric ones, which in CLDR 48.2 are
//! exactly ECMA-402's table of numbering systems with simple digit mappings;
//! the list must include every row of that table. ICU4X's compiled data has
//! digits for only 27 of them, so NumberFormat must supply the digits of the
//! others itself; the table and CLDR's numberingSystems.xml give the ten
//! digits of each.

use crate::host::primary_time_zone;
use icu::time::zone::iana::IanaParserExtended;
use icu::time::TimeZone;

/// Which list; matches HiValueKind in lib.rs.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Calendar,
    Collation,
    Currency,
    NumberingSystem,
    TimeZone,
}

/// The calendars ICU4X implements, as CLDR calendar ids. "islamic" and
/// "islamic-rgsa" are left out: ICU4X has no algorithm for them.
const CALENDARS: &[&str] = &[
    "buddhist",
    "chinese",
    "coptic",
    "dangi",
    "ethioaa",
    "ethiopic",
    "gregory",
    "hebrew",
    "indian",
    "islamic-civil",
    "islamic-tbla",
    "islamic-umalqura",
    "iso8601",
    "japanese",
    "persian",
    "roc",
];

/// CLDR's aliases of calendar ids, with their canonical forms. ECMA-402
/// 6.9.1 AvailableCalendars must list either both an alias and its
/// canonical form or neither, so each alias whose canonical form is in
/// CALENDARS is listed too; Intl.supportedValuesOf drops the aliases again
/// (ECMA-402 8.3.2 step 2.b). locale.rs canonicalizes them the same way.
const CALENDAR_ALIASES: &[(&str, &str)] = &[
    ("ethiopic-amete-alem", "ethioaa"),
    ("islamicc", "islamic-civil"),
];

/// The collation types ICU4X's compiled data has tailorings for. ECMA-402
/// excludes "standard" and "search". The data has none for "ducet",
/// "searchjl" (ICU4X leaves the search collations out), "phonetic" (CLDR has
/// it only for "ln", which is not among the compiled locales), or "big5han",
/// "direct", "gb2312" and "reformed".
const COLLATIONS: &[&str] = &[
    "compat", "dict", "emoji", "eor", "phonebk", "pinyin", "stroke", "trad", "unihan", "zhuyin",
];

/// The sorted, unique values of `kind`.
pub fn values(kind: Kind) -> Vec<String> {
    let mut list: Vec<String> = match kind {
        Kind::Calendar => CALENDARS
            .iter()
            .copied()
            .chain(
                CALENDAR_ALIASES
                    .iter()
                    .filter(|(_, canonical)| CALENDARS.contains(canonical))
                    .map(|(alias, _)| *alias),
            )
            .map(str::to_owned)
            .collect(),
        Kind::Collation => COLLATIONS.iter().map(|s| s.to_string()).collect(),
        Kind::Currency => data_lines(include_str!("../data/currencies.txt")),
        Kind::NumberingSystem => data_lines(include_str!("../data/numbering-systems.txt")),
        Kind::TimeZone => IanaParserExtended::new()
            .iter()
            .filter(|z| z.time_zone != TimeZone::UNKNOWN)
            .map(|z| primary_time_zone(z.canonical).to_owned())
            .collect(),
    };
    list.sort();
    list.dedup();
    list
}

/// The non-comment lines of a data file.
fn data_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use icu_provider::prelude::*;

    #[test]
    fn calendar_aliases_are_the_cldr_ones() {
        let cldr: Vec<(&str, &str)> = crate::locale::value_aliases()
            .iter()
            .filter(|((ext, key, _), value)| {
                (*ext, *key) == ("u", "ca") && CALENDARS.contains(value)
            })
            .map(|((_, _, alias), value)| (*alias, *value))
            .collect();
        assert_eq!(cldr, CALENDAR_ALIASES);
    }

    #[test]
    fn calendars_are_icu4x_calendars() {
        use icu::calendar::preferences::CalendarAlgorithm;
        use icu::calendar::AnyCalendarKind;
        use icu::locale::extensions::unicode::Value;
        for id in CALENDARS {
            let value = Value::try_from_str(id).unwrap();
            let algorithm = CalendarAlgorithm::try_from(&value).unwrap();
            assert!(AnyCalendarKind::try_from(algorithm).is_ok(), "{id}");
        }
        // The check can fail: ICU4X has no algorithm for plain "islamic".
        let islamic =
            CalendarAlgorithm::try_from(&Value::try_from_str("islamic").unwrap()).unwrap();
        assert!(AnyCalendarKind::try_from(islamic).is_err());
    }

    #[test]
    fn calendar_aliases_are_listed_with_their_canonical_forms() {
        let list = values(Kind::Calendar);
        for (alias, canonical) in CALENDAR_ALIASES {
            // ECMA-402 6.9.1: both or neither.
            assert!(CALENDARS.contains(canonical), "{canonical}");
            assert!(list.iter().any(|x| x == alias), "{alias}");
            // The alias canonicalizes to the listed canonical form.
            assert_eq!(
                crate::locale::canonicalize(&format!("und-u-ca-{alias}")),
                Some(format!("und-u-ca-{canonical}")),
                "{alias}"
            );
        }
        // Every listed value is a canonical calendar or one of the aliases.
        assert_eq!(list.len(), CALENDARS.len() + CALENDAR_ALIASES.len());
        assert!(list.windows(2).all(|w| w[0] < w[1]));
    }

    /// Load the collation tailoring for (`co`, `locale`) without fallback
    /// warnings. The baked provider falls back through the locale's parents
    /// (so "emoji" loads for every locale, from "und"), never across
    /// collation types.
    fn tailoring_loads(co: &str, locale: &str) -> bool {
        use icu::collator::provider::{Baked, CollationTailoringV1};
        let loc: icu::locale::Locale = locale.parse().unwrap();
        let data_locale = DataLocale::from(&loc);
        let mut metadata = DataRequestMetadata::default();
        metadata.silent = true;
        DataProvider::<CollationTailoringV1>::load(
            &Baked,
            DataRequest {
                id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
                    DataMarkerAttributes::try_from_str(co).unwrap(),
                    &data_locale,
                ),
                metadata,
            },
        )
        .is_ok()
    }

    #[test]
    fn collations_have_data() {
        let cases = [
            ("compat", "ar"),
            ("dict", "si"),
            ("emoji", "und"),
            ("eor", "und"),
            ("phonebk", "de"),
            ("pinyin", "zh"),
            ("stroke", "zh"),
            ("trad", "es"),
            ("unihan", "zh"),
            ("zhuyin", "zh"),
        ];
        // One case per listed collation, in the same (sorted) order.
        assert_eq!(cases.map(|(co, _)| co), COLLATIONS);
        for (co, locale) in cases {
            assert!(tailoring_loads(co, locale), "{co} for {locale}");
        }
        // The check can fail: there is no such collation, and ICU4X has no
        // data for these two.
        assert!(!tailoring_loads("zzzz", "de"));
        assert!(!tailoring_loads("phonetic", "ln"));
        assert!(!tailoring_loads("searchjl", "ko"));
    }

    #[test]
    fn currencies_are_well_formed() {
        let list = values(Kind::Currency);
        assert!(list.len() > 150);
        for c in &list {
            assert!(
                c.len() == 3 && c.bytes().all(|b| b.is_ascii_uppercase()),
                "{c}"
            );
        }
        for c in ["EUR", "JPY", "USD"] {
            assert!(list.iter().any(|x| x == c), "{c}");
        }
    }

    #[test]
    fn numbering_systems_cover_the_digit_table() {
        let list = values(Kind::NumberingSystem);
        // ECMA-402's table of numbering systems with simple digit mappings
        // (Draft ECMA-402, October 22, 2026, Table 30).
        let table = [
            "adlm", "ahom", "arab", "arabext", "bali", "beng", "bhks", "brah", "cakm", "cham",
            "deva", "diak", "fullwide", "gara", "gong", "gonm", "gujr", "gukh", "guru", "hanidec",
            "hmng", "hmnp", "java", "kali", "kawi", "khmr", "knda", "krai", "lana", "lanatham",
            "laoo", "latn", "lepc", "limb", "mathbold", "mathdbl", "mathmono", "mathsanb",
            "mathsans", "mlym", "modi", "mong", "mroo", "mtei", "mymr", "mymrepka", "mymrpao",
            "mymrshan", "mymrtlng", "nagm", "newa", "nkoo", "olck", "onao", "orya", "osma",
            "outlined", "rohg", "saur", "segment", "shrd", "sind", "sinh", "sora", "sund", "sunu",
            "takr", "talu", "tamldec", "telu", "thai", "tibt", "tirh", "tnsa", "tols", "vaii",
            "wara", "wcho",
        ];
        assert_eq!(list, table);
    }

    #[test]
    fn time_zones_use_primary_identifiers() {
        let list = values(Kind::TimeZone);
        for z in ["UTC", "Asia/Kolkata", "Europe/Kyiv", "America/New_York"] {
            assert!(list.iter().any(|x| x == z), "{z}");
        }
        // CST6CDT, EST5EDT, MST7MDT and PST8PDT are links since tzdb 2024b.
        for z in [
            "Etc/UTC",
            "Etc/Unknown",
            "Asia/Calcutta",
            "CST6CDT",
            "EST5EDT",
            "MST7MDT",
            "PST8PDT",
            "EST",
        ] {
            assert!(!list.iter().any(|x| x == z), "{z}");
        }
    }
}
