// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The list formatter behind Intl.ListFormat (plan decision 2 of Phase 5).
//! JS performs CreatePartsFromList; native code supplies only the literal
//! text ICU4X's ListFormatter writes between the elements. ICU4X never
//! reorders elements, and its conditional patterns (Spanish y/e and o/u,
//! Hebrew ו/ו‑) test only the last element, so the literals of a list
//! depend on its length (two, or more) and its last element alone.

use crate::locale::language_id;
use icu::list::options::{ListFormatterOptions, ListLength};
use icu::list::{parts, ListFormatter, ListFormatterPreferences};
use std::fmt;
use writeable::{Part, PartsWrite, Writeable};

/// An ICU4X list formatter for one locale, type and style.
pub(crate) struct ListFormat(ListFormatter);

/// UTF-16 text as a Writeable, with each unpaired surrogate read as
/// U+FFFD. It only chooses ICU4X's pattern; its text never comes back.
#[derive(Clone)]
struct Utf16Lossy<'a>(&'a [u16]);

impl Writeable for Utf16Lossy<'_> {
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        for c in char::decode_utf16(self.0.iter().copied()) {
            sink.write_char(c.unwrap_or(char::REPLACEMENT_CHARACTER))?;
        }
        Ok(())
    }
}

/// A PartsWrite that keeps the text of each top-level LITERAL part, in
/// order, empty ones included, and drops everything else.
#[derive(Default)]
struct LiteralSink {
    /// The literals so far.
    literals: Vec<String>,
    /// The nesting depth of parts.
    depth: usize,
    /// Whether text goes to the last literal: inside a top-level LITERAL.
    collecting: bool,
}

impl fmt::Write for LiteralSink {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.collecting {
            if let Some(last) = self.literals.last_mut() {
                last.push_str(s);
            }
        }
        Ok(())
    }
}

impl PartsWrite for LiteralSink {
    type SubPartsWrite = Self;

    fn with_part(
        &mut self,
        part: Part,
        mut f: impl FnMut(&mut Self) -> fmt::Result,
    ) -> fmt::Result {
        let literal = self.depth == 0 && part == parts::LITERAL;
        if literal {
            self.literals.push(String::new());
            self.collecting = true;
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        if literal {
            self.collecting = false;
        }
        result
    }
}

impl ListFormat {
    /// The list format for the language identifier of `locale` (a
    /// canonical tag), `kind` "conjunction", "disjunction" or "unit" (CLDR's
    /// "standard", "or" and "unit") and `style` "long", "short" or
    /// "narrow". None for any other value, if `locale` does not parse, or
    /// if ICU4X has no data.
    pub(crate) fn new(locale: &str, kind: &str, style: &str) -> Option<Self> {
        let length = match style {
            "long" => ListLength::Wide,
            "short" => ListLength::Short,
            "narrow" => ListLength::Narrow,
            _ => return None,
        };
        let prefs = ListFormatterPreferences::from(&language_id(locale)?);
        let options = ListFormatterOptions::default().with_length(length);
        let formatter = match kind {
            "conjunction" => ListFormatter::try_new_and(prefs, options),
            "disjunction" => ListFormatter::try_new_or(prefs, options),
            "unit" => ListFormatter::try_new_unit(prefs, options),
            _ => return None,
        };
        formatter.ok().map(ListFormat)
    }

    /// The literal text around the elements of a list whose last element is
    /// `last` (UTF-16; an unpaired surrogate reads as U+FFFD): for a list
    /// of two elements (`pair`) the 3 pieces before, between and after
    /// them; otherwise the 5 pieces of a list of four, before the first,
    /// after the first, between two middle elements, before the last and
    /// after the last. A piece may be empty.
    pub(crate) fn literals(&self, pair: bool, last: &[u16]) -> Vec<String> {
        let count = if pair { 2 } else { 4 };
        let elements = std::iter::repeat_n(Utf16Lossy(&[]), count - 1)
            .chain(std::iter::once(Utf16Lossy(last)));
        let mut sink = LiteralSink::default();
        // Neither the sink nor the elements ever fail.
        let result = self.0.format(elements).write_to_parts(&mut sink);
        debug_assert!(result.is_ok());
        debug_assert_eq!(sink.literals.len(), count + 1);
        sink.literals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list format of `locale`, `kind` and `style`, which must exist.
    fn lf(locale: &str, kind: &str, style: &str) -> ListFormat {
        ListFormat::new(locale, kind, style).unwrap_or_else(|| panic!("{locale} {kind} {style}"))
    }

    /// UTF-16 of `s`.
    fn u16s(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn english() {
        let and = lf("en", "conjunction", "long");
        assert_eq!(and.literals(true, &[]), ["", " and ", ""]);
        assert_eq!(and.literals(false, &[]), ["", ", ", ", ", ", and ", ""]);
        let or = lf("en", "disjunction", "long");
        assert_eq!(or.literals(true, &[]), ["", " or ", ""]);
        assert_eq!(or.literals(false, &[])[3], ", or ");
        assert_eq!(
            lf("en", "conjunction", "short").literals(false, &[])[3],
            ", & "
        );
        assert_eq!(
            lf("en", "unit", "narrow").literals(true, &[]),
            ["", " ", ""]
        );
    }

    #[test]
    fn conditional() {
        let es = lf("es", "conjunction", "long");
        assert_eq!(es.literals(true, &u16s("Ibiza"))[1], " e ");
        assert_eq!(es.literals(true, &u16s("hielo"))[1], " y ");
        // The end pattern is conditional too.
        assert_eq!(es.literals(false, &u16s("Ibiza"))[3], " e ");
        let es_or = lf("es", "disjunction", "long");
        assert_eq!(es_or.literals(true, &u16s("ocho"))[1], " u ");
        assert_eq!(es_or.literals(true, &u16s("siete"))[1], " o ");
        let he = lf("he", "conjunction", "long");
        // U+2011 NON-BREAKING HYPHEN, ICU4X's data (plan decision 2).
        assert_eq!(he.literals(true, &u16s("Ibiza"))[1], " ו\u{2011}");
        assert_eq!(he.literals(true, &u16s("בננה"))[1], " ו");
        // An unpaired surrogate reads as U+FFFD, which is not Hebrew.
        assert_eq!(he.literals(true, &[0xD800])[1], " ו\u{2011}");
        assert_eq!(he.literals(true, &[0xDFFF, 0x05D1])[1], " ו\u{2011}");
    }

    #[test]
    fn suffixes() {
        assert_eq!(
            lf("ti", "conjunction", "long").literals(true, &[]),
            ["", "ን ", "ን"]
        );
        let mn = lf("mn", "disjunction", "long").literals(false, &[]);
        assert_eq!(mn[4], " зэргийн аль нэг");
    }

    /// Plan Review Focus 5: every locale, kind and style has data.
    #[test]
    fn every_locale() {
        for locale in crate::locale::test_locales() {
            for kind in ["conjunction", "disjunction", "unit"] {
                for style in ["long", "short", "narrow"] {
                    let f = lf(locale, kind, style);
                    assert_eq!(f.literals(true, &[]).len(), 3, "{locale} {kind} {style}");
                    assert_eq!(f.literals(false, &[]).len(), 5, "{locale} {kind} {style}");
                }
            }
        }
    }

    #[test]
    fn rejects() {
        assert!(ListFormat::new("en", "and", "long").is_none());
        assert!(ListFormat::new("en", "conjunction", "wide").is_none());
        assert!(ListFormat::new("en_US", "conjunction", "long").is_none());
    }
}
