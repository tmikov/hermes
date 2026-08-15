/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Ported from juno atom_table for the Hermes Rust lexer; carries encapsulated
//! unsafe; adds a byte/WTF-8 intern path.

use std::cell::Cell;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt::Formatter;
use std::ptr::null;

/// Type used to hold a string index internally.
type NumIndex = u32;

/// A string uniquing table - only one copy of a string is stored and all attempts
/// to add the same string again return the same atom. This table is intended to
/// be easily shareable, so it utilizes interior mutability. UnsafeCell<> is safe
/// because we never allow reference to it to escape.
#[derive(Debug, Default)]
pub struct AtomTable(UnsafeCell<Inner>);

/// A string uniquing table - only one copy of a string is stored and all attempts
/// to add the same string again return the same atom.
#[derive(Default)]
struct Inner {
    /// Strings are added here and never removed or mutated.
    strings: Vec<String>,
    /// Maps from a reference inside [`Inner::strings`] to the index in [`Inner::strings`].
    /// Since strings are never removed or modified, the lifetime of the key
    /// is effectively static.
    map: HashMap<&'static str, NumIndex>,

    /// Strings are added here and never removed or mutated.
    strings_u16: Vec<Vec<u16>>,
    /// Maps from a reference inside [`Inner::strings_u16`] to the index in [`Inner::strings_u16`].
    /// Since strings are never removed or modified, the lifetime of the key
    /// is effectively static.
    map_u16: HashMap<&'static [u16], NumIndex>,

    /// Byte strings are added here and never removed or mutated.
    /// The bytes need not be valid UTF-8 (they may be WTF-8 or arbitrary byte
    /// sequences, e.g. JS string literals containing lone surrogates).
    strings_bytes: Vec<Vec<u8>>,
    /// Maps from a reference inside [`Inner::strings_bytes`] to the index in
    /// [`Inner::strings_bytes`]. Since strings are never removed or modified,
    /// the lifetime of the key is effectively static.
    map_bytes: HashMap<&'static [u8], NumIndex>,

    /// String renderings of byte atoms that are *not* valid UTF-8, built on
    /// demand. This is a lifetime anchor, not a cache: the conversion has to
    /// put its newly built string somewhere in order for
    /// [`AtomTable::bytes_str_lossy`] and [`AtomTable::try_bytes_str`] to hand
    /// out a `&str`. Both share one entry per atom, so a given atom is
    /// converted at most once. Atoms whose bytes are already valid UTF-8 never
    /// reach it — they are borrowed straight out of [`Inner::strings_bytes`] —
    /// so it stays empty unless a string literal (or a hand-built atom) holds
    /// surrogates; identifiers never can, because the lexer rejects unpaired
    /// surrogates there. Entries are never removed or mutated, so a returned
    /// `&str` stays valid (rehashing moves the `String` structs, never their
    /// heap buffers) — the same argument as [`Inner::strings_bytes`].
    converted_bytes: HashMap<AtomBytes, Converted>,
}

/// This represents a unique string index in the table.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Atom(NumIndex);

/// This represents a unique string index in the table.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct AtomU16(NumIndex);

/// This represents a unique byte-string index in the table.
/// The bytes need not be valid UTF-8.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct AtomBytes(NumIndex);

thread_local! {
    /// Stores the active table used for debug formatting.
    static DEBUG_TABLE: Cell<* const AtomTable> = Cell::new(null());
}

// An implementation of Debug which optionally obtains the Atom value from the
// active debug map.
impl std::fmt::Debug for Atom {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut t = f.debug_tuple("Atom");
        t.field(&self.0);

        // If the debug table is set and the atom is valid in it, add the value
        DEBUG_TABLE.with(|debug_table| {
            let p = debug_table.get();
            if let Some(r) = unsafe { p.as_ref() } {
                if let Some(value) = r.try_str(*self) {
                    t.field(&value);
                }
            }
        });
        t.finish()
    }
}

// An implementation of Debug which optionally obtains the Atom value from the
// active debug map.
impl std::fmt::Debug for AtomU16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut t = f.debug_tuple("Atom");
        t.field(&self.0);

        // If the debug table is set and the atom is valid in it, add the value
        DEBUG_TABLE.with(|debug_table| {
            let p = debug_table.get();
            if let Some(r) = unsafe { p.as_ref() } {
                if let Some(value) = r.try_str_u16(*self) {
                    t.field(&value);
                }
            }
        });
        t.finish()
    }
}

