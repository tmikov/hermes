// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The CLDR number data ICU4X 2.3 lacks (decision 4 of the Phase 3 plan),
//! from five generated files in core/data, each parsed once on first use:
//!
//! - `number-symbols.txt`: per locale and numbering system, the percent
//!   sign, exponent symbol, infinity, NaN, approximately sign, percent
//!   pattern, range pattern, currency separators and time separator;
//! - `units.txt`: the unit patterns, by locale, width, unit and plural key,
//!   each locale holding only what differs from its parents;
//! - `currency-symbols.txt`: the currency symbols and narrow symbols, which
//!   ICU4X leaves out where they are the currency's code, and the
//!   per-currency formats (pattern, decimal and grouping separators), in the
//!   same form;
//! - `explicit-number-systems.txt`: the explicit symbols, percent, range
//!   and currency patterns that CLDR's XML (root and the locales) has for
//!   numbering systems outside a locale's default and other numbering
//!   systems, which neither cldr-json nor ICU4X has;
//! - `compact-currency.txt`: the short (compact) currency patterns, CLDR's
//!   currencyFormatLength short, which ICU4X does not have, each locale
//!   holding only what differs from its parents.
//!
//! The entry for a locale is the first one ICU4X's LocaleFallbacker reaches
//! from its language identifier (decision 7); every locale of locales.txt is
//! a key in each file. The files are ours, so a malformed line is a panic.

use icu::locale::fallback::{LocaleFallbackConfig, LocaleFallbacker};
use icu::locale::LanguageIdentifier;
use icu_provider::DataLocale;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The lines of a generated file after its leading `#` comment lines, as
/// they are: a value may begin or end with a space.
pub(crate) fn body_lines(text: &'static str) -> impl Iterator<Item = &'static str> {
    text.lines().skip_while(|l| l.starts_with('#'))
}

/// The string table that opens a generated file: a `strings <count>` line
/// and that many lines, taken from `lines` as they are (a string may begin
/// or end with a space). `file` names the file in panics.
pub(crate) fn string_table(
    lines: &mut impl Iterator<Item = &'static str>,
    file: &str,
) -> Vec<&'static str> {
    let count: usize = lines
        .next()
        .and_then(|l| l.strip_prefix("strings "))
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{file}: no string count"));
    let strings: Vec<&'static str> = lines.take(count).collect();
    assert_eq!(strings.len(), count, "{file}: short string table");
    strings
}

/// The parent of the locale `tag` from its `locale <tag> <parent>` line: None
/// for `-`. Panics unless `parent` is `-` or satisfies `known` (a locale
/// already read: parents come first). `file` names the file in panics.
pub(crate) fn locale_parent(
    file: &str,
    tag: &str,
    parent: &'static str,
    known: impl Fn(&str) -> bool,
) -> Option<&'static str> {
    let parent = (parent != "-").then_some(parent);
    assert!(
        parent.is_none_or(known),
        "{file}: {tag} comes before its parent"
    );
    parent
}

/// The first locale ICU4X's locale fallback reaches from `id` for which
/// `has` is true, as a tag. Panics if the fallback reaches `und` and `has`
/// is false for it too: every file has an `und` entry.
pub(crate) fn fallback_key(id: &LanguageIdentifier, has: impl Fn(&str) -> bool) -> String {
    let fallbacker = LocaleFallbacker::new().for_config(LocaleFallbackConfig::default());
    let mut it = fallbacker.fallback_for(DataLocale::from(id));
    loop {
        let tag = it.get().to_string();
        if has(&tag) {
            return tag;
        }
        assert!(!it.get().is_unknown(), "data file: no und entry");
        it.step();
    }
}

/// One line of number-symbols.txt: the symbols and patterns CLDR has for a
/// locale and numbering system, as CLDR writes them.
#[derive(Debug, PartialEq)]
pub(crate) struct NumberSymbols {
    /// The numbering system.
    pub nu: &'static str,
    /// `percentSign`.
    pub percent_sign: &'static str,
    /// `exponential`, the exponent symbol of scientific notation.
    pub exponential: &'static str,
    /// `infinity`.
    pub infinity: &'static str,
    /// `nan`.
    pub nan: &'static str,
    /// `approximatelySign`.
    pub approximately_sign: &'static str,
    /// The standard percent pattern in CLDR's syntax (`#,##0%`).
    pub percent_pattern: &'static str,
    /// The range pattern (`{0}–{1}`).
    pub range_pattern: &'static str,
    /// `currencyDecimal`, the decimal separator of currency formats; empty
    /// where the locale has none of its own (only fr-CH has one).
    pub currency_decimal: &'static str,
    /// `currencyGroup`, the grouping separator of currency formats; empty
    /// where the locale has none of its own (only de-AT has one).
    pub currency_group: &'static str,
    /// `timeSeparator`, between hours, minutes and seconds (`:`; da's
    /// latn `.`).
    pub time_separator: &'static str,
}

/// number-symbols.txt by locale.
type SymbolsTable = HashMap<&'static str, Vec<NumberSymbols>>;

/// Parse number-symbols.txt.
fn parse_symbols(text: &'static str) -> SymbolsTable {
    let mut table = SymbolsTable::new();
    for line in body_lines(text) {
        let f: Vec<&'static str> = line.split('\t').collect();
        let [locale, nu, percent_sign, exponential, infinity, nan, approximately_sign, percent_pattern, range_pattern, currency_decimal, currency_group, time_separator] =
            f[..]
        else {
            panic!("number-symbols.txt: not 12 fields: {line:?}");
        };
        let rows = table.entry(locale).or_default();
        assert!(
            rows.iter().all(|r| r.nu != nu),
            "number-symbols.txt: repeated {locale} {nu}"
        );
        rows.push(NumberSymbols {
            nu,
            percent_sign,
            exponential,
            infinity,
            nan,
            approximately_sign,
            percent_pattern,
            range_pattern,
            currency_decimal,
            currency_group,
            time_separator,
        });
    }
    assert!(table.contains_key("und"), "number-symbols.txt: no und");
    table
}

