//! Unicode character properties for the JS lexer, ported from
//! include/hermes/Platform/Unicode/CharacterProperties.{h,cpp}. The range tables
//! in `tables` are generated from lib/Platform/Unicode/UnicodeData.inc by
//! gen_tables.py and pinned to Hermes's Unicode version (17.0.0). RegExp
//! canonicalization / property escapes are intentionally not ported here.

pub mod tables;

use std::cmp::Ordering;

// ---------------------------------------------------------------------------
// Constants (from CharacterProperties.h)
// ---------------------------------------------------------------------------

/// The maximum valid Unicode code point.
pub const UNICODE_MAX_VALUE: u32 = 0x10FFFF;
/// The start of the surrogate range.
pub const UNICODE_SURROGATE_FIRST: u32 = 0xD800;
/// The last character of the surrogate range (inclusive).
pub const UNICODE_SURROGATE_LAST: u32 = 0xDFFF;
/// The start of the UTF-16 high-surrogate range.
pub const UTF16_HIGH_SURROGATE: u32 = 0xD800;
pub const UTF16_LOW_SURROGATE: u32 = 0xDC00;
pub const UNICODE_REPLACEMENT_CHARACTER: u32 = 0xFFFD;
/// The last member of the BMP.
pub const UNICODE_LAST_BMP: u32 = 0xFFFF;

pub const UNICODE_LINE_SEPARATOR: u32 = 0x2028;
pub const UNICODE_PARAGRAPH_SEPARATOR: u32 = 0x2029;

pub const UNICODE_ZWNJ: u32 = 0x200C;
pub const UNICODE_ZWJ: u32 = 0x200D;

// ---------------------------------------------------------------------------
// Inline helpers (from CharacterProperties.h)
// ---------------------------------------------------------------------------

/// \return true if \p cp is a valid Unicode code point (not a surrogate, <= U+10FFFF).
#[inline]
pub fn is_valid_code_point(cp: u32) -> bool {
    !((cp >= UNICODE_SURROGATE_FIRST && cp <= UNICODE_SURROGATE_LAST) || cp > UNICODE_MAX_VALUE)
}

/// \return whether cp is part of the Basic Multilingual Plane.
/// Surrogate characters are considered part of the BMP.
#[inline]
pub fn is_member_of_bmp(cp: u32) -> bool {
    cp <= UNICODE_LAST_BMP
}

/// \return whether cp is a high surrogate.
#[inline]
pub fn is_high_surrogate(cp: u32) -> bool {
    UNICODE_SURROGATE_FIRST <= cp && cp < UTF16_LOW_SURROGATE
}

/// \return whether cp is a low surrogate.
#[inline]
pub fn is_low_surrogate(cp: u32) -> bool {
    UTF16_LOW_SURROGATE <= cp && cp <= UNICODE_SURROGATE_LAST
}

/// Decode a surrogate pair [lead, trail] into a code point.
/// ES14 11.1.3
#[inline]
pub fn utf16_surrogate_pair_to_code_point(lead: u32, trail: u32) -> u32 {
    debug_assert!(is_high_surrogate(lead) && is_low_surrogate(trail), "Not a surrogate pair");
    ((lead - UTF16_HIGH_SURROGATE) << 10) + (trail - UTF16_LOW_SURROGATE) + 0x10000
}

/// \return true if the character is an ASCII digit (0-9).
/// This is a safe replacement for isdigit() that handles non-ASCII characters
/// correctly on all platforms.
#[inline]
pub fn is_ascii_digit(ch: u32) -> bool {
    ch >= b'0' as u32 && ch <= b'9' as u32
}

/// \return true if the codepoint has the ID_Start property and is ASCII.
#[inline]
pub fn is_ascii_identifier_start(ch: u32) -> bool {
    ch == b'_' as u32
        || ch == b'$' as u32
        || ((ch | 32) >= b'a' as u32 && (ch | 32) <= b'z' as u32)
}

/// \return true if the codepoint has the ID_Continue property and is ASCII.
#[inline]
pub fn is_ascii_identifier_continue(ch: u32) -> bool {
    is_ascii_identifier_start(ch) || (b'0' as u32 <= ch && ch <= b'9' as u32)
}

/// \return true if the codepoint has the ID_Start property.
#[inline]
pub fn is_unicode_id_start(cp: u32) -> bool {
    is_ascii_identifier_start(cp) || is_unicode_only_id_start(cp)
}

/// \return true if the codepoint has the ID_Continue property.
#[inline]
pub fn is_unicode_id_continue(cp: u32) -> bool {
    is_ascii_identifier_continue(cp) || is_unicode_only_id_continue(cp)
}

// ---------------------------------------------------------------------------
// Binary-search lookup (port of UnicodeRangeComp + lookup from
// CharacterProperties.cpp lines 22-41)
// ---------------------------------------------------------------------------

