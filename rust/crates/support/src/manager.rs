/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! `SourceErrorManager`: owns source buffers, resolves locations, and
//! dispatches diagnostics. This module covers buffer registration, names,
//! virtual buffers, source-URL storage, and the central diagnostic emit
//! pipeline: handler storage, message counts, error limit, warning
//! categories, and message buffering/coalescing.
//! Port of `hermes::SourceErrorManager`.

use std::collections::HashMap;
use std::rc::Rc;

use crate::buffer::SourceBuffer;
use crate::diag::{
    CoordTranslator, DiagHandler, DiagKind, OutputOptions, ResolvedDiagnostic, Subsystem, Warning,
};
use crate::location::{SMLoc, SMRange, SourceCoords, SourceId};

/// A single diagnostic payload awaiting flush while buffering is active, or
/// held by an active collector. Port of the `MessageData` inner struct in
/// `SourceErrorManager.cpp`.
pub struct MessageData {
    dk: DiagKind,
    loc: Option<SMLoc>,
    range: Option<SMRange>,
    msg: String,
}

/// A buffered top-level message (non-note) plus the contiguous slice of its
/// attached notes stored in `buffered_notes`.
/// Port of `BufferedMessage` in `SourceErrorManager.cpp:26-83`.
struct BufferedMessage {
    data: MessageData,
    /// Index into `buffered_notes` where this message's notes begin.
    first_note: usize,
    /// Number of notes attached to this message.
    note_count: usize,
}

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
    /// Advisory output options for the installed diagnostic renderer.
    /// The `DiagHandler` owns the actual rendering; this is the manager's copy
    /// for callers that need to inspect or pass it along.
    /// Port of `outputOptions_` in the C++ class.
    output_options: OutputOptions,
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
    /// If `Some(s)`, messages from subsystem `s` are silently dropped.
    /// If `Some(Subsystem::Unspecified)`, ALL messages are dropped.
    /// Port of `SaveAndSuppressMessages` in C++.
    ///
    /// # Rust vs C++ design note
    /// The C++ uses an RAII guard (`SaveAndSuppressMessages`) that holds a
    /// pointer to the manager and restores the previous value on drop.  In safe
    /// Rust that pattern would require the guard to hold a `&mut
    /// SourceErrorManager`, which prevents the manager from also being borrowed
    /// for emitting through the same scope.  Instead, callers (e.g. the lexer)
    /// save the old value, set the new one, and restore it when done — an
    /// explicit save/restore that is equivalent but borrow-checker-friendly.
    suppressed_messages: Option<Subsystem>,
    /// Reference count for buffering. While > 0, generated messages are stored
    /// instead of dispatched; `disable_buffering` decrements and flushes when
    /// it reaches 0. Port of `bufferingEnabled_` and the `enableBuffering` /
    /// `disableBuffering` pair in `SourceErrorManager.cpp:26-83`.
    ///
    /// # Rust vs C++ design note
    /// The C++ uses an RAII guard (`SaveAndBufferMessages`) for the same
    /// reason as `SaveAndSuppressMessages` above: a `&mut`-holding guard
    /// cannot coexist with emitting through the manager in safe Rust.
    /// Callers use explicit `enable_buffering` / `disable_buffering` instead.
    buffering_enabled: u32,
    /// Top-level buffered messages (non-notes) in insertion order.
    buffered_messages: Vec<BufferedMessage>,
    /// Notes attached to buffered messages, stored in a single flat Vec;
    /// each `BufferedMessage` indexes into this Vec via `first_note`/`note_count`.
    buffered_notes: Vec<MessageData>,
    /// Active message collector. While `Some`, filtered messages are captured
    /// here instead of being counted or dispatched. Port of
    /// `externalMessageBuffer_` in `SourceErrorManager.h`.
    ///
    /// # Rust vs C++ design note
    /// The C++ uses an RAII guard (`CollectMessagesRAII`) that holds a pointer
    /// to the manager and restores the previous collector on drop. For the
    /// same borrow-checker reasons as `suppressed_messages`, callers use
    /// explicit `begin_collecting` / `end_collecting` instead.
    message_collector: Option<Vec<MessageData>>,
}

