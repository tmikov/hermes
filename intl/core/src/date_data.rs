// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The CLDR date data ICU4X 2.3 lacks (decision 5 of the Phase 4 plan), from
//! two generated files in core/data, each parsed once on first use:
//!
//! - `date-patterns.txt`: per locale and calendar, the date, time and
//!   date-time styles, `availableFormats`, `intervalFormats`, the interval
//!   fallback, the `Timezone` append item and the numeric leap month
//!   pattern, each locale holding only what differs from its parent;
//! - `day-periods.txt`: CLDR's flexible day period rules per language, and
//!   the names of the periods, each locale holding only what differs from
//!   its parent.
//!
//! The entry for a locale is the first one ICU4X's LocaleFallbacker reaches
//! from its language identifier, as in cldr.rs. The files are ours, so a
//! malformed line is a panic.

use crate::cldr::{body_lines, fallback_key, locale_parent, string_table};
use crate::locale::language_id;
use crate::supported::CALENDARS;
use std::collections::HashMap;
use std::sync::OnceLock;

/// What a locale's line of date-patterns.txt says about one calendar.
enum CalData {
    /// `<calendar> = <other>`: the locale's whole data for this calendar is
    /// its data for `other`, which the locale may only inherit.
    Alias(&'static str),
    /// `<calendar> <key>=<index>...`: the entries that differ from the
    /// parent's, in the file's order.
    Own(Vec<(&'static str, &'static str)>),
}

/// One locale of date-patterns.txt.
struct DateLocale {
    /// The locale whose data this one overrides; None for und.
    parent: Option<&'static str>,
    /// The data of each calendar the locale has a line for.
    cals: HashMap<&'static str, CalData>,
}

/// date-patterns.txt by locale.
type DateTable = HashMap<&'static str, DateLocale>;

/// The calendar names a line of date-patterns.txt may start with: the 16
/// calendars of `CALENDARS` and CLDR's `generic`.
fn is_file_calendar(name: &str) -> bool {
    name == "generic" || CALENDARS.contains(&name)
}

/// Parse date-patterns.txt.
fn parse_date_patterns(text: &'static str) -> DateTable {
    let mut lines = body_lines(text);
    // Not trimmed: a pattern may end in U+202F or U+2009.
    let strings = string_table(&mut lines, "date-patterns.txt");
    let mut table = DateTable::new();
    let mut current: Option<&'static str> = None;
    for line in lines {
        let mut words = line.split(' ');
        let first = words.next().unwrap_or_default();
        if first == "locale" {
            let (Some(tag), Some(parent), None) = (words.next(), words.next(), words.next()) else {
                panic!("date-patterns.txt: bad locale line {line:?}");
            };
            let parent = locale_parent("date-patterns.txt", tag, parent, |p| table.contains_key(p));
            assert_eq!(
                parent.is_none(),
                tag == "und",
                "date-patterns.txt: {line:?}"
            );
            let fresh = DateLocale {
                parent,
                cals: HashMap::new(),
            };
            assert!(
                table.insert(tag, fresh).is_none(),
                "date-patterns.txt: repeated locale {tag}"
            );
            current = Some(tag);
            continue;
        }
        let tag = current.expect("date-patterns.txt: entry before any locale");
        assert!(
            is_file_calendar(first),
            "date-patterns.txt: bad calendar in {line:?}"
        );
        let locale = table.get_mut(tag).unwrap();
        let rest: Vec<&'static str> = words.collect();
        let data = if let ["=", other] = rest[..] {
            assert!(
                is_file_calendar(other) && other != first,
                "date-patterns.txt: bad alias in {line:?}"
            );
            CalData::Alias(other)
        } else {
            let mut entries: Vec<(&'static str, &'static str)> = Vec::with_capacity(rest.len());
            for kv in rest {
                let (key, index) = kv
                    .rsplit_once('=')
                    .unwrap_or_else(|| panic!("date-patterns.txt: bad entry {kv:?}"));
                let index: usize = index.parse().expect("date-patterns.txt: bad index");
                let value = *strings
                    .get(index)
                    .expect("date-patterns.txt: index out of range");
                assert!(
                    !key.is_empty() && entries.iter().all(|(k, _)| *k != key),
                    "date-patterns.txt: bad or repeated key in {line:?}"
                );
                entries.push((key, value));
            }
            assert!(
                !entries.is_empty(),
                "date-patterns.txt: empty line {line:?}"
            );
            CalData::Own(entries)
        };
        assert!(
            locale.cals.insert(first, data).is_none(),
            "date-patterns.txt: repeated {tag} {first}"
        );
    }
    assert!(table.contains_key("und"), "date-patterns.txt: no und");
    table
}

/// The parsed date-patterns.txt.
fn locale_table() -> &'static DateTable {
    static T: OnceLock<DateTable> = OnceLock::new();
    T.get_or_init(|| parse_date_patterns(include_str!("../data/date-patterns.txt")))
}