// An implementation of Debug which optionally obtains the AtomBytes value from
// the active debug map.
impl std::fmt::Debug for AtomBytes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut t = f.debug_tuple("AtomBytes");
        t.field(&self.0);

        // If the debug table is set and the atom is valid in it, add the value
        DEBUG_TABLE.with(|debug_table| {
            let p = debug_table.get();
            if let Some(r) = unsafe { p.as_ref() } {
                if let Some(value) = r.try_bytes(*self) {
                    t.field(&value);
                }
            }
        });
        t.finish()
    }
}

/// A special value reserved for the invalid atom.
pub const INVALID_ATOM: Atom = Atom(NumIndex::MAX);

/// A special value reserved for the invalid atom bytes.
pub const INVALID_ATOM_BYTES: AtomBytes = AtomBytes(NumIndex::MAX);

impl Inner {
    /// Add a string to the table and return its atom index. The same
    /// string always returns the same index.
    fn add_atom<V: Into<String> + AsRef<str>>(&mut self, value: V) -> Atom {
        if let Some(index) = self.map.get(value.as_ref()) {
            return Atom(*index);
        }
        self.add(value.into())
    }

    /// Perform the actual addition of the owned string.
    fn add(&mut self, owned: String) -> Atom {
        // Remember the index of the new element.
        let index = self.strings.len();
        assert!(index < INVALID_ATOM.0 as usize, "More than 4GB atoms?");

        // Obtain a reference to the existing string on the heap. That reference
        // is valid while `self` is valid.
        let key: *const str = owned.as_str();

        // Push the new string.
        self.strings.push(owned);

        self.map.insert(unsafe { &*key }, index as NumIndex);
        Atom(index as NumIndex)
    }

    /// Return the contents of the specified atom.
    #[inline]
    fn str(&self, ident: Atom) -> &str {
        self.strings[ident.0 as usize].as_str()
    }

    fn try_str(&self, ident: Atom) -> Option<&str> {
        if (ident.0 as usize) < self.strings.len() {
            Some(self.str(ident))
        } else {
            None
        }
    }

    /// Add a string to the table and return its atom index. The same
    /// string always returns the same index.
    fn add_atom_u16<V: Into<Vec<u16>> + AsRef<[u16]>>(&mut self, value: V) -> AtomU16 {
        if let Some(index) = self.map_u16.get(value.as_ref()) {
            return AtomU16(*index);
        }
        self.add_u16(value.into())
    }

    /// Perform the actual addition of the owned string.
    fn add_u16(&mut self, owned: Vec<u16>) -> AtomU16 {
        // Remember the index of the new element.
        let index = self.strings_u16.len();
        assert!(index < INVALID_ATOM.0 as usize, "More than 4GB atoms?");

        // Obtain a reference to the existing string on the heap. That reference
        // is valid while `self` is valid.
        let key: *const [u16] = owned.as_slice();

        // Push the new string.
        self.strings_u16.push(owned);

        self.map_u16.insert(unsafe { &*key }, index as NumIndex);
        AtomU16(index as NumIndex)
    }

    /// Return the contents of the specified atom.
    #[inline]
    fn str_u16(&self, ident: AtomU16) -> &[u16] {
        self.strings_u16[ident.0 as usize].as_slice()
    }

    fn try_str_u16(&self, ident: AtomU16) -> Option<&[u16]> {
        if (ident.0 as usize) < self.strings_u16.len() {
            Some(self.str_u16(ident))
        } else {
            None
        }
    }

    /// Add a byte string to the table and return its atom index. The same
    /// byte sequence always returns the same index. The bytes need not be
    /// valid UTF-8.
    fn add_atom_bytes<V: Into<Vec<u8>> + AsRef<[u8]>>(&mut self, value: V) -> AtomBytes {
        if let Some(index) = self.map_bytes.get(value.as_ref()) {
            return AtomBytes(*index);
        }
        self.add_bytes(value.into())
    }