impl SourceErrorManager {
    pub fn new() -> SourceErrorManager {
        SourceErrorManager {
            entries: Vec::new(),
            by_name: HashMap::new(),
            output_options: OutputOptions::default(),
            translator: None,
            handler: None,
            message_count: [0; 3],
            error_limit: u32::MAX,
            error_limit_reached: false,
            last_message_suppressed: false,
            warning_enabled: vec![true; Warning::COUNT],
            warning_as_error: vec![false; Warning::COUNT],
            suppressed_messages: None,
            buffering_enabled: 0,
            buffered_messages: Vec::new(),
            buffered_notes: Vec::new(),
            message_collector: None,
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

    /// Obtain the buffer containing `loc` (cloning the `Rc`). The location
    /// carries its buffer, so this is a direct lookup. Port of `findBufferForLoc`.
    pub fn find_buffer_for_loc(&self, loc: SMLoc) -> Rc<SourceBuffer> {
        self.source_buffer(loc.source)
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

    /// Return the 1-based line span `[start, end)` for `line` in buffer `buf`
    /// as an `SMRange`, after LF and CR trimming. Matches the line-span branch of
    /// `findForCoordsImpl` (cpp:356-397). Returns `None` if `line` is out of range.
    pub fn find_smrange_for_line(&self, buf: SourceId, line: u32) -> Option<SMRange> {
        let entry = &self.entries[buf.index() as usize];
        entry.buffer.with_line_index(|idx, bytes| {
            if line < 1 || line > idx.line_count() {
                return None;
            }
            let ls = idx.line_start(line);
            let raw = idx.line_ref(bytes, line);
            // `raw` may include a trailing '\n'; strip it.
            let lf_trimmed = raw.len() as u32 - if raw.ends_with(b"\n") { 1 } else { 0 };
            let mut start = ls;
            let mut end = ls + lf_trimmed;
            // Trim lone CR at start/end to handle \r, \r\n, \n\r line endings.
            // Port of cpp:391-394.
            if start < end && bytes[start as usize] == b'\r' {
                start += 1;
            }
            if start < end && bytes[(end - 1) as usize] == b'\r' {
                end -= 1;
            }
            Some(SMRange {
                start: SMLoc {
                    source: buf,
                    offset: start,
                },
                end: SMLoc {
                    source: buf,
                    offset: end,
                },
            })
        })
    }

    /// Resolve a `SourceCoords` (buffer + 1-based line + 1-based col) to an
    /// `SMLoc`. Port of `findSMLocFromCoords` / `findForCoordsImpl` (cpp:396-438).
    /// Returns `None` if the line is out of range or the column is past the line.
    pub fn find_smloc_from_coords(&self, coords: SourceCoords) -> Option<SMLoc> {
        let buf = coords.buf;
        let line = coords.line;
        let col = coords.col;
        let entry = &self.entries[buf.index() as usize];
        entry.buffer.with_line_index(|idx, bytes| {
            if line < 1 || line > idx.line_count() {
                return None;
            }
            let ls = idx.line_start(line);
            let raw = idx.line_ref(bytes, line);
            // Strip trailing '\n' (the LF itself is not part of the line content).
            let lf_trimmed = raw.len() as u32 - if raw.ends_with(b"\n") { 1 } else { 0 };
            let mut start = ls;
            let mut end = ls + lf_trimmed;
            // CR trim — port of cpp:391-394.
            if start < end && bytes[start as usize] == b'\r' {
                start += 1;
            }
            if start < end && bytes[(end - 1) as usize] == b'\r' {
                end -= 1;
            }
            // Special case: empty line — port of cpp:401-406.
            if start == end {
                if col <= 1 {
                    return Some(SMLoc {
                        source: buf,
                        offset: start,
                    });
                }
                return None;
            }
            // Detect presence of any non-ASCII byte — port of cpp:408-415.
            let has_non_ascii = bytes[start as usize..end as usize]
                .iter()
                .any(|&b| b & 0x80 != 0);
            if !has_non_ascii {
                // ASCII fast path — port of cpp:418-425.
                if col > end - start {
                    return None;
                }
                return Some(SMLoc {
                    source: buf,
                    offset: start + col - 1,
                });
            }
            // UTF-8 path: scan code points, skipping continuation bytes.
            // Port of cpp:428-436.
            let mut column: u32 = 0;
            let mut offset = start;
            while offset < end {
                let b = bytes[offset as usize];
                // Skip UTF-8 continuation bytes (0b10xx_xxxx).
                if (b & 0b1100_0000) != 0b1000_0000 {
                    column += 1;
                    if column == col {
                        return Some(SMLoc {
                            source: buf,
                            offset,
                        });
                    }
                }
                offset += 1;
            }
            None
        })
    }

    /// Return `(buf, 1-based-line, byte-range-of-line)` for the line containing
    /// `loc`. Equivalent to `findBufferAndLine` (C++ header:428).
    ///
    /// # Lifetime note
    /// The C++ returns a `LineCoord` with a borrow into the buffer's bytes.
    /// Because `with_line_index` scopes the borrow to a closure, we cannot
    /// return a `LineCoord<'_>` without lifetime gymnastics. Instead we return
    /// the byte range as a `std::ops::Range<u32>` (owned, zero-copy). Callers
    /// can retrieve the actual slice via `source_buffer(buf).bytes()[range]`.
    pub fn find_buffer_and_line(&self, loc: SMLoc) -> (SourceId, u32, std::ops::Range<u32>) {
        let entry = &self.entries[loc.source.index() as usize];
        entry.buffer.with_line_index(|idx, bytes| {
            let (line, _col) = idx.line_col(loc.offset);
            let start = idx.line_start(line);
            // end = start of next line (includes the '\n'), or end of bytes.
            let line_ref = idx.line_ref(bytes, line);
            let end = start + line_ref.len() as u32;
            (loc.source, line, start..end)
        })
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

    // ---- Small helpers --------------------------------------------------------

    /// Build the smallest `SMRange` covering both `a` and `b` (both must be in
    /// the same buffer). Delegates to `SMRange::combine`.
    /// Port of `combineIntoRange` (C++ header:601-607).
    pub fn combine_into_range(&self, a: SMLoc, b: SMLoc) -> SMRange {
        SMRange::combine(a, b)
    }

    /// Convert the exclusive end of `range` to an inclusive location by
    /// subtracting one. If the range is empty (start == end), returns the start
    /// unchanged. Port of `convertEndToLocation` (cpp:669-675).
    pub fn convert_end_to_location(range: SMRange) -> SMLoc {
        if range.start == range.end {
            range.start
        } else {
            SMLoc {
                source: range.end.source,
                offset: range.end.offset - 1,
            }
        }
    }

    /// The display name of a buffer: its source URL if one was set (e.g. from a
    /// `//# sourceURL=` comment or a source map), otherwise its file name.
    /// Port of `getSourceUrl` (C++ header:415-421).
    fn source_url_or_name(&self, id: SourceId) -> &str {
        self.source_url(id)
            .unwrap_or_else(|| self.buffer_file_name(id))
    }

    /// Format `coords` as `"name:line:col"`, where `name` prefers the buffer's
    /// source URL over its file name (matching C++ `dumpCoords`, which uses
    /// `getSourceUrl`). Port of `dumpCoords(OS, SourceCoords)` (cpp:108-116).
    pub fn dump_coords(&self, coords: SourceCoords) -> String {
        format!(
            "{}:{}:{}",
            self.source_url_or_name(coords.buf),
            coords.line,
            coords.col
        )
    }

    /// Resolve `loc` to coordinates and format as `"filename:line:col"`.
    /// Port of `dumpCoords(OS, SMLoc)` (cpp:118-122).
    pub fn dump_coords_loc(&self, loc: SMLoc) -> String {
        let coords = self.find_coords(loc);
        self.dump_coords(coords)
    }

    // ---- Output options and translator accessors --------------------------------

    /// Return the current advisory output options.
    /// Port of `getOutputOptions` (C++ header:331).
    pub fn output_options(&self) -> OutputOptions {
        self.output_options
    }

    /// Replace the current advisory output options.
    /// Port of `setOutputOptions` (C++ header:335).
    pub fn set_output_options(&mut self, o: OutputOptions) {
        self.output_options = o;
    }

    /// Clone the installed translator, if any.
    /// Port of `getTranslator` (C++ header:355).
    pub fn translator(&self) -> Option<Rc<dyn CoordTranslator>> {
        self.translator.as_ref().map(Rc::clone)
    }

    // ---- Additional count accessors -------------------------------------------

    /// Number of messages of kind `dk` emitted so far.
    /// Port of `getMessageCount` (C++ header:586).
    pub fn get_message_count(&self, dk: DiagKind) -> u32 {
        self.message_count[dk as usize]
    }

    /// Convenience: number of notes emitted.
    /// Port of `getNoteCount` (C++ header:597).
    pub fn note_count(&self) -> u32 {
        self.message_count[DiagKind::Note as usize]
    }
}

impl Default for SourceErrorManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Diagnostic dispatch. Port of SourceErrorManager::message, countAndGenMessage,
// doGenMessage / doPrintMessage in SourceErrorManager.cpp. Includes subsystem
// suppression, message buffering/coalescing, and external message collection.
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

    /// Return the current error limit. Port of `getErrorLimit` (C++ header:285).
    pub fn get_error_limit(&self) -> u32 {
        self.error_limit
    }

    /// Return `true` if the error limit has been reached.
    pub fn is_error_limit_reached(&self) -> bool {
        self.error_limit_reached
    }

    /// Clear the error-limit-reached flag AND reset the error counter, so a
    /// fresh batch of errors can be emitted (e.g. to resume after recovery).
    /// Port of `clearErrorLimitReached` (C++ header:295-298), which resets both.
    pub fn clear_error_limit_reached(&mut self) {
        self.error_limit_reached = false;
        self.message_count[DiagKind::Error as usize] = 0;
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

    // ---- Subsystem suppression ----------------------------------------------

    /// Set (or clear) the subsystem whose messages are suppressed.
    /// Pass `Some(Subsystem::Unspecified)` to suppress all messages.
    /// Pass `None` to lift suppression.
    ///
    /// See the `suppressed_messages` field comment for the Rust vs C++ design
    /// rationale.
    pub fn set_suppressed_messages(&mut self, s: Option<Subsystem>) {
        self.suppressed_messages = s;
    }

    /// Return the currently suppressed subsystem, if any.
    pub fn suppressed_messages(&self) -> Option<Subsystem> {
        self.suppressed_messages
    }

    // ---- Public reporting overloads -----------------------------------------

    /// Emit an error at `loc` (subsystem: `Unspecified`).
    pub fn error(&mut self, loc: SMLoc, msg: impl Into<String>) {
        self.emit(
            DiagKind::Error,
            Warning::NoWarning,
            Subsystem::Unspecified,
            Some(loc),
            None,
            msg.into(),
        );
    }

    /// Emit an error at the start of `range`, underscoring the full range
    /// (subsystem: `Unspecified`).
    pub fn error_range(&mut self, range: SMRange, msg: impl Into<String>) {
        self.emit(
            DiagKind::Error,
            Warning::NoWarning,
            Subsystem::Unspecified,
            Some(range.start),
            Some(range),
            msg.into(),
        );
    }

    /// Emit an error at `loc` with an optional `range` and explicit `subsystem`.
    /// The lexer calls this form to allow per-subsystem suppression.
    pub fn error_at(
        &mut self,
        loc: SMLoc,
        range: Option<SMRange>,
        msg: impl Into<String>,
        subsystem: Subsystem,
    ) {
        self.emit(
            DiagKind::Error,
            Warning::NoWarning,
            subsystem,
            Some(loc),
            range,
            msg.into(),
        );
    }

    /// Emit a note at `loc` (subsystem: `Unspecified`).
    pub fn note(&mut self, loc: SMLoc, msg: impl Into<String>) {
        self.emit(
            DiagKind::Note,
            Warning::NoWarning,
            Subsystem::Unspecified,
            Some(loc),
            None,
            msg.into(),
        );
    }

    /// Emit a note over `range` (the caret sits at `range.start`).
    pub fn note_range(&mut self, range: SMRange, msg: impl Into<String>, subsystem: Subsystem) {
        self.emit(
            DiagKind::Note,
            Warning::NoWarning,
            subsystem,
            Some(range.start),
            Some(range),
            msg.into(),
        );
    }

    /// Emit a note at `loc`, optionally underlining `range`, in `subsystem`.
    pub fn note_at(
        &mut self,
        loc: SMLoc,
        range: Option<SMRange>,
        msg: impl Into<String>,
        subsystem: Subsystem,
    ) {
        self.emit(
            DiagKind::Note,
            Warning::NoWarning,
            subsystem,
            Some(loc),
            range,
            msg.into(),
        );
    }

    /// Emit a warning of the given category at `loc` (subsystem: `Unspecified`).
    pub fn warning(&mut self, w: Warning, loc: SMLoc, msg: impl Into<String>) {
        self.emit(
            DiagKind::Warning,
            w,
            Subsystem::Unspecified,
            Some(loc),
            None,
            msg.into(),
        );
    }

    /// Emit a `Misc` warning at `loc` (subsystem: `Unspecified`).
    pub fn warning_misc(&mut self, loc: SMLoc, msg: impl Into<String>) {
        self.emit(
            DiagKind::Warning,
            Warning::Misc,
            Subsystem::Unspecified,
            Some(loc),
            None,
            msg.into(),
        );
    }

    /// Emit a warning of the given category at the start of `range`,
    /// underscoring the full range, with an explicit `subsystem`.
    pub fn warning_range(
        &mut self,
        w: Warning,
        range: SMRange,
        msg: impl Into<String>,
        subsystem: Subsystem,
    ) {
        self.emit(
            DiagKind::Warning,
            w,
            subsystem,
            Some(range.start),
            Some(range),
            msg.into(),
        );
    }

    /// General-purpose overload: caller supplies all parameters.
    /// Port of the `message(DiagKind, Warning, Subsystem, ...)` C++ overloads.
    pub fn message(
        &mut self,
        dk: DiagKind,
        w: Warning,
        subsystem: Subsystem,
        loc: Option<SMLoc>,
        range: Option<SMRange>,
        msg: impl Into<String>,
    ) {
        self.emit(dk, w, subsystem, loc, range, msg.into());
    }

    // ---- Central dispatch ---------------------------------------------------

    /// Central dispatch. Port of `SourceErrorManager::message` +
    /// `countAndGenMessage`. Subsystem suppression is checked first (port of
    /// `cpp:180-187`), before the error-limit check, matching C++ ordering.
    fn emit(
        &mut self,
        mut dk: DiagKind,
        w: Warning,
        subsystem: Subsystem,
        loc: Option<SMLoc>,
        range: Option<SMRange>,
        msg: String,
    ) {
        // Suppress messages from the suppressed subsystem (or all, if
        // Unspecified). Port of SourceErrorManager.cpp:180-187.
        // Note: suppressed messages do NOT update `last_message_suppressed`,
        // matching C++ behavior — they just return.
        if let Some(s) = self.suppressed_messages {
            if s == Subsystem::Unspecified || subsystem == s {
                return;
            }
        }
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
        // If a collector is active, capture the (already-filtered) message
        // instead of generating it. Port of the externalMessageBuffer_ check
        // (cpp:206-209). Collected messages are NOT counted at collect time;
        // they are replayed through count_and_gen in end_collecting.
        if let Some(collector) = self.message_collector.as_mut() {
            collector.push(MessageData {
                dk,
                loc,
                range,
                msg,
            });
            return;
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
        self.do_gen_message(dk, loc, range, msg);
        // Check after calling do_gen_message so the original message is emitted
        // (or buffered) first, then the "too many errors" sentinel.  Matches
        // C++ behavior.
        if dk == DiagKind::Error && self.message_count[DiagKind::Error as usize] == self.error_limit
        {
            self.error_limit_reached = true;
            self.do_gen_message(
                DiagKind::Error,
                None,
                None,
                "too many errors emitted".to_string(),
            );
        }
    }

    /// Route a message either to the buffer (if buffering is active) or
    /// directly to the handler.  Notes are attached to the last buffered
    /// non-note message; if no such message exists yet, the note becomes a
    /// standalone buffered message (edge case, matches C++).
    /// Port of `doGenMessage` in `SourceErrorManager.cpp:124-155`.
    fn do_gen_message(
        &mut self,
        dk: DiagKind,
        loc: Option<SMLoc>,
        range: Option<SMRange>,
        msg: String,
    ) {
        if self.buffering_enabled > 0 {
            if dk == DiagKind::Note && !self.buffered_messages.is_empty() {
                // Attach note to the last buffered non-note message.
                let note = MessageData {
                    dk,
                    loc,
                    range,
                    msg,
                };
                let first = self.buffered_notes.len();
                self.buffered_notes.push(note);
                let last = self.buffered_messages.last_mut().unwrap();
                if last.note_count == 0 {
                    last.first_note = first;
                }
                last.note_count += 1;
            } else {
                // Buffer as a standalone top-level message (includes the edge
                // case of a Note when no top-level message is buffered yet).
                self.buffered_messages.push(BufferedMessage {
                    data: MessageData {
                        dk,
                        loc,
                        range,
                        msg,
                    },
                    first_note: 0,
                    note_count: 0,
                });
            }
        } else {
            self.gen_message(dk, loc, range, msg);
        }
    }

    // ---- Message buffering / coalescing (Phase 3) ---------------------------

    /// Increment the buffering reference count.  While the count is > 0,
    /// messages are queued rather than dispatched to the handler.
    /// Port of `enableBuffering` in `SourceErrorManager.cpp`.
    pub fn enable_buffering(&mut self) {
        self.buffering_enabled += 1;
    }

    /// Decrement the buffering reference count.  When it reaches zero, flush
    /// all buffered messages — stable-sorted by source position — to the
    /// handler.  Port of `disableBuffering` in `SourceErrorManager.cpp`.
    ///
    /// # Flush ordering
    /// Messages with a source location are emitted in (buffer-index, offset)
    /// order.  The "too many errors emitted" sentinel (Error with no location)
    /// is forced last.  No deduplication — matches C++ behavior.
    /// Each top-level message is immediately followed by its attached notes in
    /// insertion order.
    pub fn disable_buffering(&mut self) {
        debug_assert!(self.buffering_enabled > 0);
        self.buffering_enabled -= 1;
        if self.buffering_enabled > 0 {
            return;
        }
        // Take ownership of both vecs so we can borrow `self` mutably for
        // gen_message while iterating.
        let msgs = std::mem::take(&mut self.buffered_messages);
        let notes = std::mem::take(&mut self.buffered_notes);
        // Build a sorted index.  Located messages sort by (source-index, offset);
        // the sentinel (loc == None) sorts last via the leading 0/1 discriminant.
        //
        // `sort_by_key` is a STABLE sort, so two messages emitted at the same
        // location keep their emission order.  This used to be a documented
        // divergence: C++ sorted the buffered messages with `std::sort`, whose
        // tie order is unspecified, so a same-location pair could come out
        // either way (in practice depending on the total buffered count).
        // Upstream `5f313a13a` ("Sort buffered diagnostics with a stable
        // sort") changed it to `std::stable_sort`
        // (`SourceErrorManager.cpp:60-73`), so both sides now break
        // same-location ties in emission order and the divergence is retired.
        let mut order: Vec<usize> = (0..msgs.len()).collect();
        order.sort_by_key(|&i| match msgs[i].data.loc {
            Some(l) => (0u8, l.source.index(), l.offset),
            None => (1u8, u32::MAX, u32::MAX),
        });
        for &i in &order {
            let m = &msgs[i];
            self.gen_message(m.data.dk, m.data.loc, m.data.range, m.data.msg.clone());
            for n in &notes[m.first_note..m.first_note + m.note_count] {
                self.gen_message(n.dk, n.loc, n.range, n.msg.clone());
            }
        }
        // Both vecs were moved out; nothing to clear.
    }

    // ---- External message collection (Phase 4) --------------------------------

    /// Begin collecting messages. Returns the previous collector (if nested) to
    /// be passed back to `end_collecting`. While active, filtered messages are
    /// captured (not counted or dispatched). Also enables buffering so the
    /// replayed messages are source-sorted on flush.
    /// Port of `CollectMessagesRAII` constructor.
    pub fn begin_collecting(&mut self) -> Option<Vec<MessageData>> {
        self.enable_buffering();
        self.message_collector.replace(Vec::new())
    }

    /// End collection. If `discard` is false, replay the collected messages
    /// through the normal count+buffer path (counting them and flushing
    /// source-sorted); otherwise drop them. Restores the previous collector.
    /// Port of the `CollectMessagesRAII` destructor.
    ///
    /// Order matches C++: replay (counts + buffers) → disable_buffering
    /// (flushes) → restore previous collector. During replay `message_collector`
    /// is `None` (taken), so `count_and_gen` → `do_gen_message` buffers them
    /// rather than re-collecting.
    pub fn end_collecting(&mut self, previous: Option<Vec<MessageData>>, discard: bool) {
        let collected = self.message_collector.take().unwrap_or_default();
        if !discard {
            for m in collected {
                self.count_and_gen(m.dk, m.loc, m.range, m.msg);
            }
        }
        self.disable_buffering();
        self.message_collector = previous;
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
                // Use UNtranslated coordinates here: the rendered diagnostic and
                // its source-line / caret-column lookups must be resolved against
                // the original buffer, exactly as the C++ primary diagnostic does
                // (`doPrintMessage` passes the original loc to PrintMessage; the
                // translator only affects a separate annotation we don't render).
                // Using translated coords would fetch the wrong source line and
                // miscompute the caret column when a CoordTranslator is installed.
                let coords = self.find_untranslated_coords(loc);
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
            // No location: `SourceMgr::GetMessage` (SourceMgr.cpp:238-298)
            // never touches the buffers, so the `SMDiagnostic` keeps its
            // `BufferID = "<unknown>"` default (:246) and its zero-initialized
            // `LineAndCol`, giving line 0 and (as `col - 1`) column -1. The
            // renderer's `col == 0` means exactly that -1, i.e. "print no
            // column". This is the shape of the `too many errors emitted`
            // sentinel, the only location-less message hermesc emits.
            None => ResolvedDiagnostic {
                kind: dk,
                file_name: "<unknown>".to_string(),
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
        // clear_error_limit_reached resets BOTH the flag and the error count
        // (matching C++), so a fresh error can be emitted afterwards.
        sm.clear_error_limit_reached();
        assert!(!sm.is_error_limit_reached());
        assert_eq!(sm.error_count(), 0);
        sm.error(
            SMLoc {
                source: id,
                offset: 2,
            },
            "third",
        );
        assert_eq!(sm.error_count(), 1);
        assert!(sm.is_error_limit_reached());
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

    #[test]
    fn suppresses_matching_subsystem() {
        use crate::diag::{CollectingHandler, Subsystem};
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abc");
        sm.set_handler(Box::new(CollectingHandler::new()));
        sm.set_suppressed_messages(Some(Subsystem::Lexer));
        // A Lexer-subsystem error is dropped (no count, no handler message).
        sm.error_at(
            SMLoc {
                source: id,
                offset: 0,
            },
            None,
            "x",
            Subsystem::Lexer,
        );
        assert_eq!(sm.error_count(), 0);
        assert_eq!(
            sm.handler_as::<CollectingHandler>()
                .unwrap()
                .messages()
                .len(),
            0
        );
        // A Parser-subsystem error still passes when only Lexer is suppressed.
        sm.error_at(
            SMLoc {
                source: id,
                offset: 1,
            },
            None,
            "y",
            Subsystem::Parser,
        );
        assert_eq!(sm.error_count(), 1);
    }

    #[test]
    fn suppress_unspecified_drops_everything() {
        use crate::diag::{CollectingHandler, Subsystem};
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abc");
        sm.set_handler(Box::new(CollectingHandler::new()));
        sm.set_suppressed_messages(Some(Subsystem::Unspecified));
        sm.error_at(
            SMLoc {
                source: id,
                offset: 0,
            },
            None,
            "x",
            Subsystem::Parser,
        );
        assert_eq!(sm.error_count(), 0);
    }

    // ---- Message buffering / coalescing tests --------------------------------

    #[test]
    fn buffering_sorts_by_source_order() {
        use crate::diag::CollectingHandler;
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abcdef");
        sm.set_handler(Box::new(CollectingHandler::new()));
        sm.enable_buffering();
        sm.error(
            SMLoc {
                source: id,
                offset: 4,
            },
            "second",
        ); // later in source
        sm.error(
            SMLoc {
                source: id,
                offset: 1,
            },
            "first",
        ); // earlier
           // Nothing emitted yet while buffering.
        assert_eq!(
            sm.handler_as::<CollectingHandler>()
                .unwrap()
                .messages()
                .len(),
            0
        );
        sm.disable_buffering();
        let h = sm.handler_as::<CollectingHandler>().unwrap();
        assert_eq!(h.messages().len(), 2);
        assert_eq!(h.messages()[0].message, "first"); // source order
        assert_eq!(h.messages()[1].message, "second");
        assert_eq!(sm.error_count(), 2); // counted when emitted, not at flush
    }

    #[test]
    fn buffered_note_follows_its_message() {
        use crate::diag::{CollectingHandler, DiagKind};
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abcdef");
        sm.set_handler(Box::new(CollectingHandler::new()));
        sm.enable_buffering();
        sm.error(
            SMLoc {
                source: id,
                offset: 0,
            },
            "err",
        );
        sm.note(
            SMLoc {
                source: id,
                offset: 2,
            },
            "a note",
        );
        sm.disable_buffering();
        let h = sm.handler_as::<CollectingHandler>().unwrap();
        assert_eq!(h.messages().len(), 2);
        assert_eq!(h.messages()[0].kind, DiagKind::Error);
        assert_eq!(h.messages()[1].kind, DiagKind::Note);
        assert_eq!(h.messages()[1].message, "a note");
    }

    #[test]
    fn nested_buffering_flushes_only_at_zero() {
        use crate::diag::CollectingHandler;
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abc");
        sm.set_handler(Box::new(CollectingHandler::new()));
        sm.enable_buffering();
        sm.enable_buffering();
        sm.error(
            SMLoc {
                source: id,
                offset: 0,
            },
            "x",
        );
        sm.disable_buffering(); // still buffering (count 1)
        assert_eq!(
            sm.handler_as::<CollectingHandler>()
                .unwrap()
                .messages()
                .len(),
            0
        );
        sm.disable_buffering(); // now flush
        assert_eq!(
            sm.handler_as::<CollectingHandler>()
                .unwrap()
                .messages()
                .len(),
            1
        );
    }

    // ---- External message collection tests ----------------------------------

    #[test]
    fn collect_then_replay() {
        use crate::diag::CollectingHandler;
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abcdef");
        sm.set_handler(Box::new(CollectingHandler::new()));
        let prev = sm.begin_collecting();
        sm.error(
            SMLoc {
                source: id,
                offset: 4,
            },
            "second",
        );
        sm.error(
            SMLoc {
                source: id,
                offset: 1,
            },
            "first",
        );
        // Collected, not counted, not dispatched.
        assert_eq!(sm.error_count(), 0);
        assert_eq!(
            sm.handler_as::<CollectingHandler>()
                .unwrap()
                .messages()
                .len(),
            0
        );
        sm.end_collecting(prev, false);
        // Replayed: counted and dispatched in source order.
        assert_eq!(sm.error_count(), 2);
        let h = sm.handler_as::<CollectingHandler>().unwrap();
        assert_eq!(h.messages().len(), 2);
        assert_eq!(h.messages()[0].message, "first");
        assert_eq!(h.messages()[1].message, "second");
    }

    #[test]
    fn collect_then_discard() {
        use crate::diag::CollectingHandler;
        use crate::location::SMLoc;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abc");
        sm.set_handler(Box::new(CollectingHandler::new()));
        let prev = sm.begin_collecting();
        sm.error(
            SMLoc {
                source: id,
                offset: 0,
            },
            "x",
        );
        sm.end_collecting(prev, true);
        assert_eq!(sm.error_count(), 0);
        assert_eq!(
            sm.handler_as::<CollectingHandler>()
                .unwrap()
                .messages()
                .len(),
            0
        );
    }

    // ---- Phase 5: find/convert/dump helpers ---------------------------------

    #[test]
    fn smloc_from_coords_roundtrip() {
        use crate::location::SourceCoords;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "ab\ncde\nf");
        // 'd' is line 2 col 2 -> offset 4.
        let loc = sm
            .find_smloc_from_coords(SourceCoords {
                buf: id,
                line: 2,
                col: 2,
            })
            .unwrap();
        assert_eq!(loc.offset, 4);
        // round-trips with find_coords
        let c = sm.find_coords(loc);
        assert_eq!((c.line, c.col), (2, 2));
    }

    #[test]
    fn smloc_from_coords_utf8() {
        use crate::location::SourceCoords;
        let mut sm = SourceErrorManager::new();
        // "aé" : a(0), é=0xC3 0xA9 (1,2). col 2 is 'é' -> offset 1 (lead byte).
        let id = sm.add_buffer("a.js", "aé");
        let loc = sm
            .find_smloc_from_coords(SourceCoords {
                buf: id,
                line: 1,
                col: 2,
            })
            .unwrap();
        assert_eq!(loc.offset, 1);
    }

    #[test]
    fn smrange_for_line_spans_content() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "ab\r\ncde");
        // Line 1 is "ab" (CR + LF trimmed): offsets [0,2).
        let r = sm.find_smrange_for_line(id, 1).unwrap();
        assert_eq!((r.start.offset, r.end.offset), (0, 2));
    }

    #[test]
    fn convert_end_to_location_subtracts_one() {
        use crate::location::{SMLoc, SMRange};
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abcdef");
        let r = SMRange {
            start: SMLoc {
                source: id,
                offset: 1,
            },
            end: SMLoc {
                source: id,
                offset: 4,
            },
        };
        assert_eq!(SourceErrorManager::convert_end_to_location(r).offset, 3);
        let empty = SMRange {
            start: SMLoc {
                source: id,
                offset: 2,
            },
            end: SMLoc {
                source: id,
                offset: 2,
            },
        };
        assert_eq!(SourceErrorManager::convert_end_to_location(empty).offset, 2);
    }

    #[test]
    fn dump_coords_prefers_source_url() {
        use crate::location::SourceCoords;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "abc");
        // Without a source URL, the file name is used.
        assert_eq!(
            sm.dump_coords(SourceCoords {
                buf: id,
                line: 1,
                col: 1
            }),
            "a.js:1:1"
        );
        // With a source URL set, it is preferred (matches C++ getSourceUrl).
        sm.set_source_url(id, "orig.ts");
        assert_eq!(
            sm.dump_coords(SourceCoords {
                buf: id,
                line: 1,
                col: 1
            }),
            "orig.ts:1:1"
        );
    }
}
