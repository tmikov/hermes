// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! Locale-information lookups: the CLDR data behind the Intl.Locale Info
//! methods (ECMA-402, "Intl.Locale.prototype.getCalendars" and friends).
//! Each table is a generated file in core/data, parsed once on first use.

use crate::host::time_zone_canonicalize;
use crate::supported::{data_lines, tailoring_loads, COLLATIONS};
use icu::calendar::week::{WeekInformation, WeekPreferences};
use icu::locale::subtags::{Region, Script};
use icu::locale::{Direction, Locale, LocaleDirectionality};
use std::collections::BTreeMap;
use std::sync::OnceLock;

type Table = BTreeMap<String, Vec<String>>;

/// Parse a generated `<key> <value>...` file; `#` lines are comments.
fn parse(text: &str) -> Table {
    let mut table = Table::new();
    for line in data_lines(text) {
        let mut words = line.split_whitespace();
        let key = words.next().expect("data file: empty line");
        let values: Vec<String> = words.map(str::to_owned).collect();
        assert!(!values.is_empty(), "data file: no values for {key}");
        assert!(
            table.insert(key.to_owned(), values).is_none(),
            "data file: repeated key {key}"
        );
    }
    table
}

fn calendar_table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| parse(include_str!("../data/calendar-preferences.txt")))
}

fn hour_cycle_table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| parse(include_str!("../data/hour-cycles.txt")))
}

/// The line of `table` for `key`, or, when `key` is a region code with no
/// line of its own, the `001` line: CLDR's `001` entry is the default for
/// every region it does not list (UTS 35 Part 4, "Calendar Preference Data"
/// and "Time Data"), as for week data. A `<language>-<region>` key with no
/// line gets `None`, so the caller can try the region next.
fn region_line<'a>(table: &'a Table, key: &str) -> Option<&'a [String]> {
    if let Some(line) = table.get(key) {
        return Some(line);
    }
    key.parse::<Region>().ok()?;
    table.get("001").map(Vec::as_slice)
}

/// The calendar types CLDR prefers in `region` (an uppercase region code or
/// `001`), most preferred first, as BCP 47 types. A region with no line of
/// its own gets the `001` line; `None` only if `region` does not parse as a
/// region.
pub fn calendar_preferences(region: &str) -> Option<&'static [String]> {
    region_line(calendar_table(), region)
}

/// The hour cycles (`h11`, `h12`, `h23`, `h24`) CLDR gives for `key`, a
/// region or `<language>-<region>`, preferred first. A region with no line
/// of its own gets the `001` line; `None` for a `<language>-<region>` key
/// with no line, or a key that is neither.
pub fn hour_cycles(key: &str) -> Option<&'static [String]> {
    region_line(hour_cycle_table(), key)
}

fn time_zone_table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| parse(include_str!("../data/time-zones-by-region.txt")))
}

/// The primary IANA ids of the time zones of `region` (an uppercase region
/// code), sorted, without repeats; empty for a region with no zones. CLDR's
/// ids are mapped to primary ids like `Intl.supportedValuesOf("timeZone")`.
pub fn time_zones_of_region(region: &str) -> Vec<String> {
    let Some(ids) = time_zone_table().get(region) else {
        return Vec::new();
    };
    let mut zones: Vec<String> = ids
        .iter()
        .map(|id| {
            time_zone_canonicalize(id)
                .unwrap_or_else(|| panic!("data file: unknown time zone {id}"))
        })
        .collect();
    zones.sort();
    zones.dedup();
    zones
}

/// The week data of `region` as ISO weekday numbers (Monday 1 to Sunday 7):
/// the first day of the week, and the weekend days ascending. Any
/// syntactically valid region, assigned or not and in any case, gets its
/// data, or `001`'s when ICU4X has no entry for it (`ZZ`, `GB`, `AQ`); None
/// only if `region` does not parse as a region.
pub fn week_info(region: &str) -> Option<(u8, Vec<u8>)> {
    let region: Region = region.parse().ok()?;
    let mut locale = Locale::UNKNOWN;
    locale.id.region = Some(region);
    let info = WeekInformation::try_new(WeekPreferences::from(&locale)).ok()?;
    let mut weekend: Vec<u8> = info.weekend().map(|d| d as u8).collect();
    weekend.sort_unstable();
    Some((info.first_weekday as u8, weekend))
}

/// The writing direction of `script` (a four-letter script code): `"ltr"`,
/// `"rtl"`, or None when ICU4X does not know it.
pub fn script_direction(script: &str) -> Option<&'static str> {
    static D: OnceLock<LocaleDirectionality> = OnceLock::new();
    let script: Script = script.parse().ok()?;
    let mut locale = Locale::UNKNOWN;
    locale.id.script = Some(script);
    match D
        .get_or_init(LocaleDirectionality::new_extended)
        .get(&locale.id)?
    {
        Direction::LeftToRight => Some("ltr"),
        Direction::RightToLeft => Some("rtl"),
        _ => None,
    }
}