    /// Perform the actual addition of the owned byte string.
    fn add_bytes(&mut self, owned: Vec<u8>) -> AtomBytes {
        // Remember the index of the new element.
        let index = self.strings_bytes.len();
        assert!(index < INVALID_ATOM_BYTES.0 as usize, "More than 4GB atoms?");

        // Obtain a reference to the existing bytes on the heap. That reference
        // is valid while `self` is valid. Pushing an owned Vec into the outer
        // Vec moves only the Vec struct, never its heap buffer — so a
        // *const [u8] captured from owned.as_slice() before the push stays
        // valid.
        let key: *const [u8] = owned.as_slice();

        // Push the new byte string.
        self.strings_bytes.push(owned);

        self.map_bytes.insert(unsafe { &*key }, index as NumIndex);
        AtomBytes(index as NumIndex)
    }

    /// Return the contents of the specified atom bytes.
    #[inline]
    fn bytes(&self, ident: AtomBytes) -> &[u8] {
        self.strings_bytes[ident.0 as usize].as_slice()
    }

    fn try_bytes(&self, ident: AtomBytes) -> Option<&[u8]> {
        if (ident.0 as usize) < self.strings_bytes.len() {
            Some(self.bytes(ident))
        } else {
            None
        }
    }

    /// Convert `ident` and anchor the result unless that has already been
    /// done. Only ever called for atoms whose bytes failed UTF-8 validation.
    fn ensure_converted(&mut self, ident: AtomBytes) {
        if !self.converted_bytes.contains_key(&ident) {
            let converted = convert_wtf8(self.bytes(ident));
            self.converted_bytes.insert(ident, converted);
        }
    }

    /// Return the conversion of `ident`, which must already have been built by
    /// [`Inner::ensure_converted`].
    #[inline]
    fn converted(&self, ident: AtomBytes) -> &Converted {
        &self.converted_bytes[&ident]
    }
}

/// A byte atom rendered as a Rust string, owned by the table.
struct Converted {
    /// The rendering: surrogate pairs folded into the character they encode,
    /// each unpaired surrogate and each other ill-formed subsequence replaced
    /// by one U+FFFD.
    text: String,
    /// Whether anything was *replaced* — as opposed to merely folded. When
    /// this is false, `text` is a faithful, lossless rendering of the atom's
    /// bytes and [`AtomTable::try_bytes_str`] may hand it out; when it is
    /// true, the atom holds something no `&str` can represent.
    replaced: bool,
}

/// If `bytes` starts with the WTF-8 encoding of a surrogate, return that
/// surrogate's code point. Such an encoding is always three bytes: `ED`, then
/// a continuation byte in `A0..=BF` (`A0..=AF` for the high surrogates
/// `U+D800..=U+DBFF`, `B0..=BF` for the low surrogates `U+DC00..=U+DFFF`),
/// then any continuation byte. Well-formed UTF-8 never produces this shape, so
/// the check is only ever reached on bytes `str::from_utf8` already rejected.
#[inline]
fn surrogate_at(bytes: &[u8]) -> Option<u32> {
    match bytes {
        [0xED, b1 @ 0xA0..=0xBF, b2 @ 0x80..=0xBF, ..] => {
            Some(0xD000 | ((*b1 as u32 & 0x3F) << 6) | (*b2 as u32 & 0x3F))
        }
        _ => None,
    }
}

