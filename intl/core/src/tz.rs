// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! Time zone rules, read from the IANA tzdb data that the jiff-tzdb crate
//! embeds (decision 2 of the Phase 4 plan).
//!
//! jiff-tzdb holds each zone as a TZif file (RFC 8536), compiled by `zic` in
//! the "rearguard" format (so Europe/Dublin's daylight saving time is in
//! summer) and the "slim" layout: explicit transitions up to some year, then
//! a POSIX TZ string in the footer for the rules after them. The plugin
//! reads it itself, not through `jiff`, because ECMAScript's time values run
//! to years ±275,760 and `jiff`'s stop at ±9999.
//!
//! - An instant before the first transition has the zone's first local time
//!   type, which is local mean time with its seconds (RFC 8536 section 3.2).
//! - An instant after the last transition has the offset of the footer's
//!   rule, computed for the instant's year with exact integer arithmetic on
//!   days, which works for any year (section 3.3).
//! - A file with no transitions follows its footer at every instant.
//!
//! Offsets are in seconds east of UTC, as TZif's `utoff`; POSIX TZ strings
//! write them west-positive (`EST5` is UTC−5), and the parser negates them.

use std::sync::OnceLock;

/// The IANA tzdb release that jiff-tzdb 0.1.9 embeds (a test checks it).
/// Only that test reads it.
#[cfg(test)]
pub const TZDB_VERSION: &str = "2026e";

/// The largest hour of a POSIX TZ offset (POSIX: 0 to 24).
const MAX_OFFSET_HOURS: u32 = 24;

/// The largest hour of a rule's transition time, either way (RFC 8536
/// section 3.3.1 extends POSIX's 0 to 24 to −167 to 167).
const MAX_RULE_HOURS: u32 = 167;

/// A local time type: its UTC offset and whether it is daylight saving time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalTimeType {
    /// Seconds east of UTC.
    offset: i32,
    /// TZif's `isdst`.
    is_dst: bool,
}

/// The day of a rule's transition, in POSIX TZ syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuleDate {
    /// `Jn`: the day 1 to 365 of the year, never counting February 29.
    Julian1(u16),
    /// `n`: the zero-based day 0 to 365 of the year, counting February 29.
    Julian0(u16),
    /// `Mm.w.d`: the day `weekday` (0 is Sunday) of week `week` (1 to 5; 5
    /// is the last) of month `month` (1 to 12).
    MonthWeekDay { month: u8, week: u8, weekday: u8 },
}

/// A transition of a rule: a day, and a time of that day in the local time
/// in effect before the transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuleTime {
    date: RuleDate,
    /// Seconds from the day's local midnight, −167 to 167 hours.
    time: i32,
}

/// The rule of a TZif footer's POSIX TZ string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    /// Standard time only: a fixed offset.
    Fixed(LocalTimeType),
    /// Standard time and daylight saving time, which starts at `start` (in
    /// standard time) and ends at `end` (in daylight saving time) each year.
    Dst {
        std_offset: i32,
        dst_offset: i32,
        start: RuleTime,
        end: RuleTime,
    },
}

/// A zone's rules, parsed from its TZif data.
#[derive(Debug)]
pub struct Zone {
    /// The transition times, in epoch seconds, strictly ascending.
    transitions: Vec<i64>,
    /// The local time type in effect from each transition on, as an index
    /// into `types`.
    transition_types: Vec<u8>,
    /// The local time types; never empty.
    types: Vec<LocalTimeType>,
    /// The footer's rule for instants after the last transition; None if
    /// the footer is empty.
    footer: Option<Rule>,
}

