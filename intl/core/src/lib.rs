// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The C interface of the Hermes Intl plugin's ICU4X core. Every function
//! here is a thin wrapper; the logic lives in the modules. C++ calls these
//! through intl/jsi/RustCore.h.

/// A UTF-8 string owned by Rust and handed to C++, which must free it with
/// hi_string_free.
#[repr(C)]
pub struct HiString {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl HiString {
    /// The value written to an out-parameter when there is no result.
    const EMPTY: HiString = HiString {
        data: std::ptr::null_mut(),
        len: 0,
        capacity: 0,
    };

    fn from_string(s: String) -> HiString {
        let mut bytes = std::mem::ManuallyDrop::new(s.into_bytes());
        HiString {
            data: bytes.as_mut_ptr(),
            len: bytes.len(),
            capacity: bytes.capacity(),
        }
    }
}

/// Free a string returned by this library. Accepts the empty value.
///
/// # Safety
/// `s` must come from this library and must not be freed twice.
#[no_mangle]
pub unsafe extern "C" fn hi_string_free(s: HiString) {
    if !s.data.is_null() {
        drop(Vec::from_raw_parts(s.data, s.len, s.capacity));
    }
}

/// A UTF-16 string owned by Rust and handed to C++, which must free it with
/// hi_string16_free.
#[repr(C)]
pub struct HiString16 {
    pub data: *mut u16,
    pub len: usize,
    pub capacity: usize,
}

impl HiString16 {
    /// The value written to an out-parameter when there is no result.
    const EMPTY: HiString16 = HiString16 {
        data: std::ptr::null_mut(),
        len: 0,
        capacity: 0,
    };

    fn from_units(v: Vec<u16>) -> HiString16 {
        let mut units = std::mem::ManuallyDrop::new(v);
        HiString16 {
            data: units.as_mut_ptr(),
            len: units.len(),
            capacity: units.capacity(),
        }
    }
}

/// Free a string returned by this library. Accepts the empty value.
///
/// # Safety
/// `s` must come from this library and must not be freed twice.
#[no_mangle]
pub unsafe extern "C" fn hi_string16_free(s: HiString16) {
    if !s.data.is_null() {
        drop(Vec::from_raw_parts(s.data, s.len, s.capacity));
    }
}

mod casemap;
mod cldr;
mod collator;
mod date_data;
mod datetime;
mod displaynames;
mod host;
mod info;
mod list;
mod locale;
mod number;
mod plurals;
mod relativetime;
mod segmenter;
mod supported;
mod tz;
mod units;

/// View (data, len) as UTF-8; None if it is not.
///
/// # Safety
/// `data` must point to `len` readable bytes, or `len` must be 0.
unsafe fn as_str<'a>(data: *const u8, len: usize) -> Option<&'a str> {
    if len == 0 {
        return Some("");
    }
    std::str::from_utf8(std::slice::from_raw_parts(data, len)).ok()
}

/// Write `result` to `out` and say whether there was one.
///
/// # Safety
/// `out` must be valid for writes.
unsafe fn write_out(out: *mut HiString, result: Option<String>) -> bool {
    match result {
        Some(s) => {
            *out = HiString::from_string(s);
            true
        }
        None => {
            *out = HiString::EMPTY;
            false
        }
    }
}

/// IsStructurallyValidLanguageTag and CanonicalizeUnicodeLocaleId.
///
/// # Safety
/// `tag` must point to `len` readable bytes; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_canonicalize(
    tag: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(out, as_str(tag, len).and_then(locale::canonicalize))
}

/// Add Likely Subtags: when it returns true, `out` is the maximized form of
/// the canonical tag. False if `tag` does not parse.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_add_likely_subtags(
    tag: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(out, as_str(tag, len).and_then(locale::add_likely_subtags))
}

/// Remove Likely Subtags: when it returns true, `out` is the minimized form
/// of the canonical tag. False if `tag` does not parse.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_remove_likely_subtags(
    tag: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(
        out,
        as_str(tag, len).and_then(locale::remove_likely_subtags),
    )
}

/// Whether `tag` (canonical, without extensions) is an available locale.
///
/// # Safety
/// `tag` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_is_available(tag: *const u8, len: usize) -> bool {
    as_str(tag, len).is_some_and(locale::is_available)
}

/// A host locale name to a DefaultLocale candidate: when it returns true,
/// `out` is a canonical tag with no extensions that is an available locale.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_from_host(
    raw: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(out, as_str(raw, len).and_then(host::locale_from_host))
}

/// An explicit default locale (RuntimeConfig's IntlDefaultLocales) to a
/// DefaultLocale: when it returns true, `out` is `tag` canonicalized, with
/// its extensions dropped and reduced to an available locale of the same
/// script that has data in that script. False if `tag` is not well-formed
/// or no prefix of it is usable; callers that must tell those apart check
/// well-formedness with hi_locale_canonicalize first. hi_locale_from_host
/// ends in the same reduction.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_resolve_default(
    tag: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(out, as_str(tag, len).and_then(locale::resolve_default))
}

/// A TZ value or /etc/localtime link target to a primary IANA name.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_time_zone_from_host(
    raw: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(out, as_str(raw, len).and_then(host::time_zone_from_host))
}

/// A case-insensitive IANA name to its primary identifier.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_time_zone_canonicalize(
    name: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(
        out,
        as_str(name, len).and_then(host::time_zone_canonicalize),
    )
}

/// The CLDR calendar preferences of a region: when it returns true, `out` is
/// the calendar types, most preferred first, separated by single spaces. A
/// region with no line of its own gets the `001` line; false only if
/// `region` does not parse as a region.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_calendar_preferences(
    region: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(
        out,
        as_str(region, len)
            .and_then(info::calendar_preferences)
            .map(|v| v.join(" ")),
    )
}

/// The CLDR hour cycles of a region or `<language>-<region>` key: when it
/// returns true, `out` is the hour cycles, preferred first, separated by
/// single spaces. A region with no line of its own gets the `001` line;
/// false for a `<language>-<region>` key with no line, or a key that is
/// neither.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_hour_cycles(
    key: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(
        out,
        as_str(key, len)
            .and_then(info::hour_cycles)
            .map(|v| v.join(" ")),
    )
}

/// The primary IANA ids of a region's time zones: when it returns true,
/// `out` is the ids, sorted, separated by single spaces (empty for a region
/// with none). False only if `region` is not UTF-8.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_time_zones_of_region(
    region: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(
        out,
        as_str(region, len).map(|r| info::time_zones_of_region(r).join(" ")),
    )
}

/// The week data of a region: when it returns true, `out` is the ISO number
/// of the first day of the week (Monday 1 to Sunday 7), then the weekend days
/// ascending, separated by single spaces. Any syntactically valid region
/// gets data (`001`'s when ICU4X has none for it); false only if `region`
/// does not parse as a region.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_week_info(
    region: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(
        out,
        as_str(region, len)
            .and_then(info::week_info)
            .map(|(first, weekend)| {
                let mut s = first.to_string();
                for day in weekend {
                    s.push(' ');
                    s.push_str(&day.to_string());
                }
                s
            }),
    )
}

/// The writing direction of a script: when it returns true, `out` is `ltr`
/// or `rtl`. False if the script is unknown or `script` is not a script
/// code.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_script_direction(
    script: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(
        out,
        as_str(script, len)
            .and_then(info::script_direction)
            .map(str::to_owned),
    )
}

/// The numbering system of a locale's decimal data. False if `locale` does
/// not parse or has no data.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_default_numbering_system(
    locale: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(
        out,
        as_str(locale, len).and_then(info::default_numbering_system),
    )
}

/// The collation types a locale has tailorings for: when it returns true,
/// `out` is the types, sorted, separated by single spaces. False only if
/// `locale` is not UTF-8.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_collations(
    locale: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(
        out,
        as_str(locale, len).map(|l| info::collations_of_locale(l).join(" ")),
    )
}

/// Which list hi_supported_values returns.
#[repr(C)]
pub enum HiValueKind {
    Calendar,
    Collation,
    Currency,
    NumberingSystem,
    TimeZone,
}