/// Render `bytes` — WTF-8, or arbitrary bytes — as a valid Rust `String`,
/// reporting whether anything had to be replaced.
///
/// A WTF-8 surrogate *pair* is **folded** back into the supplementary-plane
/// character it encodes, which loses nothing: the pair and the character are
/// two encodings of the same string, so the result is exact and
/// [`Converted::replaced`] stays false. An **unpaired** surrogate has no UTF-8
/// form at all and becomes exactly one U+FFFD, as does every other maximal
/// ill-formed subsequence; either sets [`Converted::replaced`].
///
/// This mirrors the C++ `convertSurrogatesInString` pipeline
/// (`JSLexer.cpp:2486-2495`), which the lexer applies when its
/// `convert_surrogates` option is on — and which it does *not* apply by
/// default, so an astral character in a string literal is stored here in its
/// surrogate-pair form and must be folded back rather than replaced.
/// `convertToCodePointAt` (UTF8.cpp:77-96) pairs a high surrogate only with an
/// immediately following low one, so a high surrogate at end of input, a high
/// followed by another high, and a low followed by a high are each unpaired;
/// the arms below reproduce that, one U+FFFD per surrogate.
///
/// Deliberately not `String::from_utf8_lossy`: std has no notion of WTF-8, so
/// it renders a lone surrogate's three bytes as three separate U+FFFD and an
/// encoded astral character as six.
fn convert_wtf8(bytes: &[u8]) -> Converted {
    let mut out = String::with_capacity(bytes.len());
    let mut replaced = false;
    let mut rest = bytes;
    loop {
        // std validates the bulk of the input; only what it rejects is
        // examined by hand below.
        let err = match std::str::from_utf8(rest) {
            Ok(valid) => {
                out.push_str(valid);
                return Converted {
                    text: out,
                    replaced,
                };
            }
            Err(err) => err,
        };
        let (valid, invalid) = rest.split_at(err.valid_up_to());
        // `valid` is by definition the prefix `from_utf8` accepted.
        out.push_str(std::str::from_utf8(valid).unwrap());

        rest = match surrogate_at(invalid) {
            // A high surrogate directly followed by a low one is the WTF-8
            // encoding of a single supplementary-plane character. Folding it
            // back is exact, so `replaced` is left alone.
            Some(high) if high < 0xDC00 => match surrogate_at(&invalid[3..]) {
                Some(low) if low >= 0xDC00 => {
                    let cp = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                    // `cp` is in 0x10000..=0x10FFFF by construction, so the
                    // fallback is unreachable.
                    out.push(char::from_u32(cp).unwrap_or(char::REPLACEMENT_CHARACTER));
                    &invalid[6..]
                }
                // A high surrogate with no low one after it: unpaired.
                _ => {
                    out.push(char::REPLACEMENT_CHARACTER);
                    replaced = true;
                    &invalid[3..]
                }
            },
            // An unpaired low surrogate. Note this arm also takes a low
            // surrogate that happens to precede a high one, which C++ likewise
            // treats as two unpaired surrogates rather than a reversed pair.
            Some(_) => {
                out.push(char::REPLACEMENT_CHARACTER);
                replaced = true;
                &invalid[3..]
            }
            // Not a surrogate: one U+FFFD for the maximal ill-formed
            // subsequence std identified. `error_len() == None` means the
            // input simply ends mid-sequence, so everything left is consumed.
            None => {
                out.push(char::REPLACEMENT_CHARACTER);
                replaced = true;
                &invalid[err.error_len().unwrap_or(invalid.len())..]
            }
        };
    }
}

impl AtomTable {
    /// Create a new empty atom table.
    pub fn new() -> AtomTable {
        Default::default()
    }

    /// Add a string to the table and return its atom index. The same
    /// string always returns the same index.
    pub fn atom<V: Into<String> + AsRef<str>>(&self, value: V) -> Atom {
        unsafe { &mut *self.0.get() }.add_atom(value)
    }

    /// Return the contents of the specified atom.
    #[inline]
    pub fn str(&self, ident: Atom) -> &str {
        unsafe { &*self.0.get() }.str(ident)
    }

    #[inline]
    pub fn try_str(&self, ident: Atom) -> Option<&str> {
        unsafe { &*self.0.get() }.try_str(ident)
    }

    /// Add a string to the table and return its atom index. The same
    /// string always returns the same index.
    pub fn atom_u16<V: Into<Vec<u16>> + AsRef<[u16]>>(&self, value: V) -> AtomU16 {
        unsafe { &mut *self.0.get() }.add_atom_u16(value)
    }

    /// Return the contents of the specified atom.
    #[inline]
    pub fn str_u16(&self, ident: AtomU16) -> &[u16] {
        unsafe { &*self.0.get() }.str_u16(ident)
    }

    #[inline]
    pub fn try_str_u16(&self, ident: AtomU16) -> Option<&[u16]> {
        unsafe { &*self.0.get() }.try_str_u16(ident)
    }

