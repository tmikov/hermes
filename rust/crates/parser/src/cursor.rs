//! The lexer's scan cursor. This is the one place the port uses `unsafe`
//! (decision "B"): a raw `*const u8` cursor over the source buffer for parity
//! with the C++ lexer's pointer arithmetic. The buffer is held as an
//! `Rc<SourceBuffer>` (stable heap address; kept alive for the cursor's life),
//! and every public method converts to/from a byte offset, so nothing `unsafe`
//! escapes this module. The buffer is NUL-terminated, so `peek_at` one past the
//! last real byte reads the terminating 0 (in-bounds).
//!
//! # Safety invariants
//! - `start`, `cur`, and `end` all point into the single contiguous allocation
//!   owned by `buffer` (the `Rc<SourceBuffer>` keeps it alive for `self`'s life,
//!   and `SourceBuffer`'s storage is a `Vec<u8>` whose data pointer is stable
//!   while the buffer is alive — it is never mutated).
//! - `start` is the first byte, `end` points at the trailing NUL (index
//!   `bytes().len()`), so `[start, end]` is in-bounds and `end` itself is a
//!   valid, readable byte (the NUL).
//! - Byte offsets are `u32` (matching the front end's `SMLoc`), so source
//!   buffers are assumed to be smaller than 4 GiB.
//! - `cur` is always kept within `[start, end]` by the public methods. The C++
//!   lexer dereferences `*curCharPtr_` at `end` (reading the NUL) and uses
//!   bounded lookahead (`curCharPtr_[1]`, ...) only after seeing a non-NUL byte,
//!   which is also in-bounds because of the terminator. We preserve that
//!   contract: `peek_at(n)` is only used by callers respecting the same
//!   lookahead invariant.
#![allow(unsafe_code)]

use hermes_support::buffer::SourceBuffer;
use std::rc::Rc;

/// A raw-pointer scan cursor over a NUL-terminated source buffer.
pub struct Cursor {
    buffer: Rc<SourceBuffer>,
    start: *const u8,
    cur: *const u8,
    /// Points at the terminating NUL (index `bytes().len()`).
    end: *const u8,
}

impl Cursor {
    /// Create a cursor positioned at the start of `buffer`.
    pub fn new(buffer: Rc<SourceBuffer>) -> Cursor {
        // `bytes()` excludes the NUL; `raw()` includes it. We need the NUL to be
        // addressable, so base pointers on the NUL-terminated storage.
        let raw = buffer.raw(); // includes trailing NUL
        let n = buffer.bytes().len(); // logical length (without NUL)
        let start = raw.as_ptr();
        // SAFETY: `raw` is a contiguous slice of length `n + 1` with a trailing
        // NUL at index `n`, so `start.add(n)` is in-bounds (one-past-the-last
        // real byte = the NUL byte, which is itself readable).
        let end = unsafe { start.add(n) };
        Cursor {
            buffer,
            start,
            cur: start,
            end,
        }
    }

    /// The current byte offset from the start of the buffer.
    #[inline]
    pub fn offset(&self) -> u32 {
        // SAFETY: `cur` is within `[start, end]` (same allocation), so the
        // distance is a valid, non-negative `isize` that fits in `u32`.
        (unsafe { self.cur.offset_from(self.start) }) as u32
    }

    /// True once the cursor has reached the terminating NUL.
    #[inline]
    pub fn at_end(&self) -> bool {
        self.cur >= self.end
    }

    /// Byte at the cursor (or the NUL terminator at end).
    #[inline]
    pub fn peek(&self) -> u8 {
        // SAFETY: `cur` is within `[start, end]`, and `end` (the NUL) is
        // readable, so the dereference is in-bounds.
        unsafe { *self.cur }
    }

    /// Byte `n` ahead of the cursor. Only valid while the bytes in between were
    /// non-NUL (the C++ lookahead invariant); reading the terminating NUL is
    /// always in-bounds.
    #[inline]
    pub fn peek_at(&self, n: usize) -> u8 {
        // SAFETY: callers respect the lexer lookahead invariant: `peek_at(n)` is
        // only used after the preceding bytes were observed non-NUL, so
        // `cur.add(n)` stays within `[start, end]` and is readable.
        unsafe { *self.cur.add(n) }
    }

