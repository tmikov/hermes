// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The native half of Intl.DateTimeFormat (decisions 3, 4 and 16 of the
//! Phase 4 plan): for an instant, the draft's ToLocalTime record in the
//! formatter's calendar and time zone, and the localized names its patterns
//! use. Formatting itself is JS.
//!
//! The calendar fields come from ICU4X's calendars, the UTC offset from the
//! tzdb (tz.rs), and the names from ICU4X's FixedCalendarDateTimeNames, one
//! per CLDR symbol, each formatting a one-symbol pattern. The names objects
//! use CompositeFieldSet, not CompositeDateTimeFieldSet: the latter has no
//! time zone name data, so it cannot load the names of `z`, `O` or `v`.

use crate::locale::language_id;
use crate::supported::CALENDARS;
use crate::tz::{self, Zone};
use icu::calendar::cal::hijri::{TabularAlgorithm, UmmAlQura};
use icu::calendar::cal::{
    Buddhist, ChineseTraditional, Coptic, Ethiopian, EthiopianEraStyle, Hebrew, Hijri, Indian,
    Japanese, KoreanTraditional, Persian, Roc,
};
use icu::calendar::preferences::CalendarAlgorithm;
use icu::calendar::types::{RataDie, YearInfo};
use icu::calendar::{AnyCalendarKind, Calendar as IcuCalendar, Date, Gregorian, Ref};
use icu::datetime::fieldsets::enums::CompositeFieldSet;
use icu::datetime::pattern::{DateTimePattern, FixedCalendarDateTimeNames};
use icu::datetime::preferences::NumberingSystem;
use icu::datetime::provider::Baked;
use icu::datetime::scaffold::CldrCalendar;
use icu::datetime::DateTimeFormatterPreferences;
use icu::locale::extensions::unicode::Value;
use icu::time::zone::models::AtTime;
use icu::time::zone::{IanaParser, UtcOffset, ZoneNameTimestamp};
use icu::time::{Time, TimeZone, TimeZoneInfo, ZonedDateTime};
use icu_provider::DataProvider;
use writeable::TryWriteable;

/// The CLDR symbols whose text DateTimeFormat gives (decision 4): era,
/// month (format and stand-alone), weekday (format and stand-alone), AM/PM,
/// cyclic year and time zone names, in the widths FormatDateTimePattern
/// uses.
pub const SYMBOLS: &[&str] = &[
    "G", "GGGG", "GGGGG", "MMM", "MMMM", "MMMMM", "LLL", "LLLL", "LLLLL", "EEE", "EEEE", "EEEEE",
    "ccc", "cccc", "ccccc", "a", "aaaa", "aaaaa", "U", "z", "zzzz", "O", "OOOO", "v", "vvvv",
];

/// The largest magnitude of an ECMAScript time value (ECMA-262 21.4.1.1).
const MAX_TIME: f64 = 8.64e15;

/// The Rata Die of 1970-01-01.
const RD_UNIX_EPOCH: i64 = 719163;

/// The milliseconds of a day.
const MS_PER_DAY: i64 = 86_400_000;

/// Whether `symbol` is one of SYMBOLS that `calendar` (a CLDR calendar id)
/// has: `U` only for the cyclic years of `chinese` and `dangi`, the era
/// symbols for every calendar but those two and `iso8601`.
fn symbol_allowed(calendar: &str, symbol: &str) -> bool {
    let cyclic = matches!(calendar, "chinese" | "dangi");
    SYMBOLS.contains(&symbol)
        && match symbol.as_bytes()[0] {
            b'U' => cyclic,
            b'G' => !cyclic && calendar != "iso8601",
            _ => true,
        }
}

/// The UTC offset in seconds of an offset time zone in the form JS passes
/// it, `±HH:MM` with an ASCII sign, HH at most 23 and MM at most 59; None
/// for anything else.
fn parse_offset(s: &str) -> Option<i32> {
    let b = s.as_bytes();
    let sign = match b.first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    if b.len() != 6 || b[3] != b':' {
        return None;
    }
    let two = |i: usize| -> Option<i32> {
        let (h, l) = (b[i], b[i + 1]);
        (h.is_ascii_digit() && l.is_ascii_digit()).then(|| ((h - b'0') * 10 + (l - b'0')) as i32)
    };
    let (hours, minutes) = (two(1)?, two(4)?);
    (hours <= 23 && minutes <= 59).then_some(sign * (hours * 3600 + minutes * 60))
}

/// Whether `epoch_ms` is an ECMAScript time value DateTimeFormat::local_time
/// takes: an integral number within ±8.64 × 10^15 (TimeClip's range).
pub fn is_time_value(epoch_ms: f64) -> bool {
    epoch_ms.abs() <= MAX_TIME && epoch_ms.fract() == 0.0
}

