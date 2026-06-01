/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! `SourceErrorManager`: owns source buffers, resolves locations, and
//! dispatches diagnostics. This module covers buffer registration, names,
//! virtual buffers, and source-URL storage. Diagnostic dispatch is added in
//! later tasks.

use std::collections::HashMap;
use std::rc::Rc;

use crate::buffer::SourceBuffer;
use crate::diag::CoordTranslator;
use crate::location::{SMLoc, SourceCoords, SourceId};

struct Entry {
    buffer: Rc<SourceBuffer>,
    is_virtual: bool,
    source_url: Option<String>,
    source_mapping_url: Option<String>,
}

/// A facade that owns source buffers and reports diagnostics against them.
/// Rust port of `hermes::SourceErrorManager`.
pub struct SourceErrorManager {
    entries: Vec<Entry>,
    by_name: HashMap<String, SourceId>,
    /// Optional hook to translate (e.g. source-map) coordinates before display.
    /// Port of `ICoordTranslator *translator_` in the C++ class.
    translator: Option<Rc<dyn CoordTranslator>>,
    // Remaining diagnostic state (counts, error limit, warnings, handler) is
    // added in later tasks.
}

impl SourceErrorManager {
    pub fn new() -> SourceErrorManager {
        SourceErrorManager {
            entries: Vec::new(),
            by_name: HashMap::new(),
            translator: None,
        }
    }

    /// Register a real source buffer and return its id.
    pub fn add_buffer(&mut self, name: &str, contents: &str) -> SourceId {
        self.push(SourceBuffer::from_str(name, contents), false)
    }

    /// Register a real source buffer from raw (possibly already NUL-terminated)
    /// bytes.
    #[allow(dead_code)]
    pub fn add_buffer_bytes(&mut self, name: &str, contents: &[u8]) -> SourceId {
        self.push(SourceBuffer::from_slice_check(name, contents), false)
    }

    /// Register a virtual buffer: a name with no contents, used for synthetic
    /// locations. Port of `addNewVirtualSourceBuffer`.
    pub fn add_virtual_buffer(&mut self, name: &str) -> SourceId {
        self.push(SourceBuffer::from_str(name, ""), true)
    }

    fn push(&mut self, buffer: SourceBuffer, is_virtual: bool) -> SourceId {
        let id = SourceId::from_index(self.entries.len() as u32);
        let name = buffer.name().to_string();
        self.entries.push(Entry {
            buffer: Rc::new(buffer),
            is_virtual,
            source_url: None,
            source_mapping_url: None,
        });
        self.by_name.entry(name).or_insert(id);
        id
    }

    pub fn is_virtual(&self, id: SourceId) -> bool {
        self.entries[id.index() as usize].is_virtual
    }

    pub fn buffer_file_name(&self, id: SourceId) -> &str {
        self.entries[id.index() as usize].buffer.name()
    }

    /// Obtain a buffer by id (cloning the `Rc`, e.g. to hand to a lexer).
    #[allow(dead_code)]
    pub fn source_buffer(&self, id: SourceId) -> Rc<SourceBuffer> {
        Rc::clone(&self.entries[id.index() as usize].buffer)
    }

    pub fn lookup_name(&self, name: &str) -> Option<SourceId> {
        self.by_name.get(name).copied()
    }

    pub fn set_source_url(&mut self, id: SourceId, url: &str) {
        self.entries[id.index() as usize].source_url = Some(url.to_string());
    }
    pub fn source_url(&self, id: SourceId) -> Option<&str> {
        self.entries[id.index() as usize].source_url.as_deref()
    }
    pub fn set_source_mapping_url(&mut self, id: SourceId, url: &str) {
        self.entries[id.index() as usize].source_mapping_url = Some(url.to_string());
    }
    pub fn source_mapping_url(&self, id: SourceId) -> Option<&str> {
        self.entries[id.index() as usize]
            .source_mapping_url
            .as_deref()
    }
}

/// Port of `SourceErrorManager.cpp:255` `adjustSourceLocation`. If `offset`
/// points at a '\r' or a UTF-8 continuation byte, walk backward (not past
/// `line_start`) until a normal byte, so the column reflects the start of the
/// character. A no-op for normal (token-start) locations.
fn adjust_source_offset(bytes: &[u8], offset: u32, line_start: u32) -> u32 {
    let mut i = offset as usize;
    if i >= bytes.len() {
        return offset;
    }
    let is_adjust = |b: u8| b == b'\r' || (b & 0b1100_0000) == 0b1000_0000;
    if is_adjust(bytes[i]) {
        while i as u32 > line_start && is_adjust(bytes[i]) {
            i -= 1;
        }
    }
    i as u32
}