    /// Add a byte string to the table and return its atom index. The same
    /// byte sequence always returns the same index. The bytes need not be
    /// valid UTF-8 (e.g., WTF-8 sequences encoding lone surrogates are
    /// accepted).
    pub fn atom_bytes<V: Into<Vec<u8>> + AsRef<[u8]>>(&self, value: V) -> AtomBytes {
        unsafe { &mut *self.0.get() }.add_atom_bytes(value)
    }

    /// Return the contents of the specified atom bytes.
    #[inline]
    pub fn bytes(&self, ident: AtomBytes) -> &[u8] {
        unsafe { &*self.0.get() }.bytes(ident)
    }

    #[inline]
    pub fn try_bytes(&self, ident: AtomBytes) -> Option<&[u8]> {
        unsafe { &*self.0.get() }.try_bytes(ident)
    }

    /// Return the contents of the specified atom bytes as a string,
    /// substituting U+FFFD for anything that cannot be represented.
    ///
    /// The atom's bytes are WTF-8, so a surrogate *pair* is folded back into
    /// the supplementary-plane character it encodes — that is exact, and it is
    /// the common case, because with the lexer's default settings an astral
    /// character in a string literal is stored in surrogate-pair form. An
    /// *unpaired* surrogate has no UTF-8 form and becomes exactly one U+FFFD,
    /// as does any other ill-formed sequence.
    ///
    /// When the bytes are already valid UTF-8, which is always the case for
    /// identifiers, the result borrows them directly with no allocation and no
    /// lookup. Otherwise the converted string is built once and owned by the
    /// table, so the returned reference stays valid for as long as the table
    /// does, whatever is interned afterwards.
    ///
    /// Use [`AtomTable::bytes`] when the exact bytes matter, and
    /// [`AtomTable::try_bytes_str`] when substitution would be data loss —
    /// notably for JS string-literal values, where an unpaired surrogate is a
    /// legal value rather than malformed data.
    ///
    /// # Panics
    ///
    /// Panics if `ident` is not a valid atom of this table, like
    /// [`AtomTable::bytes`].
    #[inline]
    pub fn bytes_str_lossy(&self, ident: AtomBytes) -> &str {
        // Validate first: on the overwhelmingly common valid path this borrows
        // the atom's own bytes and never touches `converted_bytes`.
        if let Ok(s) = std::str::from_utf8(unsafe { &*self.0.get() }.bytes(ident)) {
            return s;
        }
        unsafe { &mut *self.0.get() }.ensure_converted(ident);
        unsafe { &*self.0.get() }.converted(ident).text.as_str()
    }

    /// Return the contents of the specified atom bytes as a string whenever
    /// they can be represented as one exactly, and `None` when they cannot.
    ///
    /// `None` means the atom holds an **unpaired surrogate** — a legal JS
    /// string value with no UTF-8 form — or bytes that are not WTF-8 at all.
    /// It does *not* mean merely "the bytes are not literally valid UTF-8":
    /// a surrogate *pair* is folded back into the character it encodes and
    /// returned as `Some`, since the pair and that character are two encodings
    /// of the same string. An emoji in a string literal is stored in
    /// surrogate-pair form and therefore comes back as `Some`, not `None`.
    ///
    /// Valid UTF-8 borrows the atom's bytes with no allocation; the folding
    /// path builds the string once and anchors it in the table, sharing the
    /// one entry with [`AtomTable::bytes_str_lossy`].
    ///
    /// Unlike [`AtomTable::bytes_str_lossy`] this never substitutes, so it is
    /// the right accessor for JS string-literal values, where replacing an
    /// unpaired surrogate with U+FFFD would silently corrupt the program's
    /// data.
    ///
    /// Also returns `None` if `ident` is not a valid atom of this table, so
    /// that — like [`AtomTable::try_bytes`] — it never panics.
    #[inline]
    pub fn try_bytes_str(&self, ident: AtomBytes) -> Option<&str> {
        // Validate first: valid UTF-8 borrows the atom's own bytes and never
        // touches `converted_bytes`.
        let bytes = unsafe { &*self.0.get() }.try_bytes(ident)?;
        if let Ok(s) = std::str::from_utf8(bytes) {
            return Some(s);
        }
        unsafe { &mut *self.0.get() }.ensure_converted(ident);
        let converted = unsafe { &*self.0.get() }.converted(ident);
        if converted.replaced {
            None
        } else {
            Some(converted.text.as_str())
        }
    }

