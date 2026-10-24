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
    fn ffi_rejects_invalid_utf8() {
        let bad = [0xffu8, 0xfe];
        let mut out = HiString::EMPTY;
        assert!(!unsafe { hi_locale_canonicalize(bad.as_ptr(), bad.len(), &mut out) });
        assert!(out.data.is_null());
    }
}
