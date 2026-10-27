// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The number data behind Intl.NumberFormat: symbols, patterns, currency
//! data and compact patterns, as lists of strings that JS reads by index
//! (decision 5 of the Phase 3 plan). Formatting itself is JS.
//!
//! The data comes from ICU4X's provider structs where ICU4X has it, read
//! directly rather than through ICU4X's formatters, and from the CLDR files
//! of `cldr` for the rest (decision 4). Every lookup takes the language
//! identifier of a canonical tag and a numbering system nu, resolved as
//! CLDR's inheritance does (decision 6, revised):
//!
//! - where nu is one of the locale's own numbering systems (its default
//!   and other numbering systems, the ones cldr-json and ICU4X carry), the
//!   data is ICU4X's and number-symbols.txt's for (locale, nu);
//! - otherwise each symbol (separators, signs, percent sign, exponent,
//!   infinity, NaN, approximately sign), the percent pattern, the range
//!   pattern and each currency pattern (standard, accounting and their
//!   alphaNextToNumber alternates) is the explicit value for nu of the
//!   nearest locale of the chain, root included, in CLDR's XML
//!   (explicit-number-systems.txt), and where the chain has none, the
//!   locale's latn value, which root's aliases point to; a missing
//!   alternate currency pattern is the plain one where root has currency
//!   patterns for nu (arab);
//! - the grouping and the compact patterns of such a nu are the locale's
//!   latn ones (root aliases decimalFormats to latn, which the generator
//!   checks; locales' own decimalFormats for such numbering systems, only
//!   az's in CLDR 48.2, are not read);
//! - the short currency patterns of compact notation are
//!   compact-currency.txt's, which the generator resolves the same way,
//!   pattern by pattern, for every numbering system
//!   (cldr::short_currency_patterns).
//!
//! Patterns use the draft's placeholders (`{number}`, `{minusSign}`,
//! `{plusSign}`, `{percentSign}`, `{currencyPrefix}`, `{currencySuffix}`),
//! except the plural-dependent ones (currency names, compact), which keep
//! CLDR's `{0}` for JS to split.

use crate::cldr;
use crate::locale::language_id;
use icu::decimal::provider::{
    Baked as DecimalBaked, CompactPatterns, DecimalCompactLongV1, DecimalCompactShortV1,
    DecimalSymbols, DecimalSymbolsV1, GroupingSizes,
};
use icu::locale::LanguageIdentifier;
use icu::plurals::provider::PluralElementsPackedULE;
use icu::plurals::{
    PluralCategory, PluralOperands, PluralRules, PluralRulesPreferences, RawPluralOperands,
};
use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};
use icu::properties::CodePointMapData;
use icu_experimental::dimension::currency::CurrencyType;
use icu_experimental::dimension::provider::currency::essentials::{
    CurrencyEssentials, CurrencyEssentialsV1,
};
use icu_experimental::dimension::provider::currency::extended::CurrencyExtendedDataV1;
use icu_experimental::dimension::provider::currency::fractions::CurrencyFractionsV1;
use icu_experimental::dimension::provider::currency::patterns::CurrencyPatternsDataV1;
use icu_experimental::provider::Baked as ExperimentalBaked;
use icu_pattern::{
    DoublePlaceholderKey, DoublePlaceholderPattern, Pattern, PatternItem, SinglePlaceholder,
};
use icu_provider::prelude::*;
use std::sync::OnceLock;

/// The plural categories in the order of the lists: zero, one, two, few,
/// many, other.
const CATEGORIES: [PluralCategory; 6] = [
    PluralCategory::Zero,
    PluralCategory::One,
    PluralCategory::Two,
    PluralCategory::Few,
    PluralCategory::Many,
    PluralCategory::Other,
];

/// Whether `nu` has the syntax of a numbering system type: 3 to 8 ASCII
/// lowercase letters or digits.
fn is_numbering_system(nu: &str) -> bool {
    (3..=8).contains(&nu.len())
        && nu
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// The currency type of `code`, three ASCII uppercase letters (JS sends
/// IsWellFormedCurrencyCode's result uppercased); None for anything else.
fn currency_type(code: &str) -> Option<CurrencyType> {
    if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
        return None;
    }
    CurrencyType::try_from_str(&code.to_ascii_lowercase()).ok()
}

/// Load `M` for `locale` and the marker attributes `attributes` (none when
/// empty), with ICU4X's locale fallback; None if there is no data. The
/// request is silent: a miss is an ordinary step of resolving a numbering
/// system (en with arab, ar with bali) or a currency CLDR has no name for,
/// which debug builds of ICU4X would otherwise report on stderr.
pub(crate) fn load<M: DataMarker>(
    provider: &impl DataProvider<M>,
    locale: &DataLocale,
    attributes: &str,
) -> Option<DataPayload<M>> {
    let id = if attributes.is_empty() {
        DataIdentifierBorrowed::for_locale(locale)
    } else {
        DataIdentifierBorrowed::for_marker_attributes_and_locale(
            DataMarkerAttributes::try_from_str(attributes).ok()?,
            locale,
        )
    };
    let mut metadata = DataRequestMetadata::default();
    metadata.silent = true;
    provider
        .load(DataRequest { id, metadata })
        .ok()
        .map(|r| r.payload)
}

/// A locale and numbering system to look data up for.
struct Lookup {
    /// The language identifier of the locale, for the CLDR files.
    id: LanguageIdentifier,
    /// The same, for ICU4X data requests.
    locale: DataLocale,
    /// The requested numbering system.
    nu: String,
    /// The numbering system of the locale's default decimal symbols, whose
    /// data ICU4X stores without marker attributes.
    default_nu: String,
}

impl Lookup {
    /// The lookup for the language identifier of `locale` (a canonical tag)
    /// and `nu`; None if either is malformed or there is no decimal data.
    fn new(locale: &str, nu: &str) -> Option<Lookup> {
        if !is_numbering_system(nu) {
            return None;
        }
        let id = language_id(locale)?;
        let data_locale = DataLocale::from(&id);
        let default = load::<DecimalSymbolsV1>(&DecimalBaked, &data_locale, "")?;
        let default_nu = default.get().strings.numsys().to_owned();
        Some(Lookup {
            id,
            locale: data_locale,
            nu: nu.to_owned(),
            default_nu,
        })
    }

    /// Load `M`, whose ICU4X data has a variant per numbering system, for
    /// (locale, `nu`) itself: from the nearest locale of the chain that
    /// defines `nu`, through ICU4X's locale fallback. The default numbering
    /// system's data has no attributes, so `nu` equal to the default is
    /// loaded without them: with the attribute, the fallback could pass
    /// over the locale's own default data and find a parent's alternate.
    /// None if `nu` is not one of the numbering systems the locale's chain
    /// carries in ICU4X (its default and other numbering systems).
    fn load_defined<M: DataMarker>(
        &self,
        provider: &impl DataProvider<M>,
        nu: &str,
    ) -> Option<DataPayload<M>> {
        let attributes = if nu == self.default_nu { "" } else { nu };
        load(provider, &self.locale, attributes)
    }

    /// Load `M` for (locale, latn), the data root's aliases point to; the
    /// locale's default data if even latn's is missing.
    fn load_latn<M: DataMarker>(&self, provider: &impl DataProvider<M>) -> Option<DataPayload<M>> {
        self.load_defined(provider, "latn")
            .or_else(|| load(provider, &self.locale, ""))
    }

    /// Load `M`, a kind of data explicit-number-systems.txt has nothing of
    /// (decimalFormats: grouping and compact patterns): (locale, nu) where
    /// ICU4X has it, else (locale, latn).
    fn load_nu<M: DataMarker>(&self, provider: &impl DataProvider<M>) -> Option<DataPayload<M>> {
        self.load_defined(provider, &self.nu)
            .or_else(|| self.load_latn(provider))
    }

    /// The line of number-symbols.txt for (locale, `nu`), where `nu` is one
    /// of the locale's own numbering systems (the file holds cldr-json's
    /// resolved data, which has only those).
    fn cldr_row(&self, nu: &str) -> Option<&'static cldr::NumberSymbols> {
        cldr::number_symbols(&self.id).iter().find(|r| r.nu == nu)
    }

    /// The line of number-symbols.txt for (locale, latn); the default
    /// numbering system's if latn has none.
    fn cldr_latn(&self) -> &'static cldr::NumberSymbols {
        self.cldr_row("latn")
            .or_else(|| self.cldr_row(&self.default_nu))
            .unwrap_or_else(|| panic!("number-symbols.txt: no latn line for {}", self.id))
    }
}

/// The decimal symbols of a lookup, resolved as the module documentation
/// says: ICU4X's (locale, nu) where nu is the locale's own; otherwise each
/// separator and sign is CLDR's explicit one along the chain, else
/// (locale, latn)'s, and the grouping is (locale, latn)'s.
struct DecimalData {
    /// The decimal separator.
    decimal: String,
    /// The grouping separator.
    group: String,
    /// The minus sign.
    minus: Sign,
    /// The plus sign.
    plus: Sign,
    /// The group sizes and minimum grouping of decimals.
    grouping: GroupingSizes,
    /// The minus sign of (locale, nu) and of (locale, latn) as ICU4X has
    /// them, which ICU4X put into the explicit negative currency patterns
    /// of those data.
    defined_minus: Option<String>,
    latn_minus: String,
}

impl DecimalData {
    /// The decimal symbols for `lookup`; None if ICU4X has no data.
    fn new(lookup: &Lookup) -> Option<DecimalData> {
        let defined = lookup.load_defined::<DecimalSymbolsV1>(&DecimalBaked, &lookup.nu);
        let latn = lookup.load_latn::<DecimalSymbolsV1>(&DecimalBaked)?;
        let signs = |d: &DecimalSymbols| {
            let s = &d.strings;
            (
                Sign::new(s.minus_sign_prefix(), s.minus_sign_suffix()),
                Sign::new(s.plus_sign_prefix(), s.plus_sign_suffix()),
            )
        };
        let l = latn.get();
        let (latn_minus, latn_plus) = signs(l);
        let defined_minus = defined.as_ref().map(|d| signs(d.get()).0.full);
        if defined.is_none() {
            // Each symbol is CLDR's explicit one for nu along the chain,
            // else (locale, latn)'s; each sign goes where latn's does.
            let symbol = |i| cldr::explicit_symbol(&lookup.id, &lookup.nu, i);
            let s = &l.strings;
            return Some(DecimalData {
                decimal: symbol(cldr::EXPLICIT_DECIMAL)
                    .unwrap_or(s.decimal_separator())
                    .to_owned(),
                group: symbol(cldr::EXPLICIT_GROUP)
                    .unwrap_or(s.grouping_separator())
                    .to_owned(),
                minus: match symbol(cldr::EXPLICIT_MINUS_SIGN) {
                    Some(m) => Sign::placed(m, &latn_minus),
                    None => latn_minus.clone(),
                },
                plus: match symbol(cldr::EXPLICIT_PLUS_SIGN) {
                    Some(p) => Sign::placed(p, &latn_plus),
                    None => latn_plus,
                },
                grouping: l.grouping_sizes,
                defined_minus,
                latn_minus: latn_minus.full,
            });
        }
        let d = defined.as_ref().map_or(l, |d| d.get());
        let (minus, plus) = signs(d);
        Some(DecimalData {
            decimal: d.strings.decimal_separator().to_owned(),
            group: d.strings.grouping_separator().to_owned(),
            minus,
            plus,
            grouping: d.grouping_sizes,
            defined_minus,
            latn_minus: latn_minus.full,
        })
    }
}

/// The cardinal plural categories of the language identifier of `locale`,
/// from the plural rules that Intl.PluralRules and NumberFormat select
/// with.
pub(crate) fn plural_categories(locale: &str) -> Option<Vec<PluralCategory>> {
    let rules = crate::plurals::new_plural_rules(locale, false)?;
    Some(rules.rules().categories().collect())
}

/// The bidi controls ICU4X's sign strings carry around the sign.
const BIDI_CONTROLS: [char; 3] = ['\u{200e}', '\u{200f}', '\u{61c}'];

/// `symbol` split into its leading bidi controls, the symbol without them,
/// and its trailing ones.
fn split_bidi(symbol: &str) -> (&str, &str, &str) {
    let bare = symbol.trim_start_matches(BIDI_CONTROLS);
    let lead = &symbol[..symbol.len() - bare.len()];
    let bare_end = bare.trim_end_matches(BIDI_CONTROLS).len();
    (lead, &bare[..bare_end], &bare[bare_end..])
}

/// `symbol` as pattern text: `placeholder` in place of the symbol, with
/// its bidi controls around it as literal text.
fn marked(symbol: &str, placeholder: &str) -> String {
    let (lead, _, trail) = split_bidi(symbol);
    format!("{lead}{placeholder}{trail}")
}

/// A sign as the decimal symbols place it: ICU4X's string for it, which
/// may carry bidi controls ("\u{200e}-" in he), the sign without them, and
/// whether it goes before the number (ICU4X's prefix) or after it (the
/// suffix). The controls are literal text of the patterns, as ICU4C's parts
/// have them: he -1 is literal "\u{200e}", minusSign "-", integer "1".
#[derive(Clone)]
struct Sign {
    full: String,
    bare: String,
    prefix: bool,
}