/// The values for Intl.supportedValuesOf, sorted, separated by single spaces.
///
/// # Safety
/// `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_supported_values(kind: HiValueKind, out: *mut HiString) {
    let kind = match kind {
        HiValueKind::Calendar => supported::Kind::Calendar,
        HiValueKind::Collation => supported::Kind::Collation,
        HiValueKind::Currency => supported::Kind::Currency,
        HiValueKind::NumberingSystem => supported::Kind::NumberingSystem,
        HiValueKind::TimeZone => supported::Kind::TimeZone,
    };
    *out = HiString::from_string(supported::values(kind).join(" "));
}

/// A collator: an ICU4X collator for one set of resolved Intl.Collator
/// options. Opaque to C++, which frees it with hi_collator_free.
pub struct HiCollator(icu::collator::CollatorBorrowed<'static>);

// A NativeState holding a HiCollator can be destroyed on any thread.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<HiCollator>();
};

/// Make a collator. `locale` is a canonical tag (only its language
/// identifier is used), `collation` is "default" or a supported collation,
/// `sensitivity` is "base", "accent", "case" or "variant" and `case_first`
/// is "upper", "lower" or "false". Null for any other value, or if there is
/// no data.
///
/// # Safety
/// Each string must point to its length of readable bytes, or the length
/// must be 0.
#[no_mangle]
pub unsafe extern "C" fn hi_collator_new(
    locale: *const u8,
    locale_len: usize,
    collation: *const u8,
    collation_len: usize,
    sensitivity: *const u8,
    sensitivity_len: usize,
    ignore_punctuation: bool,
    numeric: bool,
    case_first: *const u8,
    case_first_len: usize,
) -> *mut HiCollator {
    let (Some(locale), Some(collation), Some(sensitivity), Some(case_first)) = (
        as_str(locale, locale_len),
        as_str(collation, collation_len),
        as_str(sensitivity, sensitivity_len),
        as_str(case_first, case_first_len),
    ) else {
        return std::ptr::null_mut();
    };
    match collator::new_collator(
        locale,
        collation,
        sensitivity,
        ignore_punctuation,
        numeric,
        case_first,
    ) {
        Some(c) => Box::into_raw(Box::new(HiCollator(c))),
        None => std::ptr::null_mut(),
    }
}

/// Free a collator from hi_collator_new. Accepts null.
///
/// # Safety
/// `collator` must be null or come from hi_collator_new, and must not be
/// freed twice.
#[no_mangle]
pub unsafe extern "C" fn hi_collator_free(collator: *mut HiCollator) {
    if !collator.is_null() {
        drop(Box::from_raw(collator));
    }
}

/// CompareStrings on two UTF-16 strings: -1, 0 or 1. An unpaired surrogate
/// compares as U+FFFD.
///
/// # Safety
/// `collator` must be valid; each string must point to its length of
/// readable u16 units, or the length must be 0.
#[no_mangle]
pub unsafe extern "C" fn hi_collator_compare(
    collator: *const HiCollator,
    x: *const u16,
    x_len: usize,
    y: *const u16,
    y_len: usize,
) -> i32 {
    let units = |p: *const u16, len: usize| -> &[u16] {
        if len == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(p, len)
        }
    };
    debug_assert!(!collator.is_null());
    (*collator)
        .0
        .compare_utf16(units(x, x_len), units(y, y_len)) as i32
}

/// The full upper (`upper` true) or lower case mapping of the UTF-16 text,
/// with the tailoring of `language` ("und", or a language tag). Unpaired
/// surrogates are copied through unchanged. When it returns true, `out` is
/// the result; false if `language` is not UTF-8 or does not parse.
///
/// # Safety
/// `text` must point to `len` readable u16 units, or `len` must be 0;
/// `language` must point to `language_len` readable bytes, or the length
/// must be 0; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_case_map(
    text: *const u16,
    len: usize,
    language: *const u8,
    language_len: usize,
    upper: bool,
    out: *mut HiString16,
) -> bool {
    let units: &[u16] = if len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(text, len)
    };
    match as_str(language, language_len).and_then(|l| casemap::case_map(units, l, upper)) {
        Some(v) => {
            *out = HiString16::from_units(v);
            true
        }
        None => {
            *out = HiString16::EMPTY;
            false
        }
    }
}

/// ICU4X's default "<caseFirst> <ignorePunctuation>" for the language
/// identifier of `locale`, for example "upper false". False if `locale`
/// does not parse.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_collator_locale_defaults(
    locale: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_out(out, as_str(locale, len).and_then(collator::locale_defaults))
}

/// Plural rules: ICU4X's plural rules and plural ranges for one locale and
/// type. Opaque to C++, which frees them with hi_plural_rules_free.
pub struct HiPluralRules(icu::plurals::PluralRulesWithRanges<icu::plurals::PluralRules>);

// A NativeState holding a HiPluralRules can be destroyed on any thread.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<HiPluralRules>();
};

/// Make the cardinal (`ordinal` false) or ordinal plural rules for the
/// language identifier of `locale` (a canonical tag). Null if `locale` is
/// not UTF-8, does not parse or has no data.
///
/// # Safety
/// `locale` must point to `len` readable bytes, or `len` must be 0.
#[no_mangle]
pub unsafe extern "C" fn hi_plural_rules_new(
    locale: *const u8,
    len: usize,
    ordinal: bool,
) -> *mut HiPluralRules {
    match as_str(locale, len).and_then(|l| plurals::new_plural_rules(l, ordinal)) {
        Some(r) => Box::into_raw(Box::new(HiPluralRules(r))),
        None => std::ptr::null_mut(),
    }
}

/// Free plural rules from hi_plural_rules_new. Accepts null.
///
/// # Safety
/// `rules` must be null or come from hi_plural_rules_new, and must not be
/// freed twice.
#[no_mangle]
pub unsafe extern "C" fn hi_plural_rules_free(rules: *mut HiPluralRules) {
    if !rules.is_null() {
        drop(Box::from_raw(rules));
    }
}

/// The plural category of `decimal` × 10^`exponent`: when it returns true,
/// `out` is "zero", "one", "two", "few", "many" or "other". `decimal` is
/// ASCII digits with an optional "." between digits; false for anything
/// else. The operands are exact for any number of digits; `c` is
/// `exponent` when it is positive, else 0.
///
/// # Safety
/// `rules` must be valid; `decimal` must point to `len` readable bytes, or
/// `len` must be 0; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_plural_rules_select(
    rules: *const HiPluralRules,
    decimal: *const u8,
    len: usize,
    exponent: i32,
    out: *mut HiString,
) -> bool {
    debug_assert!(!rules.is_null());
    write_out(
        out,
        as_str(decimal, len)
            .and_then(|d| plurals::select(&(*rules).0, d, exponent))
            .map(|c| plurals::category_name(c).to_owned()),
    )
}

/// The plural category of a range whose ends have the categories `start`
/// and `end` (CLDR names): when it returns true, `out` is a category name.
/// False if either is not a category name.
///
/// # Safety
/// `rules` must be valid; each string must point to its length of readable
/// bytes, or the length must be 0; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_plural_rules_select_range(
    rules: *const HiPluralRules,
    start: *const u8,
    start_len: usize,
    end: *const u8,
    end_len: usize,
    out: *mut HiString,
) -> bool {
    debug_assert!(!rules.is_null());
    let (Some(start), Some(end)) = (as_str(start, start_len), as_str(end, end_len)) else {
        *out = HiString::EMPTY;
        return false;
    };
    write_out(
        out,
        plurals::select_range(&(*rules).0, start, end)
            .map(|c| plurals::category_name(c).to_owned()),
    )
}

/// The categories of `rules`, in the order zero, one, two, few, many,
/// other, separated by U+0000 like every list (write_list); "other" is
/// always last. Always returns true.
///
/// # Safety
/// `rules` must be valid; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_plural_rules_categories(
    rules: *const HiPluralRules,
    out: *mut HiString,
) -> bool {
    debug_assert!(!rules.is_null());
    let categories = plurals::categories(&(*rules).0);
    write_list(
        out,
        Some(categories.into_iter().map(str::to_owned).collect()),
    )
}