    /// Execute the callback in a context where this table is used for debug
    /// printing of atoms.
    pub fn in_debug_context<R, F: FnOnce() -> R>(&self, f: F) -> R {
        DEBUG_TABLE.with(|debug_table| {
            let prev_table = debug_table.replace(self);
            let res = f();
            debug_assert!(
                debug_table.get() == self,
                "debug context unexpectedly changed"
            );
            debug_table.set(prev_table);
            res
        })
    }

    /// Set a table or nullptr as the Atom debug context. If non-null, debug
    /// printing of atoms will use it. Return the previous debug context.
    ///
    /// # Safety
    /// The table must not be destroyed or moved while it is set.
    pub unsafe fn unsafe_set_debug_context(ptr: *const Self) -> *const Self {
        DEBUG_TABLE.with(|debug_table| debug_table.replace(ptr))
    }
}

impl std::ops::Index<Atom> for AtomTable {
    type Output = str;

    fn index(&self, index: Atom) -> &Self::Output {
        self.str(index)
    }
}

impl std::ops::Index<AtomU16> for AtomTable {
    type Output = [u16];

    fn index(&self, index: AtomU16) -> &Self::Output {
        self.str_u16(index)
    }
}

impl std::ops::Index<AtomBytes> for AtomTable {
    type Output = [u8];

