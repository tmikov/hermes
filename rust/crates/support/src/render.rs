//! Byte-compatible source-line + caret rendering. Port of
//! `SourceErrorManager::buildSourceAndCaretLine` and `printDiagnosticHelper`.

use crate::diag::{DiagHandler, DiagKind, OutputOptions, ResolvedDiagnostic};

/// Build the (expanded) source line and the caret/underline line for a
/// diagnostic. Faithful port of `buildSourceAndCaretLine`.
///
/// `col` is the 1-based byte column of the caret. `ranges` are 0-based byte
/// `[start, end)` ranges to underline with `~`.
pub fn build_source_and_caret_line(
    source_line_text: &str,
    col: u32,
    ranges: &[(usize, usize)],
    opts: &OutputOptions,
) -> (String, String) {
    // Decode our source line to UTF-32 (here: Vec<char>). Map from narrow byte
    // to column as we go.
    let mut byte_to_column: Vec<usize> = Vec::new();
    let mut source_line: Vec<char> = Vec::new();
    for ch in source_line_text.chars() {
        // The column (code-point index) for this char is the current length of
        // the decoded line; push it once per narrow byte the char spans.
        let column = source_line.len();
        for _ in 0..ch.len_utf8() {
            byte_to_column.push(column);
        }
        source_line.push(ch);
    }
    let num_columns = source_line.len();

    // Map a 0-based narrow byte offset to a code-point column. Out-of-range
    // offsets map to one-past-the-end.
    let widen_column = |narrow_column: usize| -> usize {
        byte_to_column
            .get(narrow_column)
            .copied()
            .unwrap_or(num_columns)
    };

    // getColumnNo() is 0-based byte col; our `col` is 1-based.
    let column_no = widen_column((col as usize).saturating_sub(1));

    let widened_ranges: Vec<(usize, usize)> = ranges
        .iter()
        .map(|&(s, e)| (widen_column(s), widen_column(e)))
        .collect();

    // Build the caret line as ASCII bytes (space/`~`/`^`/`.`).
    let mut caret_line: Vec<u8> = vec![b' '; num_columns + 1];
    for &(first, second) in &widened_ranges {
        if first < caret_line.len() {
            let end = std::cmp::min(second, caret_line.len());
            for c in &mut caret_line[first..end] {
                *c = b'~';
            }
        }
    }
    caret_line[std::cmp::min(column_no, num_columns)] = b'^';

    // Trim trailing spaces: erase everything after the last non-space char.
    if let Some(last) = caret_line.iter().rposition(|&c| c != b' ') {
        caret_line.truncate(last + 1);
    } else {
        caret_line.clear();
    }

    // Expand tabs to spaces in both lines.
    let tab_stop = OutputOptions::TAB_STOP;
    let mut pos = 0;
    while pos < source_line.len() {
        if source_line[pos] == '\t' {
            let expand_count = tab_stop - (pos % tab_stop);
            // Replace the tab in the source line with `expand_count` spaces.
            source_line.splice(pos..pos + 1, std::iter::repeat_n(' ', expand_count));
            // Mirror the expansion in the caret line: a tab under '~' becomes
            // more '~', otherwise spaces.
            if pos < caret_line.len() {
                let fill = caret_line[pos];
                caret_line.splice(pos..pos + 1, std::iter::repeat_n(fill, expand_count));
            }
            pos += expand_count;
        } else {
            pos += 1;
        }
    }

    // Trim to preferredMaxErrorWidth, focusing around caret / intersecting
    // range. preferredMaxErrorWidth defaults to "unlimited" (usize::MAX), which
    // skips the trim branch unless a finite width is set.
    let preferred_max_error_width = opts.preferred_max_error_width.unwrap_or(usize::MAX);
    let mut focus_start: usize = column_no;
    let mut focus_length: usize = 1;
    for &(first, second) in &widened_ranges {
        if first <= column_no && column_no < second {
            focus_start = first;
            focus_length = second - first;
            break;
        }
    }
    let desired_line_length = std::cmp::max(
        preferred_max_error_width,
        focus_length + OutputOptions::MINIMUM_SOURCE_CONTEXT,
    );
    if source_line.len() > desired_line_length {
        let focus_center = focus_start + focus_length / 2;
        // leftTrimAmount can be negative in C++; guard with a signed compare.
        let half = desired_line_length / 2;
        if focus_center > half {
            let left_trim_amount = focus_center - half;
            // Erase the leading portion of both lines.
            let ct = std::cmp::min(left_trim_amount, caret_line.len());
            caret_line.drain(0..ct);
            let st = std::cmp::min(left_trim_amount, source_line.len());
            source_line.drain(0..st);
            // Mark the truncation with up to three '.'.
            for c in source_line.iter_mut().take(3) {
                *c = '.';
            }
        }
        if source_line.len() > desired_line_length {
            let ce = std::cmp::min(caret_line.len(), desired_line_length);
            caret_line.truncate(ce);
            source_line.truncate(desired_line_length);
            // Mark the right truncation with up to three '.'.
            let len = source_line.len();
            for c in source_line.iter_mut().skip(len.saturating_sub(3)) {
                *c = '.';
            }
        }
    }

    // Re-encode sourceLine (UTF-32) back to narrow UTF-8.
    let narrow_source_line: String = source_line.into_iter().collect();
    let caret_string: String = caret_line.into_iter().map(|c| c as char).collect();
    (narrow_source_line, caret_string)
}