/// Write a list to `out` as its entries separated by U+0000 (an entry may
/// be empty; none contains U+0000), and say whether there was one.
///
/// # Safety
/// `out` must be valid for writes.
unsafe fn write_list(out: *mut HiString, result: Option<Vec<String>>) -> bool {
    debug_assert!(result
        .as_ref()
        .is_none_or(|v| !v.is_empty() && v.iter().all(|s| !s.contains('\0'))));
    write_out(out, result.map(|v| v.join("\0")))
}

/// NumberFormat's symbols and decimal and percent patterns for the language
/// identifier of `locale` (a canonical tag) and the numbering system `nu`:
/// when it returns true, `out` is the 23 entries of number::symbols,
/// separated by U+0000. False if either string is not UTF-8, `locale` does
/// not parse or `nu` is not a numbering system type.
///
/// # Safety
/// Each string must point to its length of readable bytes, or the length
/// must be 0; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_number_symbols(
    locale: *const u8,
    locale_len: usize,
    nu: *const u8,
    nu_len: usize,
    out: *mut HiString,
) -> bool {
    let (Some(locale), Some(nu)) = (as_str(locale, locale_len), as_str(nu, nu_len)) else {
        *out = HiString::EMPTY;
        return false;
    };
    write_list(out, number::symbols(locale, nu))
}

/// NumberFormat's currency patterns for `locale`, `nu`, the currency
/// `currency` (three uppercase letters) and the currencyDisplay `display`
/// ("code", "symbol", "narrowSymbol" or "name"), standard or `accounting`:
/// when it returns true, `out` is the entries of number::currency_patterns,
/// separated by U+0000. False for any other value, or as
/// hi_number_symbols.
///
/// # Safety
/// As hi_number_symbols.
#[no_mangle]
pub unsafe extern "C" fn hi_number_currency_patterns(
    locale: *const u8,
    locale_len: usize,
    nu: *const u8,
    nu_len: usize,
    currency: *const u8,
    currency_len: usize,
    display: *const u8,
    display_len: usize,
    accounting: bool,
    out: *mut HiString,
) -> bool {
    let (Some(locale), Some(nu), Some(currency), Some(display)) = (
        as_str(locale, locale_len),
        as_str(nu, nu_len),
        as_str(currency, currency_len),
        as_str(display, display_len),
    ) else {
        *out = HiString::EMPTY;
        return false;
    };
    write_list(
        out,
        number::currency_patterns(locale, nu, currency, display, accounting),
    )
}

/// NumberFormat's patterns for compact notation with the currency style,
/// for `locale`, `nu`, the currency `currency` (three uppercase letters)
/// and the currencyDisplay `display` ("code", "symbol" or "narrowSymbol"),
/// standard or `accounting`: when it returns true, `out` is the entries of
/// number::compact_currency_patterns, separated by U+0000. False for any
/// other value, or as hi_number_symbols.
///
/// # Safety
/// As hi_number_symbols.
#[no_mangle]
pub unsafe extern "C" fn hi_number_compact_currency_patterns(
    locale: *const u8,
    locale_len: usize,
    nu: *const u8,
    nu_len: usize,
    currency: *const u8,
    currency_len: usize,
    display: *const u8,
    display_len: usize,
    accounting: bool,
    out: *mut HiString,
) -> bool {
    let (Some(locale), Some(nu), Some(currency), Some(display)) = (
        as_str(locale, locale_len),
        as_str(nu, nu_len),
        as_str(currency, currency_len),
        as_str(display, display_len),
    ) else {
        *out = HiString::EMPTY;
        return false;
    };
    write_list(
        out,
        number::compact_currency_patterns(locale, nu, currency, display, accounting),
    )
}

/// The currency style's decimal and grouping separators for `locale`, `nu`
/// and the currency `currency` (three uppercase letters): when it returns
/// true, `out` is the two entries of number::currency_separators, separated
/// by U+0000. False for any other currency, or as hi_number_symbols.
///
/// # Safety
/// As hi_number_symbols.
#[no_mangle]
pub unsafe extern "C" fn hi_number_currency_separators(
    locale: *const u8,
    locale_len: usize,
    nu: *const u8,
    nu_len: usize,
    currency: *const u8,
    currency_len: usize,
    out: *mut HiString,
) -> bool {
    let (Some(locale), Some(nu), Some(currency)) = (
        as_str(locale, locale_len),
        as_str(nu, nu_len),
        as_str(currency, currency_len),
    ) else {
        *out = HiString::EMPTY;
        return false;
    };
    write_list(out, number::currency_separators(locale, nu, currency))
}

/// NumberFormat's compact patterns, short or (`long`) long, for `locale`
/// and `nu`: when it returns true, `out` is the entries of
/// number::compact_patterns, separated by U+0000. False as
/// hi_number_symbols.
///
/// # Safety
/// As hi_number_symbols.
#[no_mangle]
pub unsafe extern "C" fn hi_number_compact_patterns(
    locale: *const u8,
    locale_len: usize,
    nu: *const u8,
    nu_len: usize,
    long: bool,
    out: *mut HiString,
) -> bool {
    let (Some(locale), Some(nu)) = (as_str(locale, locale_len), as_str(nu, nu_len)) else {
        *out = HiString::EMPTY;
        return false;
    };
    write_list(out, number::compact_patterns(locale, nu, long))
}

/// NumberFormat's unit patterns for the language identifier of `locale`,
/// `unit` and `width`: when it returns true, `out` is the six entries of
/// units::unit_patterns, separated by U+0000. False if a string is not UTF-8,
/// `locale` does not parse, or `unit` or `width` is not one it accepts.
///
/// # Safety
/// Each string must point to its length of readable bytes, or the length
/// must be 0; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_unit_patterns(
    locale: *const u8,
    locale_len: usize,
    unit: *const u8,
    unit_len: usize,
    width: *const u8,
    width_len: usize,
    out: *mut HiString,
) -> bool {
    let (Some(locale), Some(unit), Some(width)) = (
        as_str(locale, locale_len),
        as_str(unit, unit_len),
        as_str(width, width_len),
    ) else {
        *out = HiString::EMPTY;
        return false;
    };
    write_list(out, units::unit_patterns(locale, unit, width))
}

/// DateTimeFormat's CLDR patterns for the language identifier of `locale`
/// (a canonical tag) and `calendar`: when it returns true, `out` is the
/// entries of date_data::date_time_patterns, separated by U+0000. False if a
/// string is not UTF-8, `locale` does not parse or `calendar` is not one of
/// the 16 supported calendars.
///
/// # Safety
/// Each string must point to its length of readable bytes, or the length
/// must be 0; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_date_time_patterns(
    locale: *const u8,
    locale_len: usize,
    calendar: *const u8,
    calendar_len: usize,
    out: *mut HiString,
) -> bool {
    let (Some(locale), Some(calendar)) =
        (as_str(locale, locale_len), as_str(calendar, calendar_len))
    else {
        *out = HiString::EMPTY;
        return false;
    };
    write_list(out, date_data::date_time_patterns(locale, calendar))
}

/// DateTimeFormat's flexible day periods for the language identifier of
/// `locale` (a canonical tag): when it returns true, `out` is the entries
/// of date_data::day_periods, separated by U+0000. False if `locale` is not
/// UTF-8 or does not parse.
///
/// # Safety
/// `locale` must point to `locale_len` readable bytes, or `locale_len` must
/// be 0; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_day_periods(
    locale: *const u8,
    locale_len: usize,
    out: *mut HiString,
) -> bool {
    let Some(locale) = as_str(locale, locale_len) else {
        *out = HiString::EMPTY;
        return false;
    };
    write_list(out, date_data::day_periods(locale))
}

/// A DateTimeFormat's native state: its ICU4X calendar, its time zone rules
/// and the names its patterns use. Opaque to C++, which frees it with
/// hi_date_time_format_free.
pub struct HiDateTimeFormat(datetime::DateTimeFormat);

