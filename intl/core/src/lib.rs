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
mod collator;
mod host;
mod info;
mod locale;
mod supported;

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
