// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The segmenters behind Intl.Segmenter (plan decision 9 of Phase 5).
//! ICU4X's break iterators only go forward and borrow their input, and the
//! rules need unbounded left context, so a Segments object's boundaries are
//! all computed at once and kept; FindBoundary is a binary search over
//! them. Indices are UTF-16 code-unit indices into the original string.
//!
//! ICU4X 2.4's word segmentation has bugs, all found against Node (ICU4C),
//! which this module works around:
//! - the UTF-16 word path gives a run of consecutive supplementary Chinese
//!   or Japanese characters no dictionary breaks ("𠀀𠀁𠀂" is one segment):
//!   it finds the script runs by code unit, and a surrogate is in none of
//!   the dictionary scripts (`ComplexScriptIteratorUtf16`,
//!   `rule_segmenter_v1/script_iter.rs:104` in `icu_segmenter` 2.4). So
//!   word segmentation runs on a UTF-8 copy of the text;
//! - it puts boundaries before characters that UAX #29's rule WB4 ignores
//!   (Word_Break Extend, Format or ZWJ): a soft hyphen after Thai
//!   ("ก\u{AD}") becomes a segment of its own. Such a boundary is repaired
//!   by dropping it, unless it follows the start of the text, CR, LF or a
//!   Word_Break Newline character (WB3a), and the segments on either side
//!   become one, word-like if either was. This repairs only boundaries
//!   before ignored characters; it is not the whole of WB4;
//! - its word types: a word that ends in a code point with Word_Break
//!   Extend, Format or ZWJ is reported `WordType::None` when more text
//!   follows it (decomposed "café x", Hindi "की एस"), a word that ends in
//!   ZWJ even at the end of the text ("ab\u{200D}"), and a Hebrew word that
//!   ends in an apostrophe ("מס'"), so such a segment is segmented again
//!   without its trailing Extend, Format, ZWJ and Single_Quote code points
//!   and takes the word type at the end of that; and a segment made only of
//!   ignored characters ("\u{E48}" alone) is reported word-like, so such a
//!   segment is word-like only if one of them is Ideographic.
//!
//! Remove each when an ICU4X upgrade fixes it.

use crate::locale::language_id;
use icu::properties::props::{Ideographic, WordBreak};
use icu::properties::{CodePointMapData, CodePointSetData};
use icu::segmenter::options::{SentenceBreakOptions, WordBreakOptions};
use icu::segmenter::{
    GraphemeClusterSegmenter, GraphemeClusterSegmenterBorrowed, SentenceSegmenter, WordSegmenter,
    WordSegmenterBorrowed,
};

/// An ICU4X segmenter for one granularity, with the language identifier of
/// a locale as its content locale. The variants differ in size (the word
/// segmenter is about 2 KB), but a Segmenter is made once per Intl.Segmenter
/// and is always on the heap, inside a boxed HiSegmenter.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Segmenter {
    /// Extended grapheme clusters; they have no locale tailoring.
    Grapheme(GraphemeClusterSegmenterBorrowed<'static>),
    /// Words, with the dictionaries for Chinese, Japanese, Thai, Lao, Khmer
    /// and Burmese.
    Word(WordSegmenter),
    /// Sentences.
    Sentence(SentenceSegmenter),
}

/// The segments of one string: every boundary, and for word granularity
/// whether each segment is word-like.
pub(crate) struct Segments {
    /// The boundaries, ascending, from 0 to the length of the string, in
    /// UTF-16 code units. The empty string has the one boundary 0.
    boundaries: Vec<u32>,
    /// For word granularity, whether the segment that starts at
    /// `boundaries[i]` is word-like, one entry per segment; None otherwise.
    word_like: Option<Vec<bool>>,
}

impl Segmenter {
    /// The segmenter of `granularity` "grapheme", "word" or "sentence" with
    /// the language identifier of `locale` (a canonical tag) as content
    /// locale. None for any other granularity, if `locale` does not parse,
    /// or if ICU4X has no data.
    pub(crate) fn new(locale: &str, granularity: &str) -> Option<Self> {
        let id = language_id(locale)?;
        match granularity {
            "grapheme" => Some(Segmenter::Grapheme(GraphemeClusterSegmenter::new())),
            "word" => {
                let mut options = WordBreakOptions::default();
                options.content_locale = Some(&id);
                WordSegmenter::try_new_dictionary(options)
                    .ok()
                    .map(Segmenter::Word)
            }
            "sentence" => {
                let mut options = SentenceBreakOptions::default();
                options.content_locale = Some(&id);
                SentenceSegmenter::try_new(options)
                    .ok()
                    .map(Segmenter::Sentence)
            }
            _ => None,
        }
    }