// A NativeState holding a HiDateTimeFormat can be destroyed on any thread.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<HiDateTimeFormat>();
};

/// Make the native state of a DateTimeFormat with datetime::DateTimeFormat::new
/// from the language identifier of `locale` (a canonical tag), `calendar`
/// (a supported CLDR calendar id), `time_zone` (a primary identifier or
/// `±HH:MM`) and `symbols`, CLDR name symbols separated by single spaces
/// (empty for none). Null if a string is not UTF-8 or any value is not one
/// datetime::DateTimeFormat::new accepts.
///
/// # Safety
/// Each string must point to its length of readable bytes, or the length
/// must be 0.
#[no_mangle]
pub unsafe extern "C" fn hi_date_time_format_new(
    locale: *const u8,
    locale_len: usize,
    calendar: *const u8,
    calendar_len: usize,
    time_zone: *const u8,
    time_zone_len: usize,
    symbols: *const u8,
    symbols_len: usize,
) -> *mut HiDateTimeFormat {
    let (Some(locale), Some(calendar), Some(time_zone), Some(symbols)) = (
        as_str(locale, locale_len),
        as_str(calendar, calendar_len),
        as_str(time_zone, time_zone_len),
        as_str(symbols, symbols_len),
    ) else {
        return std::ptr::null_mut();
    };
    // An empty entry (a doubled, leading or trailing space) is no symbol, so
    // new rejects it.
    let symbols: Vec<&str> = if symbols.is_empty() {
        vec![]
    } else {
        symbols.split(' ').collect()
    };
    match datetime::DateTimeFormat::new(locale, calendar, time_zone, &symbols) {
        Some(f) => Box::into_raw(Box::new(HiDateTimeFormat(f))),
        None => std::ptr::null_mut(),
    }
}

/// Free the native state from hi_date_time_format_new. Accepts null.
///
/// # Safety
/// `format` must be null or come from hi_date_time_format_new, and must not
/// be freed twice.
#[no_mangle]
pub unsafe extern "C" fn hi_date_time_format_free(format: *mut HiDateTimeFormat) {
    if !format.is_null() {
        drop(Box::from_raw(format));
    }
}

/// The result of hi_date_time_format_local_time.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum HiLocalTimeStatus {
    /// `out` holds the local time and the names.
    Ok,
    /// `epoch_ms` is not an integer within ±8.64 × 10^15.
    NotATimeValue,
    /// ICU4X reported an error formatting a name.
    FormatFailed,
}

/// The local time of the time value `epoch_ms` in `format`'s calendar and
/// time zone, and the text of its symbols: when it returns Ok, `out` is the
/// entries of datetime::DateTimeFormat::local_time, separated by U+0000.
/// Otherwise `out` is empty, and the status says which of the two failures
/// it was.
///
/// # Safety
/// `format` must be valid; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_date_time_format_local_time(
    format: *const HiDateTimeFormat,
    epoch_ms: f64,
    out: *mut HiString,
) -> HiLocalTimeStatus {
    debug_assert!(!format.is_null());
    if !datetime::is_time_value(epoch_ms) {
        *out = HiString::EMPTY;
        return HiLocalTimeStatus::NotATimeValue;
    }
    if write_list(out, (*format).0.local_time(epoch_ms)) {
        HiLocalTimeStatus::Ok
    } else {
        HiLocalTimeStatus::FormatFailed
    }
}

/// CurrencyDigits: CLDR's number of fraction digits for `currency` (three
/// uppercase letters), 2 for a currency CLDR does not list; -1 if
/// `currency` is not three uppercase letters.
///
/// # Safety
/// `currency` must point to `len` readable bytes, or `len` must be 0.
#[no_mangle]
pub unsafe extern "C" fn hi_number_currency_digits(currency: *const u8, len: usize) -> i32 {
    as_str(currency, len)
        .and_then(number::currency_digits)
        .map_or(-1, i32::from)
}

/// A list format: ICU4X's list formatter for one locale, type and style.
/// Opaque to C++, which frees it with hi_list_format_free.
pub struct HiListFormat(list::ListFormat);

// A NativeState holding a HiListFormat can be destroyed on any thread.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<HiListFormat>();
};

/// Make the list format of the language identifier of `locale` (a canonical
/// tag), `kind` ("conjunction", "disjunction" or "unit") and `style`
/// ("long", "short" or "narrow"). Null if a string is not UTF-8, for any
/// other value, if `locale` does not parse, or if there is no data.
///
/// # Safety
/// Each string must point to its length of readable bytes, or the length
/// must be 0.
#[no_mangle]
pub unsafe extern "C" fn hi_list_format_new(
    locale: *const u8,
    locale_len: usize,
    kind: *const u8,
    kind_len: usize,
    style: *const u8,
    style_len: usize,
) -> *mut HiListFormat {
    let (Some(locale), Some(kind), Some(style)) = (
        as_str(locale, locale_len),
        as_str(kind, kind_len),
        as_str(style, style_len),
    ) else {
        return std::ptr::null_mut();
    };
    match list::ListFormat::new(locale, kind, style) {
        Some(f) => Box::into_raw(Box::new(HiListFormat(f))),
        None => std::ptr::null_mut(),
    }
}

/// Free a list format from hi_list_format_new. Accepts null.
///
/// # Safety
/// `format` must be null or come from hi_list_format_new, and must not be
/// freed twice.
#[no_mangle]
pub unsafe extern "C" fn hi_list_format_free(format: *mut HiListFormat) {
    if !format.is_null() {
        drop(Box::from_raw(format));
    }
}

/// The literal text around the elements of a list whose last element is the
/// UTF-16 text `last` (an unpaired surrogate reads as U+FFFD), separated by
/// U+0000 like every list (write_list): 3 pieces for a list of two (`pair`),
/// before, between and after; otherwise 5, before the first, after the
/// first, between two middle elements, before the last and after the last.
/// Always returns true.
///
/// # Safety
/// `format` must be valid; `last` must point to `last_len` readable u16
/// units, or `last_len` must be 0; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_list_format_literals(
    format: *const HiListFormat,
    pair: bool,
    last: *const u16,
    last_len: usize,
    out: *mut HiString,
) -> bool {
    debug_assert!(!format.is_null());
    let last: &[u16] = if last_len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(last, last_len)
    };
    write_list(out, Some((*format).0.literals(pair, last)))
}

/// A segmenter: ICU4X's segmenter for one granularity and content locale.
/// Opaque to C++, which frees it with hi_segmenter_free.
pub struct HiSegmenter(segmenter::Segmenter);

/// The segments of one string: its boundaries, and for word granularity
/// whether each segment is word-like. Opaque to C++, which frees it with
/// hi_segments_free.
pub struct HiSegments(segmenter::Segments);

// A NativeState holding a HiSegmenter or a HiSegments can be destroyed on
// any thread.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<HiSegmenter>();
    assert_send::<HiSegments>();
};

/// Make the segmenter of `granularity` ("grapheme", "word" or "sentence")
/// with the language identifier of `locale` (a canonical tag) as content
/// locale. Null if a string is not UTF-8, for any other granularity, if
/// `locale` does not parse, or if there is no data.
///
/// # Safety
/// Each string must point to its length of readable bytes, or the length
/// must be 0.
#[no_mangle]
pub unsafe extern "C" fn hi_segmenter_new(
    locale: *const u8,
    locale_len: usize,
    granularity: *const u8,
    granularity_len: usize,
) -> *mut HiSegmenter {
    let (Some(locale), Some(granularity)) = (
        as_str(locale, locale_len),
        as_str(granularity, granularity_len),
    ) else {
        return std::ptr::null_mut();
    };
    match segmenter::Segmenter::new(locale, granularity) {
        Some(s) => Box::into_raw(Box::new(HiSegmenter(s))),
        None => std::ptr::null_mut(),
    }
}

/// Free a segmenter from hi_segmenter_new. Accepts null.
///
/// # Safety
/// `segmenter` must be null or come from hi_segmenter_new, and must not be
/// freed twice.
#[no_mangle]
pub unsafe extern "C" fn hi_segmenter_free(segmenter: *mut HiSegmenter) {
    if !segmenter.is_null() {
        drop(Box::from_raw(segmenter));
    }
}