/// Where a formatter's UTC offsets come from.
enum Rules {
    /// A named time zone: the tzdb's rules.
    Tzdb(&'static Zone),
    /// An offset time zone: this offset in seconds.
    Fixed(i32),
}

/// Whether `symbol` (one of SYMBOLS) is a time zone name.
fn is_zone_symbol(symbol: &str) -> bool {
    matches!(symbol.as_bytes().first(), Some(b'z' | b'O' | b'v'))
}

/// How to give the zone names of a UTC offset that ICU4X's UtcOffset cannot
/// hold (beyond ±18:00; ECMAScript's offset time zones go to ±23:59). Such
/// an offset matches no metazone, so every zone symbol gives the localized
/// GMT format. The names are formatted with a template offset of the same
/// sign whose fields are recognizable (7 hours; 47 minutes, or 0 if the real
/// minutes are 0; 23 seconds, or 0 if the real seconds are 0), and then each
/// field's ASCII digits are replaced with the real field, keeping the
/// format's own width: "GMT+7" becomes "GMT+23", "GMT+07:47" "GMT+22:27".
/// The digits are ASCII because the names use `latn`, and the GMT format's
/// own text has none.
struct OffsetFix {
    /// The template's hours, minutes and seconds.
    template: [u32; 3],
    /// The real offset's hours, minutes and seconds.
    real: [u32; 3],
}

impl OffsetFix {
    /// The template UtcOffset to format with for `offset` (in seconds), and
    /// the fix for the names it gives.
    fn new(offset: i32) -> (UtcOffset, OffsetFix) {
        let abs = offset.unsigned_abs();
        let real = [abs / 3600, abs / 60 % 60, abs % 60];
        let template = [
            7,
            if real[1] == 0 { 0 } else { 47 },
            if real[2] == 0 { 0 } else { 23 },
        ];
        let seconds = (template[0] * 3600 + template[1] * 60 + template[2]) as i32;
        let utc = UtcOffset::from_seconds_unchecked(if offset < 0 { -seconds } else { seconds });
        (utc, OffsetFix { template, real })
    }

