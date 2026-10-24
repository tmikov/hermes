// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! Locale-information lookups: the CLDR data behind the Intl.Locale Info
//! methods (ECMA-402, "Intl.Locale.prototype.getCalendars" and friends).
//! Each table is a generated file in core/data, parsed once on first use.

use std::collections::BTreeMap;
use std::sync::OnceLock;

type Table = BTreeMap<String, Vec<String>>;

/// Parse a generated `<key> <value>...` file; `#` lines are comments.
fn parse(text: &str) -> Table {
    let mut table = Table::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
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

/// The calendar types CLDR prefers in `region` (an uppercase region code or
/// `001`), most preferred first, as BCP 47 types; `None` when CLDR has no
/// line for it (`001` applies then; the caller falls back).
pub fn calendar_preferences(region: &str) -> Option<&'static [String]> {
    calendar_table().get(region).map(Vec::as_slice)
}

/// The hour cycles (`h11`, `h12`, `h23`, `h24`) CLDR gives for `key`, a
/// region or `<language>-<region>`, preferred first; `None` when CLDR has no
/// line for it.
pub fn hour_cycles(key: &str) -> Option<&'static [String]> {
    hour_cycle_table().get(key).map(Vec::as_slice)
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
        assert_eq!(f("US"), None);
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
        assert_eq!(f("AQ"), None);
    }

    #[test]
    fn data_files_are_well_formed() {
        let calendars = values(Kind::Calendar);
        for (name, table) in [
            ("calendar", calendar_table()),
            ("hour cycle", hour_cycle_table()),
        ] {
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
                    if name == "calendar" {
                        assert!(
                            calendars.contains(v) || v == "islamic" || v == "islamic-rgsa",
                            "{name} key {key}: unsupported calendar {v}"
                        );
                    }
                }
            }
        }
    }
}