/// The data of `tag` for `cal`: the parent's, in its order, with the
/// locale's own entries replacing theirs in place and its new keys appended
/// in the order of its line. An alias line is the other calendar's data of
/// the same locale.
fn read_calendar(
    table: &'static DateTable,
    tag: &'static str,
    cal: &str,
) -> Vec<(&'static str, &'static str)> {
    let locale = &table[tag];
    let own = match locale.cals.get(cal) {
        Some(CalData::Alias(other)) => return read_calendar(table, tag, other),
        Some(CalData::Own(entries)) => entries.as_slice(),
        None => &[],
    };
    let mut merged = match locale.parent {
        Some(parent) => read_calendar(table, parent, cal),
        None => Vec::new(),
    };
    let index: HashMap<&str, usize> = merged
        .iter()
        .enumerate()
        .map(|(i, (k, _))| (*k, i))
        .collect();
    let mut appended = Vec::new();
    for &(key, value) in own {
        match index.get(key) {
            Some(&i) => merged[i].1 = value,
            None => appended.push((key, value)),
        }
    }
    merged.extend(appended);
    merged
}

/// The keys of the 16 style entries of date-patterns.txt, in the order of
/// the list `date_time_patterns` returns them: dateFormats, timeFormats,
/// standard dateTimeFormats and `atTime` dateTimeFormats, full to short.
const STYLE_KEYS: [&str; 16] = [
    "d0", "d1", "d2", "d3", "t0", "t1", "t2", "t3", "c0", "c1", "c2", "c3", "a0", "a1", "a2", "a3",
];

/// The CLDR patterns for DateTimeFormat, for the language identifier of
/// `locale` (a canonical tag) and one of the 16 calendars of Intl's list:
///
/// - 0-3 dateFormats full, long, medium, short; 4-7 timeFormats; 8-11 the
///   standard dateTimeFormats; 12-15 the `atTime` ones (the standard ones
///   where the locale has none);
/// - 16 intervalFormatFallback; 17 the numeric leap month pattern (`""` if
///   the calendar has none); 18 the `Timezone` append item;
/// - 19 the number N of availableFormats entries, then N pairs (skeleton,
///   pattern); then the number M of interval entries, then M triples
///   (skeleton, greatest-difference letter, pattern). Both lists are in the
///   order of the parent's entries, a child's overrides in place and its
///   new keys after them.
///
/// None for a calendar outside the 16, or a locale that does not parse.
pub fn date_time_patterns(locale: &str, calendar: &str) -> Option<Vec<String>> {
    if !CALENDARS.contains(&calendar) {
        return None;
    }
    let table = locale_table();
    let tag = fallback_key(&language_id(locale)?, |t| table.contains_key(t));
    let tag = table.get_key_value(tag.as_str())?.0;
    let entries = read_calendar(table, tag, calendar);
    let get = |key: &str| -> Option<&'static str> {
        entries.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    };
    let need = |key: &str| -> &'static str {
        get(key).unwrap_or_else(|| panic!("date-patterns.txt: {tag} {calendar} has no {key}"))
    };
    let mut out: Vec<String> = Vec::new();
    for (i, key) in STYLE_KEYS.iter().enumerate() {
        let value = if i >= 12 {
            // atTime: the standard dateTimeFormat where there is none.
            get(key).unwrap_or_else(|| need(STYLE_KEYS[i - 4]))
        } else {
            need(key)
        };
        out.push(value.to_owned());
    }
    out.push(need("fb").to_owned());
    out.push(get("lp").unwrap_or("").to_owned());
    out.push(need("tz").to_owned());
    let avail: Vec<_> = entries
        .iter()
        .filter_map(|(k, v)| k.strip_prefix("f.").map(|s| (s, *v)))
        .collect();
    out.push(avail.len().to_string());
    for (skeleton, pattern) in avail {
        out.push(skeleton.to_owned());
        out.push(pattern.to_owned());
    }
    let intervals: Vec<_> = entries
        .iter()
        .filter_map(|(k, v)| k.strip_prefix("i.").map(|s| (s, *v)))
        .collect();
    out.push(intervals.len().to_string());
    for (key, pattern) in intervals {
        let (skeleton, letter) = key
            .rsplit_once('.')
            .unwrap_or_else(|| panic!("date-patterns.txt: bad interval key {key:?}"));
        out.push(skeleton.to_owned());
        out.push(letter.to_owned());
        out.push(pattern.to_owned());
    }
    Some(out)
}