/// The parsed number-symbols.txt.
fn symbols_table() -> &'static SymbolsTable {
    static T: OnceLock<SymbolsTable> = OnceLock::new();
    T.get_or_init(|| parse_symbols(include_str!("../data/number-symbols.txt")))
}

/// The locale of number-symbols.txt whose lines `id` uses (decision 7).
pub(crate) fn symbols_locale(id: &LanguageIdentifier) -> String {
    let table = symbols_table();
    fallback_key(id, |t| table.contains_key(t))
}

/// The lines of number-symbols.txt for `id`, one per numbering system, in
/// the file's order (sorted by numbering system). Never empty.
pub(crate) fn number_symbols(id: &LanguageIdentifier) -> &'static [NumberSymbols] {
    &symbols_table()[symbols_locale(id).as_str()]
}

/// The symbols of explicit-number-systems.txt, in the order of its lines.
pub(crate) const EXPLICIT_DECIMAL: usize = 0;
pub(crate) const EXPLICIT_GROUP: usize = 1;
pub(crate) const EXPLICIT_MINUS_SIGN: usize = 2;
pub(crate) const EXPLICIT_PLUS_SIGN: usize = 3;
pub(crate) const EXPLICIT_PERCENT_SIGN: usize = 4;
pub(crate) const EXPLICIT_EXPONENTIAL: usize = 5;
pub(crate) const EXPLICIT_INFINITY: usize = 6;
pub(crate) const EXPLICIT_NAN: usize = 7;
pub(crate) const EXPLICIT_APPROXIMATELY_SIGN: usize = 8;
pub(crate) const EXPLICIT_TIME_SEPARATOR: usize = 9;
/// The number of symbols of a symbols line of explicit-number-systems.txt.
const EXPLICIT_SYMBOLS: usize = 10;

/// The currency patterns of a currency line of explicit-number-systems.txt,
/// in its order: standard, its alphaNextToNumber alternate, accounting
/// (or "=standard" for an alias to the standard pattern), and its
/// alternate.
pub(crate) const CURRENCY_STANDARD: usize = 0;
pub(crate) const CURRENCY_STANDARD_ALPHA: usize = 1;
pub(crate) const CURRENCY_ACCOUNTING: usize = 2;
pub(crate) const CURRENCY_ACCOUNTING_ALPHA: usize = 3;
/// The accounting pattern of a locale whose accounting pattern aliases to
/// the standard one, which is then the resolved standard pattern.
pub(crate) const ACCOUNTING_IS_STANDARD: &str = "=standard";

/// The explicit data one locale of explicit-number-systems.txt has for
/// numbering systems that cldr-json leaves out; an empty value is not the
/// locale's own.
#[derive(Default)]
struct ExplicitLocale {
    /// The CLDR parent of the locale; None for und.
    parent: Option<&'static str>,
    /// The symbols (EXPLICIT_DECIMAL and the rest) by numbering system.
    symbols: HashMap<&'static str, [&'static str; EXPLICIT_SYMBOLS]>,
    /// The percent pattern by numbering system.
    percent: HashMap<&'static str, &'static str>,
    /// The range pattern by numbering system.
    range: HashMap<&'static str, &'static str>,
    /// The currency patterns (CURRENCY_STANDARD and the rest) by numbering
    /// system.
    currency: HashMap<&'static str, [&'static str; 4]>,
}

/// explicit-number-systems.txt by locale.
type ExplicitTable = HashMap<&'static str, ExplicitLocale>;

/// Parse explicit-number-systems.txt.
fn parse_explicit(text: &'static str) -> ExplicitTable {
    let mut table = ExplicitTable::new();
    let mut current: Option<&'static str> = None;
    for line in body_lines(text) {
        let f: Vec<&'static str> = line.split('\t').collect();
        if let ["locale", tag, parent] = f[..] {
            let parent = (parent != "-").then_some(parent);
            assert!(
                parent.is_none_or(|p| table.contains_key(p)),
                "explicit-number-systems.txt: {tag} comes before its parent"
            );
            let fresh = ExplicitLocale {
                parent,
                ..Default::default()
            };
            assert!(
                table.insert(tag, fresh).is_none(),
                "explicit-number-systems.txt: repeated locale {tag}"
            );
            current = Some(tag);
            continue;
        }
        let tag = current.expect("explicit-number-systems.txt: entry before any locale");
        let entry = table.get_mut(tag).unwrap();
        let fresh = match f[..] {
            ["symbols", nu, a, b, c, d, e, g, h, i, j, k] => entry
                .symbols
                .insert(nu, [a, b, c, d, e, g, h, i, j, k])
                .is_none(),
            ["percent", nu, pattern] => entry.percent.insert(nu, pattern).is_none(),
            ["range", nu, pattern] => entry.range.insert(nu, pattern).is_none(),
            ["currency", nu, a, b, c, d] => entry.currency.insert(nu, [a, b, c, d]).is_none(),
            _ => panic!("explicit-number-systems.txt: bad line {line:?}"),
        };
        assert!(
            fresh,
            "explicit-number-systems.txt: repeated {tag} {line:?}"
        );
    }
    assert!(
        table.contains_key("und"),
        "explicit-number-systems.txt: no und"
    );
    table
}

/// The parsed explicit-number-systems.txt.
fn explicit_table() -> &'static ExplicitTable {
    static T: OnceLock<ExplicitTable> = OnceLock::new();
    T.get_or_init(|| parse_explicit(include_str!("../data/explicit-number-systems.txt")))
}

/// The locales of explicit-number-systems.txt from the entry of `id`
/// (decision 7) along its CLDR parents to und.
fn explicit_chain(id: &LanguageIdentifier) -> impl Iterator<Item = &'static ExplicitLocale> {
    let table = explicit_table();
    let mut next = Some(&table[fallback_key(id, |t| table.contains_key(t)).as_str()]);
    std::iter::from_fn(move || {
        let entry = next?;
        next = entry.parent.map(|p| &table[p]);
        Some(entry)
    })
}