impl Sign {
    /// The sign from ICU4X's prefix and suffix strings for it, one of which
    /// is empty: the prefix, unless only the suffix is set.
    fn new(prefix: &str, suffix: &str) -> Sign {
        let (full, prefix) = if prefix.is_empty() && !suffix.is_empty() {
            (suffix, false)
        } else {
            (prefix, true)
        };
        Sign {
            full: full.to_owned(),
            bare: split_bidi(full).1.to_owned(),
            prefix,
        }
    }

    /// The sign `text` (root's, for a numbering system the locale does not
    /// define) placed where `like` goes: the locale's decimal pattern, which
    /// root takes from latn, decides that.
    fn placed(text: &str, like: &Sign) -> Sign {
        Sign {
            full: text.to_owned(),
            bare: split_bidi(text).1.to_owned(),
            prefix: like.prefix,
        }
    }

    /// The sign as pattern text: `placeholder` in place of the sign, with
    /// the bidi controls around it.
    fn marked(&self, placeholder: &str) -> String {
        marked(&self.full, placeholder)
    }

    /// `pattern` with the sign, as `marked` writes it, where it goes: before
    /// the whole pattern for a prefix, after it for a suffix (decision 11).
    fn wrap(&self, pattern: &str, placeholder: &str) -> String {
        let sign = self.marked(placeholder);
        if self.prefix {
            format!("{sign}{pattern}")
        } else {
            format!("{pattern}{sign}")
        }
    }
}

/// The positive (explicit plus sign) and negative patterns for the
/// unsigned pattern `zero` and its explicit negative pattern, if CLDR has
/// one, where the minus sign is `minus.marked("{minusSign}")`. With an
/// explicit negative pattern that shows the minus sign, the plus sign takes
/// the minus sign's place (UTS 35, "Explicit Plus Signs"); otherwise the
/// signs go where the decimal symbols put them.
fn signed_patterns(
    zero: &str,
    negative: Option<String>,
    plus: &Sign,
    minus: &Sign,
) -> (String, String) {
    let minus_text = minus.marked("{minusSign}");
    match negative {
        Some(n) if n.contains(&minus_text) => {
            (n.replacen(&minus_text, &plus.marked("{plusSign}"), 1), n)
        }
        Some(n) => (plus.wrap(zero, "{plusSign}"), n),
        None => (
            plus.wrap(zero, "{plusSign}"),
            minus.wrap(zero, "{minusSign}"),
        ),
    }
}

/// A CLDR number pattern (`#,##0%`, `%#,##0`, `% #,#0;% -#,#0`,
/// `#,##0.00\u{a0}¤`) as the positive subpattern and the explicit negative
/// one, if any, with the number as `{number}`, `%` as `{percentSign}`, `¤` as
/// `{currencyPrefix}` before the number and `{currencySuffix}` after it, `-`
/// as `{minusSign}` and `+` as `{plusSign}`; quoted text is literal (UTS 35,
/// "Number Patterns").
pub(crate) fn convert_cldr_pattern(pattern: &str) -> (String, Option<String>) {
    let mut subpatterns = vec![String::new()];
    let mut chars = pattern.chars().peekable();
    let mut quoted = false;
    let mut seen_number = false;
    while let Some(c) = chars.next() {
        let out = subpatterns.last_mut().unwrap();
        if c == '\'' {
            if chars.peek() == Some(&'\'') {
                chars.next();
                out.push('\'');
            } else {
                quoted = !quoted;
            }
        } else if quoted {
            out.push(c);
        } else if c == '#' || c == '@' || c.is_ascii_digit() {
            while chars
                .peek()
                .is_some_and(|&d| matches!(d, '#' | '@' | ',' | '.') || d.is_ascii_digit())
            {
                chars.next();
            }
            out.push_str("{number}");
            seen_number = true;
        } else {
            match c {
                '%' => out.push_str("{percentSign}"),
                '¤' if seen_number => out.push_str("{currencySuffix}"),
                '¤' => out.push_str("{currencyPrefix}"),
                '-' => out.push_str("{minusSign}"),
                '+' => out.push_str("{plusSign}"),
                ';' => {
                    subpatterns.push(String::new());
                    seen_number = false;
                }
                _ => out.push(c),
            }
        }
    }
    let negative = subpatterns.get(1).cloned();
    (subpatterns.swap_remove(0), negative)
}

/// The primary and secondary group sizes of a CLDR percent pattern's
/// positive subpattern, as ICU4X derives them from the decimal pattern:
/// the digits after the last `,` of the integer part and between the last
/// two (the primary again with one `,`), and (0, 0), no grouping, with
/// none. Quoted text is skipped.
fn percent_grouping(pattern: &str) -> (u8, u8) {
    let mut quoted = false;
    let mut chars = pattern.chars().peekable();
    let mut number = String::new();
    while let Some(c) = chars.next() {
        if c == '\'' {
            quoted = !quoted;
        } else if !quoted && (c == '#' || c == '@' || c.is_ascii_digit()) {
            number.push(c);
            while let Some(&d) = chars.peek() {
                if !(matches!(d, '#' | '@' | ',' | '.') || d.is_ascii_digit()) {
                    break;
                }
                number.push(d);
                chars.next();
            }
            break;
        }
    }
    let integer = number.split('.').next().unwrap_or_default();
    let groups: Vec<&str> = integer.split(',').collect();
    let size = |g: &str| u8::try_from(g.len()).unwrap_or(u8::MAX);
    match groups[..] {
        [] | [_] => (0, 0),
        [_, primary] => (size(primary), size(primary)),
        [.., secondary, primary] => (size(primary), size(secondary)),
    }
}

/// The zero, positive and negative percent patterns of the CLDR percent
/// pattern `pattern` with the percent sign `percent_sign`, the signs where
/// CLDR puts them (decision 11) and the bidi controls of every symbol as
/// literal text. A CLDR pattern's `-` is the localized minus sign, so an
/// explicit negative subpattern's sign gets the minus sign's controls too.
fn percent_patterns(pattern: &str, percent_sign: &str, plus: &Sign, minus: &Sign) -> [String; 3] {
    let (zero, negative) = convert_cldr_pattern(pattern);
    let negative = negative.map(|n| n.replacen("{minusSign}", &minus.marked("{minusSign}"), 1));
    let (positive, negative) = signed_patterns(&zero, negative, plus, minus);
    let percent_sign = marked(percent_sign, "{percentSign}");
    [zero, positive, negative].map(|p| p.replace("{percentSign}", &percent_sign))
}

/// NumberFormat's symbols and decimal and percent patterns for the language
/// identifier of `locale` (a canonical tag) and the numbering system `nu`,
/// resolved as the module documentation says:
///
/// - 0 decimal separator, 1 group separator, 2 minus sign, 3 plus sign,
///   4 percent sign, 5 exponent symbol, 6 infinity, 7 NaN, 8 approximately
///   sign;
/// - 9 primary group size, 10 secondary group size, 11 minimum grouping
///   digits, as decimal strings;
/// - 12 to 14 the decimal zero, positive and negative patterns, 15 to 17
///   the percent ones, with the signs where CLDR puts them (decision 11);
/// - 18 the range pattern in CLDR's form (`{0}–{1}`);
/// - 19 and 20 the primary and secondary group sizes of percent patterns,
///   from the CLDR percent pattern (bn groups decimals 3 and 2, percents 3
///   and 3); the minimum grouping is entry 11 for both;
/// - 21 the time separator, CLDR's `timeSeparator`, which DurationFormat's
///   digital style puts between hours, minutes and seconds (`:`; da's latn
///   `.`, ug's arabext `٫`).
///
/// The currency style's separators are currency_separators'.
///
/// None if `locale` does not parse or `nu` is not 3 to 8 lowercase letters
/// or digits.
pub fn symbols(locale: &str, nu: &str) -> Option<Vec<String>> {
    let lookup = Lookup::new(locale, nu)?;
    let DecimalData {
        decimal,
        group,
        minus,
        plus,
        grouping: g,
        ..
    } = DecimalData::new(&lookup)?;
    // number-symbols.txt's data, resolved as the module documentation
    // says: (locale, nu) where nu is the locale's own, else value by value
    // CLDR's explicit one along the chain, else (locale, latn)'s.
    let row = lookup.cldr_row(nu);
    let latn = lookup.cldr_latn();
    let id = &lookup.id;
    let field = |own: fn(&cldr::NumberSymbols) -> &'static str, explicit: Option<&'static str>| {
        row.map(own).or(explicit).unwrap_or_else(|| own(latn))
    };
    let symbol = |i| cldr::explicit_symbol(id, nu, i);
    let percent_sign = field(|r| r.percent_sign, symbol(cldr::EXPLICIT_PERCENT_SIGN));
    let exponential = field(|r| r.exponential, symbol(cldr::EXPLICIT_EXPONENTIAL));
    let infinity = field(|r| r.infinity, symbol(cldr::EXPLICIT_INFINITY));
    let nan = field(|r| r.nan, symbol(cldr::EXPLICIT_NAN));
    let approximately_sign = field(
        |r| r.approximately_sign,
        symbol(cldr::EXPLICIT_APPROXIMATELY_SIGN),
    );
    let percent_pattern = field(
        |r| r.percent_pattern,
        cldr::explicit_percent_pattern(id, nu),
    );
    let range_pattern = field(|r| r.range_pattern, cldr::explicit_range_pattern(id, nu));
    let time_separator = field(|r| r.time_separator, symbol(cldr::EXPLICIT_TIME_SEPARATOR));
    let secondary = if g.secondary == 0 {
        g.primary
    } else {
        g.secondary
    };
    let (decimal_positive, decimal_negative) = signed_patterns("{number}", None, &plus, &minus);
    // The percent sign's bidi controls are literal text too (ar latn's sign
    // is "\u{200e}%\u{200e}"; ICU4C's parts have literal "\u{200e}" on
    // each side of percentSign "%").
    let [percent, percent_positive, percent_negative] =
        percent_patterns(percent_pattern, percent_sign, &plus, &minus);
    let (percent_primary, percent_secondary) = percent_grouping(percent_pattern);
    Some(vec![
        decimal,
        group,
        minus.bare.clone(),
        plus.bare.clone(),
        split_bidi(percent_sign).1.to_owned(),
        exponential.to_owned(),
        infinity.to_owned(),
        nan.to_owned(),
        approximately_sign.to_owned(),
        g.primary.to_string(),
        secondary.to_string(),
        g.min_grouping.to_string(),
        "{number}".to_owned(),
        decimal_positive,
        decimal_negative,
        percent,
        percent_positive,
        percent_negative,
        range_pattern.to_owned(),
        percent_primary.to_string(),
        percent_secondary.to_string(),
        time_separator.to_owned(),
    ])
}

/// The number-symbols.txt line whose currency data (currency separators,
/// and the locale's own per-currency formats) applies to (locale, nu): the
/// line of nu where nu is the locale's own; for another nu, (locale,
/// latn)'s where root aliases nu's symbols to latn, and none where root's
/// symbols for nu are explicit (arab, arabext), which have no currency
/// separators. No locale has currency separators for a numbering system
/// outside its own (the generator checks that).
fn currency_row(lookup: &Lookup) -> Option<&'static cldr::NumberSymbols> {
    match lookup.cldr_row(&lookup.nu) {
        Some(r) => Some(r),
        None if cldr::root_has_symbols(&lookup.nu) => None,
        None => Some(lookup.cldr_latn()),
    }
}

/// The decimal and grouping separators of the currency style for the
/// language identifier of `locale` (a canonical tag), the numbering system
/// `nu` and the currency `currency` (three uppercase letters), in every
/// display and notation:
///
/// - the currency's own separators, CLDR's `<currency type="X">`
///   `<decimal>` and `<group>` (pt-CV's CVE decimal "$"), which apply to
///   that currency only;
/// - else CLDR's currencyDecimal and currencyGroup (fr-CH's ".", de-AT's
///   ".");
/// - else the decimal and grouping separators (UTS 35).
///
/// The currency's own separators are resolved by UTS 35's lateral
/// inheritance (cldr::currency_format) and apply in every numbering system
/// (pt-CV-u-nu-arab CVE keeps "$"). currencyDecimal and currencyGroup are
/// the locale's latn ones where root aliases nu's symbols to latn (de-AT
/// with deva), and none where root's symbols for nu are explicit (arab,
/// arabext: currency_row). None if `locale` does not parse, `nu` is not a
/// numbering system type or `currency` is not three uppercase letters.
pub fn currency_separators(locale: &str, nu: &str, currency: &str) -> Option<Vec<String>> {
    let code = currency_type(currency)?.iso_code();
    let lookup = Lookup::new(locale, nu)?;
    let decimal = DecimalData::new(&lookup)?;
    let row = currency_row(&lookup);
    let [_, own_decimal, own_group] = cldr::currency_format(&lookup.id, &code, nu, false);
    let pick =
        |own: Option<&'static str>, f: fn(&cldr::NumberSymbols) -> &'static str, plain: &String| {
            own.or_else(|| row.map(f).filter(|s| !s.is_empty()))
                .map_or_else(|| plain.clone(), str::to_owned)
        };
    Some(vec![
        pick(own_decimal, |r| r.currency_decimal, &decimal.decimal),
        pick(own_group, |r| r.currency_group, &decimal.group),
    ])
}