/// One locale of day-periods.txt.
struct DayLocale {
    /// The locale whose names this one overrides; None for und.
    parent: Option<&'static str>,
    /// The key of the rule set the locale's periods follow.
    rules_key: &'static str,
    /// Period to abbreviated, wide and narrow names, where they differ
    /// from the parent chain's.
    names: HashMap<&'static str, [&'static str; 3]>,
}

/// A period of a rule set: its id and its range in minutes of the day. A
/// range with `from == before` is an `at` rule (noon, midnight); one with
/// `from > before` wraps midnight.
type Rule = (&'static str, u32, u32);

/// The parsed day-periods.txt.
struct DayPeriods {
    /// The rule sets by key, each in the file's order.
    rules: HashMap<&'static str, Vec<Rule>>,
    /// The locales.
    locales: HashMap<&'static str, DayLocale>,
}

/// Parse day-periods.txt.
fn parse_day_periods(text: &'static str) -> DayPeriods {
    let mut rules: HashMap<&'static str, Vec<Rule>> = HashMap::new();
    let mut locales: HashMap<&'static str, DayLocale> = HashMap::new();
    let mut current_rules: Option<&'static str> = None;
    let mut current: Option<&'static str> = None;
    for line in body_lines(text) {
        let bad = || -> ! { panic!("day-periods.txt: bad line {line:?}") };
        let (first, rest) = line.split_once(' ').unwrap_or_else(|| bad());
        if first == "rules" && locales.is_empty() {
            assert!(
                rules.insert(rest, Vec::new()).is_none(),
                "day-periods.txt: repeated rules {rest}"
            );
            current_rules = Some(rest);
        } else if first == "locale" {
            let f: Vec<&'static str> = rest.split(' ').collect();
            let [tag, parent, rules_key] = f[..] else {
                bad()
            };
            let parent = (parent != "-").then_some(parent);
            assert!(
                parent.is_none_or(|p| locales.contains_key(p)) && rules.contains_key(rules_key),
                "day-periods.txt: {line:?}"
            );
            assert_eq!(parent.is_none(), tag == "und", "day-periods.txt: {line:?}");
            let fresh = DayLocale {
                parent,
                rules_key,
                names: HashMap::new(),
            };
            assert!(
                locales.insert(tag, fresh).is_none(),
                "day-periods.txt: repeated locale {tag}"
            );
            current = Some(tag);
        } else if let Some(tag) = current {
            // Not trimmed or split on spaces: a name may have both.
            let f: Vec<&'static str> = rest.split('\t').collect();
            let [abbreviated, wide, narrow] = f[..] else {
                bad()
            };
            assert!(
                locales
                    .get_mut(tag)
                    .unwrap()
                    .names
                    .insert(first, [abbreviated, wide, narrow])
                    .is_none(),
                "day-periods.txt: repeated {line:?}"
            );
        } else {
            let key = current_rules.unwrap_or_else(|| bad());
            let f: Vec<&str> = rest.split(' ').collect();
            let [from, before] = f[..] else { bad() };
            let (Ok(from), Ok(before)) = (from.parse::<u32>(), before.parse::<u32>()) else {
                bad()
            };
            assert!(from <= 1440 && before <= 1440, "day-periods.txt: {line:?}");
            rules.get_mut(key).unwrap().push((first, from, before));
        }
    }
    assert!(
        locales.contains_key("und") && rules.contains_key("und"),
        "day-periods.txt: no und"
    );
    DayPeriods { rules, locales }
}

/// The parsed day-periods.txt.
fn day_period_table() -> &'static DayPeriods {
    static T: OnceLock<DayPeriods> = OnceLock::new();
    T.get_or_init(|| parse_day_periods(include_str!("../data/day-periods.txt")))
}