/// Binary-search `table` (sorted, non-overlapping inclusive ranges) for `cp`.
/// Mirrors C++ UnicodeRangeComp: a range (first, last) is inclusive; the
/// comparator returns Less if `last < cp`, Greater if `cp < first`, else Equal.
fn lookup(table: &[(u32, u32)], cp: u32) -> bool {
    table
        .binary_search_by(|&(first, last)| {
            if last < cp {
                Ordering::Less
            } else if cp < first {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
        .is_ok()
}

// ---------------------------------------------------------------------------
// Table predicates (port of CharacterProperties.cpp lines 43-146)
// ---------------------------------------------------------------------------

use crate::tables::*;

/// \return true if the codepoint is not ASCII and is a Unicode letter.
/// "any character in the Unicode categories "Uppercase letter (Lu)",
/// "Lowercase letter (Ll)", "Titlecase letter (Lt)", "Modifier letter (Lm)",
/// "Other letter (Lo)", or "Letter number (Nl)"".
/// ASCII characters are not "UnicodeOnly" and so we return false.
pub fn is_unicode_only_letter(cp: u32) -> bool {
    if cp <= 0x7F {
        return false;
    }
    lookup(&UNICODE_LETTERS, cp)
}

/// \return true if the codepoint is not ASCII and is a Unicode ID_Start.
/// ASCII characters are not "UnicodeOnly" and so we return false.
///
/// Unicode spec defines ID_Start as:
///     Lu + Ll + Lt + Lm + Lo + Nl
///   + Other_ID_Start
///   - Pattern_Syntax
///   - Pattern_White_Space
///
/// So check this by checking all UNICODE_LETTERS that aren't
/// UNICODE_PATTERN_LETTER, and then check for Other_ID_Start.
///
/// UNICODE_PATTERN_LETTER is all the Pattern_White_Space and Pattern_Syntax
/// that are also in the Letter categories.
pub fn is_unicode_only_id_start(cp: u32) -> bool {
    if cp <= 0x7F {
        return false;
    }
    (lookup(&UNICODE_LETTERS, cp) && !lookup(&UNICODE_PATTERN_LETTER, cp))
        || lookup(&UNICODE_OTHER_ID_START, cp)
}

/// \return true if the codepoint is not ASCII and is a Unicode ID_Continue.
/// ASCII characters are not "UnicodeOnly" and so we return false.
///
/// Unicode spec defines ID_Continue as (generated from):
///   ID_Start + Mn + Mc + Nd + Pc + Other_ID_Continue
///   - Pattern_Syntax - Pattern_White_Space
///
/// UNICODE_PATTERN_CONTINUE is all the Pattern_White_Space and Pattern_Syntax
/// that are also in the Mn, Mc, Nd, Pc categories.
pub fn is_unicode_only_id_continue(cp: u32) -> bool {
    if cp <= 0x7F {
        return false;
    }
    is_unicode_only_id_start(cp)
        || ((lookup(&UNICODE_COMBINING_MARK, cp)
            || lookup(&UNICODE_DIGIT, cp)
            || lookup(&UNICODE_CONNECTOR_PUNCTUATION, cp))
            && !lookup(&UNICODE_PATTERN_CONTINUE, cp))
        || lookup(&UNICODE_OTHER_ID_CONTINUE, cp)
}

/// Special cased due to small number of separate values.
/// "Other category "Zs": Any other Unicode "space separator""
/// Exclude ASCII.
pub fn is_unicode_only_space(cp: u32) -> bool {
    if cp <= 0x7F {
        return false;
    }
    matches!(
        cp,
        0xa0 | 0x1680
            | 0x2000
            | 0x2001
            | 0x2002
            | 0x2003
            | 0x2004
            | 0x2005
            | 0x2006
            | 0x2007
            | 0x2008
            | 0x2009
            | 0x200a
            | 0x202f
            | 0x205f
            | 0x3000
    )
}

/// \return true if the codepoint is in the Non-Spacing Mark or
/// Combining-Spacing Mark categories.
pub fn is_unicode_combining_mark(cp: u32) -> bool {
    lookup(&UNICODE_COMBINING_MARK, cp)
}

/// \return true if the codepoint is in the Decimal Number category.
/// 0-9 is the common case.
pub fn is_unicode_digit(cp: u32) -> bool {
    (cp >= b'0' as u32 && cp <= b'9' as u32) || lookup(&UNICODE_DIGIT, cp)
}

/// \return true if the codepoint is in the Connector Punctuation category.
/// _ is the common case.
pub fn is_unicode_connector_punctuation(cp: u32) -> bool {
    // '_' (U+005F) is also in the table, but the fast path avoids the binary search.
    cp == b'_' as u32 || lookup(&UNICODE_CONNECTOR_PUNCTUATION, cp)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod table_tests {
    use super::tables::*;

    #[test]
    fn table_lengths() {
        assert_eq!(UNICODE_LETTERS.len(), 378);
        assert_eq!(UNICODE_COMBINING_MARK.len(), 266);
        assert_eq!(UNICODE_DIGIT.len(), 69);
        assert_eq!(UNICODE_CONNECTOR_PUNCTUATION.len(), 6);
        assert_eq!(UNICODE_OTHER_ID_START.len(), 4);
        assert_eq!(UNICODE_OTHER_ID_CONTINUE.len(), 7);
        assert_eq!(UNICODE_PATTERN_LETTER.len(), 1);
        assert_eq!(UNICODE_PATTERN_CONTINUE.len(), 0);
    }

    #[test]
    fn tables_sorted_non_overlapping() {
        for t in [
            &UNICODE_LETTERS[..], &UNICODE_COMBINING_MARK[..], &UNICODE_DIGIT[..],
            &UNICODE_CONNECTOR_PUNCTUATION[..], &UNICODE_OTHER_ID_START[..],
            &UNICODE_OTHER_ID_CONTINUE[..], &UNICODE_PATTERN_LETTER[..],
            &UNICODE_PATTERN_CONTINUE[..],
        ] {
            for w in t.windows(2) {
                assert!(w[0].1 < w[1].0, "ranges must be sorted & non-overlapping");
            }
            for &(first, last) in t {
                assert!(first <= last, "range first must be <= last");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_helpers() {
        assert!(is_ascii_identifier_start(b'_' as u32));
        assert!(is_ascii_identifier_start(b'$' as u32));
        assert!(is_ascii_identifier_start(b'A' as u32));
        assert!(is_ascii_identifier_start(b'z' as u32));
        assert!(!is_ascii_identifier_start(b'0' as u32));
        assert!(is_ascii_identifier_continue(b'0' as u32));
        assert!(!is_ascii_identifier_continue(b'-' as u32));
        assert!(is_ascii_digit(b'7' as u32));
        assert!(!is_ascii_digit(b'a' as u32));
    }

    #[test]
    fn surrogate_helpers() {
        assert!(is_high_surrogate(0xD800));
        assert!(!is_high_surrogate(0xDC00));
        assert!(is_low_surrogate(0xDC00));
        assert!(is_member_of_bmp(0xFFFF));
        assert!(!is_member_of_bmp(0x10000));
        assert!(!is_valid_code_point(0xD800)); // surrogate
        assert!(is_valid_code_point(0x10FFFF));
        assert!(!is_valid_code_point(0x110000));
        assert_eq!(utf16_surrogate_pair_to_code_point(0xD83D, 0xDE00), 0x1F600);
    }

    #[test]
    fn id_start() {
        // ASCII handled by the ASCII fast path.
        assert!(is_unicode_id_start(b'a' as u32));
        assert!(is_unicode_id_start(b'_' as u32));
        assert!(!is_unicode_id_start(b'1' as u32));
        assert!(!is_unicode_id_start(b' ' as u32));
        // Non-ASCII letters.
        assert!(is_unicode_id_start(0x00E9)); // é (Latin small e with acute)
        assert!(is_unicode_id_start(0x03B1)); // α Greek small alpha
        assert!(is_unicode_id_start(0x4E2D)); // 中 CJK
        // Non-ID start.
        assert!(!is_unicode_id_start(0x00A0)); // no-break space
        assert!(!is_unicode_id_start(0x0660)); // Arabic-Indic digit zero (Nd, not start)
        // U+2E2F (VERTICAL TILDE) is in UNICODE_LETTERS AND UNICODE_PATTERN_LETTER,
        // so it is a letter but NOT an ID_Start (exercises the PATTERN_LETTER subtraction).
        assert!(is_unicode_only_letter(0x2E2F));      // a Unicode letter
        assert!(!is_unicode_only_id_start(0x2E2F));   // but excluded by PATTERN_LETTER
        assert!(!is_unicode_id_start(0x2E2F));
    }

    #[test]
    fn id_continue() {
        assert!(is_unicode_id_continue(b'a' as u32));
        assert!(is_unicode_id_continue(b'0' as u32));
        assert!(!is_unicode_id_continue(b' ' as u32));
        assert!(is_unicode_id_continue(0x00E9)); // é
        assert!(is_unicode_id_continue(0x0660)); // Arabic-Indic digit zero (Nd)
        assert!(is_unicode_id_continue(0x0300)); // combining grave accent (Mn)
        assert!(is_unicode_id_continue(0x200C)); // ZWNJ (Other_ID_Continue)
        assert!(is_unicode_id_continue(0x200D)); // ZWJ  (Other_ID_Continue)
        assert!(!is_unicode_id_continue(0x0020)); // space
        assert!(is_unicode_id_continue(0x203F)); // Pc -> ID_Continue
    }

    #[test]
    fn only_space() {
        assert!(is_unicode_only_space(0x00A0));
        assert!(!is_unicode_only_space(0x2028)); // line separator is Zl, not Zs
        assert!(is_unicode_only_space(0x3000));
        assert!(!is_unicode_only_space(0x0020)); // ASCII excluded
        assert!(!is_unicode_only_space(b'a' as u32));
    }

    #[test]
    fn category_predicates() {
        assert!(is_unicode_only_letter(0x00E9));
        assert!(!is_unicode_only_letter(b'a' as u32)); // ASCII excluded
        assert!(is_unicode_combining_mark(0x0300));
        assert!(is_unicode_digit(b'5' as u32));
        assert!(is_unicode_digit(0x0660));
        assert!(is_unicode_connector_punctuation(b'_' as u32));
        assert!(is_unicode_connector_punctuation(0x203F)); // ‿ UNDERTIE (Pc)
    }
}