/// Render a `ResolvedDiagnostic` to a `String`. The returned string ends with
/// a newline. Produces:
/// - `file:line:col: kind: message\n`
/// - the source line + `\n` (if available)
/// - the caret/underline line + `\n` (only for all-ASCII source lines)
///
/// Port of `printDiagnosticHelper`.
pub fn render_diagnostic(diag: &ResolvedDiagnostic, opts: &OutputOptions) -> String {
    let kind_str = match diag.kind {
        DiagKind::Error => "error",
        DiagKind::Warning => "warning",
        DiagKind::Note => "note",
    };
    // The location prefix is conditional, exactly as in
    // `printDiagnosticHelper` (SourceErrorManager.cpp:575-583): an empty
    // filename prints no prefix at all, `-` prints as `<stdin>`, and the
    // column is omitted when C++'s `columnNo` is -1. C++ builds that
    // `columnNo` as `col - 1`, so "no column" is exactly `col == 0` here —
    // unreachable for a resolved location (columns are 1-based) and what a
    // location-less message carries. `lineNo` is never -1 in Hermes's use
    // (`SourceMgr::GetMessage` leaves it 0 for an invalid location,
    // SourceMgr.cpp:238-298), so it is always printed with the filename;
    // that is what makes the "too many errors emitted" sentinel print as
    // `<unknown>:0: error: ...`.
    let mut out = String::new();
    if !diag.file_name.is_empty() {
        if diag.file_name == "-" {
            out.push_str("<stdin>");
        } else {
            out.push_str(&diag.file_name);
        }
        out.push(':');
        out.push_str(&diag.line.to_string());
        if diag.col != 0 {
            out.push(':');
            out.push_str(&diag.col.to_string());
        }
        out.push_str(": ");
    }
    out.push_str(kind_str);
    out.push_str(": ");
    out.push_str(&diag.message);
    out.push('\n');
    if let Some(src) = &diag.source_line {
        // Convert Option<(u32,u32)> to a one-element or empty slice of
        // (usize, usize) so we can pass it to build_source_and_caret_line.
        let range_arr: [(usize, usize); 1];
        let ranges: &[(usize, usize)] = match diag.range_cols {
            Some((s, e)) => {
                range_arr = [(s as usize, e as usize)];
                &range_arr
            }
            None => &[],
        };
        // Like C++ printDiagnosticHelper, always print the tab-expanded source
        // line returned by build_source_and_caret_line (not the raw line), so the
        // source and caret columns stay aligned. The caret line itself is only
        // shown for all-ASCII source lines.
        let (src_expanded, caret) = build_source_and_caret_line(src, diag.col, ranges, opts);
        out.push_str(&src_expanded);
        out.push('\n');
        if src.is_ascii() {
            out.push_str(&caret);
            out.push('\n');
        }
    }
    out
}

/// Default handler: prints `file:line:col: kind: message`, the source line, and
/// (for all-ASCII lines) a caret/underline. Port of `printDiagnosticHelper`.
pub struct StderrHandler {
    opts: OutputOptions,
}

impl StderrHandler {
    pub fn new(opts: OutputOptions) -> StderrHandler {
        StderrHandler { opts }
    }
}