/// CLDR's explicit symbol `index` (EXPLICIT_DECIMAL and the rest) of the
/// numbering system `nu` for `id`, for a numbering system cldr-json and
/// ICU4X do not have for `id`: the value of the nearest locale of the chain,
/// root included, that has one. None where the chain has none: there the
/// value is the locale's latn one, which root's aliases point to.
pub(crate) fn explicit_symbol(
    id: &LanguageIdentifier,
    nu: &str,
    index: usize,
) -> Option<&'static str> {
    explicit_chain(id)
        .filter_map(|l| l.symbols.get(nu).map(|s| s[index]))
        .find(|s| !s.is_empty())
}

/// CLDR's explicit percent pattern of `nu` for `id`, as explicit_symbol
/// finds a symbol.
pub(crate) fn explicit_percent_pattern(id: &LanguageIdentifier, nu: &str) -> Option<&'static str> {
    explicit_chain(id).find_map(|l| l.percent.get(nu).copied())
}

/// CLDR's explicit range pattern of `nu` for `id`, as explicit_symbol finds
/// a symbol.
pub(crate) fn explicit_range_pattern(id: &LanguageIdentifier, nu: &str) -> Option<&'static str> {
    explicit_chain(id).find_map(|l| l.range.get(nu).copied())
}

/// CLDR's explicit currency pattern `index` (CURRENCY_STANDARD and the
/// rest) of `nu` for `id`, as explicit_symbol finds a symbol; the
/// accounting pattern may be ACCOUNTING_IS_STANDARD.
pub(crate) fn explicit_currency_pattern(
    id: &LanguageIdentifier,
    nu: &str,
    index: usize,
) -> Option<&'static str> {
    explicit_chain(id)
        .filter_map(|l| l.currency.get(nu).map(|c| c[index]))
        .find(|s| !s.is_empty())
}

/// Whether root has explicit currency patterns for `nu`, so that a pattern
/// the chain does not have is not the locale's latn one (root's other
/// numbering systems alias to latn).
pub(crate) fn root_has_currency_patterns(nu: &str) -> bool {
    explicit_table()["und"].currency.contains_key(nu)
}

/// Whether root has explicit symbols for `nu` (arab, arabext), so that a
/// symbol the chain does not have is missing rather than the locale's latn
/// one (root's other numbering systems alias their symbols to latn).
pub(crate) fn root_has_symbols(nu: &str) -> bool {
    explicit_table()["und"].symbols.contains_key(nu)
}

/// The plural counts of compact-currency.txt, in the order of its lines.
pub(crate) const SHORT_CURRENCY_COUNTS: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

/// One pattern of compact-currency.txt: a short currency pattern of CLDR
/// in its syntax, with its run of digits written as one `0`, and that
/// run's length; the pattern `0` (no compact form) has length 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShortPattern {
    /// The pattern, its digits as one `0`.
    pub text: &'static str,
    /// The number of digits.
    pub zeros: u8,
}

/// A short currency pattern of one magnitude and plural count, and its
/// alphaNextToNumber alternate where it has one that differs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShortCurrencyPattern {
    /// The pattern.
    pub plain: ShortPattern,
    /// The alternate.
    pub alt: Option<ShortPattern>,
}

/// The short currency patterns of a numbering system, by the power of ten
/// of the magnitude and the index of the plural count in
/// SHORT_CURRENCY_COUNTS.
pub(crate) type ShortCurrencyTable = std::collections::BTreeMap<(u8, usize), ShortCurrencyPattern>;

/// The patterns one locale of compact-currency.txt has of its own.
struct CompactCurrencyLocale {
    /// The locale the missing patterns are inherited from; None for und.
    parent: Option<&'static str>,
    /// The patterns by numbering system.
    patterns: HashMap<&'static str, ShortCurrencyTable>,
}

/// compact-currency.txt by locale.
type CompactCurrencyFile = HashMap<&'static str, CompactCurrencyLocale>;

/// Parse compact-currency.txt.
fn parse_compact_currency(text: &'static str) -> CompactCurrencyFile {
    let mut lines = body_lines(text);
    let strings = string_table(&mut lines, "compact-currency.txt");
    let string = |i: &str| -> &'static str {
        let i: usize = i.parse().expect("compact-currency.txt: bad index");
        strings[i]
    };
    let mut table = CompactCurrencyFile::new();
    let mut current: Option<&'static str> = None;
    for line in lines {
        let f: Vec<&'static str> = line.split('\t').collect();
        if let ["locale", tag, parent] = f[..] {
            let parent = locale_parent("compact-currency.txt", tag, parent, |p| {
                table.contains_key(p)
            });
            let fresh = CompactCurrencyLocale {
                parent,
                patterns: HashMap::new(),
            };
            assert!(
                table.insert(tag, fresh).is_none(),
                "compact-currency.txt: repeated locale {tag}"
            );
            current = Some(tag);
            continue;
        }
        let [nu, span, zeros, items] = f[..] else {
            panic!("compact-currency.txt: bad line {line:?}");
        };
        let tag = current.expect("compact-currency.txt: entry before any locale");
        let (first, last) = span.split_once('-').unwrap_or((span, span));
        let (first, last): (u8, u8) = (first.parse().unwrap(), last.parse().unwrap());
        let zeros: u8 = zeros.parse().expect("compact-currency.txt: bad digits");
        let patterns = table.get_mut(tag).unwrap().patterns.entry(nu).or_default();
        for item in items.split(' ') {
            let (count, refs) = item
                .split_once('=')
                .expect("compact-currency.txt: bad item");
            let count = SHORT_CURRENCY_COUNTS
                .iter()
                .position(|c| *c == count)
                .expect("compact-currency.txt: bad count");
            let (plain, alt) = match refs.split_once(',') {
                Some((p, a)) => (p, Some(a)),
                None => (refs, None),
            };
            let alt = alt.map(|a| match a.find(['+', '-']) {
                Some(i) => (&a[..i], a[i..].parse::<i8>().unwrap()),
                None => (a, 0),
            });
            for power in first..=last {
                let step = if zeros == 0 { 0 } else { power - first };
                let shape = |text: &'static str, delta: i8| ShortPattern {
                    text,
                    zeros: if text == "0" {
                        0
                    } else {
                        (zeros + step).checked_add_signed(delta).unwrap()
                    },
                };
                let pattern = ShortCurrencyPattern {
                    plain: shape(string(plain), 0),
                    alt: alt.map(|(a, delta)| shape(string(a), delta)),
                };
                assert!(
                    patterns.insert((power, count), pattern).is_none(),
                    "compact-currency.txt: repeated {tag} {nu} {power} {count}"
                );
            }
        }
    }
    assert!(table.contains_key("und"), "compact-currency.txt: no und");
    table
}