    /// The segments of the UTF-16 text `text`, which may have unpaired
    /// surrogates. `text.len()` must fit in a u32.
    pub(crate) fn segment(&self, text: &[u16]) -> Segments {
        debug_assert!(u32::try_from(text.len()).is_ok());
        match self {
            Segmenter::Grapheme(s) => {
                Segments::new(s.segment_utf16(text).map(|b| b as u32).collect(), None)
            }
            Segmenter::Sentence(s) => Segments::new(
                s.as_borrowed()
                    .segment_utf16(text)
                    .map(|b| b as u32)
                    .collect(),
                None,
            ),
            Segmenter::Word(s) => segment_words(s.as_borrowed(), text),
        }
    }
}

/// The Word_Break value of `c`.
fn word_break(c: char) -> WordBreak {
    CodePointMapData::<WordBreak>::new().get(c)
}

/// Whether `c` has Word_Break Extend, Format or ZWJ: a character UAX #29's
/// rule WB4 ignores after any character but a newline.
fn is_ignored(c: char) -> bool {
    let wb = word_break(c);
    wb == WordBreak::Extend || wb == WordBreak::Format || wb == WordBreak::ZWJ
}

/// Whether `c` is CR, LF or has Word_Break Newline, after which WB3a keeps
/// a boundary before an ignored character.
fn is_newline(c: char) -> bool {
    let wb = word_break(c);
    wb == WordBreak::CR || wb == WordBreak::LF || wb == WordBreak::Newline
}

/// Whether a word boundary between `before` and `after` (None at the start
/// or end of the text) falls before an ignored character where WB4 forbids
/// one: `after` is ignored and `before` exists and is not a newline.
fn drops_boundary(before: Option<char>, after: Option<char>) -> bool {
    after.is_some_and(is_ignored) && before.is_some_and(|c| !is_newline(c))
}

/// Whether the word-type re-check removes `c` from the end of a segment:
/// Word_Break Extend, Format, ZWJ or Single_Quote. This is not
/// `is_ignored`: an apostrophe after a word stays a segment of its own.
fn trimmed_for_recheck(c: char) -> bool {
    is_ignored(c) || word_break(c) == WordBreak::SingleQuote
}

/// The word type of a segment that ICU4X 2.4 gets wrong when it is made
/// only of ignored characters: word-like only if one of them is
/// Ideographic (U+16FE4 KHITAN SMALL SCRIPT FILLER, as in ICU4C). None for
/// any other segment.
fn ignored_only_word_like(segment: &str) -> Option<bool> {
    if segment.is_empty() || !segment.chars().all(is_ignored) {
        return None;
    }
    let ideographic = CodePointSetData::new::<Ideographic>();
    Some(segment.chars().any(|c| ideographic.contains(c)))
}

/// The word segments of the UTF-16 text `text`, with the workarounds of the
/// module comment.
fn segment_words(segmenter: WordSegmenterBorrowed<'_>, text: &[u16]) -> Segments {
    // The UTF-8 copy, each unpaired surrogate replaced by U+FFFD. Both have
    // Word_Break Other, so no boundary moves, and both are one UTF-16 code
    // unit, so the UTF-16 length of the copy's characters maps its byte
    // offsets back to indices into `text`.
    let mut utf8 = String::with_capacity(text.len());
    utf8.extend(
        char::decode_utf16(text.iter().copied()).map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER)),
    );
    // The byte offset in `utf8` and the UTF-16 index of the last boundary
    // ICU4X gave.
    let mut prev_byte = 0;
    let mut prev_index = 0u32;
    let words = segmenter
        .segment_str(&utf8)
        .iter_with_word_type()
        .filter(|&(byte, _)| byte != 0)
        .map(|(byte, word_type)| {
            let segment = &utf8[prev_byte..byte];
            let mut is_word = word_type.is_word_like();
            if !is_word && segment.chars().next_back().is_some_and(trimmed_for_recheck) {
                let base = segment.trim_end_matches(trimmed_for_recheck);
                if let Some((_, t)) = segmenter.segment_str(base).iter_with_word_type().last() {
                    is_word = t.is_word_like();
                }
            }
            if let Some(w) = ignored_only_word_like(segment) {
                is_word = w;
            }
            prev_byte = byte;
            prev_index += segment.chars().map(|c| c.len_utf16() as u32).sum::<u32>();
            let drop = drops_boundary(segment.chars().next_back(), utf8[byte..].chars().next());
            (prev_index, is_word, drop)
        });
    let (boundaries, word_like) = merge_segments(words);
    debug_assert_eq!(boundaries.last().copied().unwrap_or(0) as usize, text.len());
    Segments::new(boundaries, Some(word_like))
}