/// Whether CLDR's currency spacing applies when `c`, the character of a
/// currency string at the end that touches the number, is next to it: `c`
/// is neither a symbol (general category S) nor a separator (Z), CLDR's
/// `currencyMatch` of `[[:^S:]&[:^Z:]]`.
fn is_alpha_next_to_number(c: char) -> bool {
    let gc = CodePointMapData::<GeneralCategory>::new().get(c);
    !GeneralCategoryGroup::Symbol.contains(gc) && !GeneralCategoryGroup::Separator.contains(gc)
}

/// `pattern` with CLDR's currency spacing, the one rule every CLDR 48.2
/// locale has (update-cldr-lists.py checks it): U+00A0 between the number
/// and a currency string that touches it with a character that is neither a
/// symbol nor a separator. `starts` and `ends` say that of the currency
/// string's first and last characters. ICU4X's alpha-next-to-number
/// patterns already have the space; this covers the patterns ICU4X has no
/// such variant of (en-ZA's standard pattern, ee's accounting negative).
fn currency_spacing(pattern: String, starts: bool, ends: bool) -> String {
    let mut p = pattern;
    if ends {
        p = p.replace("{currencyPrefix}{number}", "{currencyPrefix}\u{a0}{number}");
    }
    if starts {
        p = p.replace("{number}{currencySuffix}", "{number}\u{a0}{currencySuffix}");
    }
    p
}

/// A currency pattern of ICU4X (`{0}` the number, `{1}` the currency) in
/// the draft's form: `{number}`, and `{currencyPrefix}` or
/// `{currencySuffix}` by the side of the number the currency is on. With
/// `minus` = (text, sign), the first occurrence of `text` in the literal
/// text (ICU4X's string for the minus sign of the data the pattern comes
/// from, which ICU4X put into an explicit negative pattern) becomes
/// `sign.marked("{minusSign}")`.
fn convert_currency_pattern(
    pattern: &DoublePlaceholderPattern,
    minus: Option<(&str, &Sign)>,
) -> String {
    let mut sign = minus.filter(|(text, _)| !text.is_empty());
    let mut out = String::new();
    let mut seen_number = false;
    for item in pattern.iter() {
        match item {
            PatternItem::Literal(s) => match sign.take() {
                Some((text, m)) if s.contains(text) => {
                    out.push_str(&s.replacen(text, &m.marked("{minusSign}"), 1))
                }
                other => {
                    sign = other;
                    out.push_str(s);
                }
            },
            PatternItem::Placeholder(DoublePlaceholderKey::Place0) => {
                seen_number = true;
                out.push_str("{number}");
            }
            PatternItem::Placeholder(DoublePlaceholderKey::Place1) => {
                out.push_str(if seen_number {
                    "{currencySuffix}"
                } else {
                    "{currencyPrefix}"
                })
            }
        }
    }
    out
}

/// The cardinal plural rules of `ar`, which have all six categories, each
/// selected by an integer: they address each category's entry of plural
/// data. Exactly 0 and 1 also select a data's explicit 0 and 1 entries, so
/// the operands for the categories zero and one add `c = 1`, which makes
/// them not exact without changing ar's rules (which do not test `c`).
fn addressing_rules() -> &'static PluralRules {
    static R: OnceLock<PluralRules> = OnceLock::new();
    R.get_or_init(|| {
        let ar = language_id("ar").unwrap();
        PluralRules::try_new_cardinal(PluralRulesPreferences::from(&ar))
            .expect("no plural rules for ar")
    })
}

/// The operands that select `category` under `addressing_rules`, or with
/// `exact`, the exact value 0 or 1 of that category.
fn addressing_operands(category: PluralCategory, exact: bool) -> PluralOperands {
    let (i, c) = match category {
        PluralCategory::Zero => (0, 1),
        PluralCategory::One => (1, 1),
        PluralCategory::Two => (2, 0),
        PluralCategory::Few => (3, 0),
        PluralCategory::Many => (11, 0),
        PluralCategory::Other => (100, 0),
    };
    PluralOperands::from(RawPluralOperands {
        i,
        v: 0,
        w: 0,
        f: 0,
        t: 0,
        c: if exact { 0 } else { c },
    })
}

/// The six entries (zero, one, two, few, many, other) of plural data as
/// `render` makes them, for a locale whose plural categories are
/// `categories`: a category the locale lacks gets other's.
pub(crate) fn by_category<'a, V: zerovec::ule::VarULE + ?Sized>(
    data: &'a PluralElementsPackedULE<V>,
    categories: &[PluralCategory],
) -> Vec<&'a V> {
    CATEGORIES
        .iter()
        .map(|&c| {
            let c = if categories.contains(&c) {
                c
            } else {
                PluralCategory::Other
            };
            data.get(addressing_operands(c, false), addressing_rules())
                .1
        })
        .collect()
}

/// The explicit 0 (`one` false) or 1 pattern of plural data, rendered;
/// empty when it has none, and also when it is the same as the pattern of
/// the category of 0 or 1, which then gives the same result.
pub(crate) fn explicit<V: zerovec::ule::VarULE + ?Sized>(
    data: &PluralElementsPackedULE<V>,
    one: bool,
    render: impl Fn(&V) -> String,
) -> String {
    let category = if one {
        PluralCategory::One
    } else {
        PluralCategory::Zero
    };
    let rules = addressing_rules();
    let exact = render(data.get(addressing_operands(category, true), rules).1);
    let inexact = render(data.get(addressing_operands(category, false), rules).1);
    if exact == inexact {
        String::new()
    } else {
        exact
    }
}

/// The currency string of a symbol display: CLDR's symbol (`narrow`
/// false) or narrow symbol of `currency` in the locale, from
/// currency-symbols.txt, and the ISO code for a currency CLDR lists in no
/// locale of the chain. ICU4X's CurrencySymbolsV1 is not used: it leaves out
/// symbols that are the code (`af` USD is "USD"), so its locale fallback
/// finds a parent's.
fn currency_symbol(lookup: &Lookup, currency: CurrencyType, narrow: bool) -> String {
    let code = currency.iso_code();
    cldr::currency_symbol(&lookup.id, &code, narrow).map_or_else(|| code.to_string(), str::to_owned)
}

/// NumberFormat's currency patterns for the language identifier of
/// `locale` (a canonical tag), the numbering system `nu` (resolved as the
/// module documentation says), the
/// currency `currency` (three uppercase letters) and the currencyDisplay
/// `display`:
///
/// - `code`, `symbol` or `narrowSymbol`: 0 to 2 the zero, positive and
///   negative patterns (standard, or with `accounting` the accounting
///   ones), with `{currencyPrefix}` or `{currencySuffix}`; 3 the currency
///   string (the ISO code for `code`, and for a currency CLDR has no symbol
///   for); 4 to 6 the zero, positive and negative patterns for a number
///   whose character next to the currency is not a digit (NaN, infinity).
///   Patterns 0 to 2 follow CLDR's currency spacing when the end of the
///   currency string next to the number is neither a symbol nor a
///   separator: ICU4X's alpha-next-to-number variant, or U+00A0 inserted
///   where ICU4X has none. Patterns 4 to 6 are the plain patterns, without
///   that spacing.
/// - `name`: 0 to 5 the name pattern for zero, one, two, few, many and
///   other, in CLDR's form with the plural name substituted (`{0} US
///   dollars`); a category the locale lacks gets other's, and a currency
///   CLDR has no name for uses its code. There is no accounting form.
///
/// A currency's own pattern (CLDR's `<currency type="X"><pattern>`, en-150's
/// EUR "¤#,##0.00") replaces the locale's standard or accounting pattern for
/// the code, symbol and narrowSymbol displays, as lateral inheritance
/// resolves it (CurrencyFormats::with_currency_patterns).
/// Compact notation uses these for a number it does not compact
/// (compact_currency_patterns has the others).
///
/// None for any other display or currency, or if `locale` does not parse.
pub fn currency_patterns(
    locale: &str,
    nu: &str,
    currency: &str,
    display: &str,
    accounting: bool,
) -> Option<Vec<String>> {
    let currency = currency_type(currency)?;
    let narrow = match display {
        "code" | "symbol" | "name" => false,
        "narrowSymbol" => true,
        _ => return None,
    };
    let lookup = Lookup::new(locale, nu)?;
    let iso = currency.iso_code();
    if display == "name" {
        let categories = plural_categories(locale)?;
        let patterns = load::<CurrencyPatternsDataV1>(&ExperimentalBaked, &lookup.locale, "")?;
        let names = load::<CurrencyExtendedDataV1>(&ExperimentalBaked, &lookup.locale, &iso);
        let names = match &names {
            Some(n) => by_category(&n.get().elements, &categories),
            None => vec![iso.as_str(); 6],
        };
        let patterns = by_category(&patterns.get().elements, &categories);
        return Some(
            patterns
                .iter()
                .zip(names)
                .map(|(p, name)| {
                    let mut out = String::new();
                    for item in p.iter() {
                        match item {
                            PatternItem::Literal(s) => out.push_str(s),
                            PatternItem::Placeholder(DoublePlaceholderKey::Place0) => {
                                out.push_str("{0}")
                            }
                            PatternItem::Placeholder(DoublePlaceholderKey::Place1) => {
                                out.push_str(name)
                            }
                        }
                    }
                    out
                })
                .collect(),
        );
    }

    let decimal = DecimalData::new(&lookup)?;
    let (minus, plus) = (&decimal.minus, &decimal.plus);
    let string = if display == "code" {
        iso.to_string()
    } else {
        currency_symbol(&lookup, currency, narrow)
    };
    // The currency patterns of (locale, nu) where the chain defines them in
    // ICU4X, else CLDR's explicit ones along the chain pattern by pattern,
    // else (locale, latn)'s.
    let formats = match lookup.load_defined::<CurrencyEssentialsV1>(&ExperimentalBaked, &lookup.nu)
    {
        Some(d) => {
            let icu4x_minus = decimal
                .defined_minus
                .as_deref()
                .unwrap_or(&decimal.latn_minus);
            CurrencyFormats::from_icu4x(d.get(), icu4x_minus, minus)
        }
        None => {
            let latn = lookup.load_latn::<CurrencyEssentialsV1>(&ExperimentalBaked)?;
            let latn = CurrencyFormats::from_icu4x(latn.get(), &decimal.latn_minus, minus);
            CurrencyFormats::explicit(&lookup.id, nu, minus, latn)
        }
    };
    // The currency's own pattern (CLDR's <currency type="X"><pattern>,
    // en-150's EUR "¤#,##0.00"), resolved by UTS 35's lateral inheritance
    // for nu and for the standard and the accounting pattern
    // (cldr::currency_format), takes their place.
    let own_standard = cldr::currency_format(&lookup.id, &iso, nu, false)[0];
    let own_accounting = cldr::currency_format(&lookup.id, &iso, nu, true)[0];
    let formats = formats.with_currency_patterns(own_standard, own_accounting, minus);
    let (standard, alpha) = if accounting {
        (&formats.accounting, formats.accounting_alpha.as_ref())
    } else {
        (&formats.standard, formats.standard_alpha.as_ref())
    };
    // ICU4X's alpha-next-to-number selection: the side of the number the
    // currency is on in the plain pattern decides which end of the currency
    // string counts.
    let starts = string.chars().next().is_some_and(is_alpha_next_to_number);
    let ends = string
        .chars()
        .next_back()
        .is_some_and(is_alpha_next_to_number);
    let patterns = |starts: bool, ends: bool| {
        let (zero, negative) = select_currency_pattern(standard, alpha, starts, ends);
        let (positive, negative) = signed_patterns(&zero, negative, plus, minus);
        [zero, positive, negative]
    };
    let [zero, positive, negative] =
        patterns(starts, ends).map(|p| currency_spacing(p, starts, ends));
    // CLDR's currency spacing applies only next to a digit, so a number that
    // is not one (NaN, infinity) takes the patterns without it: ICU4C gives
    // "USDNaN" and "USD∞".
    let [bare_zero, bare_positive, bare_negative] = patterns(false, false);
    Some(vec![
        zero,
        positive,
        negative,
        string,
        bare_zero,
        bare_positive,
        bare_negative,
    ])
}

/// A short currency pattern of CLDR split for the draft's two levels of
/// patterns: the currency, its spacing and the text around them go to the
/// sign pattern, and the number with the compact text next to it to the
/// compact pattern.
struct ShortSplit {
    /// The zero sign pattern, `{number}` where the compact pattern goes.
    zero: String,
    /// The compact pattern in CLDR's form (`{0}K`).
    compact: String,
    /// Whether the currency is before the number.
    prefix: bool,
    /// Whether the currency touches the number's digits, with no space or
    /// compact text between them.
    adjacent: bool,
}