/// The parsed compact-currency.txt.
fn compact_currency_file() -> &'static CompactCurrencyFile {
    static T: OnceLock<CompactCurrencyFile> = OnceLock::new();
    T.get_or_init(|| parse_compact_currency(include_str!("../data/compact-currency.txt")))
}

/// The short currency patterns of `nu` for `id`, as the generator resolves
/// them: latn's are the nearest along the chain of compact-currency.txt's
/// parents; another numbering system's are the chain's for it, and latn's
/// where the chain has none (root aliases them to latn, and a numbering
/// system with explicit currency formats but no short ones, root's arab,
/// takes latn's too, as ICU4C does).
pub(crate) fn short_currency_patterns(id: &LanguageIdentifier, nu: &str) -> ShortCurrencyTable {
    let file = compact_currency_file();
    let merged = |nu: &str| {
        let mut out = ShortCurrencyTable::new();
        let mut entry = Some(&file[fallback_key(id, |t| file.contains_key(t)).as_str()]);
        while let Some(e) = entry {
            for (k, v) in e.patterns.get(nu).into_iter().flatten() {
                out.entry(*k).or_insert(*v);
            }
            entry = e.parent.map(|p| &file[p]);
        }
        out
    };
    let mut table = merged("latn");
    if nu != "latn" {
        table.extend(merged(nu));
    }
    table
}

/// The symbols one locale of currency-symbols.txt has of its own.
struct CurrenciesLocale {
    /// The locale the missing symbols are inherited from; None for und.
    parent: Option<&'static str>,
    /// Currency code to [symbol, narrow symbol, pattern, decimal
    /// separator, grouping separator]; an empty one is inherited (the
    /// symbols) or not the locale's own (the format fields).
    symbols: HashMap<&'static str, [&'static str; 5]>,
    /// The locale's own values that a per-currency format field inherits
    /// laterally from: (field, numbering system, "standard", "accounting"
    /// or "-"), with the field CURRENCY_FIELD_PATTERN and the rest.
    lateral: Vec<(usize, &'static str, &'static str)>,
}

/// currency-symbols.txt by locale.
type CurrenciesTable = HashMap<&'static str, CurrenciesLocale>;

/// Parse currency-symbols.txt.
fn parse_currencies(text: &'static str) -> CurrenciesTable {
    let mut table = CurrenciesTable::new();
    let mut current: Option<&'static str> = None;
    for line in body_lines(text) {
        let f: Vec<&'static str> = line.split('\t').collect();
        if let ["lateral", field, nu, kind] = f[..] {
            let tag = current.expect("currency-symbols.txt: entry before any locale");
            let field = match field {
                "pattern" => CURRENCY_FIELD_PATTERN,
                "decimal" => CURRENCY_FIELD_DECIMAL,
                "group" => CURRENCY_FIELD_GROUP,
                _ => panic!("currency-symbols.txt: bad line {line:?}"),
            };
            assert!(
                matches!(kind, "standard" | "accounting") == (field == CURRENCY_FIELD_PATTERN)
                    && (kind == "-") != (field == CURRENCY_FIELD_PATTERN),
                "currency-symbols.txt: bad line {line:?}"
            );
            table.get_mut(tag).unwrap().lateral.push((field, nu, kind));
            continue;
        }
        let (first, a, b, formats) = match f[..] {
            [first, a, b] => (first, a, b, ["", "", ""]),
            [first, a, b, p, d, g] if first != "locale" => (first, a, b, [p, d, g]),
            _ => panic!("currency-symbols.txt: not 3 or 6 fields: {line:?}"),
        };
        if first == "locale" {
            let parent = (b != "-").then_some(b);
            assert!(
                parent.is_none_or(|p| table.contains_key(p)),
                "currency-symbols.txt: {a} comes before its parent"
            );
            assert_eq!(
                parent.is_none(),
                a == "und",
                "currency-symbols.txt: {line:?}"
            );
            let fresh = CurrenciesLocale {
                parent,
                symbols: HashMap::new(),
                lateral: Vec::new(),
            };
            assert!(
                table.insert(a, fresh).is_none(),
                "currency-symbols.txt: repeated locale {a}"
            );
            current = Some(a);
            continue;
        }
        let tag = current.expect("currency-symbols.txt: entry before any locale");
        assert!(
            first.len() == 3 && first.bytes().all(|c| c.is_ascii_uppercase()),
            "currency-symbols.txt: bad code in {line:?}"
        );
        assert!(
            !a.is_empty() || !b.is_empty() || formats.iter().any(|v| !v.is_empty()),
            "currency-symbols.txt: empty entry {line:?}"
        );
        let symbols = &mut table.get_mut(tag).unwrap().symbols;
        let [p, d, g] = formats;
        assert!(
            symbols.insert(first, [a, b, p, d, g]).is_none(),
            "currency-symbols.txt: repeated {tag} {first}"
        );
    }
    assert!(table.contains_key("und"), "currency-symbols.txt: no und");
    table
}

/// The parsed currency-symbols.txt.
fn currencies_table() -> &'static CurrenciesTable {
    static T: OnceLock<CurrenciesTable> = OnceLock::new();
    T.get_or_init(|| parse_currencies(include_str!("../data/currency-symbols.txt")))
}

/// The locale of currency-symbols.txt whose symbols `id` uses (decision 7).
pub(crate) fn currencies_locale(id: &LanguageIdentifier) -> String {
    let table = currencies_table();
    fallback_key(id, |t| table.contains_key(t))
}

