/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Source buffer primitives: a NUL-terminated byte buffer and a named wrapper.

use std::cell::RefCell;
use std::io::Read;

use crate::line_index::LineIndex;

/// A null terminated memory buffer.
#[derive(Debug)]
pub struct NullTerminatedBuf(Vec<u8>);

impl NullTerminatedBuf {
    /// Create from a reader and null terminate.
    pub fn from_reader(reader: &mut dyn Read) -> Result<NullTerminatedBuf, std::io::Error> {
        let mut v = Vec::<u8>::new();
        reader.read_to_end(&mut v)?;
        v.push(0);

        Ok(NullTerminatedBuf(v))
    }

    /// Create from a file and null terminate it.
    pub fn from_file(f: &'_ mut std::fs::File) -> Result<NullTerminatedBuf, std::io::Error> {
        // TODO: this is an extremely naive implementation, it can be optimized in multiple ways:
        //       - obtain the size of the file and perform a single allocation and few syscalls
        //       - memory map the file
        //       - just use LLVM's MemoryBuffer
        //       One problem is that there isn't an obvious way in Rust to check portably whether
        //       something has a fixed size and is memory mappable (i.e. is not a pipe).

        Self::from_reader(f)
    }

    /// Create by copying a slice and appending null-termination.
    pub fn from_slice_copy(s: &[u8]) -> NullTerminatedBuf {
        let mut v = Vec::with_capacity(s.len() + 1);
        v.extend_from_slice(s);
        v.push(0);
        NullTerminatedBuf(v)
    }

    /// Create from a slice that may already be null-terminated.
    pub fn from_slice_check(s: &[u8]) -> NullTerminatedBuf {
        Self::from_slice_copy(if let [head @ .., 0] = s { head } else { s })
    }

    /// Create by copying a string and appending null-termination.
    pub fn from_str_copy(s: &str) -> NullTerminatedBuf {
        Self::from_slice_copy(s.as_bytes())
    }

    /// Create from a string that may already be null-terminated.
    pub fn from_str_check(s: &str) -> NullTerminatedBuf {
        Self::from_slice_check(s.as_bytes())
    }

    /// Return the length of the data including the null terminator.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Just a placeholder always returning `true`, since the there is always
    /// at least a null terminator.
    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl AsRef<[u8]> for NullTerminatedBuf {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

/// A named source buffer: its file name, its NUL-terminated bytes, and a lazily
/// built line index. This is the Rust analog of an `llvh::MemoryBuffer`
/// registered in a `SourceMgr`, but it carries its own name.
pub struct SourceBuffer {
    name: String,
    buf: NullTerminatedBuf,
    /// Lazily built on first line/col resolution. Interior mutability so that
    /// resolution can happen through a shared `&SourceBuffer`.
    line_index: RefCell<Option<LineIndex>>,
}

impl SourceBuffer {
    pub fn from_str(name: impl Into<String>, contents: &str) -> SourceBuffer {
        SourceBuffer {
            name: name.into(),
            buf: NullTerminatedBuf::from_str_copy(contents),
            line_index: RefCell::new(None),
        }
    }

    pub fn from_slice_check(name: impl Into<String>, contents: &[u8]) -> SourceBuffer {
        SourceBuffer {
            name: name.into(),
            buf: NullTerminatedBuf::from_slice_check(contents),
            line_index: RefCell::new(None),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The source bytes, excluding the trailing NUL terminator.
    pub fn bytes(&self) -> &[u8] {
        let raw = self.buf.as_bytes();
        &raw[..raw.len() - 1]
    }

    /// The raw bytes, including the trailing NUL terminator.
    pub fn raw(&self) -> &[u8] {
        self.buf.as_bytes()
    }

    /// Run `f` with this buffer's line index, building and caching it on first use.
    pub fn with_line_index<R>(&self, f: impl FnOnce(&LineIndex, &[u8]) -> R) -> R {
        {
            let mut slot = self.line_index.borrow_mut();
            if slot.is_none() {
                *slot = Some(LineIndex::build(self.bytes()));
            }
        }
        let slot = self.line_index.borrow();
        f(slot.as_ref().unwrap(), self.bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_terminated_and_named() {
        let b = SourceBuffer::from_str("foo.js", "abc");
        assert_eq!(b.name(), "foo.js");
        // bytes() excludes the terminator; raw includes it.
        assert_eq!(b.bytes(), b"abc");
        assert_eq!(b.raw()[b.raw().len() - 1], 0u8);
    }

    #[test]
    fn already_terminated_input_not_doubled() {
        let b = SourceBuffer::from_slice_check("x", b"ab\0");
        assert_eq!(b.bytes(), b"ab");
    }

    #[test]
    fn empty_input_gives_empty_bytes() {
        // Edge case: the only construction where raw().len() - 1 == 0.
        let b = SourceBuffer::from_str("empty.js", "");
        assert_eq!(b.bytes(), b"");
        assert_eq!(b.raw(), b"\0");
        assert_eq!(b.raw().len(), b.bytes().len() + 1);
    }
}