impl SourceErrorManager {
    /// Install (or clear) the coordinate translator applied during resolution.
    pub fn set_translator(&mut self, translator: Option<Rc<dyn CoordTranslator>>) {
        self.translator = translator;
    }

    /// The buffer containing `loc`. Trivial: the location carries its buffer.
    /// Port of `findBufferIdForLoc`.
    pub fn find_buffer_id(&self, loc: SMLoc) -> SourceId {
        loc.source
    }

    /// Decode `loc` to 1-based (buffer, line, col), applying the coordinate
    /// translator if one is installed. Port of `findBufferLineAndLoc`.
    pub fn find_coords(&self, loc: SMLoc) -> SourceCoords {
        let mut coords = self.find_untranslated_coords(loc);
        if let Some(t) = &self.translator {
            t.translate(&mut coords);
        }
        coords
    }

    /// Decode `loc` without applying the translator, including the
    /// `adjustSourceLocation` correction for '\r'/UTF-8 continuation bytes.
    /// Port of `findUntranslatedBufferLineAndLoc`.
    pub fn find_untranslated_coords(&self, loc: SMLoc) -> SourceCoords {
        let entry = &self.entries[loc.source.index() as usize];
        let (line, col) = entry.buffer.with_line_index(|idx, bytes| {
            // Find the line for the raw offset, then derive the line's start
            // offset (col is 1-based byte distance from it).
            let (line, raw_col) = idx.line_col(loc.offset);
            let line_start = loc.offset - (raw_col - 1);
            let adjusted = adjust_source_offset(bytes, loc.offset, line_start);
            (line, adjusted - line_start + 1)
        });
        SourceCoords {
            buf: loc.source,
            line,
            col,
        }
    }
}

impl Default for SourceErrorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_loc_to_coords() {
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "ab\ncde");
        let loc = SMLoc {
            source: id,
            offset: 4,
        }; // 'd' on line 2 col 2
        let coords = sm.find_coords(loc);
        assert_eq!((coords.buf, coords.line, coords.col), (id, 2, 2));
        assert_eq!(sm.find_buffer_id(loc), id);
    }

    #[test]
    fn translator_is_applied() {
        use crate::location::{SMLoc, SourceCoords};
        use std::rc::Rc;
        struct Shift;
        impl crate::diag::CoordTranslator for Shift {
            fn translate(&self, c: &mut SourceCoords) {
                c.line += 100;
            }
        }
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "ab\ncde");
        sm.set_translator(Some(Rc::new(Shift)));
        let coords = sm.find_coords(SMLoc {
            source: id,
            offset: 4,
        });
        assert_eq!(coords.line, 102);
    }

    #[test]
    fn cr_before_lf_adjusts_back() {
        use crate::location::SMLoc;
        // "ab\r\ncd": a0 b1 \r2 \n3 c4 d5. Offset 2 is the '\r'; it adjusts back to
        // 'b' (line 1, col 2).
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "ab\r\ncd");
        let coords = sm.find_coords(SMLoc {
            source: id,
            offset: 2,
        });
        assert_eq!((coords.line, coords.col), (1, 2));
    }

    #[test]
    fn mid_utf8_byte_adjusts_to_char_start() {
        use crate::location::SMLoc;
        // "aé": a=0x61 at 0, 'é'=0xC3 0xA9 at offsets 1,2. Offset 2 is the
        // continuation byte; it adjusts back to the lead byte (line 1, col 2).
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "aé");
        let coords = sm.find_coords(SMLoc {
            source: id,
            offset: 2,
        });
        assert_eq!((coords.line, coords.col), (1, 2));
    }

    #[test]
    fn register_real_and_lookup() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "let x = 1;");
        assert_eq!(sm.buffer_file_name(id), "a.js");
        assert_eq!(sm.lookup_name("a.js"), Some(id));
        assert!(!sm.is_virtual(id));
    }

    #[test]
    fn virtual_buffer_is_tagged() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_virtual_buffer("<native>");
        assert!(sm.is_virtual(id));
        assert_eq!(sm.buffer_file_name(id), "<native>");
    }

    #[test]
    fn source_urls_roundtrip() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "x");
        sm.set_source_url(id, "https://example/a.js");
        sm.set_source_mapping_url(id, "a.js.map");
        assert_eq!(sm.source_url(id), Some("https://example/a.js"));
        assert_eq!(sm.source_mapping_url(id), Some("a.js.map"));
    }
}
