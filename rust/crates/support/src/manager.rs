/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! `SourceErrorManager`: owns source buffers, resolves locations, and
//! dispatches diagnostics. This module covers buffer registration, names,
//! virtual buffers, source-URL storage, and the central diagnostic emit
//! pipeline: handler storage, message counts, error limit, and warning
//! categories. Port of `hermes::SourceErrorManager`.

use std::collections::HashMap;
use std::rc::Rc;

use crate::buffer::SourceBuffer;
use crate::diag::{CoordTranslator, DiagHandler, DiagKind, ResolvedDiagnostic, Warning};
use crate::location::{SMLoc, SMRange, SourceCoords, SourceId};

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
    /// Installed diagnostic sink. None means diagnostics are silently dropped.
    handler: Option<Box<dyn DiagHandler>>,
    /// Per-kind message counters: indexed by `DiagKind as usize` (Error=0,
    /// Warning=1, Note=2). Port of `messageCount_[kMessageCount]`.
    message_count: [u32; 3],
    /// Maximum number of errors to emit before setting `error_limit_reached`.
    /// Port of `errorLimit_`. Defaults to `u32::MAX` (unlimited).
    error_limit: u32,
    /// Set to `true` once the error count reaches `error_limit`.
    /// Port of `errorLimitReached_`.
    error_limit_reached: bool,
    /// Tracks whether the immediately preceding message was suppressed, so that
    /// follow-on Notes can be suppressed as well. Port of `lastMessageSuppressed_`.
    last_message_suppressed: bool,
    /// Per-category enabled flag. Indexed by `Warning::index()`.
    /// Port of the `warningStatuses_` bitset (enabled side).
    warning_enabled: Vec<bool>,
    /// Per-category error-promotion flag. Indexed by `Warning::index()`.
    /// Port of the `warningStatuses_` bitset (is-error side).
    warning_as_error: Vec<bool>,
}