impl DiagHandler for StderrHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn handle(&mut self, diag: &ResolvedDiagnostic) {
        let s = render_diagnostic(diag, &self.opts);
        // The string already ends with '\n'; use eprint! to avoid a double newline.
        eprint!("{}", s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::{DiagKind, OutputOptions};

    /// `printDiagnosticHelper`'s location prefix is conditional
    /// (SourceErrorManager.cpp:575-583): the whole prefix is skipped for an
    /// empty filename, `-` renders as `<stdin>`, and the column is omitted
    /// when `columnNo == -1` — which is what a location-less diagnostic has,
    /// since `SMDiagnostic` gets `col - 1` and a location-less message has
    /// col 0. The "too many errors emitted" sentinel is the one such message
    /// hermesc actually emits, and it prints as `<unknown>:0: error: ...`
    /// (`BufferID = "<unknown>"`, SourceMgr.cpp:246; `LineAndCol` defaults to
    /// {0,0}).
    #[test]
    fn header_prefix_is_conditional() {
        use crate::diag::ResolvedDiagnostic;
        let base = |file: &str, line: u32, col: u32| ResolvedDiagnostic {
            kind: DiagKind::Error,
            file_name: file.into(),
            line,
            col,
            message: "m".into(),
            source_line: None,
            range_cols: None,
        };
        let opts = OutputOptions::default();
        // The sentinel's shape: no column, line 0, `<unknown>` filename.
        assert_eq!(
            render_diagnostic(&base("<unknown>", 0, 0), &opts),
            "<unknown>:0: error: m\n"
        );
        // An empty filename drops the prefix entirely.
        assert_eq!(render_diagnostic(&base("", 0, 0), &opts), "error: m\n");
        // `-` is the stdin buffer name; it renders as `<stdin>`.
        assert_eq!(
            render_diagnostic(&base("-", 3, 4), &opts),
            "<stdin>:3:4: error: m\n"
        );
        // The ordinary case is unchanged.
        assert_eq!(
            render_diagnostic(&base("t.js", 3, 4), &opts),
            "t.js:3:4: error: m\n"
        );
    }

    #[test]
    fn ranged_caret_underline() {
        use crate::diag::ResolvedDiagnostic;
        let d = ResolvedDiagnostic {
            kind: DiagKind::Error,
            file_name: "t".into(),
            line: 1,
            col: 5,
            message: "m".into(),
            source_line: Some("let x = 1;".into()),
            range_cols: Some((4, 9)),
        };
        let s = render_diagnostic(&d, &OutputOptions::default());
        assert!(
            s.contains("t:1:5: error: m"),
            "header not found in: {:?}",
            s
        );
        assert!(
            s.contains("    ^~~~~"),
            "caret underline not found in: {:?}",
            s
        );
    }

    #[test]
    fn render_expands_tabs_in_source_line() {
        use crate::diag::ResolvedDiagnostic;
        // Like C++, the printed source line is tab-expanded so it stays aligned
        // with the caret line. "\tx" with the caret on 'x' (col 2) -> 8 spaces.
        let d = ResolvedDiagnostic {
            kind: DiagKind::Error,
            file_name: "t".into(),
            line: 1,
            col: 2,
            message: "m".into(),
            source_line: Some("\tx".into()),
            range_cols: None,
        };
        let s = render_diagnostic(&d, &OutputOptions::default());
        assert!(
            s.contains("        x\n"),
            "source not tab-expanded: {:?}",
            s
        );
        assert!(s.contains("        ^\n"), "caret misaligned: {:?}", s);
    }

    #[test]
    fn caret_under_single_column() {
        let (src, caret) =
            build_source_and_caret_line("let x = 1;", 5, &[], &OutputOptions::default());
        assert_eq!(src, "let x = 1;");
        assert_eq!(caret, "    ^");
    }

    #[test]
    fn tabs_expand_to_spaces_tabstop_8() {
        let (src, caret) = build_source_and_caret_line("\tx", 2, &[], &OutputOptions::default());
        assert_eq!(src, "        x");
        assert_eq!(caret, "        ^");
    }

    #[test]
    fn range_underlined_with_tildes() {
        let (_src, caret) =
            build_source_and_caret_line("let x = 1;", 5, &[(4, 9)], &OutputOptions::default());
        // Faithful C++ port: range [4,9) underlines columns 4..=8 (5 columns),
        // and the '^' overwrites column 4. So 4 spaces, '^', then 4 '~'.
        assert_eq!(caret, "    ^~~~~");
    }

    #[test]
    fn non_ascii_columns_are_codepoints() {
        // "éx": 'é' is 2 bytes (byte indices 0,1) / 1 column; 'x' is byte 2.
        // `col` is a 1-based *byte* column (faithful to C++ getColumnNo, which
        // is a 0-based byte offset). col=2 -> 0-based byte 1, which is the
        // second byte of 'é' and widens back to column 0. So the caret lands on
        // 'é' (column 0), yielding "^".
        let (_src, caret) = build_source_and_caret_line("éx", 2, &[], &OutputOptions::default());
        assert_eq!(caret, "^");
    }

    #[test]
    fn non_ascii_columns_caret_on_second_char() {
        // To land on 'x' (column 1), the caret must point at byte 2 ('x'),
        // i.e. 1-based byte col 3. This confirms the byte->codepoint mapping:
        // both bytes of 'é' map to column 0, and 'x' is column 1.
        let (_src, caret) = build_source_and_caret_line("éx", 3, &[], &OutputOptions::default());
        assert_eq!(caret, " ^");
    }
}