/// Drop word boundaries. `words` is each word segment's end boundary,
/// ascending and ending at the length of the text, with whether the segment
/// is word-like and whether to drop the boundary (`drops_boundary`; never
/// at the length). Returns the kept boundaries, from 0, and for each
/// segment between them whether it is word-like: a segment merged from
/// several word segments is word-like if any of them was.
fn merge_segments(words: impl IntoIterator<Item = (u32, bool, bool)>) -> (Vec<u32>, Vec<bool>) {
    let mut boundaries = vec![0];
    let mut word_like = Vec::new();
    // Whether a part of the segment that ends at the next kept boundary is
    // word-like.
    let mut pending_word = false;
    for (end, is_word, drop) in words {
        pending_word |= is_word;
        if drop {
            continue;
        }
        boundaries.push(end);
        word_like.push(pending_word);
        pending_word = false;
    }
    (boundaries, word_like)
}

impl Segments {
    /// The segments with `boundaries` and `word_like`, without the spare
    /// capacity of the vectors, which would otherwise live as long as the
    /// Segments object.
    fn new(mut boundaries: Vec<u32>, mut word_like: Option<Vec<bool>>) -> Self {
        boundaries.shrink_to_fit();
        if let Some(w) = &mut word_like {
            w.shrink_to_fit();
        }
        Segments {
            boundaries,
            word_like,
        }
    }

    /// The length of the string, the last boundary.
    fn len(&self) -> u32 {
        *self.boundaries.last().unwrap_or(&0)
    }

    /// FindBoundary: for `index` below the length of the string, the last
    /// boundary at or before `index` (`after` false) or the first boundary
    /// after it (`after` true). None if `index` is not below the length.
    pub(crate) fn find_boundary(&self, index: u32, after: bool) -> Option<u32> {
        if index >= self.len() {
            return None;
        }
        // The number of boundaries at or before index, at least 1 (the
        // boundary 0) and below the count (the length is after index).
        let at_or_before = self.boundaries.partition_point(|&b| b <= index);
        Some(if after {
            self.boundaries[at_or_before]
        } else {
            self.boundaries[at_or_before - 1]
        })
    }