    fn index(&self, index: AtomBytes) -> &Self::Output {
        self.bytes(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab() {
        let idtab = AtomTable::new();

        let id_foo = idtab.atom("foo");
        let p_foo: *const str = idtab.str(id_foo);
        let id_bar = idtab.atom("bar");
        assert_ne!(id_foo, id_bar);

        assert_eq!(idtab.atom("foo"), id_foo);
        assert_eq!(idtab.atom("bar"), id_bar);

        assert_eq!(idtab.atom(String::from("foo")), id_foo);
        assert_eq!(idtab.atom(String::from("bar")), id_bar);

        assert_eq!(idtab.str(id_foo), "foo");
        assert_eq!(idtab.str(id_bar), "bar");

        assert_eq!(idtab.str(id_foo) as *const str, p_foo);
    }

    #[test]
    fn test_bytes() {
        let tab = AtomTable::new();
        let foo = tab.atom_bytes(b"foo".as_slice());
        let bar = tab.atom_bytes(b"bar".as_slice());
        assert_ne!(foo, bar);
        assert_eq!(tab.atom_bytes(b"foo".as_slice()), foo);
        assert_eq!(tab.atom_bytes(Vec::from(*b"bar")), bar);
        assert_eq!(tab.bytes(foo), b"foo");
        assert_eq!(&tab[bar], b"bar");
        let p_foo: *const [u8] = tab.bytes(foo);
        let _ = tab.atom_bytes(b"baz".as_slice());
        assert_eq!(tab.bytes(foo) as *const [u8], p_foo);
    }

    #[test]
    fn test_bytes_ill_formed_utf8() {
        let tab = AtomTable::new();
        let lone_surrogate: &[u8] = &[0xed, 0xa0, 0x80];
        let a = tab.atom_bytes(lone_surrogate);
        assert_eq!(tab.bytes(a), lone_surrogate);
        assert_eq!(tab.atom_bytes(lone_surrogate), a);
        let s = tab.atom("foo");
        let b = tab.atom_bytes(b"foo".as_slice());
        assert_eq!(tab.str(s), "foo");
        assert_eq!(tab.bytes(b), b"foo");
    }

    #[test]
    fn test_bytes_try_and_invalid() {
        let tab = AtomTable::new();
        let a = tab.atom_bytes(b"x".as_slice());
        assert_eq!(tab.try_bytes(a), Some(b"x".as_slice()));
        assert_eq!(tab.try_bytes(INVALID_ATOM_BYTES), None);
    }

    #[test]
    fn lone_surrogate_becomes_exactly_one_replacement_char() {
        let t = AtomTable::new();
        // WTF-8 for U+D800, i.e. `"\uD800"` as Hermes stores it.
        let a = t.atom_bytes(vec![0xED, 0xA0, 0x80]);
        assert_eq!(t.try_bytes_str(a), None);
        let s = t.bytes_str_lossy(a);
        assert_eq!(
            s.chars().filter(|c| *c == '\u{FFFD}').count(),
            1,
            "std::from_utf8_lossy would give 3 here; we must be WTF-8 aware"
        );
        assert_eq!(s, "\u{FFFD}");
    }

    #[test]
    fn valid_utf8_is_borrowed_unchanged() {
        let t = AtomTable::new();
        let a = t.atom_bytes("greet".as_bytes().to_vec());
        assert_eq!(t.try_bytes_str(a), Some("greet"));
        assert_eq!(t.bytes_str_lossy(a), "greet");
        // Zero-copy: the returned str points into the table's own bytes.
        assert_eq!(t.bytes_str_lossy(a).as_ptr(), t.bytes(a).as_ptr());
    }

    #[test]
    fn surrogates_mixed_with_text_replace_only_the_surrogate() {
        let t = AtomTable::new();
        let mut v = b"a".to_vec();
        v.extend_from_slice(&[0xED, 0xA0, 0x80]);
        v.extend_from_slice("b".as_bytes());
        let a = t.atom_bytes(v);
        assert_eq!(t.bytes_str_lossy(a), "a\u{FFFD}b");
    }

    /// How many atoms are anchored in the conversion map. Tests use it to pin
    /// that the valid-UTF-8 path never touches it.
    fn anchored(t: &AtomTable) -> usize {
        unsafe { &*t.0.get() }.converted_bytes.len()
    }

    #[test]
    fn surrogate_pair_folds_into_the_astral_char() {
        let t = AtomTable::new();
        // How the lexer stores `"\u{1F600}"` by default: a WTF-8 surrogate
        // pair, which is *not* valid UTF-8 (see the `convert_surrogates` test
        // in the parser's lexer).
        let a = t.atom_bytes(vec![0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        // A pair IS representable: folding it back loses nothing, so this is
        // `Some`, not `None`. Returning `None` here would tell a
        // correctness-sensitive caller that a perfectly ordinary emoji is
        // corrupt.
        assert_eq!(t.try_bytes_str(a), Some("\u{1F600}"));
        assert_eq!(t.bytes_str_lossy(a), "\u{1F600}");
        // Both accessors share the one anchored string: converted once.
        assert_eq!(
            t.try_bytes_str(a).unwrap().as_ptr(),
            t.bytes_str_lossy(a).as_ptr()
        );
        assert_eq!(anchored(&t), 1);
    }

    #[test]
    fn a_pair_beside_an_unpaired_surrogate_is_not_representable() {
        let t = AtomTable::new();
        let mut v = vec![0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]; // U+1F600
        v.extend_from_slice(&[0xED, 0xA0, 0x80]); // unpaired U+D800
        v.extend_from_slice(b"!");
        let a = t.atom_bytes(v);
        // The unpaired surrogate has no UTF-8 form, so nothing exact exists.
        assert_eq!(t.try_bytes_str(a), None);
        // But the emoji still survives the lossy rendering intact, and only
        // the unpaired surrogate is replaced.
        let s = t.bytes_str_lossy(a);
        assert_eq!(s, "\u{1F600}\u{FFFD}!");
        assert_eq!(s.chars().filter(|c| *c == '\u{FFFD}').count(), 1);
    }

    #[test]
    fn the_valid_path_never_anchors() {
        let t = AtomTable::new();
        let a = t.atom_bytes(b"plain".as_slice());
        // Zero-copy from both accessors, and the map is never touched.
        assert_eq!(t.try_bytes_str(a), Some("plain"));
        assert_eq!(t.try_bytes_str(a).unwrap().as_ptr(), t.bytes(a).as_ptr());
        assert_eq!(t.bytes_str_lossy(a).as_ptr(), t.bytes(a).as_ptr());
        assert_eq!(anchored(&t), 0);
    }

    #[test]
    fn the_folded_result_is_stable_across_calls() {
        let t = AtomTable::new();
        let a = t.atom_bytes(vec![0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        // Held live across everything below.
        let first: &str = t.try_bytes_str(a).unwrap();
        for i in 0..100u8 {
            let b = t.atom_bytes(vec![0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80, i]);
            assert!(t.try_bytes_str(b).unwrap().starts_with('\u{1F600}'));
        }
        assert_eq!(first, "\u{1F600}");
        assert_eq!(t.try_bytes_str(a).unwrap().as_ptr(), first.as_ptr());
    }

    /// The pairing edge cases, matching `convertToCodePointAt`
    /// (UTF8.cpp:77-96): a high surrogate pairs *only* with an immediately
    /// following low one. Everything else is unpaired — one U+FFFD each, and
    /// never representable.
    #[test]
    fn unpaired_surrogates_are_one_replacement_each() {
        let t = AtomTable::new();
        let cases: &[(&[u8], &str)] = &[
            // A high surrogate at end of input (C++ `cur + 1 == end`).
            (&[0xED, 0xA0, 0x80], "\u{FFFD}"),
            // Two high surrogates in a row: neither pairs.
            (&[0xED, 0xA0, 0x80, 0xED, 0xA0, 0x80], "\u{FFFD}\u{FFFD}"),
            // A low surrogate followed by a high one: NOT a reversed pair,
            // two unpaired surrogates.
            (&[0xED, 0xB0, 0x80, 0xED, 0xA0, 0x80], "\u{FFFD}\u{FFFD}"),
            // Two low surrogates in a row.
            (&[0xED, 0xB0, 0x80, 0xED, 0xB0, 0x80], "\u{FFFD}\u{FFFD}"),
            // A lone low surrogate followed by text.
            (&[0xED, 0xB0, 0x80, b'z'], "\u{FFFD}z"),
            // A high surrogate whose successor is text, not a low surrogate.
            (&[0xED, 0xA0, 0x80, b'z'], "\u{FFFD}z"),
        ];
        for (bytes, expected) in cases {
            let a = t.atom_bytes(*bytes);
            assert_eq!(t.bytes_str_lossy(a), *expected, "bytes {bytes:02X?}");
            assert_eq!(t.try_bytes_str(a), None, "bytes {bytes:02X?}");
        }
    }

    #[test]
    fn non_surrogate_garbage_is_replaced_per_ill_formed_sequence() {
        let t = AtomTable::new();
        // Two invalid lead bytes.
        let a = t.atom_bytes(vec![0xFF, 0xFE]);
        assert_eq!(t.bytes_str_lossy(a), "\u{FFFD}\u{FFFD}");
        // A truncated 3-byte sequence at the very end (error_len() == None).
        let b = t.atom_bytes(vec![b'x', 0xE2, 0x82]);
        assert_eq!(t.bytes_str_lossy(b), "x\u{FFFD}");
        // Valid text on both sides of a bad byte.
        let c = t.atom_bytes(vec![b'o', 0x80, b'k']);
        assert_eq!(t.bytes_str_lossy(c), "o\u{FFFD}k");
    }

    #[test]
    fn an_earlier_lossy_str_survives_later_conversions() {
        let t = AtomTable::new();
        let a = t.atom_bytes(vec![0xED, 0xA0, 0x80]);
        // Held across everything below: this is the anchoring claim.
        let first: &str = t.bytes_str_lossy(a);
        // Grow and rehash the anchor map with 100 further lossy conversions.
        for i in 0..100u8 {
            let b = t.atom_bytes(vec![0xED, 0xA0, 0x80, b'a' + i % 26, i]);
            assert!(t.bytes_str_lossy(b).starts_with('\u{FFFD}'));
        }
        assert_eq!(first, "\u{FFFD}");
    }

    #[test]
    fn try_bytes_str_rejects_invalid_atoms() {
        let t = AtomTable::new();
        assert_eq!(t.try_bytes_str(INVALID_ATOM_BYTES), None);
    }

    #[test]
    fn the_lossy_result_is_stable_across_calls() {
        let t = AtomTable::new();
        let a = t.atom_bytes(vec![0xED, 0xA0, 0x80]);
        let p1 = t.bytes_str_lossy(a).as_ptr();
        // Interning more atoms must not invalidate an earlier result.
        for i in 0..1000 {
            t.atom_bytes(format!("filler{i}").into_bytes());
        }
        let p2 = t.bytes_str_lossy(a).as_ptr();
        assert_eq!(p1, p2, "the anchored String must not be rebuilt or moved");
    }
}