    /// `text`, a zone name formatted with the template offset, with the
    /// real offset's digits. None unless `text`'s ASCII digit runs are one
    /// to three of the template's fields, in order.
    fn apply(&self, text: &str) -> Option<String> {
        let mut out = String::with_capacity(text.len());
        let mut field = 0;
        let mut rest = text;
        while let Some(start) = rest.find(|c: char| c.is_ascii_digit()) {
            out.push_str(&rest[..start]);
            rest = &rest[start..];
            let len = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            if field == 3 || rest[..len].parse::<u32>().ok()? != self.template[field] {
                return None;
            }
            out.push_str(&format!("{:0len$}", self.real[field]));
            rest = &rest[len..];
            field += 1;
        }
        out.push_str(rest);
        (field > 0).then_some(out)
    }
}

/// One ICU4X calendar and the names of a formatter's symbols in it.
struct Names<C: CldrCalendar> {
    calendar: C,
    /// For each symbol, in order, its one-symbol pattern, the names that
    /// pattern needs, and whether it is a zone name.
    names: Vec<(
        DateTimePattern,
        FixedCalendarDateTimeNames<C, CompositeFieldSet>,
        bool,
    )>,
}

impl<C: CldrCalendar + IcuCalendar> Names<C> {
    /// Load the names of each of `symbols` for `prefs`; None if a symbol is
    /// not a pattern or its data does not load.
    fn new(calendar: C, prefs: DateTimeFormatterPreferences, symbols: &[&str]) -> Option<Self>
    where
        Baked: DataProvider<C::YearNamesV1> + DataProvider<C::MonthNamesV1>,
    {
        let names = symbols
            .iter()
            .map(|s| {
                let pattern = DateTimePattern::try_from_pattern_str(s).ok()?;
                let mut names = FixedCalendarDateTimeNames::try_new(prefs).ok()?;
                names.include_for_pattern(&pattern).ok()?;
                Some((pattern, names, is_zone_symbol(s)))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Names { calendar, names })
    }

    /// Entries 1 to 7 of DateTimeFormat::local_time for the local day `day`
    /// (days since 1970-01-01), and the text of each symbol for that day,
    /// `time` and `zone`. `iso` gives ISO years and no era (`iso8601`, which
    /// uses Gregorian names). If `zone` has a template offset, `fix` turns
    /// the zone names into the real offset's. None if ICU4X reports an
    /// error, or if `fix` does not apply.
    fn local_time(
        &self,
        iso: bool,
        day: i64,
        time: Time,
        zone: TimeZoneInfo<AtTime>,
        fix: Option<&OffsetFix>,
    ) -> Option<([String; 7], Vec<String>)> {
        let date = Date::from_rata_die(RataDie::new(day + RD_UNIX_EPOCH), Ref(&self.calendar));
        let month = date.month();
        let code = month.to_input().code().0.to_string();
        let leap = if month.to_input().is_leap() { "1" } else { "0" };
        let (era, year, related, number) = match date.year() {
            y if iso => (
                String::new(),
                y.extended_year(),
                String::new(),
                month.ordinal,
            ),
            YearInfo::Era(e) => (e.era.to_string(), e.year, String::new(), month.ordinal),
            // chinese and dangi: the month is the number of its code, and a
            // leap month has the leap flag.
            YearInfo::Cyclic(c) => (
                String::new(),
                i32::from(c.year),
                c.related_iso.to_string(),
                month.number(),
            ),
            _ => return None,
        };
        let fields = [
            era,
            year.to_string(),
            related,
            number.to_string(),
            code,
            leap.to_owned(),
            date.day_of_month().0.to_string(),
        ];
        let input = ZonedDateTime { date, time, zone };
        let names = self
            .names
            .iter()
            .map(|(pattern, names, is_zone)| {
                let formatted = names.with_pattern_unchecked(pattern).format(&input);
                let text = formatted.try_write_to_string().ok()?;
                match fix {
                    Some(fix) if *is_zone => fix.apply(&text),
                    _ => Some(text.into_owned()),
                }
            })
            .collect::<Option<Vec<_>>>()?;
        Some((fields, names))
    }
}

/// Run `$body` with `$n` bound to the Names of whichever calendar `$cal`
/// holds, and `$iso` to whether it is `iso8601`.
macro_rules! with_names {
    ($cal:expr, $n:ident, $iso:ident => $body:expr) => {
        match $cal {
            Calendars::Buddhist($n) => {
                let $iso = false;
                $body
            }
            Calendars::Chinese($n) => {
                let $iso = false;
                $body
            }
            Calendars::Coptic($n) => {
                let $iso = false;
                $body
            }
            Calendars::Dangi($n) => {
                let $iso = false;
                $body
            }
            Calendars::Ethiopian($n) => {
                let $iso = false;
                $body
            }
            Calendars::Gregorian($n) => {
                let $iso = false;
                $body
            }
            Calendars::Hebrew($n) => {
                let $iso = false;
                $body
            }
            Calendars::HijriTabular($n) => {
                let $iso = false;
                $body
            }
            Calendars::HijriUmmAlQura($n) => {
                let $iso = false;
                $body
            }
            Calendars::Indian($n) => {
                let $iso = false;
                $body
            }
            Calendars::Iso8601($n) => {
                let $iso = true;
                $body
            }
            Calendars::Japanese($n) => {
                let $iso = false;
                $body
            }
            Calendars::Persian($n) => {
                let $iso = false;
                $body
            }
            Calendars::Roc($n) => {
                let $iso = false;
                $body
            }
        }
    };
}

/// A formatter's calendar, one variant per ICU4X formattable calendar type.
/// `ethiopic` and `ethioaa` are both Ethiopian, with their era styles;
/// `islamic-civil` and `islamic-tbla` are both tabular, with their epochs;
/// `iso8601` uses Gregorian's names.
enum Calendars {
    Buddhist(Names<Buddhist>),
    Chinese(Names<ChineseTraditional>),
    Coptic(Names<Coptic>),
    Dangi(Names<KoreanTraditional>),
    Ethiopian(Names<Ethiopian>),
    Gregorian(Names<Gregorian>),
    Hebrew(Names<Hebrew>),
    HijriTabular(Names<Hijri<TabularAlgorithm>>),
    HijriUmmAlQura(Names<Hijri<UmmAlQura>>),
    Indian(Names<Indian>),
    Iso8601(Names<Gregorian>),
    Japanese(Names<Japanese>),
    Persian(Names<Persian>),
    Roc(Names<Roc>),
}

impl Calendars {
    /// The calendar of the CLDR id `calendar`, one of the 16 supported ones,
    /// with the names of `symbols`. The id goes to ICU4X's calendar kind as
    /// Phase 0 maps it (supported.rs, `calendars_are_icu4x_calendars`), and
    /// the kind to its calendar as `AnyCalendar::new` builds it.
    fn new(calendar: &str, prefs: DateTimeFormatterPreferences, symbols: &[&str]) -> Option<Self> {
        use icu::calendar::cal::hijri::{TabularAlgorithmEpoch, TabularAlgorithmLeapYears};
        if !CALENDARS.contains(&calendar) {
            return None;
        }
        let algorithm = CalendarAlgorithm::try_from(&Value::try_from_str(calendar).ok()?).ok()?;
        let tabular = |epoch| Hijri::new_tabular(TabularAlgorithmLeapYears::TypeII, epoch);
        Some(match AnyCalendarKind::try_from(algorithm).ok()? {
            AnyCalendarKind::Buddhist => Self::Buddhist(Names::new(Buddhist, prefs, symbols)?),
            AnyCalendarKind::Chinese => {
                Self::Chinese(Names::new(ChineseTraditional::new(), prefs, symbols)?)
            }
            AnyCalendarKind::Coptic => Self::Coptic(Names::new(Coptic, prefs, symbols)?),
            AnyCalendarKind::Dangi => {
                Self::Dangi(Names::new(KoreanTraditional::new(), prefs, symbols)?)
            }
            AnyCalendarKind::Ethiopian => Self::Ethiopian(Names::new(
                Ethiopian::new_with_era_style(EthiopianEraStyle::AmeteMihret),
                prefs,
                symbols,
            )?),
            AnyCalendarKind::EthiopianAmeteAlem => Self::Ethiopian(Names::new(
                Ethiopian::new_with_era_style(EthiopianEraStyle::AmeteAlem),
                prefs,
                symbols,
            )?),
            AnyCalendarKind::Gregorian => Self::Gregorian(Names::new(Gregorian, prefs, symbols)?),
            AnyCalendarKind::Hebrew => Self::Hebrew(Names::new(Hebrew, prefs, symbols)?),
            AnyCalendarKind::HijriTabularTypeIIFriday => Self::HijriTabular(Names::new(
                tabular(TabularAlgorithmEpoch::Friday),
                prefs,
                symbols,
            )?),
            AnyCalendarKind::HijriTabularTypeIIThursday => Self::HijriTabular(Names::new(
                tabular(TabularAlgorithmEpoch::Thursday),
                prefs,
                symbols,
            )?),
            AnyCalendarKind::HijriUmmAlQura => {
                Self::HijriUmmAlQura(Names::new(Hijri::new_umm_al_qura(), prefs, symbols)?)
            }
            AnyCalendarKind::Indian => Self::Indian(Names::new(Indian, prefs, symbols)?),
            AnyCalendarKind::Iso => Self::Iso8601(Names::new(Gregorian, prefs, symbols)?),
            AnyCalendarKind::Japanese => {
                Self::Japanese(Names::new(Japanese::new(), prefs, symbols)?)
            }
            AnyCalendarKind::Persian => Self::Persian(Names::new(Persian, prefs, symbols)?),
            AnyCalendarKind::Roc => Self::Roc(Names::new(Roc, prefs, symbols)?),
            // CALENDARS has no other calendar.
            _ => return None,
        })
    }
}

/// The calendar, time zone and names of one Intl.DateTimeFormat (decision
/// 16). It holds only immutable data and is Send.
pub struct DateTimeFormat {
    calendar: Calendars,
    rules: Rules,
    /// The zone's ICU4X id; TimeZone::UNKNOWN for an offset time zone.
    zone_id: TimeZone,
}

impl DateTimeFormat {
    /// A formatter for the language identifier of `locale` (a canonical
    /// tag), `calendar` (one of the 16 supported CLDR calendar ids),
    /// `time_zone` (a primary time zone identifier, or an offset time zone
    /// `±HH:MM`), with the names of `symbols`, each one of SYMBOLS that the
    /// calendar has (`U` only for `chinese` and `dangi`; the era symbols
    /// for every calendar but those and `iso8601`). Zone names use Latin
    /// digits. None for any other value, or if ICU4X's data does not load.
    pub fn new(locale: &str, calendar: &str, time_zone: &str, symbols: &[&str]) -> Option<Self> {
        if !symbols.iter().all(|s| symbol_allowed(calendar, s)) {
            return None;
        }
        let (rules, zone_id) = match parse_offset(time_zone) {
            Some(offset) => (Rules::Fixed(offset), TimeZone::UNKNOWN),
            None => {
                let primary = crate::host::time_zone_canonicalize(time_zone)?;
                if primary != time_zone {
                    return None;
                }
                (
                    Rules::Tzdb(tz::zone(time_zone)?),
                    IanaParser::new().parse(time_zone),
                )
            }
        };
        let mut prefs = DateTimeFormatterPreferences::from(&language_id(locale)?);
        prefs.numbering_system =
            Some(NumberingSystem::try_from(Value::try_from_str("latn").ok()?).ok()?);
        Some(DateTimeFormat {
            calendar: Calendars::new(calendar, prefs, symbols)?,
            rules,
            zone_id,
        })
    }