/// The flexible day periods of CLDR for DateTimeFormat, for the language
/// identifier of `locale` (a canonical tag):
///
/// - 0 the number N of periods with a range (`morning1`, `night2`...; not
///   the `at` rules noon and midnight, and midnight is never shown);
/// - then N groups of six: the id, the start minute of the day, the end
///   minute (as in the file: 1440 is the end of the day, and a range with
///   a start after its end wraps midnight), and the abbreviated, wide and
///   narrow names;
/// - then noon's abbreviated, wide and narrow names (`""` if the locale's
///   rules have no noon).
///
/// A period CLDR has no name for in the locale reads as `""`. None for a
/// locale that does not parse.
pub fn day_periods(locale: &str) -> Option<Vec<String>> {
    let table = day_period_table();
    let tag = fallback_key(&language_id(locale)?, |t| table.locales.contains_key(t));
    let names_of = |period: &str| -> [&'static str; 3] {
        let mut t = Some(tag.as_str());
        while let Some(current) = t {
            let locale = &table.locales[current];
            if let Some(names) = locale.names.get(period) {
                return *names;
            }
            t = locale.parent;
        }
        [""; 3]
    };
    let rules = &table.rules[table.locales[tag.as_str()].rules_key];
    let ranged: Vec<&Rule> = rules
        .iter()
        .filter(|(_, from, before)| from != before)
        .collect();
    let mut out = vec![ranged.len().to_string()];
    for (id, from, before) in ranged {
        out.push((*id).to_owned());
        out.push(from.to_string());
        out.push(before.to_string());
        out.extend(names_of(id).map(str::to_owned));
    }
    let noon = if rules.iter().any(|(id, _, _)| *id == "noon") {
        names_of("noon")
    } else {
        [""; 3]
    };
    out.extend(noon.map(str::to_owned));
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::test_locales;
    use crate::supported::CALENDARS;

    /// The entries of `v` from `start`: the number N at `start`, then `width`
    /// strings for each of N.
    fn counted(v: &[String], start: usize, width: usize) -> (Vec<&[String]>, usize) {
        let n: usize = v[start].parse().unwrap();
        let end = start + 1 + n * width;
        (v[start + 1..end].chunks(width).collect(), end)
    }

    fn avail(v: &[String]) -> Vec<&[String]> {
        counted(v, 19, 2).0
    }

    fn intervals(v: &[String]) -> Vec<&[String]> {
        let (_, end) = counted(v, 19, 2);
        counted(v, end, 3).0
    }

    fn pat(locale: &str, cal: &str) -> Vec<String> {
        date_time_patterns(locale, cal).unwrap_or_else(|| panic!("{locale} {cal}"))
    }

    #[test]
    fn en_gregory() {
        let v = pat("en", "gregory");
        assert_eq!(
            v[0..4],
            ["EEEE, MMMM d, y", "MMMM d, y", "MMM d, y", "M/d/yy"]
        );
        assert_eq!(v[7], "h:mm\u{202f}a");
        assert_eq!(v[12], "{1} 'at' {0}");
        assert!(avail(&v)
            .iter()
            .any(|e| e[0] == "yMMMd" && e[1] == "MMM d, y"));
        assert!(avail(&v).iter().any(|e| e[0] == "Hm" && e[1] == "HH:mm"));
        assert!(intervals(&v)
            .iter()
            .any(|e| e[0] == "yMMMd" && e[1] == "d" && e[2] == "MMM d\u{2009}–\u{2009}d, y"));
        // Every entry of the fixed part is there, and the list ends after M
        // triples.
        let (_, end) = counted(&v, 19, 2);
        assert_eq!(counted(&v, end, 3).1, v.len());
    }

    #[test]
    fn calendars() {
        // CLDR's ja japanese has no `yMMMd` of its own: its year-with-month-
        // and-day entries are `GyMMMd` and `yyyyMMMd`, both with an era.
        let ja = pat("ja", "japanese");
        for skeleton in ["GyMMMd", "yyyyMMMd"] {
            assert!(
                avail(&ja)
                    .iter()
                    .any(|e| e[0] == skeleton && e[1].contains('G')),
                "{skeleton}"
            );
        }
        assert_eq!(pat("en", "chinese")[17], "{0}bis");
        assert_eq!(pat("zh", "chinese")[17], "闰{0}");
        assert_eq!(pat("en", "gregory")[17], "");
        assert_eq!(pat("und", "iso8601")[3], "y-MM-dd");
        // An `=` line or inheritance: the same as the chain read by hand.
        let hand = merged_by_hand("en-GB", "buddhist");
        let got = pat("en-GB", "buddhist");
        assert_eq!(got[16], hand.iter().find(|(k, _)| *k == "fb").unwrap().1);
        assert_eq!(got, expected_list(&hand));
    }

    /// The list `date_time_patterns` returns for the merged entries `hand`,
    /// built from the keys one by one.
    fn expected_list(hand: &[(&str, &str)]) -> Vec<String> {
        let get = |k: &str| hand.iter().find(|(hk, _)| *hk == k).map(|(_, v)| *v);
        let mut want: Vec<String> = vec![];
        for k in [
            "d0", "d1", "d2", "d3", "t0", "t1", "t2", "t3", "c0", "c1", "c2", "c3",
        ] {
            want.push(get(k).unwrap().to_owned());
        }
        for (i, k) in ["a0", "a1", "a2", "a3"].iter().enumerate() {
            let standard = ["c0", "c1", "c2", "c3"][i];
            want.push(get(k).or(get(standard)).unwrap().to_owned());
        }
        want.push(get("fb").unwrap().to_owned());
        want.push(get("lp").unwrap_or("").to_owned());
        want.push(get("tz").unwrap().to_owned());
        let f: Vec<_> = hand.iter().filter(|(k, _)| k.starts_with("f.")).collect();
        want.push(f.len().to_string());
        for (k, v) in f {
            want.extend([k[2..].to_owned(), (*v).to_owned()]);
        }
        let i: Vec<_> = hand.iter().filter(|(k, _)| k.starts_with("i.")).collect();
        want.push(i.len().to_string());
        for (k, v) in i {
            let (skeleton, letter) = k[2..].rsplit_once('.').unwrap();
            want.extend([skeleton.to_owned(), letter.to_owned(), (*v).to_owned()]);
        }
        want
    }

    /// The data of `locale` and `cal` read as the generator's reader does,
    /// from und down the chain of `locale`: an entry of a locale replaces
    /// the one with its key in place or is appended, and an `=` line makes
    /// the locale's whole merged data for the other calendar the data from
    /// there on. Independent of the production recursion.
    fn merged_by_hand(locale: &str, cal: &str) -> Vec<(&'static str, &'static str)> {
        let t = locale_table();
        let mut chain = vec![];
        let mut tag = t.keys().copied().find(|k| *k == locale).unwrap();
        loop {
            chain.push(tag);
            match t[tag].parent {
                Some(p) => tag = p,
                None => break,
            }
        }
        chain.reverse();
        fn up_to(
            t: &'static DateTable,
            chain: &[&'static str],
            cal: &str,
        ) -> Vec<(&'static str, &'static str)> {
            let mut out: Vec<(&'static str, &'static str)> = vec![];
            for (n, tag) in chain.iter().enumerate() {
                match t[tag].cals.get(cal) {
                    Some(CalData::Alias(other)) => out = up_to(t, &chain[..=n], other),
                    Some(CalData::Own(entries)) => {
                        for (k, v) in entries {
                            match out.iter_mut().find(|(ok, _)| ok == k) {
                                Some(slot) => slot.1 = v,
                                None => out.push((*k, *v)),
                            }
                        }
                    }
                    None => {}
                }
            }
            out
        }
        up_to(t, &chain, cal)
    }

    #[test]
    fn fallback() {
        // en-AU's own differences are over en's (via en-001).
        let au = pat("en-AU", "gregory");
        let en = pat("en", "gregory");
        assert_eq!(au[3], "d/M/yy");
        assert_ne!(au[3], en[3]);
        assert_eq!(au[0], "EEEE, d MMMM y");
        assert_eq!(au[4], en[4]);
        // A tag with no entry of its own reaches its language's.
        assert_eq!(pat("en-Latn-US-u-ca-gregory", "gregory"), en);
        assert_eq!(pat("en-US", "gregory"), en);
        assert_eq!(day_periods("en-US"), day_periods("en"));
    }

    #[test]
    fn merged_order() {
        // The parent's entries in their order, overrides in place, new keys
        // appended: en-AU's list starts with und's order.
        let und = pat("und", "gregory");
        let au = pat("en-AU", "gregory");
        let keys =
            |v: &[String]| -> Vec<String> { avail(v).iter().map(|e| e[0].clone()).collect() };
        let (ku, ka) = (keys(&und), keys(&au));
        assert_eq!(ka[..ku.len()], ku[..]);
        // Overridden in place.
        let i = ku.iter().position(|k| k == "yMd").unwrap();
        assert_eq!(avail(&au)[i][0], "yMd");
        assert_ne!(avail(&au)[i][1], avail(&und)[i][1]);
    }

    fn period(v: &[String], i: usize) -> &[String] {
        &v[1 + 6 * i..1 + 6 * i + 6]
    }

    #[test]
    fn day_periods_en() {
        let v = day_periods("en").unwrap();
        assert_eq!(v[0], "4");
        let ids: Vec<(&str, &str, &str)> = (0..4)
            .map(|i| {
                let p = period(&v, i);
                (p[0].as_str(), p[1].as_str(), p[2].as_str())
            })
            .collect();
        assert_eq!(
            ids,
            [
                ("morning1", "0", "720"),
                ("afternoon1", "720", "1080"),
                ("evening1", "1080", "1260"),
                ("night1", "1260", "1440")
            ]
        );
        assert_eq!(period(&v, 0)[3..], ["in the morning"; 3]);
        assert_eq!(v.len(), 1 + 24 + 3);
        assert_eq!(v[25..], ["noon", "noon", "n"]);
        // es-CO has its own rules; minute 0 is in morning2, in es morning1.
        let first = |tag: &str| {
            let v = day_periods(tag).unwrap();
            let n: usize = v[0].parse().unwrap();
            (0..n)
                .map(|i| period(&v, i))
                .find(|p| p[1].parse::<u32>().unwrap() == 0)
                .map(|p| p[0].clone())
                .unwrap()
        };
        assert_eq!(first("es-CO"), "morning2");
        assert_eq!(first("es"), "morning1");
        // ht's rules have no noon, although its chain has a noon name (fr's).
        let ht = day_periods("ht").unwrap();
        assert_eq!(ht[ht.len() - 3..], ["", "", ""]);
        // hi-Latn has a noon, hi has none.
        let hi = day_periods("hi").unwrap();
        assert_eq!(hi[hi.len() - 3..], ["", "", ""]);
        let latn = day_periods("hi-Latn").unwrap();
        assert!(latn[latn.len() - 3..].iter().all(|s| !s.is_empty()));
    }

    /// The pattern letters outside quotes, a run of the same letter once.
    fn letters(p: &str) -> Vec<char> {
        let mut out = vec![];
        let mut quoted = false;
        let mut prev = ' ';
        for c in p.chars() {
            if c == '\'' {
                quoted = !quoted;
            } else if !quoted && c.is_ascii_alphabetic() && c != prev {
                out.push(c);
            }
            prev = c;
        }
        out
    }

    const ALLOWED: &str = "GyUrMLdEcabBhHKkmsSzOv";

    fn check_letters(p: &str, what: &str) {
        for c in letters(p) {
            assert!(ALLOWED.contains(c), "{what}: {c} in {p:?}");
        }
    }

    /// The field a pattern letter belongs to, for "a field repeats".
    fn field(c: char) -> char {
        match c {
            'M' | 'L' => 'M',
            'E' | 'c' => 'E',
            'h' | 'H' | 'K' | 'k' => 'h',
            'a' | 'b' | 'B' => 'a',
            'z' | 'O' | 'v' => 'z',
            c => c,
        }
    }

    #[test]
    fn every_locale_every_calendar() {
        let mut n = 0;
        for locale in test_locales().chain(["und"]) {
            for cal in CALENDARS {
                let what = format!("{locale} {cal}");
                let v = date_time_patterns(locale, cal).unwrap_or_else(|| panic!("{what}"));
                for (i, p) in v[..16].iter().enumerate() {
                    check_letters(p, &format!("{what} [{i}]"));
                    if (8..16).contains(&i) {
                        assert!(p.contains("{0}") && p.contains("{1}"), "{what} [{i}]: {p}");
                    }
                }
                assert!(v[16].contains("{0}") && v[16].contains("{1}"), "{what}");
                assert!(
                    v[18].contains("{0}") && v[18].contains("{1}"),
                    "{what}: Timezone append item {}",
                    v[18]
                );
                for e in avail(&v) {
                    assert!(!e[0].is_empty(), "{what}");
                    check_letters(&e[1], &format!("{what} {}", e[0]));
                }
                for e in intervals(&v) {
                    let w = format!("{what} {} {}", e[0], e[1]);
                    check_letters(&e[2], &w);
                    let mut seen = vec![];
                    let mut repeats = false;
                    let ls = letters(&e[2]);
                    for c in ls.iter() {
                        let f = field(*c);
                        repeats |= seen.contains(&f);
                        seen.push(f);
                    }
                    assert!(repeats, "{w}: {}", e[2]);
                }
                n += 1;
            }
            let v = day_periods(locale).unwrap_or_else(|| panic!("{locale}"));
            let count: usize = v[0].parse().unwrap();
            let mut covered = [0u8; 1440];
            for i in 0..count {
                let p = period(&v, i);
                let (from, before): (usize, usize) = (p[1].parse().unwrap(), p[2].parse().unwrap());
                assert_ne!(from, before, "{locale} {}", p[0]);
                let mut m = from;
                while m != before % 1440 {
                    covered[m] += 1;
                    m = (m + 1) % 1440;
                }
            }
            assert!(covered.iter().all(|c| *c == 1), "{locale}");
        }
        assert_eq!(n, (test_locales().count() + 1) * 16);
    }

    #[test]
    fn rejects() {
        assert_eq!(date_time_patterns("en", "julian"), None);
        assert_eq!(date_time_patterns("en", "generic"), None);
        assert_eq!(date_time_patterns("en_US", "gregory"), None);
        assert_eq!(day_periods("en_US"), None);
    }
}