/// The days from 1970-01-01 to the proleptic Gregorian date `y`-`m`-`d`
/// (Rata Die minus 719163). Exact for every year whose days fit an i64
/// (Howard Hinnant's `days_from_civil`).
pub(crate) fn days_from_civil(y: i64, m: u8, d: u8) -> i64 {
    let (m, d) = (m as i64, d as i64);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// The proleptic Gregorian year of the day `days` after 1970-01-01, for any
/// i64 (Howard Hinnant's `civil_from_days`).
fn year_of_day(days: i64) -> i64 {
    // Days from 0000-03-01; i64::MIN / 86400 is far from overflowing.
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    // Months 10 and 11 of this March-based year are January and February.
    yoe + era * 400 + (mp >= 10) as i64
}

/// Whether `y` is a leap year of the proleptic Gregorian calendar.
fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

/// The number of days in month `m` of year `y`.
fn days_in_month(y: i64, m: u8) -> i64 {
    match m {
        2 => 28 + is_leap(y) as i64,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl RuleDate {
    /// The day of this date in year `y`, in days from 1970-01-01.
    fn day_in(self, y: i64) -> i64 {
        match self {
            RuleDate::Julian1(n) => {
                let n = n as i64;
                let skip = (is_leap(y) && n >= 60) as i64;
                days_from_civil(y, 1, 1) + n - 1 + skip
            }
            RuleDate::Julian0(n) => days_from_civil(y, 1, 1) + n as i64,
            RuleDate::MonthWeekDay {
                month,
                week,
                weekday,
            } => {
                let first = days_from_civil(y, month, 1);
                // 1970-01-01 was a Thursday (4).
                let first_weekday = (first + 4).rem_euclid(7);
                let mut d = (weekday as i64 - first_weekday).rem_euclid(7) + (week as i64 - 1) * 7;
                // Week 5 is the last such weekday of the month.
                while d >= days_in_month(y, month) {
                    d -= 7;
                }
                first + d
            }
        }
    }
}

impl Rule {
    /// The UTC offset in seconds and the daylight saving flag at
    /// `epoch_seconds`, for any i64.
    fn at(&self, epoch_seconds: i64) -> (i32, bool) {
        let (std_offset, dst_offset, start, end) = match *self {
            Rule::Fixed(t) => return (t.offset, t.is_dst),
            Rule::Dst {
                std_offset,
                dst_offset,
                start,
                end,
            } => (std_offset, dst_offset, start, end),
        };
        // The UTC instant of a transition of year `y`, in i128 so that no
        // instant near the ends of i64 overflows.
        let instant = |rt: RuleTime, y: i64, offset: i32| -> i128 {
            rt.date.day_in(y) as i128 * 86400 + rt.time as i128 - offset as i128
        };
        // A transition may fall up to 167 hours and an offset from its day,
        // so it can land in the year before or after its own. The state at
        // the instant is that of the latest transition at or before it,
        // among those of the instant's UTC year, the year after, and the two
        // years before (the latest one of which is always before it). This
        // also covers daylight saving time across the new year (the southern
        // hemisphere) and all year (RFC 8536 section 3.3.1).
        let year = year_of_day(epoch_seconds.div_euclid(86400));
        let t = epoch_seconds as i128;
        // (instant, order within ties, is daylight saving time from then).
        let mut latest: Option<(i128, u8, bool)> = None;
        for (k, y) in (year - 2..=year + 1).enumerate() {
            let k = k as u8;
            for (seq, at, dst) in [
                (2 * k, instant(start, y, std_offset), true),
                (2 * k + 1, instant(end, y, dst_offset), false),
            ] {
                if at <= t && latest.is_none_or(|(l, lseq, _)| (at, seq) > (l, lseq)) {
                    latest = Some((at, seq, dst));
                }
            }
        }
        // Always set: the transitions of year - 2 fall before year - 1 ends,
        // and those of the years after year + 1 all fall after the instant.
        let dst = latest.is_some_and(|(_, _, dst)| dst);
        (if dst { dst_offset } else { std_offset }, dst)
    }
}

/// A reader of a POSIX TZ string's bytes.
struct TzReader<'a> {
    s: &'a [u8],
    pos: usize,
}

impl TzReader<'_> {
    /// The next byte, if any, without consuming it.
    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    /// Consume the next byte if it is `c`; whether it was.
    fn eat(&mut self, c: u8) -> bool {
        let found = self.peek() == Some(c);
        self.pos += found as usize;
        found
    }

    /// Whether every byte has been consumed.
    fn at_end(&self) -> bool {
        self.pos == self.s.len()
    }

    /// A zone abbreviation: three or more letters, or `<...>` around three
    /// or more letters, digits, `+` and `-`.
    fn name(&mut self) -> Option<()> {
        let quoted = self.eat(b'<');
        let begin = self.pos;
        while let Some(c) = self.peek() {
            let ok = c.is_ascii_alphabetic()
                || (quoted && (c.is_ascii_digit() || c == b'+' || c == b'-'));
            if !ok {
                break;
            }
            self.pos += 1;
        }
        let len = self.pos - begin;
        if quoted && !self.eat(b'>') {
            return None;
        }
        (len >= 3).then_some(())
    }

    /// An unsigned decimal number of 1 to `max_digits` digits.
    fn number(&mut self, max_digits: usize) -> Option<u32> {
        let begin = self.pos;
        let mut n: u32 = 0;
        while let Some(c @ b'0'..=b'9') = self.peek() {
            if self.pos - begin == max_digits {
                return None;
            }
            n = n * 10 + (c - b'0') as u32;
            self.pos += 1;
        }
        (self.pos > begin).then_some(n)
    }

    /// `[+|-]hh[:mm[:ss]]` with `hh` at most `max_hours`, in seconds.
    fn hms(&mut self, max_hours: u32) -> Option<i32> {
        let negative = self.eat(b'-');
        if !negative {
            self.eat(b'+');
        }
        let h = self.number(3).filter(|&h| h <= max_hours)?;
        let mut seconds = h * 3600;
        for scale in [60, 1] {
            if !self.eat(b':') {
                break;
            }
            let v = self.number(2).filter(|&v| v <= 59)?;
            seconds += v * scale;
        }
        let seconds = seconds as i32;
        Some(if negative { -seconds } else { seconds })
    }

    /// A rule's `date[/time]`; the time defaults to 02:00:00.
    fn rule_time(&mut self) -> Option<RuleTime> {
        let date = if self.eat(b'J') {
            RuleDate::Julian1(self.number(3).filter(|n| (1..=365).contains(n))? as u16)
        } else if self.eat(b'M') {
            let month = self.number(2).filter(|n| (1..=12).contains(n))? as u8;
            self.eat(b'.').then_some(())?;
            let week = self.number(1).filter(|n| (1..=5).contains(n))? as u8;
            self.eat(b'.').then_some(())?;
            let weekday = self.number(1).filter(|&n| n <= 6)? as u8;
            RuleDate::MonthWeekDay {
                month,
                week,
                weekday,
            }
        } else {
            RuleDate::Julian0(self.number(3).filter(|&n| n <= 365)? as u16)
        };
        let time = if self.eat(b'/') {
            self.hms(MAX_RULE_HOURS)?
        } else {
            7200
        };
        Some(RuleTime { date, time })
    }
}

/// Parse a TZif footer's POSIX TZ string (RFC 8536 section 3.3):
/// `std offset [dst [offset] ,start[/time],end[/time]]`. Some(None) for the
/// empty string; None if it does not parse. A daylight saving time name
/// without a rule is rejected, since POSIX leaves its rule to the
/// implementation (zic always writes one).
fn parse_footer(s: &[u8]) -> Option<Option<Rule>> {
    if s.is_empty() {
        return Some(None);
    }
    let mut r = TzReader { s, pos: 0 };
    r.name()?;
    let std_offset = -r.hms(MAX_OFFSET_HOURS)?;
    if r.at_end() {
        return Some(Some(Rule::Fixed(LocalTimeType {
            offset: std_offset,
            is_dst: false,
        })));
    }
    r.name()?;
    let dst_offset = if r.peek() == Some(b',') {
        std_offset + 3600
    } else {
        -r.hms(MAX_OFFSET_HOURS)?
    };
    r.eat(b',').then_some(())?;
    let start = r.rule_time()?;
    r.eat(b',').then_some(())?;
    let end = r.rule_time()?;
    r.at_end().then_some(())?;
    Some(Some(Rule::Dst {
        std_offset,
        dst_offset,
        start,
        end,
    }))
}

/// A reader of TZif bytes that fails, rather than panics, past the end.
struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bytes<'a> {
    /// The next `n` bytes, consumed; None if fewer remain.
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }

    /// The next four bytes as a big-endian u32, consumed.
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }
}