    /// The draft's ToLocalTime record of the time value `epoch_ms`, an
    /// integral number of milliseconds within ±8.64 × 10^15, and the text of
    /// the formatter's symbols at that instant:
    ///  0 [[Weekday]], "0" (Sunday) to "6";
    ///  1 [[Era]], ICU4X's era code ("ce", "bce", "reiwa"), "" for
    ///    `chinese`, `dangi` and `iso8601`;
    ///  2 [[Year]]: the era year, the cyclic year 1 to 60 of `chinese` and
    ///    `dangi`, or the ISO year of `iso8601`;
    ///  3 [[RelatedYear]], the related ISO year of `chinese` and `dangi`,
    ///    else "";
    ///  4 the month number shown: the ordinal month, or for `chinese` and
    ///    `dangi` the number of the month code;
    ///  5 the month code ("M04L"); 6 "1" for a leap month, else "0";
    ///  7-11 [[Day]], [[Hour]], [[Minute]], [[Second]], [[Millisecond]];
    ///  12 [[InDST]], "1" or "0"; 13 the UTC offset in seconds;
    ///  14 + i the text of symbol i.
    /// Numbers are decimal integers. Entries 0 to 13 come from the offset in
    /// seconds, so offsets beyond ICU4X's ±18:00 (offset time zones go to
    /// ±23:59) work; their zone names are the localized GMT format, made as
    /// OffsetFix says. None for any other `epoch_ms`, or if ICU4X reports an
    /// error formatting a name.
    pub fn local_time(&self, epoch_ms: f64) -> Option<Vec<String>> {
        if !is_time_value(epoch_ms) {
            return None;
        }
        let epoch_ms = epoch_ms as i64;
        let epoch_s = epoch_ms.div_euclid(1000);
        let (offset, dst) = match self.rules {
            Rules::Tzdb(zone) => zone.offset_at(epoch_s),
            Rules::Fixed(offset) => (offset, false),
        };
        let local = epoch_ms + i64::from(offset) * 1000;
        let (day, ms) = (local.div_euclid(MS_PER_DAY), local.rem_euclid(MS_PER_DAY));
        let (h, m, s, milli) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
        let time = Time::try_new(h as u8, m as u8, s as u8, milli as u32 * 1_000_000).ok()?;
        // An offset beyond ±18:00 matches no metazone: its names are the
        // GMT format of an unknown zone, made from a template offset.
        let (zone_id, utc_offset, fix) = match UtcOffset::try_from_seconds(offset) {
            Ok(utc_offset) => (self.zone_id, utc_offset, None),
            Err(_) => {
                let (template, fix) = OffsetFix::new(offset);
                (TimeZone::UNKNOWN, template, Some(fix))
            }
        };
        let zone = zone_id
            .with_offset(Some(utc_offset))
            .with_zone_name_timestamp(ZoneNameTimestamp::from_epoch_seconds(epoch_s));
        let (fields, names) = with_names!(
            &self.calendar, n, iso => n.local_time(iso, day, time, zone, fix.as_ref())
        )?;
        let mut out = Vec::with_capacity(14 + names.len());
        // ECMA-262 WeekDay: 1970-01-01 was a Thursday.
        out.push((day + 4).rem_euclid(7).to_string());
        out.extend(fields);
        out.extend([
            h.to_string(),
            m.to_string(),
            s.to_string(),
            milli.to_string(),
            (if dst { "1" } else { "0" }).to_owned(),
            offset.to_string(),
        ]);
        out.extend(names);
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::test_locales;
    use crate::supported::CALENDARS;

    /// The time value of a UTC date and time.
    fn utc(y: i64, mo: u8, d: u8, h: i64, mi: i64) -> f64 {
        ((crate::tz::days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60) * 1000) as f64
    }

    /// A formatter that must exist.
    fn make(locale: &str, calendar: &str, zone: &str, symbols: &[&str]) -> DateTimeFormat {
        DateTimeFormat::new(locale, calendar, zone, symbols)
            .unwrap_or_else(|| panic!("{locale} {calendar} {zone} {symbols:?}"))
    }

    /// The local time of `ms`, which must exist.
    fn at(f: &DateTimeFormat, ms: f64) -> Vec<String> {
        f.local_time(ms).unwrap_or_else(|| panic!("{ms}"))
    }

    #[test]
    fn gregory_fields() {
        let f = make("en", "gregory", "UTC", &[]);
        // 2026-01-05T15:07:09.123Z, a Monday.
        assert_eq!(
            at(&f, 1767625629123.0),
            ["1", "ce", "2026", "", "1", "M01", "0", "5", "15", "7", "9", "123", "0", "0"]
        );
        // -0001-01-01, 2 BC: the era year, which the draft's 1 - value step
        // leaves alone.
        assert_eq!(
            at(&f, -62198755200000.0),
            ["5", "bce", "2", "", "1", "M01", "0", "1", "0", "0", "0", "0", "0", "0"]
        );
        // The ends of the ECMAScript time range.
        assert_eq!(
            at(&f, 8.64e15),
            ["6", "ce", "275760", "", "9", "M09", "0", "13", "0", "0", "0", "0", "0", "0"]
        );
        assert_eq!(
            at(&f, -8.64e15),
            ["2", "bce", "271822", "", "4", "M04", "0", "20", "0", "0", "0", "0", "0", "0"]
        );
        // One millisecond before the epoch is the last of 1969-12-31.
        assert_eq!(
            at(&f, -1.0),
            ["3", "ce", "1969", "", "12", "M12", "0", "31", "23", "59", "59", "999", "0", "0"]
        );
    }

    #[test]
    fn names() {
        let f = make(
            "en",
            "gregory",
            "UTC",
            &["EEEE", "MMMM", "LLLL", "G", "GGGG", "a"],
        );
        assert_eq!(
            at(&f, 1767625629123.0)[14..],
            ["Monday", "January", "January", "AD", "Anno Domini", "PM"]
        );
        let f = make("de", "gregory", "UTC", &["MMMM"]);
        assert_eq!(at(&f, 1767625629123.0)[14..], ["Januar"]);
        // Polish months: format context is genitive, stand-alone nominative.
        let f = make("pl", "gregory", "UTC", &["MMMM", "LLLL"]);
        assert_eq!(at(&f, 1767625629123.0)[14..], ["stycznia", "styczeń"]);
    }

    #[test]
    fn zones() {
        let symbols = ["z", "zzzz", "O", "OOOO", "v", "vvvv"];
        let f = make("en", "gregory", "America/New_York", &symbols);
        let v = at(&f, utc(2026, 7, 1, 12, 0));
        assert_eq!(
            v[14..],
            [
                "EDT",
                "Eastern Daylight Time",
                "GMT-4",
                "GMT-04:00",
                "ET",
                "Eastern Time"
            ]
        );
        assert_eq!(
            (v[8].as_str(), v[12].as_str(), v[13].as_str()),
            ("8", "1", "-14400")
        );
        let v = at(&f, utc(2026, 1, 5, 12, 0));
        assert_eq!(v[14..16], ["EST", "Eastern Standard Time"]);
        assert_eq!((v[12].as_str(), v[13].as_str()), ("0", "-18000"));

        let f = make("en", "gregory", "+05:30", &["z", "v"]);
        let v = at(&f, utc(2026, 7, 1, 12, 0));
        assert_eq!(v[14..], ["GMT+5:30", "GMT+5:30"]);
        assert_eq!((v[8].as_str(), v[9].as_str()), ("17", "30"));
        assert_eq!((v[12].as_str(), v[13].as_str()), ("0", "19800"));
        let f = make("en", "gregory", "-09:00", &["OOOO"]);
        let v = at(&f, utc(2026, 7, 1, 3, 0));
        assert_eq!(v[14..], ["GMT-09:00"]);
        // The local day is the previous one.
        assert_eq!((v[7].as_str(), v[8].as_str()), ("30", "18"));

        let f = make("en", "gregory", "UTC", &["z", "zzzz"]);
        assert_eq!(at(&f, 0.0)[14..], ["UTC", "Coordinated Universal Time"]);

        // Local mean time matches neither of CLDR's offsets, so z falls back
        // to the localized GMT format, with its seconds.
        let f = make("en", "gregory", "America/New_York", &["OOOO", "z"]);
        let v = at(&f, utc(1800, 1, 1, 12, 0));
        assert_eq!(v[14..], ["GMT-04:56:02", "GMT-4:56:02"]);
        assert_eq!(v[13], "-17762");

        // Rearguard data: Dublin's summer time is daylight saving time.
        let f = make("en", "gregory", "Europe/Dublin", &["zzzz"]);
        let v = at(&f, utc(2026, 7, 1, 12, 0));
        assert_eq!(v[14..], ["Irish Standard Time"]);
        assert_eq!((v[12].as_str(), v[13].as_str()), ("1", "3600"));
    }

    #[test]
    fn offsets_beyond_icu4x() {
        // ECMAScript's offset time zones go to ±23:59, ICU4X's UtcOffset to
        // ±18:00; test262 uses these three.
        let symbols = ["z", "zzzz", "O", "OOOO", "v", "vvvv"];
        let cases = [
            (
                "+23:00",
                ["4", "1", "23", "0", "82800"],
                [
                    "GMT+23",
                    "GMT+23:00",
                    "GMT+23",
                    "GMT+23:00",
                    "GMT+23",
                    "GMT+23:00",
                ],
            ),
            ("+22:27", ["4", "1", "22", "27", "80820"], ["GMT+22:27"; 6]),
            ("-22:23", ["3", "31", "1", "37", "-80580"], ["GMT-22:23"; 6]),
        ];
        for (zone, [weekday, day, hour, minute, offset], names) in cases {
            let f = make("en", "gregory", zone, &symbols);
            let v = at(&f, 0.0);
            let year_month = if day == "31" {
                ["1969", "12"]
            } else {
                ["1970", "1"]
            };
            assert_eq!(
                v[..14],
                [
                    weekday,
                    "ce",
                    year_month[0],
                    "",
                    year_month[1],
                    &format!("M{:02}", year_month[1].parse::<u8>().unwrap()),
                    "0",
                    day,
                    hour,
                    minute,
                    "0",
                    "0",
                    "0",
                    offset
                ],
                "{zone}"
            );
            assert_eq!(v[14..], names, "{zone}");
        }
        // The fields come from the offset even with no zone symbol.
        let f = make("en", "gregory", "+23:59", &[]);
        assert_eq!(at(&f, 0.0)[8..10], ["23", "59"]);
        // The format's own separators and text stay: Danish uses ".", and
        // Persian puts the offset first, after a left-to-right mark.
        let f = make("da", "gregory", "+22:27", &["OOOO"]);
        assert_eq!(at(&f, 0.0)[14], "GMT+22.27");
        let f = make("fa", "gregory", "+23:00", &["O"]);
        assert_eq!(at(&f, 0.0)[14], "\u{200e}+23 گرینویچ");
    }

    #[test]
    fn offset_fix() {
        let (template, fix) = OffsetFix::new(-(22 * 3600 + 23 * 60));
        assert_eq!(template.to_seconds(), -(7 * 3600 + 47 * 60));
        assert_eq!(fix.apply("GMT-7:47").as_deref(), Some("GMT-22:23"));
        assert_eq!(fix.apply("GMT-07.47").as_deref(), Some("GMT-22.23"));
        // Any other digits are not the template's fields.
        assert_eq!(fix.apply("GMT-7:48"), None);
        assert_eq!(fix.apply("GMT-7:47:01"), None);
        assert_eq!(fix.apply("GMT-7:47:00:00"), None);
        assert_eq!(fix.apply("GMT"), None);
        let (template, fix) = OffsetFix::new(23 * 3600);
        assert_eq!(template.to_seconds(), 7 * 3600);
        assert_eq!(fix.apply("GMT+7").as_deref(), Some("GMT+23"));
        assert_eq!(fix.apply("GMT+07:00").as_deref(), Some("GMT+23:00"));
        // Seconds, which only a tzdb offset could have.
        let (template, fix) = OffsetFix::new(18 * 3600 + 5);
        assert_eq!(template.to_seconds(), 7 * 3600 + 23);
        assert_eq!(fix.apply("GMT+7:00:23").as_deref(), Some("GMT+18:00:05"));
    }

    #[test]
    fn zone_names_have_ascii_digits() {
        // Without `latn`, ar-EG and fa format these with Arabic-Indic and
        // extended Arabic-Indic digits.
        for locale in ["ar-EG", "ar-EG-u-nu-arab", "fa", "fa-u-nu-arabext"] {
            let f = make(locale, "gregory", "+05:30", &["O", "OOOO", "z"]);
            let v = at(&f, 0.0);
            let want: [&str; 3] = if locale.starts_with("ar") {
                // ICU4X writes the sign with the locale's plus sign, which for
                // ar's latn digits is U+200E +; ICU4C (Node) has a bare "+".
                [
                    "غرينتش\u{200e}+5:30",
                    "غرينتش\u{200e}+05:30",
                    "غرينتش\u{200e}+5:30",
                ]
            } else {
                [
                    "\u{200e}+5:30 گرینویچ",
                    "\u{200e}+05:30 گرینویچ",
                    "\u{200e}+5:30 گرینویچ",
                ]
            };
            assert_eq!(v[14..], want, "{locale}");
            for name in &v[14..] {
                assert!(
                    !name.chars().any(|c| c.is_numeric() && !c.is_ascii_digit()),
                    "{locale}: {name}"
                );
            }
        }
        // A -u-nu in the tag cannot reach ICU4X: only the language
        // identifier is used.
        assert_eq!(
            language_id("ar-EG-u-nu-arab")
                .map(|id| id.to_string())
                .as_deref(),
            Some("ar-EG")
        );
    }

    #[test]
    fn lunisolar() {
        // 2020-05-23 is in the leap fourth month of 庚子, cyclic year 37.
        let f = make("en", "chinese", "UTC", &["U", "MMMM"]);
        let v = at(&f, utc(2020, 5, 23, 0, 0));
        assert_eq!(v[1..7], ["", "37", "2020", "4", "M04L", "1"]);
        assert_eq!(v[14], "geng-zi");
        assert!(!v[15].is_empty());
        let f = make("zh", "chinese", "UTC", &["U"]);
        let v = at(&f, utc(2019, 6, 1, 0, 0));
        assert_eq!(v[14..], ["己亥"]);
        assert_eq!((v[2].as_str(), v[3].as_str()), ("36", "2019"));
        let f = make("ko", "dangi", "UTC", &["U"]);
        let v = at(&f, utc(2020, 5, 23, 0, 0));
        assert_eq!(v[3..7], ["2020", "4", "M04L", "1"]);
        assert!(!v[14].is_empty());

        // Hebrew 5784 is a leap year: Adar I is M05L, the sixth month.
        let f = make("en", "hebrew", "UTC", &["MMMM"]);
        let v = at(&f, utc(2024, 2, 15, 0, 0));
        assert_eq!(v[1..7], ["am", "5784", "", "6", "M05L", "1"]);
        assert_eq!(v[14], "Adar I");
        let v = at(&f, utc(2024, 3, 15, 0, 0));
        assert_eq!(v[4..7], ["7", "M06", "0"]);
        assert_eq!(v[14], "Adar II");
        let v = at(&f, utc(2023, 3, 15, 0, 0));
        assert_eq!(v[4..7], ["6", "M06", "0"]);
        assert_eq!(v[14], "Adar");
    }

    #[test]
    fn eras() {
        let f = make("ja", "japanese", "UTC", &["G"]);
        let v = at(&f, utc(2019, 4, 30, 0, 0));
        assert_eq!(
            (v[1].as_str(), v[2].as_str(), v[14].as_str()),
            ("heisei", "31", "平成")
        );
        let v = at(&f, utc(2019, 5, 1, 0, 0));
        assert_eq!(
            (v[1].as_str(), v[2].as_str(), v[14].as_str()),
            ("reiwa", "1", "令和")
        );

        let f = make("en", "roc", "UTC", &["G", "GGGG"]);
        let v = at(&f, utc(1911, 6, 1, 0, 0));
        assert_eq!((v[1].as_str(), v[2].as_str()), ("broc", "1"));
        // CLDR 49 en has "B.R.O.C." for both widths, as Node shows.
        assert_eq!(v[14..], ["B.R.O.C.", "B.R.O.C."]);
        let v = at(&f, utc(1912, 6, 1, 0, 0));
        assert_eq!((v[1].as_str(), v[2].as_str()), ("roc", "1"));

        // Amete Alem has one era, over the whole time range.
        let f = make("en", "ethioaa", "UTC", &["G"]);
        let eras: Vec<_> = [-8.64e15, 0.0, 8.64e15]
            .iter()
            .map(|&t| {
                let v = at(&f, t);
                (v[1].clone(), v[14].clone())
            })
            .collect();
        assert_eq!(eras[0].0, "aa");
        assert!(eras.iter().all(|e| *e == eras[0]), "{eras:?}");
        assert_eq!(at(&f, 0.0)[2], "7462");
    }

    #[test]
    fn iso8601() {
        let f = make("en", "iso8601", "UTC", &["MMMM", "EEEE"]);
        // Date.parse("0000-01-01T00:00:00Z"); Date.UTC(0, 0, 1) is 1900.
        let v = at(&f, -62167219200000.0);
        assert_eq!(v[..8], ["6", "", "0", "", "1", "M01", "0", "1"]);
        assert_eq!(v[14..], ["January", "Saturday"]);
        assert_eq!(at(&f, -62198755200000.0)[1..3], ["", "-1"]);
        assert_eq!(at(&f, 1767625629123.0)[1..3], ["", "2026"]);
    }

    /// The symbols decision 4 allows for `calendar`.
    fn allowed(calendar: &str) -> Vec<&'static str> {
        SYMBOLS
            .iter()
            .copied()
            .filter(|s| symbol_allowed(calendar, s))
            .collect()
    }