/// The numbering system of `locale`'s decimal symbols data (found with
/// ICU4X's locale fallback); None if the language identifier of `locale`
/// does not parse.
pub fn default_numbering_system(locale: &str) -> Option<String> {
    use icu::decimal::provider::{Baked, DecimalSymbolsV1};
    use icu_provider::prelude::*;
    let data_locale = DataLocale::from(&crate::locale::language_id(locale)?);
    let response = DataProvider::<DecimalSymbolsV1>::load(
        &Baked,
        DataRequest {
            id: DataIdentifierBorrowed::for_locale(&data_locale),
            ..Default::default()
        },
    )
    .ok()?;
    Some(response.payload.get().numsys().to_owned())
}

/// The collation types (among those `Intl.supportedValuesOf("collation")`
/// lists) that have a tailoring for `locale` or a locale on its fallback
/// chain, sorted. Empty if the language identifier of `locale` does not
/// parse.
pub fn collations_of_locale(locale: &str) -> Vec<&'static str> {
    let mut types: Vec<&'static str> = COLLATIONS
        .iter()
        .copied()
        .filter(|co| tailoring_loads(co, locale))
        .collect();
    types.sort_unstable();
    types
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::canonicalize;
    use crate::supported::{values, Kind};

    fn strs(v: Option<&[String]>) -> Option<Vec<&str>> {
        v.map(|v| v.iter().map(String::as_str).collect())
    }

    #[test]
    fn calendar_preferences_follow_cldr() {
        let f = |r| strs(calendar_preferences(r));
        assert_eq!(f("TH"), Some(vec!["buddhist", "gregory"]));
        assert_eq!(
            f("SA"),
            Some(vec![
                "gregory",
                "islamic-umalqura",
                "islamic",
                "islamic-rgsa"
            ])
        );
        assert_eq!(f("001"), Some(vec!["gregory"]));
        assert_eq!(f("TW"), Some(vec!["gregory", "roc", "chinese"]));
        // Regions with no line of their own get the 001 line.
        for region in ["US", "AQ", "ZZ", "QO", "419", "GB"] {
            assert_eq!(f(region), Some(vec!["gregory"]), "{region}");
        }
        assert_eq!(f("english"), None);
        assert_eq!(f(""), None);
    }

    #[test]
    fn hour_cycles_follow_cldr() {
        let f = |k| strs(hour_cycles(k));
        assert_eq!(f("US"), Some(vec!["h12", "h23"]));
        assert_eq!(f("JP"), Some(vec!["h23", "h11", "h12"]));
        assert_eq!(f("hi-IN"), Some(vec!["h12", "h23"]));
        assert_eq!(f("DE"), Some(vec!["h23", "h12"]));
        assert_eq!(f("001"), Some(vec!["h23", "h12"]));
        assert_eq!(f("en-001"), Some(vec!["h12", "h23"]));
        // Regions with no line of their own get the 001 line, but a
        // language-region key with no line gets None, so the caller tries
        // the region next.
        for region in ["AQ", "ZZ", "QO", "QM"] {
            assert_eq!(f(region), Some(vec!["h23", "h12"]), "{region}");
        }
        assert_eq!(f("en-AQ"), None);
        assert_eq!(f("en-US"), None);
        assert_eq!(f("english"), None);
        assert_eq!(f(""), None);
    }

    /// Check the keys and value lists every table must have.
    fn check_table(name: &str, table: &Table, check_value: impl Fn(&str) -> bool) {
        for (key, list) in table {
            let region = match key.rsplit_once('-') {
                None => key.as_str(),
                Some((language, region)) => {
                    assert_eq!(
                        canonicalize(language).as_deref(),
                        Some(language),
                        "{name} key {key}: language"
                    );
                    region
                }
            };
            let und = format!("und-{region}");
            assert_eq!(
                canonicalize(&und).as_deref(),
                Some(und.as_str()),
                "{name} key {key}"
            );
            assert!(!list.is_empty(), "{name} key {key}: empty");
            let mut seen = std::collections::BTreeSet::new();
            for v in list {
                assert!(seen.insert(v), "{name} key {key}: repeated {v}");
                assert!(check_value(v), "{name} key {key}: bad value {v}");
            }
        }
    }

    #[test]
    fn data_files_are_well_formed() {
        let calendars = values(Kind::Calendar);
        check_table("calendar", calendar_table(), |v| {
            calendars.iter().any(|c| c == v) || v == "islamic" || v == "islamic-rgsa"
        });
        check_table("hour cycle", hour_cycle_table(), |v| {
            ["h11", "h12", "h23", "h24"].contains(&v)
        });
        // Every region without a line of its own gets the 001 line.
        assert!(calendar_table().contains_key("001"));
        assert!(hour_cycle_table().contains_key("001"));
    }

    #[test]
    fn time_zones_of_regions() {
        let f = |r| time_zones_of_region(r);
        assert_eq!(f("JP"), ["Asia/Tokyo"]);
        // The primary id, not CLDR's first alias Asia/Calcutta.
        assert_eq!(f("IN"), ["Asia/Kolkata"]);
        assert_eq!(f("PS"), ["Asia/Gaza", "Asia/Hebron"]);
        let us = f("US");
        for z in [
            "America/New_York",
            "America/Los_Angeles",
            "America/Indiana/Vevay",
            "Pacific/Honolulu",
        ] {
            assert!(us.iter().any(|x| x == z), "{z}");
        }
        assert!(!us.iter().any(|z| z.starts_with("Etc/")));
        assert!(f("419").is_empty());
        assert!(f("ZZ").is_empty());
    }

    #[test]
    fn time_zones_are_supported_primary_ids() {
        let supported = values(Kind::TimeZone);
        let table = time_zone_table();
        assert!(table.len() > 200);
        for region in table.keys() {
            let zones = time_zones_of_region(region);
            assert!(!zones.is_empty(), "{region}");
            assert!(zones.windows(2).all(|w| w[0] < w[1]), "{region}");
            for z in zones {
                assert!(supported.contains(&z), "{region}: {z}");
            }
        }
    }

    #[test]
    fn week_info_follows_cldr() {
        let f = |r| week_info(r);
        assert_eq!(f("US"), Some((7, vec![6, 7])));
        assert_eq!(f("GB"), Some((1, vec![6, 7])));
        assert_eq!(f("IL"), Some((7, vec![5, 6])));
        assert_eq!(f("AF"), Some((6, vec![4, 5])));
        assert_eq!(f("IN"), Some((7, vec![7])));
        assert_eq!(f("001"), Some((1, vec![6, 7])));
        assert_eq!(f("AQ"), Some((1, vec![6, 7])));
        assert_eq!(f("english"), None);
        assert_eq!(f(""), None);
    }

    /// CLDR's 001 data applies to regions it does not list, and ICU4X drops
    /// entries equal to 001, so these regions only get data by fallback.
    #[test]
    fn week_info_of_regions_equal_to_001() {
        for region in ["GB", "AQ", "001", "ZZ"] {
            assert_eq!(week_info(region), Some((1, vec![6, 7])), "{region}");
        }
    }

    #[test]
    fn script_directions() {
        for s in ["Arab", "Hebr", "Thaa"] {
            assert_eq!(script_direction(s), Some("rtl"), "{s}");
        }
        for s in ["Latn", "Cyrl", "Hani"] {
            assert_eq!(script_direction(s), Some("ltr"), "{s}");
        }
        for s in ["Zyyy", "Qaaa", "en", ""] {
            assert_eq!(script_direction(s), None, "{s}");
        }
    }

    #[test]
    fn default_numbering_systems() {
        let f = |l| default_numbering_system(l);
        assert_eq!(f("en").as_deref(), Some("latn"));
        assert_eq!(f("ar-EG").as_deref(), Some("arab"));
        assert_eq!(f("fa").as_deref(), Some("arabext"));
        assert_eq!(f("hi-IN").as_deref(), Some("latn"));
        assert_eq!(f("und").as_deref(), Some("latn"));
        assert_eq!(f("not a tag"), None);
        // Only the language identifier is used; ICU4X does not parse this
        // whole tag.
        assert_eq!(f("ar-EG-t-posix").as_deref(), Some("arab"));
    }

    #[test]
    fn collations_of_locales() {
        let f = |l| collations_of_locale(l);
        assert_eq!(f("en"), ["emoji", "eor"]);
        assert_eq!(f("de"), ["emoji", "eor", "phonebk"]);
        assert_eq!(f("es"), ["emoji", "eor", "trad"]);
        let zh = f("zh");
        for c in ["pinyin", "stroke", "zhuyin"] {
            assert!(zh.contains(&c), "{c}");
        }
        for l in ["en", "de", "es", "zh", "ar", "und", "ja", "ko"] {
            let list = f(l);
            assert!(!list.contains(&"standard") && !list.contains(&"search"));
            assert!(list.windows(2).all(|w| w[0] < w[1]), "{l}");
        }
        assert!(f("not a tag").is_empty());
        // Only the language identifier is used; ICU4X does not parse this
        // whole tag.
        assert_eq!(f("de-t-posix"), ["emoji", "eor", "phonebk"]);
    }
}
