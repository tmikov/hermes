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
use crate::location::SourceId;

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
    // Diagnostic state (counts, error limit, warnings, handler, translator) is
    // added in later tasks.
}

impl SourceErrorManager {
    pub fn new() -> SourceErrorManager {
        SourceErrorManager {
            entries: Vec::new(),
            by_name: HashMap::new(),
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

impl Default for SourceErrorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