    #[test]
    fn every_calendar_every_locale() {
        let mut n = 0;
        for locale in test_locales().chain(["und"]) {
            for cal in CALENDARS {
                let symbols = allowed(cal);
                let f = make(locale, cal, "America/New_York", &symbols);
                for t in [-8.64e15, 0.0, 8.64e15] {
                    let v = f
                        .local_time(t)
                        .unwrap_or_else(|| panic!("{locale} {cal} {t}"));
                    assert_eq!(v.len(), 14 + symbols.len(), "{locale} {cal} {t}");
                    for (s, name) in symbols.iter().zip(&v[14..]) {
                        assert!(!name.is_empty(), "{locale} {cal} {t} {s}");
                    }
                }
                n += 1;
            }
        }
        assert_eq!(n, (test_locales().count() + 1) * CALENDARS.len());
    }

    #[test]
    fn symbols_by_calendar() {
        assert!(allowed("chinese").contains(&"U"));
        assert!(allowed("dangi").contains(&"U"));
        assert!(!allowed("chinese").contains(&"G"));
        assert!(!allowed("iso8601").contains(&"GGGG"));
        assert!(!allowed("gregory").contains(&"U"));
        assert_eq!(allowed("gregory").len(), SYMBOLS.len() - 1);
        assert_eq!(allowed("iso8601").len(), SYMBOLS.len() - 4);
        assert_eq!(allowed("dangi").len(), SYMBOLS.len() - 3);
    }