impl ShortSplit {
    /// Split `pattern`, a pattern of compact-currency.txt (its digits as one
    /// `0`; the generator checks that it has one currency sign, with only
    /// bidi marks and white space on its far side). With `spacing`, CLDR's
    /// currency spacing: U+00A0 between the currency and digits it touches.
    fn new(pattern: &str, spacing: bool) -> ShortSplit {
        const SPACE: [char; 3] = [' ', '\u{a0}', '\u{202f}'];
        let edge = |c: char| SPACE.contains(&c) || BIDI_CONTROLS.contains(&c);
        let (draft, _) = convert_cldr_pattern(pattern);
        let (zero, core, prefix) = if let Some((outer, rest)) = draft.split_once("{currencyPrefix}")
        {
            let core = rest.trim_start_matches(SPACE);
            let space = &rest[..rest.len() - core.len()];
            let inner = core.trim_end_matches(edge);
            let tail = &core[inner.len()..];
            let space = if space.is_empty() && spacing && inner.starts_with("{number}") {
                "\u{a0}"
            } else {
                space
            };
            (
                format!("{outer}{{currencyPrefix}}{space}{{number}}{tail}"),
                inner.to_owned(),
                true,
            )
        } else {
            let (rest, outer) = draft
                .split_once("{currencySuffix}")
                .expect("compact-currency.txt: a pattern without a currency");
            let core = rest.trim_end_matches(SPACE);
            let space = &rest[core.len()..];
            let inner = core.trim_start_matches(edge);
            let head = &core[..core.len() - inner.len()];
            let space = if space.is_empty() && spacing && inner.ends_with("{number}") {
                "\u{a0}"
            } else {
                space
            };
            (
                format!("{head}{{number}}{space}{{currencySuffix}}{outer}"),
                inner.to_owned(),
                false,
            )
        };
        let adjacent = if prefix {
            zero.contains("{currencyPrefix}{number}") && core.starts_with("{number}")
        } else {
            zero.contains("{number}{currencySuffix}") && core.ends_with("{number}")
        };
        ShortSplit {
            zero,
            compact: core.replacen("{number}", "{0}", 1),
            prefix,
            adjacent,
        }
    }
}

/// `pattern` with its currency, the number and what is between them (the
/// span from `{currencyPrefix}` to `{number}`, or from `{number}` to
/// `{currencySuffix}`) replaced by `with`'s; None if `pattern` has no
/// currency.
fn replace_currency_span(pattern: &str, with: &str) -> Option<String> {
    let span = |p: &str| {
        let number = p.find("{number}")?;
        if let Some(c) = p.find("{currencyPrefix}").filter(|&c| c < number) {
            Some((c, number + "{number}".len()))
        } else {
            let c = p.find("{currencySuffix}").filter(|&c| c > number)?;
            Some((number, c + "{currencySuffix}".len()))
        }
    };
    let (a, b) = span(pattern)?;
    let (c, d) = span(with)?;
    Some(format!("{}{}{}", &pattern[..a], &with[c..d], &pattern[b..]))
}

/// NumberFormat's patterns for compact notation with the currency style, a
/// table like compact_patterns' built from CLDR's short currency formats
/// (compact-currency.txt) for the language identifier of `locale` (a
/// canonical tag), the numbering system `nu`, the currency `currency` (three
/// uppercase letters) and the currencyDisplay `display` ("code", "symbol" or
/// "narrowSymbol"; the name display spells the number out with the decimal
/// compact patterns), standard or `accounting`. Entry 0 is the number `N` of
/// entries; then for each entry, in increasing magnitude, 26 strings: the
/// magnitude it starts at, its exponent (0 where CLDR's pattern is "0", for
/// which the number is not compacted), and for each of zero, one, two, few,
/// many and other (a category the locale lacks gets other's) its compact
/// pattern in CLDR's form (`{0}K`) and its zero, positive and negative
/// patterns in the draft's form, with `{number}` for the compact pattern (all
/// empty for an exponent of 0).
///
/// Each pattern is the one UTS 35's Compact Number Formats choose: the
/// alphaNextToNumber alternate where the currency string touches the digits
/// with a character that is neither a symbol nor a separator, and CLDR's
/// currency spacing after that. The exponent is the magnitude less the
/// pattern's digits, plus one. The pattern's text around the currency and
/// the number stays in the sign patterns, so the signs go outside it and
/// outside the compact text (sw's "¤ elfu -0" is not used), where CLDR's
/// minus sign goes. CLDR has no short accounting patterns: the accounting
/// negative pattern is the ordinary one with its currency, spacing and number
/// replaced by the short pattern's ("($1.2K)"), unless it shows a minus sign.
/// A count whose pattern is "0" where the others are compact (vec's 10^3
/// "one") has the compact pattern `{0}` and empty sign patterns: JS then
/// formats the number unscaled, without compaction (UTS 35).
///
/// None for any other display or currency, or if `locale` does not parse.
pub fn compact_currency_patterns(
    locale: &str,
    nu: &str,
    currency: &str,
    display: &str,
    accounting: bool,
) -> Option<Vec<String>> {
    let code = currency;
    let currency = currency_type(currency)?;
    let narrow = match display {
        "code" | "symbol" => false,
        "narrowSymbol" => true,
        _ => return None,
    };
    let lookup = Lookup::new(locale, nu)?;
    let categories = plural_categories(locale)?;
    let decimal = DecimalData::new(&lookup)?;
    let (minus, plus) = (&decimal.minus, &decimal.plus);
    let string = if display == "code" {
        currency.iso_code().to_string()
    } else {
        currency_symbol(&lookup, currency, narrow)
    };
    let starts = string.chars().next().is_some_and(is_alpha_next_to_number);
    let ends = string
        .chars()
        .next_back()
        .is_some_and(is_alpha_next_to_number);
    let ordinary = currency_patterns(locale, nu, code, display, accounting)?;
    // The accounting negative pattern that wraps the short ones: the
    // ordinary one, unless it is a signed pattern.
    let accounting_negative =
        (accounting && !ordinary[2].contains("{minusSign}")).then(|| ordinary[2].clone());
    let table = cldr::short_currency_patterns(&lookup.id, nu);
    let mut powers: Vec<u8> = table.keys().map(|k| k.0).collect();
    powers.dedup();
    let mut out = vec![powers.len().to_string()];
    for power in powers {
        // The pattern of each category, and the digits of the compact ones.
        let mut entry: Vec<[String; 4]> = Vec::new();
        let mut exponent: Option<u8> = None;
        let other = table[&(power, 5)];
        for category in CATEGORIES {
            let index = CATEGORIES.iter().position(|c| *c == category).unwrap();
            let pattern = if categories.contains(&category) {
                table.get(&(power, index)).copied().unwrap_or(other)
            } else {
                other
            };
            if pattern.plain.text == "0" {
                // No compact form for this count: JS formats the number
                // unscaled with the ordinary patterns, so the sign patterns
                // are not used.
                entry.push([
                    "{0}".to_owned(),
                    String::new(),
                    String::new(),
                    String::new(),
                ]);
                continue;
            }
            let plain = ShortSplit::new(pattern.plain.text, false);
            let alpha = if plain.prefix { ends } else { starts };
            let chosen = match pattern.alt {
                Some(alt) if alpha && plain.adjacent => alt,
                _ => pattern.plain,
            };
            let split = ShortSplit::new(chosen.text, alpha);
            let e = power + 1 - chosen.zeros;
            assert!(
                exponent.is_none_or(|x| x == e),
                "compact-currency.txt: {locale} {nu} 10^{power}: the counts' digits differ"
            );
            exponent = Some(e);
            let (positive, negative) = signed_patterns(&split.zero, None, plus, minus);
            let negative = accounting_negative
                .as_deref()
                .and_then(|n| replace_currency_span(n, &split.zero))
                .unwrap_or(negative);
            entry.push([split.compact, split.zero, positive, negative]);
        }
        out.push(power.to_string());
        match exponent {
            Some(e) => {
                out.push(e.to_string());
                out.extend(entry.into_iter().flatten());
            }
            None => {
                out.push("0".to_owned());
                out.extend(std::iter::repeat_n(String::new(), 24));
            }
        }
    }
    Some(out)
}

/// A currency pattern in the draft's form: the zero pattern, and the
/// explicit negative pattern if there is one, with the resolved minus sign.
#[derive(Clone)]
struct CurrencyPattern {
    /// The pattern without a sign.
    zero: String,
    /// The explicit negative pattern.
    negative: Option<String>,
}

impl CurrencyPattern {
    /// A CLDR currency pattern (`¤#,##0.00;(¤#,##0.00)`) with `minus` for
    /// its `-`.
    fn from_cldr(pattern: &str, minus: &Sign) -> CurrencyPattern {
        let (zero, negative) = convert_cldr_pattern(pattern);
        CurrencyPattern {
            zero,
            negative: negative.map(|n| n.replacen("{minusSign}", &minus.marked("{minusSign}"), 1)),
        }
    }
}

/// The standard and accounting currency patterns of a numbering system,
/// with their alphaNextToNumber alternates (CLDR's currency spacing) where
/// there are any.
struct CurrencyFormats {
    standard: CurrencyPattern,
    standard_alpha: Option<CurrencyPattern>,
    accounting: CurrencyPattern,
    accounting_alpha: Option<CurrencyPattern>,
}

impl CurrencyFormats {
    /// ICU4X's patterns. ICU4X wrote the minus sign of its own data,
    /// `icu4x_minus`, into the explicit negative patterns; that text becomes
    /// `minus`, the resolved sign.
    fn from_icu4x(e: &CurrencyEssentials, icu4x_minus: &str, minus: &Sign) -> CurrencyFormats {
        let pattern = |index: u8| e.patterns.get(usize::from(index));
        let convert = |zero: u8, negative: Option<u8>| CurrencyPattern {
            zero: pattern(zero).map_or_else(String::new, |p| convert_currency_pattern(p, None)),
            negative: negative
                .and_then(pattern)
                .map(|n| convert_currency_pattern(n, Some((icu4x_minus, minus)))),
        };
        let ix = &e.indices;
        CurrencyFormats {
            standard: convert(ix.standard, ix.standard_negative),
            standard_alpha: Some(convert(
                ix.standard_alpha_next_to_number,
                ix.standard_alpha_next_to_number_negative,
            )),
            accounting: convert(ix.accounting_positive, ix.accounting_negative),
            accounting_alpha: Some(convert(
                ix.accounting_alpha_next_to_number_positive,
                ix.accounting_alpha_next_to_number_negative,
            )),
        }
    }

    /// These formats with the standard and the accounting pattern replaced
    /// by a currency's own pattern for each, where it has one (CLDR's
    /// syntax, with no negative subpattern, which the generator checks). It
    /// has no alphaNextToNumber form, so CLDR's currency spacing applies to
    /// it instead. Lateral inheritance finds the currency's pattern before
    /// the locale's accounting pattern, so it is the accounting pattern too
    /// (tr TRY "-₺1.234,50", as ICU4C gives), unless a locale nearer in the
    /// chain has an accounting pattern of its own (en-PL).
    fn with_currency_patterns(
        self,
        standard: Option<&str>,
        accounting: Option<&str>,
        minus: &Sign,
    ) -> CurrencyFormats {
        let (standard, standard_alpha) = match standard {
            Some(p) => (CurrencyPattern::from_cldr(p, minus), None),
            None => (self.standard, self.standard_alpha),
        };
        let (accounting, accounting_alpha) = match accounting {
            Some(p) => (CurrencyPattern::from_cldr(p, minus), None),
            None => (self.accounting, self.accounting_alpha),
        };
        CurrencyFormats {
            standard,
            standard_alpha,
            accounting,
            accounting_alpha,
        }
    }

    /// CLDR's explicit patterns of `nu` along the chain of `id`, pattern by
    /// pattern (explicit-number-systems.txt). A pattern the chain does not
    /// have is `latn`'s where root aliases nu's currency formats to latn;
    /// where root has them, a missing alternate falls back to the plain
    /// pattern (CLDR's lateral inheritance). An accounting pattern that
    /// aliases to the standard one is the resolved standard pattern, and so
    /// is its alternate.
    fn explicit(
        id: &LanguageIdentifier,
        nu: &str,
        minus: &Sign,
        latn: CurrencyFormats,
    ) -> CurrencyFormats {
        let root = cldr::root_has_currency_patterns(nu);
        let pattern = |index| {
            cldr::explicit_currency_pattern(id, nu, index)
                .map(|p| CurrencyPattern::from_cldr(p, minus))
        };
        let standard = pattern(cldr::CURRENCY_STANDARD).unwrap_or(latn.standard);
        let standard_alpha = pattern(cldr::CURRENCY_STANDARD_ALPHA).or(if root {
            None
        } else {
            latn.standard_alpha
        });
        let accounting_is_standard =
            cldr::explicit_currency_pattern(id, nu, cldr::CURRENCY_ACCOUNTING)
                == Some(cldr::ACCOUNTING_IS_STANDARD);
        let (accounting, accounting_alpha) = if accounting_is_standard {
            (
                standard.clone(),
                pattern(cldr::CURRENCY_ACCOUNTING_ALPHA).or_else(|| standard_alpha.clone()),
            )
        } else {
            (
                pattern(cldr::CURRENCY_ACCOUNTING).unwrap_or(latn.accounting),
                pattern(cldr::CURRENCY_ACCOUNTING_ALPHA).or(if root {
                    None
                } else {
                    latn.accounting_alpha
                }),
            )
        };
        CurrencyFormats {
            standard,
            standard_alpha,
            accounting,
            accounting_alpha,
        }
    }
}

