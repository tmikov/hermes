//! Unicode character properties for the JS lexer, ported from
//! include/hermes/Platform/Unicode/CharacterProperties.{h,cpp}. The range tables
//! in `tables` are generated from lib/Platform/Unicode/UnicodeData.inc by
//! gen_tables.py and pinned to Hermes's Unicode version (17.0.0). RegExp
//! canonicalization / property escapes are intentionally not ported here.

pub mod tables;

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