    /// Advance the cursor by `n` bytes.
    #[inline]
    pub fn advance(&mut self, n: usize) {
        // SAFETY: callers only advance within the buffer (bounded by the NUL
        // terminator the lexer stops at), keeping `cur` within `[start, end]`.
        unsafe {
            self.cur = self.cur.add(n);
        }
    }

    /// Seek to an absolute byte offset.
    #[inline]
    pub fn seek(&mut self, offset: u32) {
        // SAFETY: `offset` is a valid byte offset into the buffer (<= the NUL's
        // index), so `start.add(offset)` is within `[start, end]`.
        unsafe {
            self.cur = self.start.add(offset as usize);
        }
    }

    /// Move the cursor to EOF (the terminating NUL). Port of `forceEOF`.
    #[inline]
    pub fn seek_end(&mut self) {
        self.cur = self.end;
    }

    /// Bytes in `[from_offset, current offset)`.
    #[inline]
    pub fn slice_from(&self, from_offset: u32) -> &[u8] {
        &self.buffer.raw()[from_offset as usize..self.offset() as usize]
    }

    /// Bytes in `[from_offset, to_offset)`.
    #[inline]
    pub fn slice(&self, from_offset: u32, to_offset: u32) -> &[u8] {
        &self.buffer.raw()[from_offset as usize..to_offset as usize]
    }

    /// The NUL-terminated raw bytes of the underlying buffer.
    #[inline]
    pub fn raw(&self) -> &[u8] {
        self.buffer.raw()
    }

    /// Decode the (non-ASCII) UTF-8 char at the cursor WITHOUT advancing.
    /// Port of `JSLexer::_peekUTF8` (JSLexer.h:1159-1167): it decodes with
    /// surrogates disallowed and swallows any errors. Returns
    /// `(code_point, offset_after)`, where `offset_after` is the byte offset of
    /// the next character.
    ///
    /// This stays in `cursor.rs` to keep the raw-pointer parity confined here,
    /// but it actually drives the safe `utf8::decode_utf8` over `raw()` at a
    /// copied byte offset (no new `unsafe`).
    pub fn peek_utf8(&self) -> (u32, u32) {
        let bytes = self.raw();
        let mut i = self.offset() as usize;
        let cp = crate::utf8::decode_utf8::<false>(bytes, &mut i, |_| {});
        (cp, i as u32)
    }

    /// The underlying buffer (cloning the `Rc` is cheap).
    pub fn buffer(&self) -> &Rc<SourceBuffer> {
        &self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_support::buffer::SourceBuffer;
    use std::rc::Rc;

    fn cur(s: &str) -> Cursor {
        Cursor::new(Rc::new(SourceBuffer::from_str("t", s)))
    }

    #[test]
    fn basic() {
        let mut c = cur("ab");
        assert_eq!(c.offset(), 0);
        assert_eq!(c.peek(), b'a');
        assert_eq!(c.peek_at(1), b'b');
        assert_eq!(c.peek_at(2), 0); // NUL terminator (in-bounds, always present)
        assert!(!c.at_end());
        c.advance(1);
        assert_eq!(c.offset(), 1);
        assert_eq!(c.peek(), b'b');
        c.advance(1);
        assert_eq!(c.peek(), 0);
        assert!(c.at_end());
    }

    #[test]
    fn peek_utf8_no_advance() {
        let c = cur("\u{4e2d}x"); // 中 = e4 b8 ad
        let (cp, next) = c.peek_utf8();
        assert_eq!(cp, 0x4E2D);
        assert_eq!(next, 3); // offset of 'x'
        assert_eq!(c.offset(), 0); // cursor did not move
    }

    #[test]
    fn slicing_and_seek() {
        let mut c = cur("hello");
        c.advance(2);
        assert_eq!(c.slice_from(0), b"he"); // bytes [0, offset)
        c.seek(4);
        assert_eq!(c.offset(), 4);
        assert_eq!(c.peek(), b'o');
    }
}
