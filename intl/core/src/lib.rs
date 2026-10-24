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
/// the calendar types, most preferred first, separated by single spaces.
/// False if CLDR has no line for `region`.
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
/// single spaces. False if CLDR has no line for `key`.
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
/// ascending, separated by single spaces. False if `region` is not a region
/// code.
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
        assert_eq!(call(c, "US"), None);
        let h = hi_locale_hour_cycles;
        assert_eq!(call(h, "en-001").as_deref(), Some("h12 h23"));
        assert_eq!(call(h, "AQ"), None);
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