/// The zero and explicit negative patterns to use of `standard` and its
/// alphaNextToNumber alternate, for a currency string that is (`starts`,
/// `ends`) neither a symbol nor a separator at its ends: the alternate when
/// the plain pattern puts that end next to the number, as ICU4X's getters
/// do. An alternate without an explicit negative pattern takes the plain
/// one's (ee's accounting "(USD 1.00)", as ICU4C gives).
fn select_currency_pattern(
    standard: &CurrencyPattern,
    alpha: Option<&CurrencyPattern>,
    starts: bool,
    ends: bool,
) -> (String, Option<String>) {
    let adjacent = (starts && standard.zero.contains("{number}{currencySuffix}"))
        || (ends && standard.zero.contains("{currencyPrefix}{number}"));
    match alpha {
        Some(a) if adjacent => (
            a.zero.clone(),
            a.negative.clone().or_else(|| standard.negative.clone()),
        ),
        _ => (standard.zero.clone(), standard.negative.clone()),
    }
}

/// A compact pattern of ICU4X in CLDR's form (`{0}K`).
pub(crate) fn render_compact(pattern: &Pattern<SinglePlaceholder>) -> String {
    let mut out = String::new();
    for item in pattern.iter() {
        match item {
            PatternItem::Literal(s) => out.push_str(s),
            PatternItem::Placeholder(_) => out.push_str("{0}"),
        }
    }
    out
}

/// NumberFormat's compact patterns, short or (`long`) long, for the
/// language identifier of `locale` (a canonical tag) and the numbering
/// system `nu` (ICU4X's (locale, nu), else (locale, latn)). Entry 0 is the
/// number `N` of ICU4X's entries (decision 12); then for each entry, in
/// increasing magnitude, 10 strings:
/// the magnitude it starts at, its exponent, its exact-0 and exact-1
/// patterns (empty when it has none, or when it equals its category's), and
/// its zero, one, two, few, many and other patterns, in CLDR's form (`{0}K`;
/// a pattern may lack `{0}`, as `fr` long's exact-1 `mille` does). A
/// category the locale lacks gets other's. None if `locale` does not parse.
pub fn compact_patterns(locale: &str, nu: &str, long: bool) -> Option<Vec<String>> {
    let lookup = Lookup::new(locale, nu)?;
    let categories = plural_categories(locale)?;
    let data: DataPayload<DecimalCompactShortV1> = if long {
        lookup
            .load_nu::<DecimalCompactLongV1>(&DecimalBaked)?
            .cast()
    } else {
        lookup.load_nu::<DecimalCompactShortV1>(&DecimalBaked)?
    };
    let CompactPatterns(entries) = data.get();
    let mut out = vec![entries.len().to_string()];
    for entry in entries.iter() {
        let magnitude = entry.sized;
        let patterns = &entry.variable;
        // ICU4X stores the exponent as its difference from the magnitude.
        let exponent = magnitude.checked_sub(patterns.get_default().0.get())?;
        out.push(magnitude.to_string());
        out.push(exponent.to_string());
        out.push(explicit(patterns, false, render_compact));
        out.push(explicit(patterns, true, render_compact));
        out.extend(
            by_category(patterns, &categories)
                .into_iter()
                .map(render_compact),
        );
    }
    Some(out)
}