/// Segment the UTF-16 text `text` (it may have unpaired surrogates) with
/// `segmenter`: every boundary, in UTF-16 code units. The segments borrow
/// neither `segmenter` nor `text`. Null if `text_len` is above i32::MAX, so
/// that every index fits in an int.
///
/// # Safety
/// `segmenter` must be valid; `text` must point to `text_len` readable u16
/// units, or `text_len` must be 0.
#[no_mangle]
pub unsafe extern "C" fn hi_segments_new(
    segmenter: *const HiSegmenter,
    text: *const u16,
    text_len: usize,
) -> *mut HiSegments {
    debug_assert!(!segmenter.is_null());
    if i32::try_from(text_len).is_err() {
        return std::ptr::null_mut();
    }
    let text: &[u16] = if text_len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(text, text_len)
    };
    Box::into_raw(Box::new(HiSegments((*segmenter).0.segment(text))))
}

/// Free segments from hi_segments_new. Accepts null.
///
/// # Safety
/// `segments` must be null or come from hi_segments_new, and must not be
/// freed twice.
#[no_mangle]
pub unsafe extern "C" fn hi_segments_free(segments: *mut HiSegments) {
    if !segments.is_null() {
        drop(Box::from_raw(segments));
    }
}

/// FindBoundary: the last boundary at or before `index` (`after` false) or
/// the first boundary after it (`after` true); -1 unless 0 <= `index` <
/// the length of the string.
///
/// # Safety
/// `segments` must be valid.
#[no_mangle]
pub unsafe extern "C" fn hi_segments_find_boundary(
    segments: *const HiSegments,
    index: i32,
    after: bool,
) -> i32 {
    debug_assert!(!segments.is_null());
    let Ok(index) = u32::try_from(index) else {
        return -1;
    };
    // Every boundary fits in an i32 (hi_segments_new).
    (*segments)
        .0
        .find_boundary(index, after)
        .map_or(-1, |b| b as i32)
}

/// Whether the segment that starts at the boundary `start` is word-like,
/// in `*out`. False, leaving `*out` alone, if the granularity is not word,
/// or if `start` is not a boundary or is the length of the string.
///
/// # Safety
/// `segments` must be valid; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_segments_is_word_like(
    segments: *const HiSegments,
    start: i32,
    out: *mut bool,
) -> bool {
    debug_assert!(!segments.is_null());
    let Some(word_like) = u32::try_from(start)
        .ok()
        .and_then(|start| (*segments).0.is_word_like(start))
    else {
        return false;
    };
    *out = word_like;
    true
}

/// RelativeTimeFormat's data for the language identifier of `locale` (a
/// canonical tag) and `style` ("long", "short" or "narrow"): when it
/// returns true, `out` is the entries of relativetime::patterns, separated
/// by U+0000. False if a string is not UTF-8, for any other style, if
/// `locale` does not parse, or if there is no data.
///
/// # Safety
/// Each string must point to its length of readable bytes, or the length
/// must be 0; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_relative_time_patterns(
    locale: *const u8,
    locale_len: usize,
    style: *const u8,
    style_len: usize,
    out: *mut HiString,
) -> bool {
    let (Some(locale), Some(style)) = (as_str(locale, locale_len), as_str(style, style_len)) else {
        *out = HiString::EMPTY;
        return false;
    };
    write_list(out, relativetime::patterns(locale, style))
}

/// A DisplayNames name of `code` for the language identifier of `locale` (a
/// canonical tag), by `kind` and `style`, as displaynames::name documents.
/// When it returns true, `*found` says whether there is a name, and `out` is
/// the name, or empty without one. False, with `*found` false, if a string
/// is not UTF-8, for any other kind or style, or if `locale` does not parse.
///
/// # Safety
/// Each string must point to its length of readable bytes, or the length
/// must be 0; `found` and `out` must be valid for writes.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn hi_display_name(
    locale: *const u8,
    locale_len: usize,
    kind: *const u8,
    kind_len: usize,
    code: *const u8,
    code_len: usize,
    style: *const u8,
    style_len: usize,
    found: *mut bool,
    out: *mut HiString,
) -> bool {
    let result = match (
        as_str(locale, locale_len),
        as_str(kind, kind_len),
        as_str(code, code_len),
        as_str(style, style_len),
    ) {
        (Some(locale), Some(kind), Some(code), Some(style)) => {
            displaynames::name(locale, kind, code, style)
        }
        _ => None,
    };
    let Some(result) = result else {
        *found = false;
        *out = HiString::EMPTY;
        return false;
    };
    *found = write_out(out, result);
    true
}