    #[test]
    fn rejects_bad_input() {
        let new = |l: &str, c: &str, z: &str, s: &[&str]| DateTimeFormat::new(l, c, z, s).is_none();
        assert!(new("en", "julian", "UTC", &[]));
        assert!(new("en", "islamic", "UTC", &[]));
        assert!(new("en", "gregory", "UTC", &["MMMMMM"]));
        assert!(new("en", "gregory", "UTC", &[""]));
        assert!(new("en", "gregory", "UTC", &["U"]));
        assert!(new("en", "chinese", "UTC", &["G"]));
        assert!(new("en", "iso8601", "UTC", &["G"]));
        assert!(new("en_US", "gregory", "UTC", &[]));
        assert!(new("en", "gregory", "Mars/Olympus_Mons", &[]));
        // Only primary identifiers, as JS passes them.
        assert!(new("en", "gregory", "Asia/Calcutta", &[]));
        assert!(new("en", "gregory", "utc", &[]));
        for z in [
            "+25:00",
            "+05:60",
            "05:30",
            "+5:30",
            "+05:30:00",
            "\u{2212}05:00",
            "+0530",
        ] {
            assert!(new("en", "gregory", z, &[]), "{z}");
        }
        let f = make("en", "gregory", "UTC", &[]);
        for t in [1.5, f64::NAN, 8.64e15 + 1.0, -8.64e15 - 1.0, f64::INFINITY] {
            assert!(!is_time_value(t), "{t}");
            assert_eq!(f.local_time(t), None, "{t}");
        }
        for t in [-0.0, 0.0, 8.64e15, -8.64e15, -1.0] {
            assert!(is_time_value(t), "{t}");
            assert!(f.local_time(t).is_some(), "{t}");
        }
    }
}