/// CurrencyDigits (decision 8): CLDR's number of fraction digits for
/// `currency` (three uppercase letters; not the cash digits), and 2 for a
/// currency CLDR does not list. None if `currency` is not three uppercase
/// letters.
pub fn currency_digits(currency: &str) -> Option<u8> {
    let currency = currency_type(currency)?;
    let fractions: DataPayload<CurrencyFractionsV1> =
        ExperimentalBaked.load(Default::default()).ok()?.payload;
    Some(fractions.get().resolve(currency).digits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(v: Option<Vec<String>>) -> Vec<String> {
        v.expect("no data")
    }

    /// Entry 21, the time separator: CLDR's per locale and numbering
    /// system, and a non-empty string for every locale of locales.txt in
    /// each numbering system number-symbols.txt has for it and its default
    /// one (the plan's Review Focus 5).
    #[test]
    fn time_separator() {
        fn sep(locale: &str, nu: &str) -> String {
            list(symbols(locale, nu)).swap_remove(21)
        }
        assert_eq!(sep("da", "latn"), ".");
        assert_eq!(sep("en", "latn"), ":");
        assert_eq!(sep("fi", "latn"), ".");
        assert_eq!(sep("ug", "arabext"), "٫");
        assert!(!sep("ar", "arab").is_empty());
        // A numbering system outside the locale's own: CLDR's explicit
        // value along the chain (root's arabext "٫", no's arab "."), else
        // the locale's latn one.
        assert_eq!(sep("en", "arabext"), "٫");
        assert_eq!(sep("nb", "arab"), ".");
        assert_eq!(sep("da", "deva"), ".");
        for locale in crate::locale::test_locales() {
            let id = language_id(locale).unwrap();
            let mut nus: Vec<String> = cldr::number_symbols(&id)
                .iter()
                .map(|r| r.nu.to_owned())
                .collect();
            nus.push(crate::info::default_numbering_system(locale).unwrap());
            for nu in nus {
                assert!(!sep(locale, &nu).is_empty(), "{locale} {nu}");
            }
        }
    }

    #[test]
    fn symbols_en_de_ar() {
        assert_eq!(
            list(symbols("en", "latn")),
            [
                ".",
                ",",
                "-",
                "+",
                "%",
                "E",
                "∞",
                "NaN",
                "~",
                "3",
                "3",
                "1",
                "{number}",
                "{plusSign}{number}",
                "{minusSign}{number}",
                "{number}{percentSign}",
                "{plusSign}{number}{percentSign}",
                "{minusSign}{number}{percentSign}",
                "{0}–{1}",
                "3",
                "3",
                ":",
            ]
        );
        let de = list(symbols("de", "latn"));
        assert_eq!(de[0], ",");
        assert_eq!(de[1], ".");
        assert_eq!(de[15], "{number}\u{a0}{percentSign}");
        let ar_arab = list(symbols("ar-EG", "arab"));
        assert_eq!(ar_arab[0], "٫");
        assert_eq!(ar_arab[1], "٬");
        assert_eq!(ar_arab[4], "٪");
        let ar_latn = list(symbols("ar-EG", "latn"));
        assert_eq!(ar_latn[0], ".");
        assert_eq!(ar_latn[1], ",");
        assert_eq!(ar_latn[4], "%");
        // Root aliases bali's symbols to the locale's latn ones, not to the
        // locale default's.
        assert_eq!(symbols("ar", "bali"), symbols("ar", "latn"));
        assert_eq!(symbols("ar-EG", "bali"), symbols("ar-EG", "latn"));
        assert_ne!(symbols("ar-EG", "latn"), symbols("ar-EG", "arab"));
        let en_in = list(symbols("en-IN", "latn"));
        assert_eq!((en_in[9].as_str(), en_in[10].as_str()), ("3", "2"));
        assert_eq!(list(symbols("pl", "latn"))[11], "2");
        // The sign goes before the whole pattern when it is a prefix.
        let tr = list(symbols("tr", "latn"));
        assert_eq!(tr[16], "{plusSign}{percentSign}{number}");
        assert_eq!(tr[17], "{minusSign}{percentSign}{number}");
        // An explicit negative percent subpattern places the sign itself.
        let blo = list(symbols("blo", "latn"));
        assert_eq!(blo[15], "{percentSign}\u{a0}{number}");
        assert_eq!(blo[16], "{percentSign}\u{a0}{plusSign}{number}");
        assert_eq!(blo[17], "{percentSign}\u{a0}{minusSign}{number}");
    }

    #[test]
    fn bidi_marks_are_literal_text() {
        // ICU4X's he minus sign is "\u{200e}-"; ICU4C's parts for -1 are
        // literal "\u{200e}", minusSign "-", integer "1".
        let he = list(symbols("he", "latn"));
        assert_eq!((he[2].as_str(), he[3].as_str()), ("-", "+"));
        assert_eq!(he[13], "\u{200e}{plusSign}{number}");
        assert_eq!(he[14], "\u{200e}{minusSign}{number}");
        assert_eq!(he[17], "\u{200e}{minusSign}{number}{percentSign}");
        let ar = list(symbols("ar", "latn"));
        assert_eq!(ar[2], "-");
        assert_eq!(ar[14], "\u{200e}{minusSign}{number}");
        let ar = list(symbols("ar-EG", "arab"));
        assert_eq!((ar[2].as_str(), ar[3].as_str()), ("-", "+"));
        assert_eq!(ar[14], "\u{61c}{minusSign}{number}");
        // The percent sign's marks too. ICU4C's parts for ar -5% with latn
        // digits: literal "\u{200e}", minusSign "-", integer "5", literal
        // "\u{200e}", percentSign "%", literal "\u{200e}"; with arab digits:
        // literal "\u{61c}", minusSign "-", integer, percentSign "٪", literal
        // "\u{61c}".
        let ar = list(symbols("ar", "latn"));
        assert_eq!(ar[4], "%");
        assert_eq!(ar[15], "{number}\u{200e}{percentSign}\u{200e}");
        assert_eq!(
            ar[16],
            "\u{200e}{plusSign}{number}\u{200e}{percentSign}\u{200e}"
        );
        assert_eq!(
            ar[17],
            "\u{200e}{minusSign}{number}\u{200e}{percentSign}\u{200e}"
        );
        let ar = list(symbols("ar", "arab"));
        assert_eq!(ar[4], "٪");
        assert_eq!(ar[16], "\u{61c}{plusSign}{number}{percentSign}\u{61c}");
        assert_eq!(ar[17], "\u{61c}{minusSign}{number}{percentSign}\u{61c}");
        // An explicit negative currency pattern keeps the marks around the
        // sign, and the positive pattern puts the plus sign's there.
        let he = list(currency_patterns("he", "latn", "ILS", "symbol", false));
        assert_eq!(
            he[2],
            "\u{200f}\u{200e}{minusSign}{number}\u{a0}\u{200f}{currencySuffix}"
        );
        assert_eq!(
            he[1],
            "\u{200f}\u{200e}{plusSign}{number}\u{a0}\u{200f}{currencySuffix}"
        );
    }

    /// Root's explicit data for numbering systems outside a locale's own:
    /// symbols for arab and arabext, and percent and currency patterns for
    /// arab; root aliases the rest to the locale's latn data. ICU4C (78.2)
    /// gives what the comments quote.
    #[test]
    fn root_numbering_systems() {
        // en-u-nu-arab -1234.5 "\u{61c}-١٬٢٣٤٫٥", +5 "\u{61c}+٥", 50%
        // "٥٠٪\u{61c}"; grouping and range ("٣–٥") are en's latn ones.
        let en = list(symbols("en", "arab"));
        assert_eq!(en[..9], ["٫", "٬", "-", "+", "٪", "اس", "∞", "NaN", "~"]);
        assert_eq!(en[9..12], ["3", "3", "1"]);
        assert_eq!(en[13], "\u{61c}{plusSign}{number}");
        assert_eq!(en[14], "\u{61c}{minusSign}{number}");
        assert_eq!(en[15], "{number}{percentSign}\u{61c}");
        assert_eq!(en[17], "\u{61c}{minusSign}{number}{percentSign}\u{61c}");
        assert_eq!(en[18], "{0}–{1}");
        assert_eq!(en[19..], ["3", "3", ":"]);
        // de-u-nu-arab: "\u{61c}-١٬٢٣٤٫٥" and "٥٠٪\u{61c}", root's percent
        // pattern rather than de's "#,##0 %".
        let de = list(symbols("de", "arab"));
        assert_eq!(de[..2], ["٫", "٬"]);
        assert_eq!(de[14], "\u{61c}{minusSign}{number}");
        assert_eq!(de[15], "{number}{percentSign}\u{61c}");
        // fr-u-nu-arabext: "\u{200e}-\u{200e}۱٬۲۳۴٫۵" and "۵۰\u{a0}٪": root's
        // arabext symbols, fr's percent pattern (root's aliases to latn).
        let fr = list(symbols("fr", "arabext"));
        assert_eq!(fr[..6], ["٫", "٬", "-", "+", "٪", "×۱۰^"]);
        assert_eq!(fr[14], "\u{200e}{minusSign}\u{200e}{number}");
        assert_eq!(fr[15], "{number}\u{a0}{percentSign}");
        // Root's other numbering systems alias to the locale's latn data:
        // ja-u-nu-deva "-१,२३४.५", en-u-nu-beng "-১,২৩৪.৫".
        assert_eq!(symbols("ja", "deva"), symbols("ja", "latn"));
        assert_eq!(symbols("en", "beng"), symbols("en", "latn"));
        // A locale that defines arab keeps its own data.
        let ar = list(symbols("ar", "arab"));
        assert_eq!(ar[..5], ["٫", "٬", "-", "+", "٪"]);
        assert_eq!(ar[14], "\u{61c}{minusSign}{number}");

        // en-u-nu-arab USD: "\u{61c}-١٬٢٣٤٫٥٠\u{a0}$" standard and accounting,
        // code "١٫٠٠\u{a0}USD": root's arab currency pattern.
        let cp = |l, nu, c, d, a| list(currency_patterns(l, nu, c, d, a));
        for accounting in [false, true] {
            let en = cp("en", "arab", "USD", "symbol", accounting);
            assert_eq!(
                en[..4],
                [
                    "{number}\u{a0}{currencySuffix}",
                    "\u{61c}{plusSign}{number}\u{a0}{currencySuffix}",
                    "\u{61c}{minusSign}{number}\u{a0}{currencySuffix}",
                    "$"
                ]
            );
            assert_eq!(en[4..], en[..3]);
        }
        let code = cp("en", "arab", "USD", "code", false);
        assert_eq!(code[0], "{number}\u{a0}{currencySuffix}");
        assert_eq!(code[3], "USD");
        // fr-u-nu-arabext: fr's patterns (root's arabext aliases to latn)
        // with root's arabext signs: accounting "(۱٬۲۳۴٫۵۰\u{a0}$US)",
        // standard "\u{200e}-\u{200e}۱٬۲۳۴٫۵۰\u{a0}$US".
        let fr = cp("fr", "arabext", "USD", "symbol", true);
        assert_eq!(fr[2], "({number}\u{a0}{currencySuffix})");
        assert_eq!(fr[3], "$US");
        let fr = cp("fr", "arabext", "USD", "symbol", false);
        assert_eq!(
            fr[2],
            "\u{200e}{minusSign}\u{200e}{number}\u{a0}{currencySuffix}"
        );
        // nl-u-nu-arab takes root's arab pattern: "\u{61c}-٥٫٠٠\u{a0}€".
        let nl = cp("nl", "arab", "EUR", "symbol", false);
        assert_eq!(nl[2], "\u{61c}{minusSign}{number}\u{a0}{currencySuffix}");
        // An explicit negative pattern of the latn data gets the sign of the
        // numbering system: nl-u-nu-arabext "€\u{a0}\u{200e}-\u{200e}۵٫۰۰".
        let nl = cp("nl", "arabext", "EUR", "symbol", false);
        assert_eq!(
            nl[2],
            "{currencyPrefix}\u{a0}\u{200e}{minusSign}\u{200e}{number}"
        );
        assert_eq!(
            nl[1],
            "{currencyPrefix}\u{a0}\u{200e}{plusSign}\u{200e}{number}"
        );
        // Compact patterns alias to latn: en-u-nu-arab 1234567 "١٫٢M".
        assert_eq!(
            compact_patterns("en", "arab", false),
            compact_patterns("en", "latn", false)
        );
    }

    /// Locale-level explicit data for numbering systems outside the
    /// locale's own (CLDR's XML; cldr-json and ICU4X leave it out), against
    /// ICU4C 78.2 (CLDR 48.0).
    #[test]
    fn locale_explicit_numbering_systems() {
        // fr-u-nu-arab +5 "\u{200f}+٥": fr.xml's arab plusSign.
        let fr = list(symbols("fr", "arab"));
        assert_eq!(fr[3], "+");
        assert_eq!(fr[13], "\u{200f}{plusSign}{number}");
        assert_eq!(fr[14], "\u{61c}{minusSign}{number}");
        // fr-CA-u-nu-arab +5 "\u{200f}+٥": fr-CA inherits fr's.
        assert_eq!(
            list(symbols("fr-CA", "arab"))[13],
            "\u{200f}{plusSign}{number}"
        );
        // ko-u-nu-arab -1234.5 "\u{200f}-١٬٢٣٤٫٥" and +5 "\u{200f}+٥".
        let ko = list(symbols("ko", "arab"));
        assert_eq!(ko[13], "\u{200f}{plusSign}{number}");
        assert_eq!(ko[14], "\u{200f}{minusSign}{number}");
        // ur-u-nu-arab -1234.5 "\u{61c}-١,٢٣٤،٥", 12345 scientific "١،٢٣٥E٤".
        let ur = list(symbols("ur", "arab"));
        assert_eq!(ur[..2], ["،", ","]);
        assert_eq!(ur[5], "E");
        assert_eq!(ur[14], "\u{61c}{minusSign}{number}");
        // nl-u-nu-arab 50% "٥٠\u{a0}٪\u{61c}": nl.xml's arab percent pattern.
        assert_eq!(
            list(symbols("nl", "arab"))[15],
            "{number}\u{a0}{percentSign}\u{61c}"
        );
        // NaN: fa-u-nu-arab "ناعدد", ar-EG-u-nu-arabext "ليس\u{a0}رقمًا" (from
        // ar.xml), cv-u-nu-bali "хисеп\u{a0}мар" with cv's latn separators.
        assert_eq!(list(symbols("fa", "arab"))[7], "ناعدد");
        assert_eq!(list(symbols("ar-EG", "arabext"))[7], "ليس\u{a0}رقمًا");
        let cv = list(symbols("cv", "bali"));
        assert_eq!(cv[7], "хисеп\u{a0}мар");
        assert_eq!(cv[..2], list(symbols("cv", "latn"))[..2]);
        // ha-u-nu-arab -1234.5 "-١,٢٣٤.٥", eu-u-nu-arab "-١٬٢٣٤٫٥".
        let ha = list(symbols("ha", "arab"));
        assert_eq!(ha[..4], [".", ",", "-", "+"]);
        assert_eq!(ha[14], "{minusSign}{number}");
        let eu = list(symbols("eu", "arab"));
        assert_eq!(eu[..2], ["٫", "٬"]);
        assert_eq!(eu[14], "{minusSign}{number}");
        // zh-u-nu-arab USD "\u{61c}-US$١٬٢٣٤٫٥٠", accounting the same (root's
        // accounting aliases to the standard pattern, zh's here).
        for accounting in [false, true] {
            let zh = list(currency_patterns("zh", "arab", "USD", "symbol", accounting));
            assert_eq!(zh[0], "{currencyPrefix}{number}");
            assert_eq!(zh[2], "\u{61c}{minusSign}{currencyPrefix}{number}");
        }
    }

    /// Compact notation's currency patterns are their own kind of data:
    /// root has none for arab or arabext, so they are the locale's latn
    /// ones, with the numbering system's signs (ICU4C 78.2).
    /// Compact currency patterns: CLDR's short currency formats split into
    /// the sign patterns, with the currency and its spacing, and the
    /// compact pattern; the expected strings are CLDR 48.2's patterns.
    #[test]
    fn compact_currency_patterns_of_locales() {
        let cc = |l, nu, c, d, a| list(compact_currency_patterns(l, nu, c, d, a));
        // The other (category 5) strings of the entry starting at 10^power.
        let other = |v: &[String], power: &str| {
            let at = v
                .iter()
                .skip(1)
                .step_by(26)
                .position(|m| m == power)
                .unwrap();
            let base = 1 + at * 26;
            (v[base + 1].clone(), v[base + 2 + 20..base + 26].to_vec())
        };
        // fa "\u{200e}¤\u{a0}0\u{a0}هزار": the space after the currency.
        let fa = cc("fa", "arabext", "USD", "symbol", false);
        let (e, p) = other(&fa, "3");
        assert_eq!(e, "3");
        assert_eq!(
            p,
            [
                "{0}\u{a0}هزار",
                "\u{200e}{currencyPrefix}\u{a0}{number}",
                "\u{200e}{plusSign}\u{200e}{currencyPrefix}\u{a0}{number}",
                "\u{200e}{minusSign}\u{200e}{currencyPrefix}\u{a0}{number}",
            ]
        );
        // en: "¤0K", and the alternate "¤\u{a0}0K" for a currency string that
        // ends in a letter; 10^5's exponent is 3.
        let (e, p) = other(&cc("en", "latn", "USD", "symbol", false), "5");
        assert_eq!(e, "3");
        assert_eq!(p[..2], ["{0}K", "{currencyPrefix}{number}"]);
        let (_, p) = other(&cc("en", "latn", "USD", "code", false), "3");
        assert_eq!(p[1], "{currencyPrefix}\u{a0}{number}");
        // The currency after the number, with the compact text and its
        // space before it (fr "0\u{a0}k\u{a0}¤"), and before it (sw
        // "¤\u{a0}elfu\u{a0}0", whose explicit negative is not used).
        let (_, p) = other(&cc("fr", "latn", "EUR", "symbol", false), "3");
        assert_eq!(p[..2], ["{0}\u{a0}k", "{number}\u{a0}{currencySuffix}"]);
        let (_, p) = other(&cc("sw", "latn", "USD", "symbol", false), "3");
        assert_eq!(p[0], "elfu\u{a0}{0}");
        assert_eq!(p[3], "{minusSign}{currencyPrefix}\u{a0}{number}");
        // ar "\u{200f}0\u{a0}ألف\u{a0}¤": the bidi mark stays in the sign
        // pattern, inside the minus sign; its few category has "آلاف".
        let ar = cc("ar", "arab", "USD", "symbol", false);
        let (_, p) = other(&ar, "3");
        assert_eq!(p[0], "{0}\u{a0}ألف");
        assert_eq!(p[1], "\u{200f}{number}\u{a0}{currencySuffix}");
        assert_eq!(ar[3 + 3 * 4], "{0}\u{a0}آلاف");
        // Accounting: the ordinary negative around the short pattern's
        // currency and number; de's accounting pattern has a minus sign.
        let (_, p) = other(&cc("en", "latn", "USD", "symbol", true), "3");
        assert_eq!(p[3], "({currencyPrefix}{number})");
        let (_, p) = other(&cc("fr", "latn", "EUR", "symbol", true), "3");
        assert_eq!(p[3], "({number}\u{a0}{currencySuffix})");
        let (_, p) = other(&cc("de", "latn", "EUR", "symbol", true), "6");
        assert_eq!(p[3], "{minusSign}{number}\u{a0}{currencySuffix}");
        // de's 10^3 to 10^5 are "0": exponent 0, no patterns.
        let de = cc("de", "latn", "EUR", "symbol", false);
        assert_eq!(de[1..3], ["3", "0"]);
        assert!(de[3..27].iter().all(String::is_empty));
        // vec's 10^3 "one" is "0": "{0}" and no sign patterns.
        let vec = cc("vec", "latn", "EUR", "symbol", false);
        assert_eq!(vec[1..3], ["3", "3"]);
        assert_eq!(vec[3 + 4..3 + 8], ["{0}", "", "", ""]);
        // kok-u-nu-thai's 10^14 alternate "¤000LCr" has three digits.
        let (e, p) = other(&cc("kok", "thai", "USD", "code", false), "14");
        assert_eq!(e, "12");
        assert_eq!(p[..2], ["{0}LCr", "{currencyPrefix}\u{a0}{number}"]);
        let (e, _) = other(&cc("kok", "thai", "USD", "symbol", false), "14");
        assert_eq!(e, "14");
        // The name display and bad values have none.
        assert_eq!(
            compact_currency_patterns("en", "latn", "USD", "name", false),
            None
        );
        assert_eq!(
            compact_currency_patterns("en", "latn", "usd", "code", false),
            None
        );
        assert_eq!(
            compact_currency_patterns("en_US", "latn", "USD", "code", false),
            None
        );
    }

    /// Every locale's compact currency patterns, for its default numbering
    /// system, latn and deva: one currency and one number in each sign
    /// pattern, no doubled spacing, and one {0} in each compact pattern.
    #[test]
    fn every_locale_has_compact_currency_patterns() {
        for locale in crate::locale::test_locales() {
            let default = crate::info::default_numbering_system(locale).unwrap();
            for nu in [default.as_str(), "latn", "deva", "arab"] {
                for (currency, display) in
                    [("USD", "symbol"), ("CHF", "code"), ("EUR", "narrowSymbol")]
                {
                    for accounting in [false, true] {
                        let what = format!("{locale} {nu} {currency} {display} {accounting}");
                        let v =
                            compact_currency_patterns(locale, nu, currency, display, accounting)
                                .unwrap_or_else(|| panic!("{what}"));
                        let n: usize = v[0].parse().unwrap();
                        assert_eq!(v.len(), 1 + n * 26, "{what}");
                        for entry in v[1..].chunks(26) {
                            if entry[1] == "0" {
                                continue;
                            }
                            for p in entry[2..].chunks(4) {
                                assert_eq!(count(&p[0], "{0}"), 1, "{what}: {p:?}");
                                if p[0] == "{0}" {
                                    continue;
                                }
                                for s in &p[1..] {
                                    let c =
                                        count(s, "{currencyPrefix}") + count(s, "{currencySuffix}");
                                    assert_eq!((c, count(s, "{number}")), (1, 1), "{what}: {s}");
                                    assert!(
                                        !s.contains("\u{a0}\u{a0}") && !s.contains("  "),
                                        "{what}: {s:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// The currency style's separators: the currency's own, else CLDR's
    /// currencyDecimal and currencyGroup where the locale has them, else
    /// the decimal and group separators.
    #[test]
    fn currency_separators_of_locales() {
        let cs = |l, nu, c| list(currency_separators(l, nu, c));
        let plain = |l, nu| list(symbols(l, nu))[..2].to_vec();
        // de-AT EUR 1234.5 "€\u{a0}1.234,50", against 1234.5 "1\u{a0}234,5".
        assert_eq!(plain("de-AT", "latn"), [",", "\u{a0}"]);
        assert_eq!(cs("de-AT", "latn", "EUR"), [",", "."]);
        // fr-CH CHF -123.5 "-123.50\u{a0}CHF", against -123.5 "-123,5".
        let fr_ch = plain("fr-CH", "latn");
        assert_eq!(fr_ch[0], ",");
        assert_eq!(cs("fr-CH", "latn", "CHF"), [".", fr_ch[1].as_str()]);
        // A numbering system root aliases to latn (deva, beng, thai) has
        // the locale's latn symbols, its currency separators included
        // (de-AT deva EUR 1234.5 "€\u{a0}१.२३४,५०", as ICU4C gives), while
        // its plain separators stay the latn ones.
        for nu in ["deva", "beng", "thai", "mymr"] {
            assert_eq!(plain("de-AT", nu), plain("de-AT", "latn"), "de-AT {nu}");
            assert_eq!(cs("de-AT", nu, "USD"), [",", "."], "de-AT {nu}");
            assert_eq!(plain("fr-CH", nu), fr_ch, "fr-CH {nu}");
            assert_eq!(
                cs("fr-CH", nu, "USD"),
                [".", fr_ch[1].as_str()],
                "fr-CH {nu}"
            );
        }
        // Locales without them, and root's explicit arab and arabext
        // symbols, which have no currency separators (UTS 35: then the
        // plain ones), use the plain ones.
        for (l, nu) in [
            ("en", "latn"),
            ("en-AT", "latn"),
            ("de", "latn"),
            ("de", "deva"),
            ("de-AT", "arab"),
            ("de-AT", "arabext"),
            ("fr-CH", "arab"),
            ("fr-CH", "arabext"),
        ] {
            assert_eq!(cs(l, nu, "EUR"), plain(l, nu), "{l} {nu}");
        }
        assert_eq!(plain("de-AT", "arab"), ["\u{66b}", "\u{66c}"]);
        // A currency's own separators apply to it alone and come before the
        // locale's: pt-CV's CVE decimal "$", pt-PT's PTE "$" and ",",
        // de-LU's LUF "." and ",", inherited along the chain (pt-CV's PTE
        // from pt-PT), and with a numbering system aliased to latn.
        assert_eq!(
            cs("pt-CV", "latn", "CVE"),
            ["$", plain("pt-CV", "latn")[1].as_str()]
        );
        assert_eq!(cs("pt-CV", "latn", "USD"), plain("pt-CV", "latn"));
        assert_eq!(cs("pt-PT", "latn", "PTE"), ["$", ","]);
        assert_eq!(cs("pt-CV", "latn", "PTE"), ["$", ","]);
        assert_eq!(cs("de-LU", "latn", "LUF"), [".", ","]);
        assert_eq!(cs("pt-CV", "deva", "CVE")[0], "$");
        assert_eq!(
            cs("pt-CV", "arab", "CVE"),
            ["$", plain("pt-CV", "arab")[1].as_str()]
        );
        assert_eq!(currency_separators("pt-CV", "latn", "cve"), None);
        assert_eq!(currency_separators("pt_CV", "latn", "CVE"), None);
    }

    /// A currency's own pattern takes the place of the locale's standard
    /// one for that currency, with CLDR's currency spacing, and the
    /// accounting negative keeps the locale's form.
    #[test]
    fn per_currency_patterns() {
        let cp = |l, nu, c, d, a| list(currency_patterns(l, nu, c, d, a));
        // en-150 EUR "¤#,##0.00" and USD its own "#,##0.00\u{a0}¤".
        let eur = cp("en-150", "latn", "EUR", "symbol", false);
        assert_eq!(
            eur[..3],
            [
                "{currencyPrefix}{number}",
                "{plusSign}{currencyPrefix}{number}",
                "{minusSign}{currencyPrefix}{number}"
            ]
        );
        assert_eq!(
            cp("en-150", "latn", "USD", "symbol", false)[0],
            "{number}\u{a0}{currencySuffix}"
        );
        // Inherited along the chain, and in every numbering system.
        assert_eq!(
            cp("en-DE", "latn", "EUR", "symbol", false)[0],
            "{currencyPrefix}{number}"
        );
        assert_eq!(
            cp("en-150", "deva", "EUR", "symbol", false)[0],
            "{currencyPrefix}{number}"
        );
        assert_eq!(
            cp("en-150", "arab", "EUR", "symbol", false)[0],
            "{currencyPrefix}{number}"
        );
        // en-CH's own patterns come first for latn (lateral inheritance).
        assert_eq!(
            cp("en-CH", "latn", "EUR", "symbol", false)[0],
            "{currencyPrefix}\u{a0}{number}"
        );
        // The code display: CLDR's currency spacing, as the pattern has no
        // alphaNextToNumber form (tr TRY "TRY\u{a0}1.234,50").
        assert_eq!(
            cp("tr", "latn", "TRY", "code", false)[0],
            "{currencyPrefix}\u{a0}{number}"
        );
        // Accounting: the currency's pattern, found before tr's own
        // accounting pattern; en-PL's own accounting pattern comes first.
        assert_eq!(
            cp("tr", "latn", "TRY", "symbol", true)[2],
            "{minusSign}{currencyPrefix}{number}"
        );
        assert_eq!(
            cp("en-PL", "latn", "EUR", "symbol", true)[2],
            "({number}\u{a0}{currencySuffix})"
        );
        assert_eq!(
            cp("en-150", "latn", "EUR", "symbol", true)[2],
            "{minusSign}{currencyPrefix}{number}"
        );
        // The name display has no currency pattern.
        assert_eq!(cp("en-150", "latn", "EUR", "name", false)[5], "{0} euros");
    }

    #[test]
    fn percent_grouping_is_its_own() {
        // bn groups decimals as #,##,##0.### and percents as #,##0%; ICU4C
        // gives bn 123456789 as "১২,৩৪,৫৬,৭৮৯" and as a percent
        // "১২,৩৪৫,৬৭৮,৯০০%".
        let bn = list(symbols("bn", "beng"));
        assert_eq!((bn[9].as_str(), bn[10].as_str()), ("3", "2"));
        assert_eq!((bn[19].as_str(), bn[20].as_str()), ("3", "3"));
        // With latn digits CLDR's bn percent pattern is #,##,##0% (ICU4C:
        // "12,34,56,78,900%").
        let bn = list(symbols("bn", "latn"));
        assert_eq!((bn[19].as_str(), bn[20].as_str()), ("3", "2"));
        let en = list(symbols("en", "latn"));
        assert_eq!((en[19].as_str(), en[20].as_str()), ("3", "3"));
        let hi = list(symbols("hi", "latn"));
        assert_eq!((hi[19].as_str(), hi[20].as_str()), ("3", "2"));
        let g = |p| percent_grouping(p);
        assert_eq!(g("#,##0%"), (3, 3));
        assert_eq!(g("#,##,##0%"), (3, 2));
        assert_eq!(g("% #,#0;% -#,#0"), (2, 2));
        assert_eq!(g("#0%"), (0, 0));
        assert_eq!(g("'#,#'#,##0.00%"), (3, 3));
    }

    #[test]
    fn explicit_percent_negative_with_marked_signs() {
        // No CLDR 48.2 locale has both; the sign's marks must still go
        // around the explicit pattern's sign, and the plus sign must take
        // its place.
        let minus = Sign::new("\u{200e}-", "");
        let plus = Sign::new("\u{200e}+", "");
        assert_eq!(
            percent_patterns("#0%;(-#0%)", "%\u{200e}", &plus, &minus),
            [
                "{number}{percentSign}\u{200e}",
                "(\u{200e}{plusSign}{number}{percentSign}\u{200e})",
                "(\u{200e}{minusSign}{number}{percentSign}\u{200e})",
            ]
        );
    }

    #[test]
    fn percent_pattern_syntax() {
        let p = |s| convert_cldr_pattern(s);
        assert_eq!(
            p("¤#,##0.00;(¤#,##0.00)"),
            (
                "{currencyPrefix}{number}".into(),
                Some("({currencyPrefix}{number})".into())
            )
        );
        assert_eq!(
            p("#,##0.00\u{a0}¤"),
            ("{number}\u{a0}{currencySuffix}".into(), None)
        );
        assert_eq!(p("#,##0%"), ("{number}{percentSign}".into(), None));
        assert_eq!(p("%#,##0"), ("{percentSign}{number}".into(), None));
        assert_eq!(p("#,##0'%'x%"), ("{number}%x{percentSign}".into(), None));
        assert_eq!(p("'#'''#0%"), ("#'{number}{percentSign}".into(), None));
        assert_eq!(
            p("%#0;(-#0%)"),
            (
                "{percentSign}{number}".into(),
                Some("({minusSign}{number}{percentSign})".into())
            )
        );
    }

    #[test]
    fn currency_patterns_of_locales() {
        let cp = |l, c, d, a| list(currency_patterns(l, "latn", c, d, a));
        let en = cp("en", "USD", "symbol", false);
        assert_eq!(
            en,
            [
                "{currencyPrefix}{number}",
                "{plusSign}{currencyPrefix}{number}",
                "{minusSign}{currencyPrefix}{number}",
                "$",
                "{currencyPrefix}{number}",
                "{plusSign}{currencyPrefix}{number}",
                "{minusSign}{currencyPrefix}{number}",
            ]
        );
        assert_eq!(
            cp("en", "USD", "symbol", true)[2],
            "({currencyPrefix}{number})"
        );
        let code = cp("en", "USD", "code", false);
        assert_eq!(code[0], "{currencyPrefix}\u{a0}{number}");
        assert_eq!(code[3], "USD");
        // Next to a number that is not a digit (NaN, infinity) there is no
        // currency spacing: ICU4C gives "USDNaN", "USD∞", "-USD∞", and with
        // accounting "(USD∞)"; de gives "NaN EUR" from its own pattern.
        assert_eq!(
            code[4..],
            [
                "{currencyPrefix}{number}",
                "{plusSign}{currencyPrefix}{number}",
                "{minusSign}{currencyPrefix}{number}"
            ]
        );
        assert_eq!(
            cp("en", "USD", "code", true)[6],
            "({currencyPrefix}{number})"
        );
        assert_eq!(
            cp("ee", "USD", "code", true)[6],
            "({currencyPrefix}{number})"
        );
        assert_eq!(
            cp("en", "CHF", "symbol", false)[4],
            "{currencyPrefix}{number}"
        );
        let de = cp("de", "EUR", "code", false);
        assert_eq!(de[0], "{number}\u{a0}{currencySuffix}");
        assert_eq!(de[4], "{number}\u{a0}{currencySuffix}");
        assert_eq!(de[6], "{minusSign}{number}\u{a0}{currencySuffix}");
        let de = cp("de", "EUR", "symbol", false);
        assert_eq!(de[0], "{number}\u{a0}{currencySuffix}");
        assert_eq!(de[3], "€");
        let name = cp("en", "USD", "name", false);
        assert_eq!(name.len(), 6);
        assert_eq!(name[1], "{0} US dollar");
        assert_eq!(name[5], "{0} US dollars");
        // en has no "two": it gets other's.
        assert_eq!(name[2], "{0} US dollars");
        assert_eq!(cp("ja", "USD", "name", false)[5], "{0}米ドル");
        assert_eq!(cp("en", "ABC", "symbol", false)[3], "ABC");
        assert_eq!(cp("en", "ABC", "name", false)[5], "{0} ABC");
        assert_eq!(cp("en", "CAD", "narrowSymbol", false)[3], "$");
        assert_eq!(cp("en", "CAD", "symbol", false)[3], "CA$");
        assert_eq!(cp("en-CA", "CAD", "symbol", false)[3], "$");
        // Symbols CLDR gives as the code itself, with the spacing a code
        // gets.
        let af = cp("af", "USD", "symbol", false);
        assert_eq!(af[0], "{currencyPrefix}\u{a0}{number}");
        assert_eq!(af[3], "USD");
        assert_eq!(cp("bg", "JPY", "symbol", false)[3], "JPY");
        assert_eq!(cp("bg", "CAD", "narrowSymbol", false)[3], "CAD");
        assert_eq!(cp("ast", "XXX", "symbol", false)[3], "XXX");
        // Explicit negative patterns carry the sign; the positive pattern
        // puts the plus sign in its place.
        let nl = cp("nl", "EUR", "symbol", false);
        assert_eq!(nl[1], "{currencyPrefix}\u{a0}{plusSign}{number}");
        assert_eq!(nl[2], "{currencyPrefix}\u{a0}{minusSign}{number}");
        let fy = cp("fy", "EUR", "symbol", false);
        assert_eq!(fy[1], "{currencyPrefix}\u{a0}{number}{plusSign}");
        // The accounting negative has no minus sign; the positive pattern
        // gets the plus sign before the whole pattern.
        let acc = cp("en", "USD", "symbol", true);
        assert_eq!(acc[1], "{plusSign}{currencyPrefix}{number}");
        // ar-EG's explicit negative pattern keeps its RLM before the sign.
        let ar = cp("ar-EG", "EGP", "symbol", false);
        assert_eq!(
            ar[2],
            "\u{200f}\u{200e}{minusSign}{number}\u{a0}{currencySuffix}"
        );
    }

    #[test]
    fn currency_spacing() {
        // A letter next to the number selects the alpha-next-to-number
        // pattern; a symbol (general category S) does not.
        let cp = |l, c, d| list(currency_patterns(l, "latn", c, d, false));
        assert_eq!(
            cp("en", "CHF", "symbol")[0],
            "{currencyPrefix}\u{a0}{number}"
        );
        assert_eq!(cp("en", "EUR", "symbol")[0], "{currencyPrefix}{number}");
        // CA$ ends in a symbol, which is the end next to the number.
        assert_eq!(cp("en", "CAD", "symbol")[0], "{currencyPrefix}{number}");
        // ee has no accounting alpha-next-to-number negative pattern, so
        // the accounting negative gets CLDR's spacing (ICU4C: "(USD 1.00)").
        let ee = list(currency_patterns("ee", "latn", "USD", "code", true));
        assert_eq!(ee[2], "({currencyPrefix}\u{a0}{number})");
        assert_eq!(ee[0], "{currencyPrefix}\u{a0}{number}");
        let ee = list(currency_patterns("ee", "latn", "USD", "symbol", true));
        assert_eq!(ee[2], "({currencyPrefix}{number})");
        let ee_tg = list(currency_patterns("ee-TG", "latn", "USD", "code", true));
        assert_eq!(ee_tg[2], "({currencyPrefix}\u{a0}{number})");
        // en-ZA's standard pattern has no such variant at all (ICU4C:
        // "-USD 1,00").
        let za = list(currency_patterns("en-ZA", "latn", "USD", "code", false));
        assert_eq!(za[2], "{minusSign}{currencyPrefix}\u{a0}{number}");
        assert!(is_alpha_next_to_number('A'));
        assert!(is_alpha_next_to_number('.'));
        assert!(!is_alpha_next_to_number('$'));
        assert!(!is_alpha_next_to_number('\u{a0}'));
    }

    #[test]
    fn compact_patterns_of_locales() {
        let cp = |l, long| list(compact_patterns(l, "latn", long));
        let en = cp("en", false);
        let n: usize = en[0].parse().unwrap();
        assert_eq!(en.len(), 1 + 10 * n);
        let entry = |v: &[String], i: usize| v[1 + 10 * i..11 + 10 * i].to_vec();
        let mags: Vec<&str> = (0..n).map(|i| en[1 + 10 * i].as_str()).collect();
        assert_eq!(mags, ["3", "6", "9", "12"]);
        let exps: Vec<&str> = (0..n).map(|i| en[2 + 10 * i].as_str()).collect();
        assert_eq!(exps, ["3", "6", "9", "12"]);
        let others: Vec<&str> = (0..n).map(|i| en[10 + 10 * i].as_str()).collect();
        assert_eq!(others, ["{0}K", "{0}M", "{0}B", "{0}T"]);
        // en short has one pattern per entry, so every category gets it.
        assert_eq!(
            entry(&en, 0),
            ["3", "3", "", "", "{0}K", "{0}K", "{0}K", "{0}K", "{0}K", "{0}K"]
        );

        let fr = cp("fr", true);
        assert_eq!(fr[1], "3");
        // CLDR 48.2 fr long: 1000-count-1 "mille", 1000-count-one
        // "0 millier", 1000-count-other "0 mille". fr has no zero, two or
        // few, which get other's, and CLDR has no many, which inherits
        // other's.
        assert_eq!(
            entry(&fr, 0),
            [
                "3",
                "3",
                "",
                "mille",
                "{0} mille",
                "{0} millier",
                "{0} mille",
                "{0} mille",
                "{0} mille",
                "{0} mille"
            ]
        );

        let de = cp("de", false);
        assert_ne!(de[1], "3");
        assert_eq!(de[1], "6");

        let ja = cp("ja", false);
        assert_eq!(&ja[1..3], ["4", "4"]);
        assert_eq!(ja[10], "{0}万");

        let en_long = cp("en", true);
        let n: usize = en_long[0].parse().unwrap();
        let million = (0..n)
            .map(|i| entry(&en_long, i))
            .find(|e| e[0] == "6")
            .unwrap();
        assert_eq!(million[9], "{0} million");
    }

    #[test]
    fn addressing_rules_select_every_category() {
        let rules = addressing_rules();
        assert_eq!(rules.categories().count(), 6);
        for c in CATEGORIES {
            assert_eq!(rules.category_for(addressing_operands(c, false)), c);
            assert_eq!(rules.category_for(addressing_operands(c, true)), c);
        }
        assert!(addressing_operands(PluralCategory::Zero, true) == PluralOperands::from(0u32));
        assert!(addressing_operands(PluralCategory::One, true) == PluralOperands::from(1u32));
        assert!(addressing_operands(PluralCategory::One, false) != PluralOperands::from(1u32));
    }

    #[test]
    fn currency_digits_follow_cldr() {
        assert_eq!(currency_digits("USD"), Some(2));
        assert_eq!(currency_digits("JPY"), Some(0));
        assert_eq!(currency_digits("KWD"), Some(3));
        assert_eq!(currency_digits("CLF"), Some(4));
        assert_eq!(currency_digits("XXX"), Some(2));
        assert_eq!(currency_digits("ABC"), Some(2));
    }

    #[test]
    fn rejects_bad_values() {
        assert_eq!(currency_patterns("en", "latn", "USD", "long", false), None);
        assert_eq!(
            currency_patterns("en", "latn", "usd", "symbol", false),
            None
        );
        assert_eq!(currency_patterns("en", "latn", "US", "code", false), None);
        assert_eq!(currency_patterns("en", "latn", "USDX", "name", false), None);
        assert_eq!(currency_digits("usd"), None);
        assert_eq!(currency_digits("US"), None);
        assert_eq!(currency_digits("U1D"), None);
        assert_eq!(symbols("en_US", "latn"), None);
        assert_eq!(compact_patterns("en_US", "latn", false), None);
        assert_eq!(
            currency_patterns("en_US", "latn", "USD", "symbol", false),
            None
        );
        // A numbering system must be 3 to 8 lowercase letters or digits.
        assert_eq!(symbols("en", ""), None);
        assert_eq!(symbols("en", "Latn"), None);
        assert_eq!(compact_patterns("en", "la-tn", true), None);
    }

    /// The number of times `needle` occurs in `s`.
    fn count(s: &str, needle: &str) -> usize {
        s.matches(needle).count()
    }

    /// Check a currency pattern list of a symbol-like display.
    fn check_currency(what: &str, v: &[String], accounting: bool) {
        assert_eq!(v.len(), 7, "{what}");
        assert!(!v[3].is_empty(), "{what}");
        for (i, p) in v[..3].iter().chain(&v[4..]).enumerate() {
            assert_eq!(count(p, "{number}"), 1, "{what} {i}: {p}");
            assert_eq!(
                count(p, "{currencyPrefix}") + count(p, "{currencySuffix}"),
                1,
                "{what} {i}: {p}"
            );
        }
        for t in [&v[..3], &v[4..]] {
            assert_eq!(count(&t[0], "{plusSign}") + count(&t[0], "{minusSign}"), 0);
            assert_eq!(count(&t[1], "{plusSign}"), 1, "{what}: {}", t[1]);
            assert_eq!(count(&t[1], "{minusSign}"), 0, "{what}: {}", t[1]);
            assert_eq!(count(&t[2], "{plusSign}"), 0, "{what}: {}", t[2]);
            if !accounting {
                assert_eq!(count(&t[2], "{minusSign}"), 1, "{what}: {}", t[2]);
            }
        }
        // CLDR's currency spacing: an ISO code never touches the number.
        if what.contains(" code ") {
            for p in &v[..3] {
                assert!(
                    !p.contains("{currencyPrefix}{number}")
                        && !p.contains("{number}{currencySuffix}"),
                    "{what}: {p}"
                );
            }
        }
    }

    /// Lookups that miss on their way (resolving a numbering system, a
    /// currency with no name) print nothing: debug builds of ICU4X report a
    /// data error on stderr unless the request is silent. The test reruns itself
    /// in a child process to see that process's stderr.
    #[test]
    fn misses_are_silent() {
        if std::env::var_os("HERMES_INTL_SILENT_CHILD").is_some() {
            for (locale, nu) in [("en", "arab"), ("ar", "bali"), ("ar-EG", "bali")] {
                assert!(symbols(locale, nu).is_some());
                assert!(compact_patterns(locale, nu, true).is_some());
                for display in ["symbol", "name"] {
                    assert!(currency_patterns(locale, nu, "ABC", display, true).is_some());
                }
            }
            assert!(crate::plurals::new_plural_rules("ku-Latn-IQ", true).is_some());
            eprintln!("child done");
            return;
        }
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["number::tests::misses_are_silent", "--exact", "--nocapture"])
            .env("HERMES_INTL_SILENT_CHILD", "1")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{stderr}");
        assert!(stderr.contains("child done"), "{stderr}");
        assert!(!stderr.contains("ICU4X"), "{stderr}");
    }

    /// Every locale has every list (the plan's Review Focus 5).
    #[test]
    fn every_locale_has_data() {
        for locale in crate::locale::test_locales() {
            let default = crate::info::default_numbering_system(locale).unwrap();
            for nu in [default.as_str(), "latn"] {
                let s = symbols(locale, nu).unwrap_or_else(|| panic!("{locale} {nu}"));
                assert_eq!(s.len(), 22, "{locale} {nu}");
                for (i, x) in s.iter().enumerate() {
                    assert!(!x.is_empty(), "{locale} {nu} symbol {i}");
                }
                for x in &s[2..5] {
                    assert!(
                        !x.contains(['\u{200e}', '\u{200f}', '\u{61c}']),
                        "{locale} {nu} sign {x:?}"
                    );
                }
                for (i, p) in s[12..18].iter().enumerate() {
                    assert_eq!(count(p, "{number}"), 1, "{locale} {nu} {i}: {p}");
                }
                for p in [&s[12], &s[15]] {
                    assert!(
                        !p.contains("{plusSign}") && !p.contains("{minusSign}"),
                        "{locale} {nu}: {p}"
                    );
                }
                assert_eq!(count(&s[13], "{plusSign}"), 1, "{locale} {nu}");
                assert_eq!(count(&s[14], "{minusSign}"), 1, "{locale} {nu}");
                assert_eq!(count(&s[16], "{plusSign}"), 1, "{locale} {nu}");
                assert_eq!(count(&s[17], "{minusSign}"), 1, "{locale} {nu}");
                for p in &s[15..18] {
                    assert_eq!(count(p, "{percentSign}"), 1, "{locale} {nu}: {p}");
                }
                assert_eq!(count(&s[18], "{0}"), 1, "{locale} {nu}");
                assert_eq!(count(&s[18], "{1}"), 1, "{locale} {nu}");

                for currency in ["USD", "EUR", "JPY", "XXX", "ABC"] {
                    for accounting in [false, true] {
                        for display in ["code", "symbol", "narrowSymbol"] {
                            let what = format!("{locale} {nu} {currency} {display} {accounting}");
                            let v = currency_patterns(locale, nu, currency, display, accounting)
                                .unwrap_or_else(|| panic!("{what}"));
                            check_currency(&what, &v, accounting);
                        }
                        let v = currency_patterns(locale, nu, currency, "name", accounting)
                            .unwrap_or_else(|| panic!("{locale} {nu} {currency} name"));
                        assert_eq!(v.len(), 6);
                        for p in &v {
                            assert_eq!(count(p, "{0}"), 1, "{locale} {currency} name: {p}");
                            assert!(!p.contains('\0'), "{locale} {currency} name: {p:?}");
                            assert!(
                                !p.replace("{0}", "").trim().is_empty(),
                                "{locale} {currency} name: {p:?}"
                            );
                        }
                    }
                }

                for long in [false, true] {
                    let what = format!("{locale} {nu} compact long {long}");
                    let v = compact_patterns(locale, nu, long).unwrap_or_else(|| panic!("{what}"));
                    let n: usize = v[0].parse().unwrap();
                    assert!(n > 0, "{what}");
                    assert_eq!(v.len(), 1 + 10 * n, "{what}");
                    let mut last = 0;
                    for e in v[1..].chunks(10) {
                        let magnitude: u8 = e[0].parse().unwrap();
                        let exponent: u8 = e[1].parse().unwrap();
                        assert!(magnitude > last && exponent <= magnitude, "{what}");
                        last = magnitude;
                        for p in &e[2..] {
                            assert!(count(p, "{0}") <= 1, "{what}: {p}");
                        }
                        for p in &e[4..] {
                            assert!(!p.is_empty(), "{what}");
                        }
                        assert_eq!(count(&e[9], "{0}"), 1, "{what}: {}", e[9]);
                    }
                }
            }
        }
    }
}