/// A TZif header's counts: isutcnt, isstdcnt, leapcnt, timecnt, typecnt,
/// charcnt (RFC 8536 section 3.1).
struct Counts {
    isut: usize,
    isstd: usize,
    leap: usize,
    time: usize,
    types: usize,
    chars: usize,
}

impl Counts {
    /// Read a header: the magic "TZif", the version (2 or later; RFC 9636's
    /// version 4 keeps version 2's layout), 15 unused bytes and the counts.
    fn read(b: &mut Bytes) -> Option<Counts> {
        let head = b.take(20)?;
        if &head[..4] != b"TZif" || head[4] < b'2' {
            return None;
        }
        let mut c = [0usize; 6];
        for v in &mut c {
            *v = b.u32()? as usize;
        }
        let [isut, isstd, leap, time, types, chars] = c;
        // RFC 8536 section 3.1.
        let valid = (isut == 0 || isut == types)
            && (isstd == 0 || isstd == types)
            && types != 0
            && chars != 0;
        valid.then_some(Counts {
            isut,
            isstd,
            leap,
            time,
            types,
            chars,
        })
    }

    /// The size of the data block that follows the header, with transition
    /// times and leap second occurrences of `time_size` bytes.
    fn block_size(&self, time_size: usize) -> Option<usize> {
        let parts = [
            self.time.checked_mul(time_size + 1)?,
            self.types.checked_mul(6)?,
            self.chars,
            self.leap.checked_mul(time_size + 4)?,
            self.isstd,
            self.isut,
        ];
        parts.iter().try_fold(0usize, |a, &p| a.checked_add(p))
    }
}

/// Why `read_headers` rejected TZif data.
#[derive(Debug, PartialEq, Eq)]
enum HeaderError {
    /// A header is cut short, lacks the magic or a version of 2 or later, or
    /// has invalid counts, or the version 1 block is cut short.
    Malformed,
    /// The version 2+ header's counts call for more bytes than the data has.
    CountsExceedData,
}

/// Read the headers of TZif data and skip the version 1 block. Then check,
/// before anything is allocated from them, that the data holds the 64-bit
/// block that the second header's counts describe and at least the footer's
/// two newlines. Returns the reader, at the 64-bit block, and its counts.
fn read_headers(data: &[u8]) -> Result<(Bytes<'_>, Counts), HeaderError> {
    let mut b = Bytes { data, pos: 0 };
    let v1 = Counts::read(&mut b).ok_or(HeaderError::Malformed)?;
    v1.block_size(4)
        .and_then(|n| b.take(n))
        .ok_or(HeaderError::Malformed)?;
    let c = Counts::read(&mut b).ok_or(HeaderError::Malformed)?;
    let need = c.block_size(8).and_then(|n| n.checked_add(2));
    if need.is_none_or(|n| n > data.len() - b.pos) {
        return Err(HeaderError::CountsExceedData);
    }
    Ok((b, c))
}