/// The locale display patterns of the language identifier of `locale` (a
/// canonical tag): when it returns true, `out` is localePattern and
/// localeSeparator in CLDR's form, separated by U+0000. False if `locale`
/// is not UTF-8 or does not parse.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_display_patterns(
    locale: *const u8,
    len: usize,
    out: *mut HiString,
) -> bool {
    write_list(
        out,
        as_str(locale, len)
            .and_then(displaynames::locale_patterns)
            .map(Vec::from),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_round_trip() {
        let s = HiString::from_string("en-US".to_owned());
        let view = unsafe { std::slice::from_raw_parts(s.data, s.len) };
        assert_eq!(view, b"en-US");
        unsafe { hi_string_free(s) };
        unsafe { hi_string_free(HiString::EMPTY) };
    }

    /// Call an FFI function of the (data, len, out) shape on `s`.
    fn call(
        f: unsafe extern "C" fn(*const u8, usize, *mut HiString) -> bool,
        s: &str,
    ) -> Option<String> {
        let mut out = HiString::EMPTY;
        if !unsafe { f(s.as_ptr(), s.len(), &mut out) } {
            assert!(out.data.is_null());
            return None;
        }
        let result = unsafe { std::slice::from_raw_parts(out.data, out.len) };
        let result = String::from_utf8(result.to_vec()).unwrap();
        unsafe { hi_string_free(out) };
        Some(result)
    }

    #[test]
    fn ffi_case_map() {
        let run = |text: &[u16], lang: &str, upper: bool| unsafe {
            let mut out = HiString16::EMPTY;
            let ok = hi_case_map(
                text.as_ptr(),
                text.len(),
                lang.as_ptr(),
                lang.len(),
                upper,
                &mut out,
            );
            let v = if ok {
                Some(std::slice::from_raw_parts(out.data, out.len).to_vec())
            } else {
                None
            };
            hi_string16_free(out);
            v
        };
        assert_eq!(run(&[0x61, 0xD800], "und", true), Some(vec![0x41, 0xD800]));
        assert_eq!(run(&[], "und", true), Some(vec![]));
        assert_eq!(run(&[0x61], "en_US", true), None);
    }

    #[test]
    fn ffi_collator() {
        let make = |locale: &str, co: &str, sens: &str, cf: &str| unsafe {
            hi_collator_new(
                locale.as_ptr(),
                locale.len(),
                co.as_ptr(),
                co.len(),
                sens.as_ptr(),
                sens.len(),
                false,
                false,
                cf.as_ptr(),
                cf.len(),
            )
        };
        let c = make("en", "default", "variant", "false");
        assert!(!c.is_null());
        let cmp = |x: &[u16], y: &[u16]| unsafe {
            hi_collator_compare(c, x.as_ptr(), x.len(), y.as_ptr(), y.len())
        };
        let (a, b) = ([0x61u16], [0x62u16]);
        assert_eq!(cmp(&a, &b), -1);
        assert_eq!(cmp(&b, &a), 1);
        assert_eq!(cmp(&a, &a), 0);
        // Empty strings may come with any pointer.
        assert_eq!(cmp(&[], &[]), 0);
        assert_eq!(cmp(&[], &a), -1);
        unsafe { hi_collator_free(c) };
        unsafe { hi_collator_free(std::ptr::null_mut()) };
        assert!(make("en", "search", "variant", "false").is_null());
        assert!(make("en", "default", "Base", "false").is_null());

        assert_eq!(
            call(hi_collator_locale_defaults, "da").as_deref(),
            Some("upper false")
        );
        assert_eq!(call(hi_collator_locale_defaults, "en_US"), None);
    }

    #[test]
    fn ffi_plural_rules() {
        let make = |locale: &str, ordinal| unsafe {
            hi_plural_rules_new(locale.as_ptr(), locale.len(), ordinal)
        };
        let take = |ok: bool, out: HiString| {
            if !ok {
                assert!(out.data.is_null());
                return None;
            }
            let s = unsafe { std::slice::from_raw_parts(out.data, out.len) };
            let s = String::from_utf8(s.to_vec()).unwrap();
            unsafe { hi_string_free(out) };
            Some(s)
        };
        let r = make("fr", false);
        assert!(!r.is_null());
        let select = |d: &[u8], e| {
            let mut out = HiString::EMPTY;
            let ok = unsafe { hi_plural_rules_select(r, d.as_ptr(), d.len(), e, &mut out) };
            take(ok, out)
        };
        assert_eq!(select(b"1", 0).as_deref(), Some("one"));
        assert_eq!(select(b"1", 6).as_deref(), Some("many"));
        assert_eq!(select(b"2", 0).as_deref(), Some("other"));
        assert_eq!(select(b"1e3", 0), None);
        assert_eq!(select(b"", 0), None);
        assert_eq!(select(&[0xff], 0), None);
        let range = |s: &[u8], e: &[u8]| {
            let mut out = HiString::EMPTY;
            let ok = unsafe {
                hi_plural_rules_select_range(r, s.as_ptr(), s.len(), e.as_ptr(), e.len(), &mut out)
            };
            take(ok, out)
        };
        assert_eq!(range(b"one", b"one").as_deref(), Some("one"));
        assert_eq!(range(b"one", b"other").as_deref(), Some("other"));
        assert_eq!(range(b"one", b"lots"), None);
        assert_eq!(range(&[0xff], b"one"), None);
        let mut out = HiString::EMPTY;
        let ok = unsafe { hi_plural_rules_categories(r, &mut out) };
        assert_eq!(take(ok, out).as_deref(), Some("one\0many\0other"));
        unsafe { hi_plural_rules_free(r) };
        unsafe { hi_plural_rules_free(std::ptr::null_mut()) };

        let r = make("en", true);
        let mut out = HiString::EMPTY;
        let ok = unsafe { hi_plural_rules_categories(r, &mut out) };
        assert_eq!(take(ok, out).as_deref(), Some("one\0two\0few\0other"));
        unsafe { hi_plural_rules_free(r) };
        assert!(make("en_US", false).is_null());
        let bad = [0xffu8];
        assert!(unsafe { hi_plural_rules_new(bad.as_ptr(), bad.len(), false) }.is_null());
    }

    #[test]
    fn ffi_list_format() {
        let make = |locale: &[u8], kind: &[u8], style: &[u8]| unsafe {
            hi_list_format_new(
                locale.as_ptr(),
                locale.len(),
                kind.as_ptr(),
                kind.len(),
                style.as_ptr(),
                style.len(),
            )
        };
        let literals = |f: *const HiListFormat, pair, last: &[u16]| {
            let mut out = HiString::EMPTY;
            assert!(unsafe {
                hi_list_format_literals(f, pair, last.as_ptr(), last.len(), &mut out)
            });
            let s = unsafe { std::slice::from_raw_parts(out.data, out.len) };
            let s = String::from_utf8(s.to_vec()).unwrap();
            unsafe { hi_string_free(out) };
            s
        };
        let es = make(b"es", b"conjunction", b"long");
        assert!(!es.is_null());
        assert_eq!(literals(es, true, &[]), "\0 y \0");
        let ibiza: Vec<u16> = "Ibiza".encode_utf16().collect();
        assert_eq!(literals(es, true, &ibiza), "\0 e \0");
        assert_eq!(literals(es, false, &ibiza), "\0, \0, \0 e \0");
        unsafe { hi_list_format_free(es) };
        unsafe { hi_list_format_free(std::ptr::null_mut()) };
        assert!(make(b"en_US", b"conjunction", b"long").is_null());
        assert!(make(b"en", b"and", b"long").is_null());
        assert!(make(b"en", b"unit", b"wide").is_null());
        assert!(make(&[0xff], b"unit", b"long").is_null());
        assert!(make(b"en", &[0xff], b"long").is_null());
        assert!(make(b"en", b"unit", &[0xff]).is_null());
    }

    #[test]
    fn ffi_segmenter() {
        let make = |locale: &[u8], granularity: &[u8]| unsafe {
            hi_segmenter_new(
                locale.as_ptr(),
                locale.len(),
                granularity.as_ptr(),
                granularity.len(),
            )
        };
        let word = make(b"en", b"word");
        assert!(!word.is_null());
        // "a\ud800b c": the indices are UTF-16 code units.
        let text = [0x61u16, 0xD800, 0x62, 0x20, 0x63];
        let s = unsafe { hi_segments_new(word, text.as_ptr(), text.len()) };
        assert!(!s.is_null());
        // The segments do not borrow the segmenter.
        unsafe { hi_segmenter_free(word) };
        let find = |index, after| unsafe { hi_segments_find_boundary(s, index, after) };
        assert_eq!(find(0, false), 0);
        assert_eq!(find(0, true), 1);
        assert_eq!(find(1, true), 2);
        assert_eq!(find(4, false), 4);
        assert_eq!(find(4, true), 5);
        assert_eq!(find(5, false), -1);
        assert_eq!(find(-1, true), -1);
        assert_eq!(find(i32::MIN, false), -1);
        let word_like = |start| {
            let mut out = false;
            unsafe { hi_segments_is_word_like(s, start, &mut out) }.then_some(out)
        };
        assert_eq!(word_like(0), Some(true));
        assert_eq!(word_like(1), Some(false));
        assert_eq!(word_like(3), Some(false));
        assert_eq!(word_like(4), Some(true));
        assert_eq!(word_like(5), None);
        assert_eq!(word_like(-1), None);
        unsafe { hi_segments_free(s) };
        unsafe { hi_segments_free(std::ptr::null_mut()) };

        let grapheme = make(b"en", b"grapheme");
        let empty = unsafe { hi_segments_new(grapheme, std::ptr::null(), 0) };
        assert_eq!(unsafe { hi_segments_find_boundary(empty, 0, true) }, -1);
        let mut out = true;
        assert!(!unsafe { hi_segments_is_word_like(empty, 0, &mut out) });
        assert!(out);
        unsafe { hi_segments_free(empty) };
        let text = [0x61u16, 0x62];
        let s = unsafe { hi_segments_new(grapheme, text.as_ptr(), text.len()) };
        assert_eq!(unsafe { hi_segments_find_boundary(s, 0, true) }, 1);
        assert!(!unsafe { hi_segments_is_word_like(s, 0, &mut out) });
        unsafe { hi_segments_free(s) };
        // Longer than i32::MAX: the pointer is never read.
        let too_long =
            unsafe { hi_segments_new(grapheme, std::ptr::NonNull::dangling().as_ptr(), 1 << 31) };
        assert!(too_long.is_null());
        unsafe { hi_segmenter_free(grapheme) };
        unsafe { hi_segmenter_free(std::ptr::null_mut()) };

        let sentence = make(b"el", b"sentence");
        assert!(!sentence.is_null());
        unsafe { hi_segmenter_free(sentence) };
        assert!(make(b"en", b"line").is_null());
        assert!(make(b"en_US", b"word").is_null());
        assert!(make(&[0xff], b"word").is_null());
        assert!(make(b"en", &[0xff]).is_null());
    }

    #[test]
    fn ffi_relative_time_patterns() {
        let patterns = |locale: &[u8], style: &[u8]| {
            let mut out = HiString::EMPTY;
            let ok = unsafe {
                hi_relative_time_patterns(
                    locale.as_ptr(),
                    locale.len(),
                    style.as_ptr(),
                    style.len(),
                    &mut out,
                )
            };
            if !ok {
                assert!(out.data.is_null());
                return None;
            }
            let s = unsafe { std::slice::from_raw_parts(out.data, out.len) };
            let s = String::from_utf8(s.to_vec()).unwrap();
            unsafe { hi_string_free(out) };
            Some(s.split('\0').map(str::to_owned).collect::<Vec<_>>())
        };
        // en long second: 12 patterns, then the one relative name "now".
        let en = patterns(b"en", b"long").unwrap();
        assert_eq!(en[5], "in {0} seconds");
        assert_eq!(en[11], "{0} seconds ago");
        assert_eq!(&en[12..15], ["1", "0", "now"]);
        // minute: en has no zero category, so zero gets other's pattern.
        assert_eq!(&en[15..17], ["in {0} minutes", "in {0} minute"]);
        assert_eq!(patterns(b"en", b"wide"), None);
        assert_eq!(patterns(b"en_US", b"long"), None);
        assert_eq!(patterns(&[0xff], b"long"), None);
        assert_eq!(patterns(b"en", &[0xff]), None);
    }

    #[test]
    fn ffi_display_names() {
        // (accepted, found, name)
        let name = |locale: &[u8], kind: &[u8], code: &[u8], style: &[u8]| {
            let mut found = true;
            let mut out = HiString::EMPTY;
            let ok = unsafe {
                hi_display_name(
                    locale.as_ptr(),
                    locale.len(),
                    kind.as_ptr(),
                    kind.len(),
                    code.as_ptr(),
                    code.len(),
                    style.as_ptr(),
                    style.len(),
                    &mut found,
                    &mut out,
                )
            };
            if !found {
                assert!(out.data.is_null());
            }
            let s = if out.data.is_null() {
                String::new()
            } else {
                let s = unsafe { std::slice::from_raw_parts(out.data, out.len) };
                String::from_utf8(s.to_vec()).unwrap()
            };
            unsafe { hi_string_free(out) };
            (ok, found, s)
        };
        let named = |s: &str| (true, true, s.to_owned());
        assert_eq!(name(b"en", b"region", b"US", b"short"), named("US"));
        assert_eq!(
            name(b"en", b"currency", b"USD", b"long"),
            named("US Dollar")
        );
        assert_eq!(name(b"de", b"dateTimeField", b"era", b"narrow"), named("E"));
        let none = (true, false, String::new());
        assert_eq!(name(b"en", b"region", b"ZZZZ", b"long"), none);
        assert_eq!(name(b"und", b"currency", b"USD", b"long"), none);
        let rejected = (false, false, String::new());
        assert_eq!(name(b"en", b"lang", b"en", b"long"), rejected);
        assert_eq!(name(b"en", b"region", b"US", b"wide"), rejected);
        assert_eq!(name(b"en_US", b"region", b"US", b"long"), rejected);
        assert_eq!(name(&[0xff], b"region", b"US", b"long"), rejected);
        assert_eq!(name(b"en", &[0xff], b"US", b"long"), rejected);
        assert_eq!(name(b"en", b"region", &[0xff], b"long"), rejected);
        assert_eq!(name(b"en", b"region", b"US", &[0xff]), rejected);

        let patterns = |locale: &[u8]| {
            let mut out = HiString::EMPTY;
            let ok = unsafe { hi_locale_display_patterns(locale.as_ptr(), locale.len(), &mut out) };
            if !ok {
                assert!(out.data.is_null());
                return None;
            }
            let s = unsafe { std::slice::from_raw_parts(out.data, out.len) };
            let s = String::from_utf8(s.to_vec()).unwrap();
            unsafe { hi_string_free(out) };
            Some(s)
        };
        assert_eq!(patterns(b"en").as_deref(), Some("{0} ({1})\0{0}, {1}"));
        assert_eq!(patterns(b"en_US"), None);
        assert_eq!(patterns(&[0xff]), None);
    }

    #[test]
    fn ffi_number_data() {
        /// Call a list function and split its result at U+0000.
        fn take(ok: bool, out: HiString) -> Option<Vec<String>> {
            if !ok {
                assert!(out.data.is_null());
                return None;
            }
            let s = unsafe { std::slice::from_raw_parts(out.data, out.len) };
            let s = String::from_utf8(s.to_vec()).unwrap();
            unsafe { hi_string_free(out) };
            Some(s.split('\0').map(str::to_owned).collect())
        }
        let symbols = |l: &[u8], nu: &[u8]| {
            let mut out = HiString::EMPTY;
            let ok =
                unsafe { hi_number_symbols(l.as_ptr(), l.len(), nu.as_ptr(), nu.len(), &mut out) };
            take(ok, out)
        };
        let en = symbols(b"en", b"latn").unwrap();
        assert_eq!(en.len(), 22);
        assert_eq!((en[0].as_str(), en[18].as_str()), (".", "{0}–{1}"));
        assert_eq!(en[21], ":");
        assert_eq!(symbols(b"en_US", b"latn"), None);
        assert_eq!(symbols(&[0xff], b"latn"), None);
        assert_eq!(symbols(b"en", &[0xff]), None);

        let currency = |c: &[u8], d: &[u8], accounting| {
            let (l, nu) = (b"en", b"latn");
            let mut out = HiString::EMPTY;
            let ok = unsafe {
                hi_number_currency_patterns(
                    l.as_ptr(),
                    l.len(),
                    nu.as_ptr(),
                    nu.len(),
                    c.as_ptr(),
                    c.len(),
                    d.as_ptr(),
                    d.len(),
                    accounting,
                    &mut out,
                )
            };
            take(ok, out)
        };
        assert_eq!(
            currency(b"USD", b"symbol", true).unwrap(),
            [
                "{currencyPrefix}{number}",
                "{plusSign}{currencyPrefix}{number}",
                "({currencyPrefix}{number})",
                "$",
                "{currencyPrefix}{number}",
                "{plusSign}{currencyPrefix}{number}",
                "({currencyPrefix}{number})",
            ]
        );
        assert_eq!(currency(b"USD", b"name", false).unwrap().len(), 6);
        assert_eq!(currency(b"usd", b"symbol", false), None);
        assert_eq!(currency(b"USD", b"long", false), None);
        assert_eq!(currency(&[0xff], b"symbol", false), None);

        let compact_currency = |c: &[u8], d: &[u8], accounting| {
            let (l, nu) = (b"en", b"latn");
            let mut out = HiString::EMPTY;
            let ok = unsafe {
                hi_number_compact_currency_patterns(
                    l.as_ptr(),
                    l.len(),
                    nu.as_ptr(),
                    nu.len(),
                    c.as_ptr(),
                    c.len(),
                    d.as_ptr(),
                    d.len(),
                    accounting,
                    &mut out,
                )
            };
            take(ok, out)
        };
        // en's 12 magnitudes, 26 strings each; 10^3's other is "{0}K".
        let en = compact_currency(b"USD", b"symbol", true).unwrap();
        assert_eq!(en.len(), 1 + 12 * 26);
        assert_eq!(en[..3], ["12", "3", "3"]);
        assert_eq!(
            en[3 + 5 * 4..3 + 6 * 4],
            [
                "{0}K",
                "{currencyPrefix}{number}",
                "{plusSign}{currencyPrefix}{number}",
                "({currencyPrefix}{number})"
            ]
        );
        assert_eq!(compact_currency(b"USD", b"name", false), None);

        let separators = |l: &[u8], c: &[u8]| {
            let nu = b"latn";
            let mut out = HiString::EMPTY;
            let ok = unsafe {
                hi_number_currency_separators(
                    l.as_ptr(),
                    l.len(),
                    nu.as_ptr(),
                    nu.len(),
                    c.as_ptr(),
                    c.len(),
                    &mut out,
                )
            };
            take(ok, out)
        };
        assert_eq!(separators(b"pt-PT", b"PTE").unwrap(), ["$", ","]);
        assert_eq!(separators(b"de-AT", b"EUR").unwrap(), [",", "."]);
        assert_eq!(separators(b"de-AT", b"eur"), None);
        assert_eq!(separators(&[0xff], b"EUR"), None);
        assert_eq!(compact_currency(b"usd", b"symbol", false), None);
        assert_eq!(compact_currency(&[0xff], b"code", false), None);

        let compact = |l: &[u8], long| {
            let nu = b"latn";
            let mut out = HiString::EMPTY;
            let ok = unsafe {
                hi_number_compact_patterns(
                    l.as_ptr(),
                    l.len(),
                    nu.as_ptr(),
                    nu.len(),
                    long,
                    &mut out,
                )
            };
            take(ok, out)
        };
        // Empty entries survive the U+0000 separators: the exact-0 and
        // exact-1 patterns of en's first entry, and fr long's exact-0.
        let en = compact(b"en", false).unwrap();
        assert_eq!(en[..5], ["4", "3", "3", "", ""]);
        assert_eq!(en.len(), 41);
        let fr = compact(b"fr", true).unwrap();
        assert_eq!(fr[1..5], ["3", "3", "", "mille"]);
        assert_eq!(compact(b"en_US", false), None);

        let unit = |u: &[u8], w: &[u8]| {
            let l = b"en";
            let mut out = HiString::EMPTY;
            let ok = unsafe {
                hi_unit_patterns(
                    l.as_ptr(),
                    l.len(),
                    u.as_ptr(),
                    u.len(),
                    w.as_ptr(),
                    w.len(),
                    &mut out,
                )
            };
            take(ok, out)
        };
        let mile = unit(b"mile", b"long").unwrap();
        assert_eq!(
            mile,
            [
                "{0} miles",
                "{0} mile",
                "{0} miles",
                "{0} miles",
                "{0} miles",
                "{0} miles"
            ]
        );
        assert_eq!(unit(b"furlong", b"long"), None);
        assert_eq!(unit(b"mile", b"Long"), None);
        assert_eq!(unit(&[0xff], b"long"), None);

        let digits = |c: &[u8]| unsafe { hi_number_currency_digits(c.as_ptr(), c.len()) };
        assert_eq!(digits(b"JPY"), 0);
        assert_eq!(digits(b"KWD"), 3);
        assert_eq!(digits(b"ABC"), 2);
        assert_eq!(digits(b"jpy"), -1);
        assert_eq!(digits(b""), -1);
        assert_eq!(digits(&[0xff, 0xfe, 0xfd]), -1);
    }

    #[test]
    fn ffi_resolves_default_locales() {
        let f = hi_locale_resolve_default;
        assert_eq!(call(f, "en-US-u-ca-buddhist").as_deref(), Some("en-US"));
        assert_eq!(call(f, "zh-TW").as_deref(), Some("zh-TW"));
        assert_eq!(call(f, "tlh"), None);
        assert_eq!(call(f, "en_US"), None);
    }

    #[test]
    fn ffi_looks_up_calendars_and_hour_cycles() {
        let c = hi_locale_calendar_preferences;
        assert_eq!(call(c, "TH").as_deref(), Some("buddhist gregory"));
        assert_eq!(call(c, "US").as_deref(), Some("gregory"));
        assert_eq!(call(c, "english"), None);
        let h = hi_locale_hour_cycles;
        assert_eq!(call(h, "en-001").as_deref(), Some("h12 h23"));
        assert_eq!(call(h, "AQ").as_deref(), Some("h23 h12"));
        assert_eq!(call(h, "en-AQ"), None);
        assert_eq!(call(h, ""), None);
    }

    #[test]
    fn ffi_looks_up_locale_information() {
        let tz = hi_locale_time_zones_of_region;
        assert_eq!(call(tz, "PS").as_deref(), Some("Asia/Gaza Asia/Hebron"));
        assert_eq!(call(tz, "419").as_deref(), Some(""));
        let w = hi_locale_week_info;
        assert_eq!(call(w, "US").as_deref(), Some("7 6 7"));
        assert_eq!(call(w, "IN").as_deref(), Some("7 7"));
        assert_eq!(call(w, "english"), None);
        let d = hi_locale_script_direction;
        assert_eq!(call(d, "Arab").as_deref(), Some("rtl"));
        assert_eq!(call(d, "Latn").as_deref(), Some("ltr"));
        assert_eq!(call(d, "Zyyy"), None);
        let n = hi_locale_default_numbering_system;
        assert_eq!(call(n, "fa").as_deref(), Some("arabext"));
        assert_eq!(call(n, "not a tag"), None);
        let c = hi_locale_collations;
        assert_eq!(call(c, "de").as_deref(), Some("emoji eor phonebk"));
    }

    #[test]
    fn ffi_date_time_format() {
        let make = |l: &[u8], c: &[u8], z: &[u8], s: &[u8]| unsafe {
            hi_date_time_format_new(
                l.as_ptr(),
                l.len(),
                c.as_ptr(),
                c.len(),
                z.as_ptr(),
                z.len(),
                s.as_ptr(),
                s.len(),
            )
        };
        let local_time = |f, t| {
            let mut out = HiString::EMPTY;
            let status = unsafe { hi_date_time_format_local_time(f, t, &mut out) };
            if status != HiLocalTimeStatus::Ok {
                assert!(out.data.is_null());
                assert_eq!(status, HiLocalTimeStatus::NotATimeValue);
                return None;
            }
            let s = unsafe { std::slice::from_raw_parts(out.data, out.len) };
            let s = String::from_utf8(s.to_vec()).unwrap();
            unsafe { hi_string_free(out) };
            Some(s)
        };
        let f = make(b"en", b"gregory", b"America/New_York", b"MMMM z");
        assert!(!f.is_null());
        // 2026-01-05T15:07:09.123Z.
        assert_eq!(
            local_time(f, 1767625629123.0)
                .map(|s| s.split('\0').map(str::to_owned).collect::<Vec<_>>()),
            Some(
                [
                    "1", "ce", "2026", "", "1", "M01", "0", "5", "10", "7", "9", "123", "0",
                    "-18000", "January", "EST"
                ]
                .map(str::to_owned)
                .to_vec()
            )
        );
        assert_eq!(local_time(f, 0.5), None);
        unsafe { hi_date_time_format_free(f) };
        unsafe { hi_date_time_format_free(std::ptr::null_mut()) };

        // No symbols: the 14 fields alone.
        let f = make(b"en", b"gregory", b"+01:00", b"");
        assert_eq!(local_time(f, 0.0).unwrap().split('\0').count(), 14);
        unsafe { hi_date_time_format_free(f) };

        assert!(make(b"en", b"gregory", b"UTC", b"MMMM  z").is_null());
        assert!(make(b"en", b"gregory", b"UTC", b" MMMM").is_null());
        assert!(make(b"en", b"gregory", b"UTC", b"MMMM ").is_null());
        assert!(make(b"en", b"gregory", b"UTC", b"U").is_null());
        assert!(make(b"en", b"julian", b"UTC", b"").is_null());
        assert!(make(b"en", b"gregory", b"Nowhere/Land", b"").is_null());
        assert!(make(&[0xff], b"gregory", b"UTC", b"").is_null());
        assert!(make(b"en", &[0xff], b"UTC", b"").is_null());
        assert!(make(b"en", b"gregory", &[0xff], b"").is_null());
        assert!(make(b"en", b"gregory", b"UTC", &[0xff]).is_null());
    }

    #[test]
    fn ffi_rejects_invalid_utf8() {
        let bad = [0xffu8, 0xfe];
        let mut out = HiString::EMPTY;
        assert!(!unsafe { hi_locale_canonicalize(bad.as_ptr(), bad.len(), &mut out) });
        assert!(out.data.is_null());
        assert!(!unsafe { hi_locale_hour_cycles(bad.as_ptr(), bad.len(), &mut out) });
        assert!(out.data.is_null());
        let fs: [unsafe extern "C" fn(*const u8, usize, *mut HiString) -> bool; 5] = [
            hi_locale_time_zones_of_region,
            hi_locale_week_info,
            hi_locale_script_direction,
            hi_locale_default_numbering_system,
            hi_locale_collations,
        ];
        for f in fs {
            assert!(!unsafe { f(bad.as_ptr(), bad.len(), &mut out) });
            assert!(out.data.is_null());
        }
    }
}