    /// Whether the segment that starts at the boundary `start` is word-like.
    /// None if the granularity is not word, or if `start` is not a boundary
    /// or is the length of the string.
    pub(crate) fn is_word_like(&self, start: u32) -> Option<bool> {
        let word_like = self.word_like.as_ref()?;
        let i = self.boundaries.binary_search(&start).ok()?;
        word_like.get(i).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// The time limit for segmenting a long run. Segmenting one normally
    /// takes about 0.1 s in the dev profile; with ICU4X 2.3's per-break
    /// copy of the pending breaks restored (in a patched icu_segmenter),
    /// which makes it quadratic, the Han run took 1.65 s and the Thai run,
    /// split after every code point, 4.75 s. The limit leaves room for a
    /// loaded machine below those.
    const LONG_RUN_LIMIT: Duration = Duration::from_secs(1);

    /// The segmenter of `locale` and `granularity`, which must exist.
    fn seg(locale: &str, granularity: &str) -> Segmenter {
        Segmenter::new(locale, granularity).unwrap_or_else(|| panic!("{locale} {granularity}"))
    }

    /// UTF-16 of `s`.
    fn u16s(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// The boundaries of `segments`, ascending, from 0 to the length, read
    /// through find_boundary only.
    fn boundaries(segments: &Segments, len: usize) -> Vec<u32> {
        let mut result = vec![0];
        let mut at = 0;
        while (at as usize) < len {
            at = segments.find_boundary(at, true).unwrap();
            result.push(at);
        }
        result
    }

    /// The segments of `text` (UTF-16) by `locale` and `granularity`,
    /// joined by `|`, each word-like one followed by `*`. An unpaired
    /// surrogate shows as U+FFFD.
    fn render16(locale: &str, granularity: &str, text: &[u16]) -> String {
        let segments = seg(locale, granularity).segment(text);
        let b = boundaries(&segments, text.len());
        let mut out = Vec::new();
        for w in b.windows(2) {
            let mut s = String::from_utf16_lossy(&text[w[0] as usize..w[1] as usize]);
            if segments.is_word_like(w[0]) == Some(true) {
                s.push('*');
            }
            out.push(s);
        }
        out.join("|")
    }

    /// render16 of the UTF-16 of `text`.
    fn render(locale: &str, granularity: &str, text: &str) -> String {
        render16(locale, granularity, &u16s(text))
    }

    #[test]
    fn grapheme() {
        let g = seg("en", "grapheme");
        let mut text = u16s("e\u{301}👩\u{200d}👩\u{200d}👧\u{200d}👦🇺🇸\r\n");
        text.push(0xD800);
        text.push(u16::from(b'x'));
        assert_eq!(
            boundaries(&g.segment(&text), text.len()),
            [0, 2, 13, 17, 19, 20, 21]
        );
        let mut text = u16s("\u{e9}👩\u{200d}👩\u{200d}👧\u{200d}👦🇺🇸\r\n");
        text.push(0xD800);
        text.push(u16::from(b'x'));
        assert_eq!(
            boundaries(&g.segment(&text), text.len()),
            [0, 1, 12, 16, 18, 19, 20]
        );
        // Unicode 15.1's GB9c keeps each conjunct whole.
        assert_eq!(render("en", "grapheme", "क्षि ক্ষ্ম"), "क्षि| |ক্ষ্ম");
    }

    #[test]
    fn word() {
        assert_eq!(
            render("en", "word", "Hello, world! It's 3.14."),
            "Hello*|,| |world*|!| |It's*| |3.14*|."
        );
        assert_eq!(render("ja", "word", "こんにちは世界"), "こんにちは*|世界*");
        assert_eq!(
            render("zh", "word", "我爱北京天安门"),
            "我*|爱*|北京*|天安门*"
        );
        assert_eq!(render("th", "word", "ทุกสองสัปดาห์"), "ทุก*|สอง*|สัปดาห์*");
        // ICU4X 2.4 shrank its Thai, Lao, Khmer and Burmese dictionaries by
        // 42% together; these sentences still segment as in Node 24.
        assert_eq!(
            render("lo", "word", "ພາສາລາວເປັນພາສາທີ່ສວຍງາມ"),
            "ພາສາ*|ລາວ*|ເປັນ*|ພາສາ*|ທີ່*|ສວຍງາມ*"
        );
        assert_eq!(
            render("km", "word", "ប្រទេសកម្ពុជាមានប្រវត្តិសាស្ត្រយូរលង់"),
            "ប្រទេស*|កម្ពុជា*|មាន*|ប្រវត្តិសាស្ត្រ*|យូរលង់*"
        );
        assert_eq!(
            render("my", "word", "ကျွန်တော်မြန်မာစကားပြောတတ်တယ်"),
            "ကျွန်တော်*|မြန်မာ*|စကားပြော*|တတ်*|တယ်*"
        );
    }

    /// Supplementary characters in Chinese and Japanese runs, alone and
    /// consecutive (which ICU4X's UTF-16 word path keeps together), as in
    /// Node.
    #[test]
    fn word_supplementary() {
        assert_eq!(render("en", "word", "𠀀𠀁𠀂 𪚥𪚥"), "𠀀*|𠀁*|𠀂*| |𪚥*|𪚥*");
        assert_eq!(
            render("ja", "word", "日本語𠀀テスト"),
            "日本語*|𠀀*|テスト*"
        );
        assert_eq!(render("zh", "word", "我爱𠀀北京"), "我*|爱*|𠀀*|北京*");
    }

    /// The word-type re-check of the module comment.
    #[test]
    fn word_recheck() {
        // A word that ends in Word_Break Extend, Format or ZWJ, followed by
        // more text.
        for (locale, text, first) in [
            ("fr", "cafe\u{301} x", "cafe\u{301}"),
            ("hi", "की एस", "की"),
            ("ar", "كَتَبَ ولد", "كَتَبَ"),
        ] {
            let r = render(locale, "word", text);
            assert!(r.starts_with(&format!("{first}*|")), "{text}: {r}");
        }
        // ICU4X gives a word that ends in ZWJ rule status 0 even at the end
        // of the text, so the word is segmented again without its trailing
        // Extend, Format and ZWJ code points.
        assert_eq!(render("en", "word", "ab\u{200d} x"), "ab\u{200d}*| |x*");
        assert_eq!(render("en", "word", "ab\u{200d}"), "ab\u{200d}*");
        assert_eq!(
            render("en", "word", "ab\u{301}\u{200d} x"),
            "ab\u{301}\u{200d}*| |x*"
        );
        assert_eq!(
            render("en", "word", "ab\u{200d}\u{301}\u{ad} x"),
            "ab\u{200d}\u{301}\u{ad}*| |x*"
        );
        // Nothing is left once they are removed: not word-like.
        assert_eq!(
            render("en", "word", "\u{301}\u{200d} x"),
            "\u{301}\u{200d}| |x*"
        );
        // Unpaired surrogates keep UTF-16 indices.
        let text = [0x61, 0xD800, 0x62, 0x20, 0xDC00];
        let segments = seg("en", "word").segment(&text);
        assert_eq!(boundaries(&segments, text.len()), [0, 1, 2, 3, 4, 5]);
        let word_like: Vec<_> = (0..5).map(|i| segments.is_word_like(i)).collect();
        assert_eq!(
            word_like,
            [
                Some(true),
                Some(false),
                Some(true),
                Some(false),
                Some(false)
            ]
        );
    }

    #[test]
    fn sentence() {
        let text = "Mr. Smith left. Did he? Yes!\nOK";
        assert_eq!(
            render("en", "sentence", text),
            "Mr. |Smith left. |Did he? |Yes!\n|OK"
        );
        // ICU4X tailors el sentences: ";" is a question mark.
        let greek = u16s("Τι κάνεις; Καλά. Ναι;Όχι");
        let count =
            |locale| boundaries(&seg(locale, "sentence").segment(&greek), greek.len()).len() - 1;
        assert_eq!(count("en"), 2);
        assert_eq!(count("el"), 4);
    }

    /// test262's containing/unbreakable-input.js inputs.
    fn unbreakable_inputs() -> Vec<Vec<u16>> {
        let mut inputs: Vec<Vec<u16>> = [
            "a",
            " ",
            "\u{10000}",
            "台",
            "\u{301}",
            "a\u{301}",
            "ซิ่",
            "𐂰",
            "\u{1F44B}\u{1F3FB}",
            "\u{1F468}\u{1F3FB}\u{200D}\u{1F9B0}",
            "\u{1102}",
            "\u{1162}",
            "\u{11A9}",
            "\u{1102}\u{1162}",
            "\u{1102}\u{1162}\u{11A9}",
            "\u{1102}\u{1102}",
            "\u{1102}\u{1102}\u{1162}",
            "\u{1102}\u{1102}\u{1162}\u{11A9}",
            "\u{1162}\u{1162}",
            "\u{1162}\u{11A9}",
            "\u{1102}\u{1162}\u{1162}",
            "\u{11A9}\u{11A9}",
            "\u{1102}\u{1162}\u{11A9}\u{11A9}",
        ]
        .iter()
        .map(|s| u16s(s))
        .collect();
        inputs.push(vec![0xD800]);
        inputs.push(vec![0xDC00]);
        inputs
    }

    /// test262's containing/breakable-input.js inputs.
    fn breakable_inputs() -> Vec<Vec<u16>> {
        vec![
            u16s("123 "),
            u16s("a "),
            u16s(" a"),
            u16s(" \u{10000}"),
            u16s("\u{10000} "),
            vec![0xDC00, 0xD800],
            vec![0xD800, 0x20],
            vec![0xDC00, 0x20],
            vec![0x20, 0xD800],
            vec![0x20, 0xDC00],
            u16s(" 台"),
            u16s("台 "),
            u16s("\u{301} "),
        ]
    }

    #[test]
    fn unbreakable() {
        for granularity in ["grapheme", "word", "sentence"] {
            let s = seg("en", granularity);
            for input in unbreakable_inputs() {
                assert_eq!(
                    boundaries(&s.segment(&input), input.len()),
                    [0, input.len() as u32],
                    "{granularity} {input:x?}"
                );
            }
        }
    }

    #[test]
    fn breakable() {
        for granularity in ["grapheme", "word"] {
            let s = seg("en", granularity);
            for input in breakable_inputs() {
                let b = boundaries(&s.segment(&input), input.len());
                assert!(b.len() > 2, "{granularity} {input:x?}: {b:?}");
            }
        }
    }

    /// Texts with Prepend characters and their word segments, as Node gives
    /// them.
    const PREPEND: [(&str, &str); 6] = [
        ("\u{600} x", "\u{600}*| |x*"),
        ("x \u{6dd} y", "x*| |\u{6dd}*| |y*"),
        ("a\u{600} b", "a\u{600}*| |b*"),
        ("\u{600}x", "\u{600}x*"),
        ("\u{70f} x", "\u{70f}*| |x*"),
        ("\u{110bd} x", "\u{110bd}*| |x*"),
    ];

    /// A Thai text with the tone mark U+0E48 on its base ช in "ช่", which
    /// must stay on it (ICU4X 2.3's dictionary split it).
    const THAI: &str = "วัดไทรตีระฆังเบาลงช่วงเข้าพรรษา";

    /// Check that no word boundary of `text` by `locale` falls before a
    /// character with Word_Break Extend, Format or ZWJ, unless it follows
    /// the start of the text, CR, LF or a Word_Break Newline character.
    fn assert_no_boundary_before_ignored(locale: &str, text: &[u16]) {
        let words = boundaries(&seg(locale, "word").segment(text), text.len());
        let decode = |units: &[u16]| {
            char::decode_utf16(units.iter().copied())
                .next()
                .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        };
        for &b in &words {
            let b = b as usize;
            // The code point that ends at b: a surrogate pair, or one unit.
            let before = match b {
                0 => None,
                _ if b >= 2 && decode(&text[b - 2..b]).is_some_and(|c| c.len_utf16() == 2) => {
                    decode(&text[b - 2..b])
                }
                _ => decode(&text[b - 1..b]),
            };
            let after = decode(&text[b..text.len().min(b + 2)]);
            assert!(
                !(after.is_some_and(is_ignored) && before.is_some_and(|c| !is_newline(c))),
                "{b} in {text:x?}"
            );
        }
    }

    /// drops_boundary: only before an ignored character, and not at the
    /// start of the text or after a newline.
    #[test]
    fn drops_boundary_rules() {
        // Before Extend (U+0301, U+0E48), Format (U+00AD) and ZWJ.
        for after in ['\u{301}', '\u{e48}', '\u{ad}', '\u{200d}'] {
            for before in ['a', ' ', '.', '\u{301}', '\u{e0a}'] {
                assert!(
                    drops_boundary(Some(before), Some(after)),
                    "{before:?} {after:?}"
                );
            }
            // The start of the text, and after CR, LF and Newline (U+000B,
            // U+0085, U+2028).
            assert!(!drops_boundary(None, Some(after)));
            for before in ['\r', '\n', '\u{b}', '\u{85}', '\u{2028}'] {
                assert!(!drops_boundary(Some(before), Some(after)), "{before:?}");
            }
        }
        // Not before anything else: U+0E33 is Grapheme_Cluster_Break
        // SpacingMark but Word_Break Other, and an apostrophe is
        // Single_Quote.
        for after in ['x', ' ', '\u{e33}', '\'', '\u{600}'] {
            assert!(!drops_boundary(Some('a'), Some(after)), "{after:?}");
        }
        assert!(!drops_boundary(Some('a'), None));
    }

    /// merge_segments on synthetic boundaries, each with whether to drop it
    /// (`D`) or keep it (`K`).
    #[test]
    fn merge_segments_rules() {
        const D: bool = true;
        const K: bool = false;
        let merge = |words: &[(u32, bool, bool)]| merge_segments(words.iter().copied());
        // Nothing dropped.
        assert_eq!(
            merge(&[(2, true, K), (5, false, K), (6, true, K)]),
            (vec![0, 2, 5, 6], vec![true, false, true])
        );
        // One boundary dropped, with only the first part word-like, only the
        // second, and neither.
        for (first, second, merged) in [
            (true, false, true),
            (false, true, true),
            (false, false, false),
        ] {
            assert_eq!(
                merge(&[(2, false, K), (3, first, D), (5, second, K), (6, false, K)]),
                (vec![0, 2, 5, 6], vec![false, merged, false]),
                "{first} {second}"
            );
        }
        // Two boundaries dropped in a row; only the middle part is word-like.
        assert_eq!(
            merge(&[
                (2, true, K),
                (3, false, D),
                (4, true, D),
                (5, false, K),
                (6, false, K)
            ]),
            (vec![0, 2, 5, 6], vec![true, true, false])
        );
        // The first boundary dropped.
        assert_eq!(
            merge(&[(1, false, D), (2, false, K), (6, true, K)]),
            (vec![0, 2, 6], vec![false, true])
        );
        // The empty text.
        assert_eq!(merge_segments([]), (vec![0], vec![]));
    }

    /// Rule A: boundaries before characters WB4 ignores are repaired.
    #[test]
    fn boundaries_before_ignored() {
        // The tone mark U+0E48 stays on its base ช.
        assert_eq!(
            render("th", "word", THAI),
            "วัด*|ไทร*|ตีระ*|ฆั*|งเ*|บาล*|งช่*|วง*|เข้า*|พรรษา*"
        );
        // A soft hyphen (Format) after Thai stays on the word, as in Node.
        assert_eq!(render("th", "word", "ก\u{ad} x"), "ก\u{ad}*| |x*");
        // U+0E33 is Grapheme_Cluster_Break SpacingMark but Word_Break Other:
        // nothing is merged, as in Node.
        assert_eq!(render("th", "word", " \u{e33}"), " |\u{e33}*");
        // Node gives "\u{e48}|\u{e33}*". ICU4X 2.4's Thai dictionary, on
        // Unicode 18's data, keeps a leading tone mark and U+0E33 as one
        // word, and ECMA-402 leaves segmentation to the implementation.
        assert_eq!(render("th", "word", "\u{e48}\u{e33}"), "\u{e48}\u{e33}*");
        // Node gives "a.\u{e33}*": ICU4C treats Thai letters as ALetter, so
        // WB6 and WB7 keep the period; ICU4X does not.
        assert_eq!(render("th", "word", "a.\u{e33}"), "a*|.|\u{e33}*");
        // After CR, LF and U+2028 (Newline), the boundary before the marks
        // stays and there is none between the marks, as in Node.
        assert_eq!(
            render("en", "word", "\r\u{301}\u{301}x"),
            "\r|\u{301}\u{301}|x*"
        );
        assert_eq!(
            render("en", "word", "\n\u{301}\u{200d}\u{ad} x"),
            "\n|\u{301}\u{200d}\u{ad}| |x*"
        );
        assert_eq!(
            render("en", "word", "\u{2028}\u{301}\u{301} x"),
            "\u{2028}|\u{301}\u{301}| |x*"
        );
        assert_eq!(
            render("en", "word", "\r\n\u{301}\u{301}"),
            "\r\n|\u{301}\u{301}"
        );
        // Prepend characters are not ignored: their boundaries stay.
        for (text, words) in PREPEND {
            assert_eq!(render("en", "word", text), *words, "{text}");
        }
        assert_eq!(
            render("en", "sentence", "Hi. \u{600}Bye"),
            "Hi. |\u{600}Bye"
        );

        let mut inputs: Vec<(&str, Vec<u16>)> = [
            ("th", THAI),
            ("th", "ก\u{ad} x"),
            ("th", " \u{e33}"),
            ("th", "\u{e48}\u{e33}"),
            ("th", " \u{e48}"),
            ("th", ".\u{e48}"),
            ("en", "\r\u{301}\u{301}x"),
            ("en", "\n\u{301}\u{200d}\u{ad} x"),
            ("en", "\u{2028}\u{301}\u{301} x"),
            ("en", "Hello, world! It's 3.14."),
            ("ja", "こんにちは世界"),
            ("zh", "我爱北京天安门"),
            ("th", "ทุกสองสัปดาห์"),
            ("en", "𠀀𠀁𠀂 𪚥𪚥"),
            ("ja", "日本語𠀀テスト"),
            ("fr", "cafe\u{301} x"),
            ("hi", "की एस"),
            ("ar", "كَتَبَ ولد"),
            ("he", "מס'\u{301} x"),
            ("en", "ab\u{200d}\u{301}\u{ad} x"),
            ("en", "क्षि ক্ষ্ম"),
        ]
        .iter()
        .map(|&(l, s)| (l, u16s(s)))
        .collect();
        for (text, _) in PREPEND {
            inputs.push(("en", u16s(text)));
        }
        inputs.push(("en", vec![0x61, 0xD800, 0x301, 0x62, 0x20, 0xDC00]));
        for input in unbreakable_inputs().into_iter().chain(breakable_inputs()) {
            inputs.push(("en", input));
        }
        for (locale, text) in inputs {
            assert_no_boundary_before_ignored(locale, &text);
        }
    }

    /// Rules B and C: the word types ICU4X 2.4 gets wrong.
    #[test]
    fn word_types() {
        // B: Hebrew letters keep a trailing apostrophe (WB7a), and the word
        // is word-like, as in Node.
        assert_eq!(render("he", "word", "מס'"), "מס'*");
        assert_eq!(render("he", "word", "מס' x"), "מס'*| |x*");
        // The trim repeats over an apostrophe and then a mark. Node gives
        // this word not word-like: ICU4C's rule status for a Hebrew letter
        // and an apostrophe does not survive a following Extend.
        assert_eq!(render("he", "word", "מס'\u{301} x"), "מס'\u{301}*| |x*");
        // An apostrophe alone or after a Latin word is its own segment and
        // not word-like; after U+0600 too (rule A does not treat it as
        // ignored).
        assert_eq!(render("en", "word", "'"), "'");
        assert_eq!(render("en", "word", "x'"), "x*|'");
        assert_eq!(render("en", "word", "\u{600}'"), "\u{600}*|'");
        // C: a segment made only of ignored characters is not word-like,
        // also after rule A merges it into a space or a period, as in Node.
        assert_eq!(render("th", "word", "\u{e48}"), "\u{e48}");
        assert_eq!(render("th", "word", " \u{e48}"), " \u{e48}");
        assert_eq!(render("th", "word", ".\u{e48}"), ".\u{e48}");
        // Unless one of them is Ideographic: U+16FE4 KHITAN SMALL SCRIPT
        // FILLER is Extend and Ideographic, word-like as in Node.
        assert_eq!(render("en", "word", "\u{16fe4}"), "\u{16fe4}*");
        assert_eq!(ignored_only_word_like("\u{16fe4}\u{301}"), Some(true));
        assert_eq!(ignored_only_word_like("\u{301}\u{200d}"), Some(false));
        assert_eq!(ignored_only_word_like("a\u{301}"), None);
        assert_eq!(ignored_only_word_like(""), None);
    }

    #[test]
    fn find_boundary() {
        let text = u16s("Hello world");
        let segments = seg("en", "word").segment(&text);
        assert_eq!(segments.find_boundary(7, false), Some(6));
        assert_eq!(segments.find_boundary(7, true), Some(11));
        assert_eq!(segments.find_boundary(5, false), Some(5));
        assert_eq!(segments.find_boundary(5, true), Some(6));
        assert_eq!(segments.find_boundary(0, false), Some(0));
        assert_eq!(segments.find_boundary(10, true), Some(11));
        assert_eq!(segments.is_word_like(0), Some(true));
        assert_eq!(segments.is_word_like(5), Some(false));
        assert_eq!(segments.is_word_like(6), Some(true));
    }

    /// Plan Review Focus 5: every locale has each granularity.
    #[test]
    fn every_locale() {
        let text = u16s("Hi there. Bye");
        for locale in crate::locale::test_locales() {
            for granularity in ["grapheme", "word", "sentence"] {
                let segments = seg(locale, granularity).segment(&text);
                assert_eq!(
                    segments.find_boundary(12, true),
                    Some(13),
                    "{locale} {granularity}"
                );
            }
            assert_eq!(
                render16(locale, "word", &text),
                "Hi*| |there*|.| |Bye*",
                "{locale}"
            );
            // No locale's sentence tailoring changes this string.
            assert_eq!(
                render16(locale, "sentence", &text),
                "Hi there. |Bye",
                "{locale}"
            );
        }
    }

    /// Plan Review Focus 4: a one-million-code-unit string.
    #[test]
    fn large() {
        let pattern = u16s("Hello, world. 日本語テスト ทุกสองสัปดาห์ 👩\u{200d}👧 ");
        let text: Vec<u16> = pattern.iter().copied().cycle().take(1_000_000).collect();
        for granularity in ["grapheme", "word", "sentence"] {
            let segments = seg("en", granularity).segment(&text);
            assert_eq!(
                segments.find_boundary(999_999, true),
                Some(1_000_000),
                "{granularity}"
            );
            assert_eq!(segments.find_boundary(0, false), Some(0), "{granularity}");
            // No spare capacity outlives the segmentation.
            let b = &segments.boundaries;
            assert_eq!(b.capacity(), b.len(), "{granularity}");
            if let Some(w) = &segments.word_like {
                assert_eq!(w.capacity(), w.len());
            }
        }
        assert_no_boundary_before_ignored("en", &text);
    }

    /// Plan Review Focus 5: a long unbroken Han run, which ICU4X 2.3
    /// segmented in time quadratic in its words, takes well under a second:
    /// 30,000 copies of 我|爱|北京|天安门, 210,000 code units.
    #[test]
    fn long_han_run() {
        let han = u16s(&"我爱北京天安门".repeat(30_000));
        let start = Instant::now();
        let segments = seg("zh", "word").segment(&han);
        let elapsed = start.elapsed();
        assert!(elapsed < LONG_RUN_LIMIT, "{elapsed:?}");
        assert_eq!(segments.boundaries.len(), 120_001);
        assert_eq!(segments.find_boundary(209_999, false), Some(209_997));
        assert_eq!(segments.find_boundary(209_999, true), Some(210_000));
        assert_eq!(segments.is_word_like(209_997), Some(true));
    }

    /// Plan Review Focus 5: a long Thai run, ก and 199,999 tone marks
    /// U+0E48 (200,000 code units), is one segment, made in well under a
    /// second (ICU4X 2.3 took time quadratic in its length).
    #[test]
    fn long_thai_run() {
        let mut thai = u16s("ก");
        thai.extend(std::iter::repeat_n(0xE48, 199_999));
        let start = Instant::now();
        let segments = seg("th", "word").segment(&thai);
        let elapsed = start.elapsed();
        assert!(elapsed < LONG_RUN_LIMIT, "{elapsed:?}");
        assert_eq!(segments.boundaries, [0, 200_000]);
        assert_eq!(segments.find_boundary(199_999, false), Some(0));
        assert_eq!(segments.is_word_like(0), Some(true));
    }

    #[test]
    fn rejects() {
        assert!(Segmenter::new("en", "line").is_none());
        assert!(Segmenter::new("en", "").is_none());
        for granularity in ["grapheme", "word", "sentence"] {
            assert!(Segmenter::new("en_US", granularity).is_none());
        }
        let text = u16s("Hello world");
        for granularity in ["grapheme", "word", "sentence"] {
            let segments = seg("en", granularity).segment(&text);
            assert_eq!(segments.find_boundary(11, false), None);
            assert_eq!(segments.find_boundary(11, true), None);
            assert_eq!(segments.find_boundary(u32::MAX, true), None);
            if granularity != "word" {
                assert_eq!(segments.is_word_like(0), None);
            }
        }
        let segments = seg("en", "word").segment(&text);
        // Not a boundary, and the length.
        assert_eq!(segments.is_word_like(1), None);
        assert_eq!(segments.is_word_like(11), None);
        assert_eq!(segments.is_word_like(12), None);
        // The empty string has the one boundary 0, and no index.
        for granularity in ["grapheme", "word", "sentence"] {
            let empty = seg("en", granularity).segment(&[]);
            assert_eq!(empty.boundaries, [0], "{granularity}");
            assert_eq!(empty.boundaries.capacity(), 1, "{granularity}");
            assert_eq!(empty.find_boundary(0, false), None);
            assert_eq!(empty.is_word_like(0), None);
        }
    }
}