impl Zone {
    /// Parse TZif data of version 2 or later (RFC 8536): the version 1 block
    /// is skipped, and the 64-bit block and the footer are read. None if the
    /// data is malformed, never a panic. Leap second records are skipped
    /// (the tzdb's zones have none); so are the standard/wall and UT/local
    /// indicators, which only matter for POSIX TZ strings without rules.
    pub fn parse(data: &[u8]) -> Option<Zone> {
        let (mut b, c) = read_headers(data).ok()?;
        let transitions: Vec<i64> = b
            .take(c.time.checked_mul(8)?)?
            .chunks_exact(8)
            .map(|x| i64::from_be_bytes(x.try_into().unwrap()))
            .collect();
        if transitions.windows(2).any(|w| w[0] >= w[1]) {
            return None;
        }
        let transition_types = b.take(c.time)?.to_vec();
        if transition_types.iter().any(|&i| i as usize >= c.types) {
            return None;
        }
        // Each type is utoff (i32), isdst (0 or 1) and desigidx (an index
        // into the designations); utoff may not be -2^31 (section 3.2).
        let types = b
            .take(c.types.checked_mul(6)?)?
            .chunks_exact(6)
            .map(|x| {
                let offset = i32::from_be_bytes(x[..4].try_into().unwrap());
                let valid = offset != i32::MIN && x[4] <= 1 && (x[5] as usize) < c.chars;
                valid.then_some(LocalTimeType {
                    offset,
                    is_dst: x[4] == 1,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        b.take(c.chars)?;
        b.take(c.leap.checked_mul(12)?)?;
        b.take(c.isstd)?;
        b.take(c.isut)?;
        // The footer: a newline, the TZ string, a newline, and nothing else.
        let rest = &data[b.pos..];
        if rest.len() < 2 || rest[0] != b'\n' || rest[rest.len() - 1] != b'\n' {
            return None;
        }
        let tz = &rest[1..rest.len() - 1];
        if tz.contains(&b'\n') {
            return None;
        }
        let footer = parse_footer(tz)?;
        Some(Zone {
            transitions,
            transition_types,
            types,
            footer,
        })
    }

    /// The UTC offset in seconds and the daylight saving flag (TZif's
    /// `isdst`, or whether the footer's rule is in daylight saving time) at
    /// `epoch_seconds`. Defined for every i64; ECMAScript's time values
    /// cover ±8.64 × 10^12 seconds.
    pub fn offset_at(&self, epoch_seconds: i64) -> (i32, bool) {
        let n = self.transitions.partition_point(|&t| t <= epoch_seconds);
        let after_last = self.transitions.last().is_none_or(|&t| epoch_seconds > t);
        let ty = match (&self.footer, n) {
            (Some(rule), _) if after_last => return rule.at(epoch_seconds),
            (_, 0) => self.types[0],
            _ => self.types[self.transition_types[n - 1] as usize],
        };
        (ty.offset, ty.is_dst)
    }
}

/// The zone of `name`: any name `jiff_tzdb::get` knows, a primary
/// identifier or a link, with ASCII case ignored. None if it knows no such
/// zone. Each zone is parsed on first use and kept for the life of the
/// process.
pub fn zone(name: &str) -> Option<&'static Zone> {
    /// Every name of jiff-tzdb, sorted, with its data and its parsed zone.
    type Table = Vec<(&'static str, &'static [u8], OnceLock<Zone>)>;
    static ZONES: OnceLock<Table> = OnceLock::new();
    let (name, _) = jiff_tzdb::get(name)?;
    let zones = ZONES.get_or_init(|| {
        let mut t: Table = jiff_tzdb::available()
            .map(|n| (n, jiff_tzdb::get(n).unwrap().1, OnceLock::new()))
            .collect();
        t.sort_by_key(|e| e.0);
        t
    });
    let i = zones.binary_search_by_key(&name, |e| e.0).ok()?;
    let (name, data, cell) = &zones[i];
    Some(cell.get_or_init(|| {
        // jiff-tzdb's data is ours: if it does not parse, the build is broken.
        Zone::parse(data).unwrap_or_else(|| panic!("jiff-tzdb's TZif data for {name} is malformed"))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::supported::{values, Kind};

    /// The epoch seconds of a UTC date and time.
    fn utc(y: i64, mo: u8, d: u8, h: i64, mi: i64, s: i64) -> i64 {
        days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + s
    }

    fn at(id: &str, t: i64) -> (i32, bool) {
        zone(id).unwrap().offset_at(t)
    }

    /// A TZif version 2 file with an empty version 1 block and the given
    /// transitions, local time types (offset, isdst) and footer.
    fn tzif(transitions: &[(i64, u8)], types: &[(i32, bool)], footer: &str) -> Vec<u8> {
        fn header(out: &mut Vec<u8>, counts: [u32; 6]) {
            out.extend_from_slice(b"TZif2");
            out.extend_from_slice(&[0; 15]);
            for c in counts {
                out.extend_from_slice(&c.to_be_bytes());
            }
        }
        let mut out = Vec::new();
        // isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt.
        header(&mut out, [0, 0, 0, 0, 1, 1]);
        out.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0]);
        let n = types.len() as u32;
        header(&mut out, [n, n, 0, transitions.len() as u32, n, 4]);
        for (t, _) in transitions {
            out.extend_from_slice(&t.to_be_bytes());
        }
        for (_, i) in transitions {
            out.push(*i);
        }
        for (offset, dst) in types {
            out.extend_from_slice(&offset.to_be_bytes());
            out.push(*dst as u8);
            out.push(0);
        }
        out.extend_from_slice(b"LMT\0");
        out.extend(std::iter::repeat_n(0, 2 * types.len()));
        out.push(b'\n');
        out.extend_from_slice(footer.as_bytes());
        out.push(b'\n');
        out
    }

    #[test]
    fn new_york_transitions() {
        let ny = "America/New_York";
        assert_eq!(at(ny, utc(2026, 3, 8, 6, 59, 59)), (-18000, false));
        assert_eq!(at(ny, utc(2026, 3, 8, 7, 0, 0)), (-14400, true));
        assert_eq!(at(ny, utc(2026, 11, 1, 5, 59, 59)), (-14400, true));
        assert_eq!(at(ny, utc(2026, 11, 1, 6, 0, 0)), (-18000, false));
        // The same instants in 2000, which the explicit transitions cover.
        assert_eq!(at(ny, utc(2000, 4, 2, 6, 59, 59)), (-18000, false));
        assert_eq!(at(ny, utc(2000, 4, 2, 7, 0, 0)), (-14400, true));
        assert_eq!(at(ny, utc(2000, 10, 29, 5, 59, 59)), (-14400, true));
        assert_eq!(at(ny, utc(2000, 10, 29, 6, 0, 0)), (-18000, false));
    }

    #[test]
    fn lord_howe_half_hour() {
        let lh = "Australia/Lord_Howe";
        assert_eq!(at(lh, utc(2026, 1, 15, 0, 0, 0)), (39600, true));
        assert_eq!(at(lh, utc(2026, 7, 15, 0, 0, 0)), (37800, false));
    }

    #[test]
    fn local_mean_time() {
        assert_eq!(
            at("America/New_York", utc(1800, 1, 1, 0, 0, 0)),
            (-17762, false)
        );
        assert_eq!(at("Asia/Kolkata", utc(2, 1, 1, 0, 0, 0)), (21208, false));
    }

    #[test]
    fn far_future_rule() {
        for y in [2100, 9999, 10000, 275000] {
            let ny = "America/New_York";
            assert_eq!(at(ny, utc(y, 7, 1, 0, 0, 0)), (-14400, true), "{y}");
            assert_eq!(at(ny, utc(y, 1, 1, 0, 0, 0)), (-18000, false), "{y}");
        }
        assert_eq!(
            at("Europe/London", utc(275000, 7, 1, 0, 0, 0)),
            (3600, true)
        );
        // The last instant of ECMAScript's range is in September 275760.
        assert_eq!(at("America/New_York", 8_640_000_000_000), (-14400, true));
        assert_eq!(at("Europe/London", 8_640_000_000_000), (3600, true));
    }

    #[test]
    fn far_past() {
        let ny = zone("America/New_York").unwrap();
        let first = (ny.types[0].offset, ny.types[0].is_dst);
        assert_eq!(first, (-17762, false));
        assert_eq!(ny.offset_at(-8_640_000_000_000), first);
        assert_eq!(ny.offset_at(i64::MIN), first);
    }

    #[test]
    fn rearguard_dublin() {
        let d = "Europe/Dublin";
        assert_eq!(at(d, utc(2026, 7, 1, 0, 0, 0)), (3600, true));
        assert_eq!(at(d, utc(2026, 1, 1, 0, 0, 0)), (0, false));
    }

    #[test]
    fn fixed_zones() {
        for t in [-8_640_000_000_000, -1, 0, 1_767_225_600, 8_640_000_000_000] {
            assert_eq!(at("UTC", t), (0, false), "{t}");
            assert_eq!(at("Etc/GMT+5", t), (-18000, false), "{t}");
        }
    }

    #[test]
    fn every_supported_zone_has_data() {
        let list = values(Kind::TimeZone);
        assert!(list.len() > 400, "{}", list.len());
        for id in &list {
            let z = zone(id).unwrap_or_else(|| panic!("no tzdb data for {id}"));
            for t in [-8_640_000_000_000, 0, 8_640_000_000_000] {
                let (offset, _) = z.offset_at(t);
                assert!(offset.abs() < 86400, "{id} {t} {offset}");
            }
        }
    }

    #[test]
    fn version() {
        assert_eq!(jiff_tzdb::VERSION, Some(TZDB_VERSION));
        assert_eq!(TZDB_VERSION, "2026e");
    }

    #[test]
    fn zone_lookup() {
        let a = zone("America/New_York").unwrap();
        // The same parsed zone each time, whatever the case of the name.
        assert!(std::ptr::eq(a, zone("america/new_york").unwrap()));
        assert!(zone("Asia/Kolkata").is_some());
        assert!(zone("UTC").is_some());
    }

    #[test]
    fn rejects() {
        assert!(zone("America/NewYork").is_none());
        assert!(zone("").is_none());
        let (_, data) = jiff_tzdb::get("America/New_York").unwrap();
        assert!(Zone::parse(data).is_some());
        // Every truncation fails.
        for n in 0..data.len() {
            assert!(Zone::parse(&data[..n]).is_none(), "{n}");
        }
        // Trailing bytes after the footer.
        let mut long = data.to_vec();
        long.push(b'x');
        assert!(Zone::parse(&long).is_none());
        // Bad magic, in either header, and version 1.
        let mut bad = data.to_vec();
        bad[0] = b'X';
        assert!(Zone::parse(&bad).is_none());
        let mut v1 = data.to_vec();
        v1[4] = 0;
        assert!(Zone::parse(&v1).is_none());
        let second = 44 + 7; // The empty version 1 block of slim data.
        assert_eq!(&data[second..second + 4], b"TZif");
        let mut bad2 = data.to_vec();
        bad2[second + 3] = b'g';
        assert!(Zone::parse(&bad2).is_none());
        // Footers that do not parse.
        let start = data[..data.len() - 1]
            .iter()
            .rposition(|&b| b == b'\n')
            .unwrap();
        assert_eq!(&data[start..], b"\nEST5EDT,M3.2.0,M11.1.0\n");
        for footer in [
            "EST",
            "EST5EDT",
            "EST5EDT,M13.2.0,M11.1.0",
            "EST5EDT,M3.6.0,M11.1.0",
            "EST5EDT,M3.2.7,M11.1.0",
            "EST5EDT,M3.2.0",
            "EST5EDT,M3.2.0,M11.1.0,",
            "EST5EDT,J0,J365",
            "EST5EDT,J1,J366",
            "EST5EDT,0,366",
            "EST5EDT,M3.2.0/168,M11.1.0",
            "EST5EDT,M3.2.0/-168,M11.1.0",
            "EST5EDT,M3.2.0/2:60,M11.1.0",
            "EST25",
            "EST5:60",
            "ES5",
            "<ES>5",
            "<EST5",
            "<E$T>5",
            "EST5 ",
            "EST\u{e9}5",
        ] {
            let mut d = data[..start + 1].to_vec();
            d.extend_from_slice(footer.as_bytes());
            d.push(b'\n');
            assert!(Zone::parse(&d).is_none(), "{footer}");
        }
        // The synthetic files parse when well formed...
        let good = tzif(&[(0, 1)], &[(-17762, false), (-18000, false)], "EST5");
        assert!(Zone::parse(&good).is_some());
        // ...but not with a transition type out of range, transitions out of
        // order, an isdst that is not 0 or 1, or no local time types.
        let types = [(-17762, false), (-18000, false)];
        assert!(Zone::parse(&tzif(&[(0, 2)], &types, "EST5")).is_none());
        assert!(Zone::parse(&tzif(&[(5, 1), (5, 0)], &types, "EST5")).is_none());
        let mut dst = good.clone();
        let isdst = good.len() - 6 - 4 - 4 - 2;
        assert_eq!(dst[isdst], 0);
        dst[isdst] = 2;
        assert!(Zone::parse(&dst).is_none());
        assert!(Zone::parse(&tzif(&[], &[], "EST5")).is_none());
        // Counts far larger than the data (timecnt; typecnt, with no
        // indicators) are rejected by the header check, before anything is
        // allocated for them.
        let counts = 44 + 7 + 20;
        let mut huge = good.clone();
        huge[counts + 12..counts + 16].copy_from_slice(&u32::MAX.to_be_bytes());
        let mut huge_types = good.clone();
        huge_types[counts..counts + 8].fill(0);
        huge_types[counts + 16..counts + 20].copy_from_slice(&u32::MAX.to_be_bytes());
        for forged in [&huge, &huge_types] {
            assert_eq!(
                read_headers(forged).err(),
                Some(HeaderError::CountsExceedData)
            );
            assert!(Zone::parse(forged).is_none());
        }
        assert!(read_headers(&good).is_ok());
        assert!(read_headers(data).is_ok());
        assert_eq!(
            read_headers(&good[..40]).err(),
            Some(HeaderError::Malformed)
        );
        // The boundary: an empty footer is the two newlines, so the block
        // and the footer fill the data exactly; one byte less is rejected.
        let exact = tzif(&[(0, 1)], &types, "");
        assert!(Zone::parse(&exact).is_some());
        assert_eq!(
            read_headers(&exact[..exact.len() - 1]).err(),
            Some(HeaderError::CountsExceedData)
        );
        assert!(Zone::parse(&tzif(&[], &[(i32::MIN, false)], "EST5")).is_none());
    }

    #[test]
    fn synthetic_zones() {
        let types = [(-17762, false), (-18000, false), (-14400, true)];
        // Before the first transition: type 0. At a transition: its type.
        // After the last: the footer.
        let z = Zone::parse(&tzif(
            &[(-100, 1), (100, 2)],
            &types,
            "<-04>4<-03>,M3.2.0,M11.1.0",
        ))
        .unwrap();
        assert_eq!(z.offset_at(-101), (-17762, false));
        assert_eq!(z.offset_at(-100), (-18000, false));
        assert_eq!(z.offset_at(99), (-18000, false));
        assert_eq!(z.offset_at(100), (-14400, true));
        // 1970-01-01 is winter in the footer's rule.
        assert_eq!(z.offset_at(101), (-14400, false));
        assert_eq!(z.offset_at(utc(1970, 7, 1, 0, 0, 0)), (-10800, true));
        // No transitions: the footer for every instant.
        let z = Zone::parse(&tzif(&[], &types, "<-04>4<-03>,M3.2.0,M11.1.0")).unwrap();
        // -8.64e12 is April 20 of -271821, in summer.
        assert_eq!(z.offset_at(-8_640_000_000_000), (-10800, true));
        assert_eq!(z.offset_at(utc(-271821, 1, 1, 0, 0, 0)), (-14400, false));
        assert_eq!(z.offset_at(utc(1800, 7, 1, 0, 0, 0)), (-10800, true));
        // An empty footer: the last transition's type continues.
        let z = Zone::parse(&tzif(&[(-100, 1), (100, 2)], &types, "")).unwrap();
        assert_eq!(z.offset_at(8_640_000_000_000), (-14400, true));
        let z = Zone::parse(&tzif(&[], &types, "")).unwrap();
        assert_eq!(z.offset_at(8_640_000_000_000), (-17762, false));
    }

    /// The UTC offset and DST flag of a footer at an instant.
    fn rule_at(footer: &str, t: i64) -> (i32, bool) {
        parse_footer(footer.as_bytes()).unwrap().unwrap().at(t)
    }

    #[test]
    fn footer_names_and_offsets() {
        assert_eq!(rule_at("EST5", 0), (-18000, false));
        assert_eq!(rule_at("<-05>5", 0), (-18000, false));
        assert_eq!(rule_at("<+0545>-5:45", 0), (20700, false));
        assert_eq!(rule_at("<+0545>-05:45", 0), (20700, false));
        assert_eq!(rule_at("<-0930>9:30", 0), (-34200, false));
        assert_eq!(rule_at("LMT+0:01:15", 0), (-75, false));
        assert_eq!(rule_at("ABC-24", 0), (86400, false));
        assert_eq!(rule_at("<+1030-x>-10:30", 0), (37800, false));
        // A DST name with no offset is one hour ahead of standard time.
        let july = utc(2026, 7, 1, 0, 0, 0);
        assert_eq!(rule_at("CET-1CEST,M3.5.0,M10.5.0/3", july), (7200, true));
        assert_eq!(rule_at("<-03>3<-02>,M3.2.0,M11.1.0", july), (-7200, true));
        // An explicit DST offset.
        assert_eq!(
            rule_at("<+1030>-10:30<+11>-11,M10.1.0,M4.1.0", 0),
            (39600, true)
        );
        assert_eq!(rule_at("XXX3YYY1,M3.2.0,M11.1.0", july), (-3600, true));
        assert_eq!(parse_footer(b"").unwrap(), None);
    }

    #[test]
    fn footer_rule_dates() {
        // M10.5.0 is the last Sunday of October; M3.5.0 the last of March.
        // 2026: March 29, October 25. Times default to 02:00.
        let eu = "CET-1CEST,M3.5.0,M10.5.0/3";
        assert_eq!(rule_at(eu, utc(2026, 3, 29, 0, 59, 59)), (3600, false));
        assert_eq!(rule_at(eu, utc(2026, 3, 29, 1, 0, 0)), (7200, true));
        assert_eq!(rule_at(eu, utc(2026, 10, 25, 0, 59, 59)), (7200, true));
        assert_eq!(rule_at(eu, utc(2026, 10, 25, 1, 0, 0)), (3600, false));
        // A fifth week that the month has: 2026-05-31 is the fifth Sunday.
        let five = "ABC0DEF,M5.5.0/0,M12.1.0/0";
        assert_eq!(rule_at(five, utc(2026, 5, 30, 23, 59, 59)), (0, false));
        assert_eq!(rule_at(five, utc(2026, 5, 31, 0, 0, 0)), (3600, true));
        // A fifth week that the month lacks is the last: February 2026 has
        // four Sundays, the last on the 22nd.
        let four = "ABC0DEF,M2.5.0/0,M12.1.0/0";
        assert_eq!(rule_at(four, utc(2026, 2, 21, 23, 59, 59)), (0, false));
        assert_eq!(rule_at(four, utc(2026, 2, 22, 0, 0, 0)), (3600, true));
        // Jn never counts February 29: J60 is March 1 in a leap year.
        let j = "ABC0DEF,J60/0,J300/0";
        assert_eq!(rule_at(j, utc(2024, 2, 29, 23, 59, 59)), (0, false));
        assert_eq!(rule_at(j, utc(2024, 3, 1, 0, 0, 0)), (3600, true));
        assert_eq!(rule_at(j, utc(2025, 2, 28, 23, 59, 59)), (0, false));
        assert_eq!(rule_at(j, utc(2025, 3, 1, 0, 0, 0)), (3600, true));
        // n is zero-based and counts February 29: 59 is February 29 in a
        // leap year and March 1 otherwise.
        let n = "ABC0DEF,59/0,300/0";
        assert_eq!(rule_at(n, utc(2024, 2, 28, 23, 59, 59)), (0, false));
        assert_eq!(rule_at(n, utc(2024, 2, 29, 0, 0, 0)), (3600, true));
        assert_eq!(rule_at(n, utc(2025, 2, 28, 23, 59, 59)), (0, false));
        assert_eq!(rule_at(n, utc(2025, 3, 1, 0, 0, 0)), (3600, true));
        // The end: J300 is October 27, 300 is October 27 (leap) or 28.
        assert_eq!(rule_at(j, utc(2024, 10, 26, 22, 59, 59)), (3600, true));
        assert_eq!(rule_at(j, utc(2024, 10, 26, 23, 0, 0)), (0, false));
        assert_eq!(rule_at(n, utc(2025, 10, 27, 22, 59, 59)), (3600, true));
        assert_eq!(rule_at(n, utc(2025, 10, 27, 23, 0, 0)), (0, false));
    }

    #[test]
    fn footer_rule_times() {
        // Times past 24 hours and negative ones (RFC 8536 3.3.1). Israel's
        // M3.4.4/26 is the fourth Thursday of March at 26:00, which is
        // Friday 02:00 (2026-03-27 00:00Z).
        let il = "IST-2IDT,M3.4.4/26,M10.5.0";
        assert_eq!(rule_at(il, utc(2026, 3, 26, 23, 59, 59)), (7200, false));
        assert_eq!(rule_at(il, utc(2026, 3, 27, 0, 0, 0)), (10800, true));
        // America/Godthab's M3.5.0/-1 is Saturday 23:00 at -02.
        let gl = "<-02>2<-01>,M3.5.0/-1,M10.5.0/0";
        assert_eq!(rule_at(gl, utc(2026, 3, 29, 0, 59, 59)), (-7200, false));
        assert_eq!(rule_at(gl, utc(2026, 3, 29, 1, 0, 0)), (-3600, true));
        // Its end, M10.5.0/0 at -01, is 2026-10-25 01:00Z.
        assert_eq!(rule_at(gl, utc(2026, 10, 25, 0, 59, 59)), (-3600, true));
        assert_eq!(rule_at(gl, utc(2026, 10, 25, 1, 0, 0)), (-7200, false));
        // The extremes, 167 hours either way, with minutes and seconds.
        let x = "ABC0DEF,M6.1.0/167,M8.1.0/-167:59:59";
        // The first Sunday of June 2026 is June 7; +167 h is June 13 23:00.
        assert_eq!(rule_at(x, utc(2026, 6, 13, 22, 59, 59)), (0, false));
        assert_eq!(rule_at(x, utc(2026, 6, 13, 23, 0, 0)), (3600, true));
        // The first Sunday of August is August 2; -167:59:59 local DST
        // is July 26 00:00:01 local, July 25 23:00:01Z.
        assert_eq!(rule_at(x, utc(2026, 7, 25, 23, 0, 0)), (3600, true));
        assert_eq!(rule_at(x, utc(2026, 7, 25, 23, 0, 1)), (0, false));
    }

    #[test]
    fn footer_southern_and_all_year() {
        // Sydney: DST from the first Sunday of October to the first Sunday
        // of April, across the new year.
        let syd = "AEST-10AEDT,M10.1.0,M4.1.0/3";
        for y in [-275000, -1, 0, 1, 2026, 275000] {
            for (mo, d) in [(1, 1), (12, 31), (2, 15)] {
                let t = utc(y, mo, d, 0, 0, 0);
                assert_eq!(rule_at(syd, t), (39600, true), "{y}-{mo}-{d}");
            }
            assert_eq!(rule_at(syd, utc(y, 7, 1, 0, 0, 0)), (36000, false), "{y}");
            // The last instant of a year and the first of the next.
            let t = utc(y + 1, 1, 1, 0, 0, 0);
            assert_eq!(rule_at(syd, t - 1), (39600, true), "{y}");
        }
        // 2026: DST ends April 5 03:00 AEDT (April 4 16:00Z) and starts
        // October 4 02:00 AEST (October 3 16:00Z).
        assert_eq!(rule_at(syd, utc(2026, 4, 4, 15, 59, 59)), (39600, true));
        assert_eq!(rule_at(syd, utc(2026, 4, 4, 16, 0, 0)), (36000, false));
        assert_eq!(rule_at(syd, utc(2026, 10, 3, 15, 59, 59)), (36000, false));
        assert_eq!(rule_at(syd, utc(2026, 10, 3, 16, 0, 0)), (39600, true));
        // DST all year (RFC 8536 3.3.1's example).
        let all = "EST5EDT,0/0,J365/25";
        for y in [-275000, 1999, 2000, 2024, 2026, 275000] {
            for (mo, d, h) in [(1, 1, 0), (1, 1, 5), (6, 1, 0), (12, 31, 23)] {
                let t = utc(y, mo, d, h, 0, 0);
                assert_eq!(rule_at(all, t), (-14400, true), "{y}-{mo}-{d} {h}");
            }
        }
    }

    #[test]
    fn civil_days() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
        // Rata Die 1 is 0001-01-01.
        assert_eq!(days_from_civil(1, 1, 1) + 719163, 1);
        assert_eq!(days_from_civil(0, 3, 1) - days_from_civil(0, 2, 28), 2);
        assert_eq!(days_from_civil(-1, 3, 1) - days_from_civil(-1, 2, 28), 1);
        // ECMAScript's first and last days.
        assert_eq!(days_from_civil(-271821, 4, 20), -100_000_000);
        assert_eq!(days_from_civil(275760, 9, 13), 100_000_000);
        // Year of a day, at each side of every new year in ±300,000.
        for y in (-300_000..=300_000).step_by(997) {
            let d = days_from_civil(y, 1, 1);
            assert_eq!(year_of_day(d), y);
            assert_eq!(year_of_day(d - 1), y - 1);
            assert_eq!(days_from_civil(y, 12, 31) - d, 364 + is_leap(y) as i64);
        }
        assert!(is_leap(2000) && is_leap(2024) && is_leap(0) && is_leap(-4));
        assert!(!is_leap(1900) && !is_leap(2026) && !is_leap(-1) && !is_leap(-100));
    }

    #[test]
    fn rule_extreme_instants() {
        // No overflow at the ends of i64.
        let syd = "AEST-10AEDT,M10.1.0,M4.1.0/3";
        for t in [i64::MIN, i64::MIN + 1, i64::MAX - 1, i64::MAX] {
            let (offset, _) = rule_at(syd, t);
            assert!(offset == 36000 || offset == 39600);
            let (offset, _) = at("America/New_York", t);
            assert!(offset == -18000 || offset == -14400 || offset == -17762);
        }
    }
}