/// CLDR's symbol (`narrow` false) or narrow symbol of the currency `code`
/// for `id`, from currency-symbols.txt and the parents it records. CLDR's
/// fallbacks are already applied: a missing narrow symbol is the symbol,
/// and a missing symbol the code. None for a currency no locale of the
/// chain lists, whose symbols are its code.
pub(crate) fn currency_symbol(
    id: &LanguageIdentifier,
    code: &str,
    narrow: bool,
) -> Option<&'static str> {
    currency_field(id, code, usize::from(narrow))
}

/// The fields of a currency line of currency-symbols.txt.
const CURRENCY_FIELD_PATTERN: usize = 2;
const CURRENCY_FIELD_DECIMAL: usize = 3;
const CURRENCY_FIELD_GROUP: usize = 4;

/// The symbol field `field` (0 or 1) of the currency `code` for `id`: the
/// nearest non-empty one along currency-symbols.txt's parents; None if the
/// chain has none.
fn currency_field(id: &LanguageIdentifier, code: &str, field: usize) -> Option<&'static str> {
    let table = currencies_table();
    let mut entry = &table[currencies_locale(id).as_str()];
    loop {
        if let Some(fields) = entry.symbols.get(code) {
            if !fields[field].is_empty() {
                return Some(fields[field]);
            }
        }
        entry = &table[entry.parent?];
    }
}

/// CLDR's own format of the currency `code` for `id`, the numbering system
/// `nu` and the standard or (`accounting`) accounting pattern
/// (`<currencies><currency type="code">`'s `<pattern>`, `<decimal>` and
/// `<group>`): the pattern in CLDR's syntax (en-150's EUR "¤#,##0.00") and
/// the decimal and grouping separators (pt-CV's CVE "$"), each None where
/// the currency has none. Each field is resolved as UTS 35's lateral
/// inheritance says (tr35.md, "Lateral Inheritance"): at each locale of the
/// chain, the currency's own value; else the locale's own currencyFormat
/// pattern of nu and of that type, or its own symbols decimal or group of
/// nu, ends the search with None (en-CH's own patterns come before en-150's
/// EUR pattern for latn); else the parent. The numbering system matters
/// only there: a currency's own value applies to every numbering system.
pub(crate) fn currency_format(
    id: &LanguageIdentifier,
    code: &str,
    nu: &str,
    accounting: bool,
) -> [Option<&'static str>; 3] {
    let kind = if accounting { "accounting" } else { "standard" };
    let table = currencies_table();
    let start = &table[currencies_locale(id).as_str()];
    [
        CURRENCY_FIELD_PATTERN,
        CURRENCY_FIELD_DECIMAL,
        CURRENCY_FIELD_GROUP,
    ]
    .map(|field| {
        let context = if field == CURRENCY_FIELD_PATTERN {
            kind
        } else {
            "-"
        };
        let mut entry = start;
        loop {
            if let Some(v) = entry.symbols.get(code).map(|f| f[field]) {
                if !v.is_empty() {
                    return Some(v);
                }
            }
            if entry.lateral.contains(&(field, nu, context)) {
                return None;
            }
            entry = &table[entry.parent?];
        }
    })
}

/// The widths of units.txt.
const WIDTHS: [&str; 3] = ["long", "short", "narrow"];
/// The keys of a units.txt entry: the plural categories, and `per` for the
/// per-unit pattern (or, for the unit `-`, the width's compound pattern).
const UNIT_KEYS: [&str; 7] = ["zero", "one", "two", "few", "many", "other", "per"];

/// The patterns one locale of units.txt has of its own.
struct UnitsLocale {
    /// The locale the missing patterns are inherited from; None for und.
    parent: Option<&'static str>,
    /// (width, unit, key) to pattern.
    patterns: HashMap<(&'static str, &'static str, &'static str), &'static str>,
}

/// The parsed units.txt.
pub(crate) struct Units {
    locales: HashMap<&'static str, UnitsLocale>,
}

/// Parse units.txt.
fn parse_units(text: &'static str) -> Units {
    let mut lines = body_lines(text);
    let strings = string_table(&mut lines, "units.txt");
    let mut locales = HashMap::new();
    let mut current: Option<&'static str> = None;
    for line in lines {
        let mut words = line.split(' ');
        let first = words.next().unwrap_or_default();
        if first == "locale" {
            let (Some(tag), Some(parent), None) = (words.next(), words.next(), words.next()) else {
                panic!("units.txt: bad locale line {line:?}");
            };
            let parent = locale_parent("units.txt", tag, parent, |p| locales.contains_key(p));
            assert_eq!(parent.is_none(), tag == "und", "units.txt: {line:?}");
            let fresh = UnitsLocale {
                parent,
                patterns: HashMap::new(),
            };
            assert!(
                locales.insert(tag, fresh).is_none(),
                "units.txt: repeated locale {tag}"
            );
            current = Some(tag);
            continue;
        }
        let tag = current.expect("units.txt: entry before any locale");
        let width = WIDTHS
            .into_iter()
            .find(|w| *w == first)
            .unwrap_or_else(|| panic!("units.txt: bad width in {line:?}"));
        let unit = words.next().expect("units.txt: no unit");
        let entry = locales.get_mut(tag).unwrap();
        let mut any = false;
        for kv in words {
            let (key, index) = kv
                .split_once('=')
                .unwrap_or_else(|| panic!("units.txt: bad entry {kv:?}"));
            let key = UNIT_KEYS
                .into_iter()
                .find(|k| *k == key)
                .unwrap_or_else(|| panic!("units.txt: bad key in {line:?}"));
            let index: usize = index.parse().expect("units.txt: bad index");
            let pattern = *strings.get(index).expect("units.txt: index out of range");
            assert!(
                entry.patterns.insert((width, unit, key), pattern).is_none(),
                "units.txt: repeated {tag} {width} {unit} {key}"
            );
            any = true;
        }
        assert!(any, "units.txt: no patterns in {line:?}");
    }
    assert!(locales.contains_key("und"), "units.txt: no und");
    Units { locales }
}

/// The parsed units.txt.
pub(crate) fn units() -> &'static Units {
    static T: OnceLock<Units> = OnceLock::new();
    T.get_or_init(|| parse_units(include_str!("../data/units.txt")))
}