impl SourceErrorManager {
    pub fn new() -> SourceErrorManager {
        SourceErrorManager {
            entries: Vec::new(),
            by_name: HashMap::new(),
            translator: None,
            handler: None,
            message_count: [0; 3],
            error_limit: u32::MAX,
            error_limit_reached: false,
            last_message_suppressed: false,
            warning_enabled: vec![true; Warning::COUNT],
            warning_as_error: vec![false; Warning::COUNT],
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

// ---------------------------------------------------------------------------
// Diagnostic dispatch (Tasks 8 + 9).  Port of SourceErrorManager::message,
// countAndGenMessage, doGenMessage / doPrintMessage in SourceErrorManager.cpp.
// Buffering and subsystem-suppression are deferred (out of scope here).
// ---------------------------------------------------------------------------
impl SourceErrorManager {
    /// Install the diagnostic sink. Replaces any previously installed handler.
    pub fn set_handler(&mut self, h: Box<dyn DiagHandler>) {
        self.handler = Some(h);
    }

    /// Downcast the installed handler to a concrete type (for tests/inspection).
    pub fn handler_as<T: 'static>(&self) -> Option<&T> {
        self.handler.as_ref()?.as_any().downcast_ref::<T>()
    }

    /// Set the maximum number of errors before suppressing further messages.
    pub fn set_error_limit(&mut self, limit: u32) {
        self.error_limit = limit;
    }

    /// Return `true` if the error limit has been reached.
    pub fn is_error_limit_reached(&self) -> bool {
        self.error_limit_reached
    }

    /// Clear the error-limit-reached flag (e.g. to resume after recovery).
    pub fn clear_error_limit_reached(&mut self) {
        self.error_limit_reached = false;
    }

    /// Number of messages of kind `dk` emitted so far.
    pub fn message_count(&self, dk: DiagKind) -> u32 {
        self.message_count[dk as usize]
    }

    /// Convenience: number of errors emitted.
    pub fn error_count(&self) -> u32 {
        self.message_count[DiagKind::Error as usize]
    }

    /// Convenience: number of warnings emitted (before any is-error promotion).
    pub fn warning_count(&self) -> u32 {
        self.message_count[DiagKind::Warning as usize]
    }

    // ---- Warning categories (Task 9) ----------------------------------------

    /// Enable or disable a warning category.
    /// Port of `setWarningStatus` / `disableAllWarnings`.
    pub fn set_warning_status(&mut self, w: Warning, enabled: bool) {
        self.warning_enabled[w.index()] = enabled;
    }

    /// Promote (or demote) a warning category to errors.
    /// Port of `setWarningIsError`.
    pub fn set_warning_is_error(&mut self, w: Warning, v: bool) {
        self.warning_as_error[w.index()] = v;
    }

    /// Promote all warning categories to errors (equivalent to `-Werror`).
    pub fn set_warnings_are_errors(&mut self, v: bool) {
        for x in &mut self.warning_as_error {
            *x = v;
        }
    }

    /// Disable all warning categories.
    pub fn disable_all_warnings(&mut self) {
        for x in &mut self.warning_enabled {
            *x = false;
        }
    }

    /// Return `true` if the warning category is currently enabled.
    pub fn is_warning_enabled(&self, w: Warning) -> bool {
        self.warning_enabled[w.index()]
    }

    /// Return `true` if the warning category is promoted to error.
    pub fn is_warning_an_error(&self, w: Warning) -> bool {
        self.warning_as_error[w.index()]
    }

    // ---- Public reporting overloads -----------------------------------------

    /// Emit an error at `loc`.
    pub fn error(&mut self, loc: SMLoc, msg: impl Into<String>) {
        self.emit(
            DiagKind::Error,
            Warning::NoWarning,
            Some(loc),
            None,
            msg.into(),
        );
    }

    /// Emit an error at the start of `range`, underscoring the full range.
    pub fn error_range(&mut self, range: SMRange, msg: impl Into<String>) {
        self.emit(
            DiagKind::Error,
            Warning::NoWarning,
            Some(range.start),
            Some(range),
            msg.into(),
        );
    }

    /// Emit a note at `loc`.
    pub fn note(&mut self, loc: SMLoc, msg: impl Into<String>) {
        self.emit(
            DiagKind::Note,
            Warning::NoWarning,
            Some(loc),
            None,
            msg.into(),
        );
    }

    /// Emit a warning of the given category at `loc`.
    pub fn warning(&mut self, w: Warning, loc: SMLoc, msg: impl Into<String>) {
        self.emit(DiagKind::Warning, w, Some(loc), None, msg.into());
    }

    /// Emit a `Misc` warning at `loc` (the most common case).
    pub fn warning_misc(&mut self, loc: SMLoc, msg: impl Into<String>) {
        self.emit(
            DiagKind::Warning,
            Warning::Misc,
            Some(loc),
            None,
            msg.into(),
        );
    }

    // ---- Central dispatch ---------------------------------------------------

    /// Central dispatch. Port of `SourceErrorManager::message` +
    /// `countAndGenMessage` (buffering/subsystem deferred).
    fn emit(
        &mut self,
        mut dk: DiagKind,
        w: Warning,
        loc: Option<SMLoc>,
        range: Option<SMRange>,
        msg: String,
    ) {
        // Suppress all messages once the error limit has been reached.
        if self.error_limit_reached {
            return;
        }
        if dk == DiagKind::Warning && !self.is_warning_enabled(w) {
            self.last_message_suppressed = true;
            return;
        }
        // Automatically suppress notes if the last message was suppressed.
        if dk == DiagKind::Note && self.last_message_suppressed {
            return;
        }
        self.last_message_suppressed = false;
        // Optionally upgrade warnings into errors.
        if dk == DiagKind::Warning && self.is_warning_an_error(w) {
            dk = DiagKind::Error;
        }
        self.count_and_gen(dk, loc, range, msg);
    }

    /// Port of `countAndGenMessage`.
    fn count_and_gen(
        &mut self,
        dk: DiagKind,
        loc: Option<SMLoc>,
        range: Option<SMRange>,
        msg: String,
    ) {
        self.message_count[dk as usize] += 1;
        self.gen_message(dk, loc, range, msg);
        // Check after calling gen_message so the original message is emitted
        // first, then the "too many errors" sentinel.  Matches C++ behavior.
        if dk == DiagKind::Error && self.message_count[DiagKind::Error as usize] == self.error_limit
        {
            self.error_limit_reached = true;
            self.gen_message(
                DiagKind::Error,
                None,
                None,
                "too many errors emitted".to_string(),
            );
        }
    }

    /// Resolve the location and hand a `ResolvedDiagnostic` to the handler.
    /// Port of `doGenMessage` → `doPrintMessage`.
    fn gen_message(
        &mut self,
        dk: DiagKind,
        loc: Option<SMLoc>,
        range: Option<SMRange>,
        msg: String,
    ) {
        // Build the resolved struct first (immutable borrows of self.entries
        // and self.translator complete and produce owned data), then hand it
        // to self.handler (mutable borrow).  This ordering satisfies the
        // borrow checker without any unsafe.
        let resolved = match loc {
            Some(loc) => {
                let coords = self.find_coords(loc);
                let file_name = self.buffer_file_name(loc.source).to_string();
                // Clone the Rc so the immutable borrow on self.entries ends here.
                let buf = self.source_buffer(loc.source);
                // Pull the source line (without trailing EOL) for the caret.
                let source_line = buf.with_line_index(|idx, bytes| {
                    let raw = idx.line_ref(bytes, coords.line);
                    let trimmed = strip_eol(raw);
                    String::from_utf8_lossy(trimmed).into_owned()
                });
                // Compute range columns relative to the source line, if the
                // range is in the same buffer as the location.
                // Port of SourceErrorManager.cpp:157-170 (caret/tilde fill).
                let range_cols = range.filter(|r| r.start.source == loc.source).map(|r| {
                    // Byte offset of the first character of this line.
                    let line_start = loc.offset - (coords.col - 1);
                    // Byte length of the EOL-stripped source line.
                    let source_line_byte_len = source_line.len() as u32;
                    let line_end = line_start + source_line_byte_len;
                    // Clamp both endpoints to the line so multi-line ranges
                    // stop at the line end, matching the C++ behavior
                    // (`min(range.second, caretLine.size())`).
                    let start = r.start.offset.saturating_sub(line_start);
                    // saturating_sub guards against an inverted/foreign range
                    // whose end precedes the line start.
                    let end = r.end.offset.min(line_end).saturating_sub(line_start);
                    (start, end)
                });
                ResolvedDiagnostic {
                    kind: dk,
                    file_name,
                    line: coords.line,
                    col: coords.col,
                    message: msg,
                    source_line: Some(source_line),
                    range_cols,
                }
            }
            None => ResolvedDiagnostic {
                kind: dk,
                file_name: String::new(),
                line: 0,
                col: 0,
                message: msg,
                source_line: None,
                range_cols: None,
            },
        };
        if let Some(h) = self.handler.as_mut() {
            h.handle(&resolved);
        }
    }
}

/// Strip a single trailing `\n` or `\r\n`/`\r` from a line slice.
/// Used to drop the EOL before handing the source line to the renderer.
fn strip_eol(line: &[u8]) -> &[u8] {
    let mut end = line.len();
    if end > 0 && line[end - 1] == b'\n' {
        end -= 1;
    }
    if end > 0 && line[end - 1] == b'\r' {
        end -= 1;
    }
    &line[..end]
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

    #[test]
    fn error_count_and_limit() {
        use crate::diag::CollectingHandler;
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abc\ndef");
        sm.set_handler(Box::new(CollectingHandler::new()));
        sm.set_error_limit(1);
        sm.error(
            SMLoc {
                source: id,
                offset: 0,
            },
            "first",
        );
        assert!(sm.is_error_limit_reached());
        assert_eq!(sm.error_count(), 1);
        // After the limit, a "too many errors" message is emitted once and further
        // errors are suppressed (port of sTooManyErrors behavior).
        sm.error(
            SMLoc {
                source: id,
                offset: 1,
            },
            "second",
        );
        assert_eq!(sm.error_count(), 1);
    }

    #[test]
    fn collecting_handler_receives_resolved() {
        use crate::diag::{CollectingHandler, DiagKind};
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abc\ndef");
        sm.set_handler(Box::new(CollectingHandler::new()));
        sm.warning_misc(
            SMLoc {
                source: id,
                offset: 4,
            },
            "watch out",
        );
        let h = sm.handler_as::<CollectingHandler>().unwrap();
        assert_eq!(h.messages().len(), 1);
        assert_eq!(h.messages()[0].kind, DiagKind::Warning);
        assert_eq!((h.messages()[0].line, h.messages()[0].col), (2, 1));
    }

    #[test]
    fn disabled_warning_is_dropped() {
        use crate::diag::{CollectingHandler, Warning};
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abc");
        sm.set_handler(Box::new(CollectingHandler::new()));
        sm.set_warning_status(Warning::Misc, false);
        sm.warning(
            Warning::Misc,
            SMLoc {
                source: id,
                offset: 0,
            },
            "x",
        );
        assert_eq!(sm.warning_count(), 0);
        assert_eq!(
            sm.handler_as::<CollectingHandler>()
                .unwrap()
                .messages()
                .len(),
            0
        );
    }

    #[test]
    fn error_range_threads_range() {
        use crate::diag::CollectingHandler;
        use crate::location::{SMLoc, SMRange};
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t.js", "let x = 1;");
        sm.set_handler(Box::new(CollectingHandler::new()));
        sm.error_range(
            SMRange {
                start: SMLoc {
                    source: id,
                    offset: 4,
                },
                end: SMLoc {
                    source: id,
                    offset: 9,
                },
            },
            "m",
        );
        let h = sm.handler_as::<CollectingHandler>().unwrap();
        assert_eq!(h.messages().len(), 1);
        assert_eq!(h.messages()[0].range_cols, Some((4, 9)));
    }

    #[test]
    fn warning_as_error_counts_as_error() {
        use crate::diag::{CollectingHandler, DiagKind, Warning};
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abc");
        sm.set_handler(Box::new(CollectingHandler::new()));
        sm.set_warning_is_error(Warning::Misc, true);
        sm.warning(
            Warning::Misc,
            SMLoc {
                source: id,
                offset: 0,
            },
            "x",
        );
        assert_eq!(sm.error_count(), 1);
        assert_eq!(
            sm.handler_as::<CollectingHandler>().unwrap().messages()[0].kind,
            DiagKind::Error
        );
    }
}