impl Units {
    /// The locale of units.txt whose patterns `id` uses (decision 7).
    pub(crate) fn locale_entry(&self, id: &LanguageIdentifier) -> String {
        fallback_key(id, |t| self.locales.contains_key(t))
    }

    /// The locale `tag` inherits missing patterns from, as the file records
    /// it; None for und or a tag that is not a locale of the file.
    pub(crate) fn parent(&self, tag: &str) -> Option<&'static str> {
        self.locales.get(tag)?.parent
    }

    /// The pattern `tag` has of its own for `width` (`long`, `short` or
    /// `narrow`), `unit` (a unit, or `-` for the width's compound pattern)
    /// and `key` (a plural category, or `per`).
    pub(crate) fn own_pattern(
        &self,
        tag: &str,
        width: &str,
        unit: &str,
        key: &str,
    ) -> Option<&'static str> {
        self.locales
            .get(tag)?
            .patterns
            .get(&(width, unit, key))
            .copied()
    }

    /// The pattern for `width`, `unit` and `key` (as `own_pattern`) of
    /// `tag` or, if it has none, of the nearest of its parents that has
    /// one. CLDR's own inheritance: a missing plural category is not
    /// replaced by "other" here.
    pub(crate) fn pattern(
        &self,
        tag: &str,
        width: &str,
        unit: &str,
        key: &str,
    ) -> Option<&'static str> {
        let mut t = tag;
        loop {
            if let Some(p) = self.own_pattern(t, width, unit, key) {
                return Some(p);
            }
            t = self.parent(t)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::language_id;

    fn id(tag: &str) -> LanguageIdentifier {
        language_id(tag).unwrap()
    }

    #[test]
    fn number_symbols_rows() {
        let en = number_symbols(&id("en"));
        assert_eq!(en.len(), 1);
        let latn = &en[0];
        assert_eq!(latn.nu, "latn");
        assert_eq!(latn.percent_sign, "%");
        assert_eq!(latn.exponential, "E");
        assert_eq!(latn.infinity, "∞");
        assert_eq!(latn.nan, "NaN");
        assert_eq!(latn.approximately_sign, "~");
        assert_eq!(latn.percent_pattern, "#,##0%");
        assert_eq!(latn.range_pattern, "{0}–{1}");
        assert_eq!((latn.currency_decimal, latn.currency_group), ("", ""));
        let de_at = &number_symbols(&id("de-AT"))[0];
        assert_eq!((de_at.currency_decimal, de_at.currency_group), ("", "."));
        let fr_ch = &number_symbols(&id("fr-CH"))[0];
        assert_eq!((fr_ch.currency_decimal, fr_ch.currency_group), (".", ""));
        let ar: Vec<&str> = number_symbols(&id("ar-EG")).iter().map(|r| r.nu).collect();
        assert_eq!(ar, ["arab", "latn"]);
        assert_eq!(number_symbols(&id("de"))[0].percent_pattern, "#,##0\u{a0}%");
    }

    #[test]
    fn entries_by_locale_fallback() {
        // Tags that are not keys reach one through ICU4X's fallback.
        assert_eq!(symbols_locale(&id("de-DE")), "de");
        assert_eq!(symbols_locale(&id("en-Latn-US")), "en");
        assert_eq!(symbols_locale(&id("zz")), "und");
        let units = units();
        assert_eq!(units.locale_entry(&id("en-GB")), "en-GB");
        assert_eq!(units.locale_entry(&id("en-US")), "en");
        assert_eq!(units.locale_entry(&id("zz")), "und");
    }

    #[test]
    fn units_inherit_along_the_file_parents() {
        let units = units();
        assert_eq!(units.parent("en-GB"), Some("en-001"));
        assert_eq!(units.parent("en-001"), Some("en"));
        assert_eq!(units.parent("en"), Some("und"));
        assert_eq!(units.parent("und"), None);
        assert_eq!(units.parent("xx"), None);
        // en-GB has no meter entries of its own, so it inherits en-001's,
        // which differ from en's.
        assert_eq!(units.own_pattern("en-GB", "long", "meter", "other"), None);
        assert_eq!(
            units.pattern("en-GB", "long", "meter", "other"),
            Some("{0} metres")
        );
        assert_eq!(
            units.pattern("en", "long", "meter", "other"),
            Some("{0} meters")
        );
        assert_eq!(
            units.pattern("en", "long", "meter", "one"),
            Some("{0} meter")
        );
        assert_eq!(
            units.pattern("en", "long", "meter", "per"),
            Some("{0} per meter")
        );
        // The width's compound pattern has the unit "-".
        assert_eq!(units.pattern("en", "long", "-", "per"), Some("{0} per {1}"));
        assert_eq!(units.pattern("und", "long", "-", "per"), Some("{0}/{1}"));
        assert_eq!(
            units.pattern("fr", "long", "meter", "one"),
            Some("{0}\u{a0}mètre")
        );
        // und has only "other"; a dedicated compound unit is kept.
        assert_eq!(units.pattern("und", "long", "meter", "one"), None);
        assert_eq!(
            units.pattern("en", "long", "kilometer-per-hour", "other"),
            Some("{0} kilometers per hour")
        );
        assert_eq!(units.pattern("en", "long", "parsec", "other"), None);
        assert_eq!(units.pattern("en", "wide", "meter", "other"), None);
    }

    #[test]
    fn explicit_number_systems_follow_cldr_xml() {
        let sym = |l, nu, i| explicit_symbol(&id(l), nu, i);
        // root.xml's arab and arabext symbols.
        assert_eq!(sym("en", "arab", EXPLICIT_DECIMAL), Some("٫"));
        assert_eq!(sym("en", "arab", EXPLICIT_MINUS_SIGN), Some("\u{61c}-"));
        assert_eq!(sym("en", "arab", EXPLICIT_PERCENT_SIGN), Some("٪\u{61c}"));
        assert_eq!(sym("en", "arabext", EXPLICIT_EXPONENTIAL), Some("×۱۰^"));
        // A locale's own value comes before root's: fr.xml's arab plusSign,
        // ur.xml's arab decimal, group and exponent, fa.xml's arab NaN,
        // ar.xml's arabext NaN for ar-EG, ko.xml's arab signs.
        assert_eq!(sym("fr", "arab", EXPLICIT_PLUS_SIGN), Some("\u{200f}+"));
        assert_eq!(sym("fr", "arab", EXPLICIT_MINUS_SIGN), Some("\u{61c}-"));
        assert_eq!(sym("fr-CA", "arab", EXPLICIT_PLUS_SIGN), Some("\u{200f}+"));
        assert_eq!(sym("ur", "arab", EXPLICIT_DECIMAL), Some("،"));
        assert_eq!(sym("ur", "arab", EXPLICIT_GROUP), Some(","));
        assert_eq!(sym("ur", "arab", EXPLICIT_EXPONENTIAL), Some("E"));
        assert_eq!(sym("fa", "arab", EXPLICIT_NAN), Some("ناعدد"));
        assert_eq!(sym("ar-EG", "arabext", EXPLICIT_NAN), Some("ليس\u{a0}رقمًا"));
        assert_eq!(sym("ko", "arab", EXPLICIT_MINUS_SIGN), Some("\u{200f}-"));
        // Root aliases other numbering systems to latn, and a locale's
        // explicit value applies where the chain has one (cv's NaN).
        assert_eq!(sym("en", "beng", EXPLICIT_DECIMAL), None);
        assert_eq!(sym("cv", "bali", EXPLICIT_NAN), Some("хисеп\u{a0}мар"));
        assert_eq!(sym("cv", "bali", EXPLICIT_DECIMAL), None);
        assert_eq!(explicit_percent_pattern(&id("en"), "arab"), Some("#,##0%"));
        assert_eq!(
            explicit_percent_pattern(&id("nl"), "arab"),
            Some("#,##0\u{a0}%")
        );
        assert_eq!(explicit_percent_pattern(&id("en"), "arabext"), None);
        assert_eq!(
            explicit_currency_pattern(&id("en"), "arab", CURRENCY_STANDARD),
            Some("#,##0.00\u{a0}¤")
        );
        assert_eq!(
            explicit_currency_pattern(&id("zh"), "arab", CURRENCY_STANDARD),
            Some("¤#,##0.00")
        );
        assert_eq!(
            explicit_currency_pattern(&id("zh"), "arab", CURRENCY_ACCOUNTING),
            Some(ACCOUNTING_IS_STANDARD)
        );
        assert_eq!(
            explicit_currency_pattern(&id("en"), "arab", CURRENCY_STANDARD_ALPHA),
            None
        );
        assert!(root_has_currency_patterns("arab"));
        assert!(!root_has_currency_patterns("arabext"));
    }

    #[test]
    fn short_currency_patterns_follow_cldr() {
        let p = |text, zeros| ShortPattern { text, zeros };
        let at = |tag, nu, power, count: &str| {
            let count = SHORT_CURRENCY_COUNTS
                .iter()
                .position(|c| *c == count)
                .unwrap();
            short_currency_patterns(&id(tag), nu)
                .get(&(power, count))
                .copied()
        };
        // en's 10^4 pattern "¤00K" and its alternate "¤ 00K".
        let en = ShortCurrencyPattern {
            plain: p("¤0K", 2),
            alt: Some(p("¤\u{a0}0K", 2)),
        };
        assert_eq!(at("en", "latn", 4, "other"), Some(en));
        // en-GB has its own "k"; en-AU inherits en's "K" through en-001.
        assert_eq!(
            at("en-GB", "latn", 4, "one"),
            Some(ShortCurrencyPattern {
                plain: p("¤0k", 2),
                alt: Some(p("¤\u{a0}0k", 2)),
            })
        );
        assert_eq!(at("en-AU", "latn", 4, "one"), Some(en));
        // The digits grow with the magnitude.
        assert_eq!(at("en", "latn", 5, "other").unwrap().plain, p("¤0K", 3));
        // de has no compact form below 10^6: the pattern "0".
        assert_eq!(at("de", "latn", 3, "other").unwrap().plain, p("0", 0));
        assert_eq!(
            at("de", "latn", 6, "other").unwrap().plain,
            p("0\u{a0}Mio'.'\u{a0}¤", 1)
        );
        // fa's arabext patterns are its latn ones, with their space; ar's
        // arab currency formats have no short ones, so they are latn's.
        assert_eq!(
            at("fa", "arabext", 3, "other"),
            at("fa", "latn", 3, "other")
        );
        assert_eq!(
            at("fa", "latn", 3, "other").unwrap().plain,
            p("\u{200e}¤\u{a0}0\u{a0}هزار", 1)
        );
        assert_eq!(at("ar", "arab", 3, "few"), at("ar", "latn", 3, "few"));
        // A numbering system root aliases to latn has the locale's latn
        // patterns; kok's explicit ones for thai override its latn 10^14
        // alternate, whose digits differ from its pattern's.
        assert_eq!(at("fr", "deva", 6, "one"), at("fr", "latn", 6, "one"));
        assert_eq!(
            at("kok", "thai", 14, "one"),
            Some(ShortCurrencyPattern {
                plain: p("¤0हज'.'निख'.'", 1),
                alt: Some(p("¤0LCr", 3)),
            })
        );
        assert_eq!(at("kok", "thai", 13, "one"), at("kok", "latn", 13, "one"));
        // vec's 10^3 "one" is "0" where "other" is compact; ja's 10^3 is
        // "0", and its patterns go up to 10^19.
        assert_eq!(at("vec", "latn", 3, "one").unwrap().plain, p("0", 0));
        assert_eq!(
            at("vec", "latn", 3, "other").unwrap().plain,
            p("0\u{a0}mila\u{202f}¤", 1)
        );
        assert_eq!(at("ja", "latn", 3, "other").unwrap().plain, p("0", 0));
        assert_eq!(at("ja", "latn", 19, "other").unwrap().plain.zeros, 4);
        assert_eq!(at("ja", "latn", 3, "one"), None);
    }

    #[test]
    fn compact_currency_lines() {
        let file = parse_compact_currency(
            "# comment\nstrings 3\n0\n¤0K\n¤ 0K\nlocale\tund\t-\nlatn\t3-5\t1\tother=1,2\n\
             latn\t6\t0\tother=0\nlocale\tx\tund\nthai\t14\t1\tone=1,2+2\n",
        );
        let und = &file["und"].patterns["latn"];
        assert_eq!(
            und[&(5, 5)].plain,
            ShortPattern {
                text: "¤0K",
                zeros: 3
            }
        );
        assert_eq!(
            und[&(5, 5)].alt,
            Some(ShortPattern {
                text: "¤ 0K",
                zeros: 3
            })
        );
        assert_eq!(
            und[&(6, 5)].plain,
            ShortPattern {
                text: "0",
                zeros: 0
            }
        );
        assert_eq!(und.len(), 4);
        let x = &file["x"];
        assert_eq!(x.parent, Some("und"));
        assert_eq!(x.patterns["thai"][&(14, 1)].alt.unwrap().zeros, 3);
    }

    #[test]
    fn currency_symbols_follow_cldr() {
        let s = |tag, code, narrow| currency_symbol(&id(tag), code, narrow);
        // CLDR's symbol is the code itself, which ICU4X's data leaves out.
        assert_eq!(s("af", "USD", false), Some("USD"));
        assert_eq!(s("bg", "JPY", false), Some("JPY"));
        assert_eq!(s("bg", "CAD", true), Some("CAD"));
        assert_eq!(s("en", "USD", false), Some("$"));
        assert_eq!(s("en", "CAD", false), Some("CA$"));
        assert_eq!(s("en", "CAD", true), Some("$"));
        assert_eq!(s("en-CA", "CAD", false), Some("$"));
        // Inherited from the parent chain: en-GB from en-001 and en.
        assert_eq!(s("en-GB", "USD", false), Some("US$"));
        assert_eq!(s("en-GB", "GBP", false), Some("£"));
        assert_eq!(s("de", "EUR", true), Some("€"));
        // A currency no locale lists has no symbol; the caller uses the code.
        assert_eq!(s("en", "ABC", false), None);
        assert_eq!(s("en", "ABC", true), None);
    }

    #[test]
    fn per_currency_formats_follow_cldr() {
        let f = |tag, code, nu, accounting| currency_format(&id(tag), code, nu, accounting);
        let p = |tag, code| f(tag, code, "latn", false);
        assert_eq!(p("en-150", "EUR"), [Some("¤#,##0.00"), None, None]);
        // Inherited: en-DE from en-150; pt-CV's PTE from pt-PT.
        assert_eq!(p("en-DE", "EUR"), [Some("¤#,##0.00"), None, None]);
        assert_eq!(p("en-AT", "EUR"), [Some("¤\u{a0}#,##0.00"), None, None]);
        assert_eq!(p("pt-CV", "CVE"), [None, Some("$"), None]);
        assert_eq!(p("pt-CV", "PTE"), [None, Some("$"), Some(",")]);
        assert_eq!(p("tr", "TRY"), [Some("¤#,##0.00"), None, None]);
        // The accounting pattern is the currency's too, found before tr's
        // own accounting pattern at the same locale.
        assert_eq!(f("tr", "TRY", "latn", true)[0], Some("¤#,##0.00"));
        // Lateral inheritance: a locale's own pattern or separators of the
        // numbering system (and of the pattern's type) end the search
        // before its parents' per-currency values. en-CH has standard and
        // accounting patterns of its own; en-PL only an accounting one;
        // it-CH its own symbols and standard pattern.
        assert_eq!(p("en-CH", "EUR"), [None; 3]);
        assert_eq!(f("en-CH", "EUR", "deva", false)[0], Some("¤#,##0.00"));
        assert_eq!(p("en-PL", "EUR")[0], Some("¤#,##0.00"));
        assert_eq!(f("en-PL", "EUR", "latn", true)[0], None);
        assert_eq!(p("it-CH", "ITL"), [None; 3]);
        assert_eq!(f("it-CH", "ITL", "latn", true)[0], Some("¤\u{a0}#,##0.00"));
        assert_eq!(
            f("it-CH", "ITL", "arab", false),
            [Some("¤\u{a0}#,##0.00"), Some(","), Some(".")]
        );
        // Every numbering system takes the currency's own values.
        assert_eq!(f("pt-CV", "CVE", "arab", false), [None, Some("$"), None]);
        // Other currencies and locales have none.
        assert_eq!(p("en-150", "USD"), [None; 3]);
        assert_eq!(p("en", "EUR"), [None; 3]);
        assert_eq!(p("pt", "PTE"), [None; 3]);
        // The symbols of a line with a format stay as they were.
        assert_eq!(
            currency_symbol(&id("pt-CV"), "CVE", false),
            Some("\u{200b}")
        );
    }

    #[test]
    fn every_locale_has_entries() {
        let units = units();
        for tag in crate::locale::test_locales() {
            assert_eq!(symbols_locale(&id(tag)), tag);
            assert!(
                number_symbols(&id(tag)).iter().any(|r| r.nu == "latn"),
                "{tag}"
            );
            assert_eq!(units.locale_entry(&id(tag)), tag);
            assert_eq!(currencies_locale(&id(tag)), tag);
            let file = compact_currency_file();
            assert_eq!(fallback_key(&id(tag), |t| file.contains_key(t)), tag);
            assert!(!short_currency_patterns(&id(tag), "latn").is_empty());
            // Every chain ends in und, which has every sanctioned unit.
            let mut t = tag;
            while let Some(p) = units.parent(t) {
                t = p;
            }
            assert_eq!(t, "und", "{tag}");
            for width in ["long", "short", "narrow"] {
                assert!(units.pattern(tag, width, "-", "per").is_some());
                assert!(units.pattern(tag, width, "byte", "other").is_some());
            }
        }
    }
}
