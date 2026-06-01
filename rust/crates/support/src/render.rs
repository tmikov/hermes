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
        let kind = match diag.kind {
            DiagKind::Error => "error",
            DiagKind::Warning => "warning",
            DiagKind::Note => "note",
        };
        eprintln!(
            "{}:{}:{}: {}: {}",
            diag.file_name, diag.line, diag.col, kind, diag.message
        );
        if let Some(src) = &diag.source_line {
            let (line, caret) = build_source_and_caret_line(src, diag.col, &[], &self.opts);
            eprintln!("{}", line);
            // Hermes shows the caret line only for all-ASCII source lines.
            if src.is_ascii() {
                eprintln!("{}", caret);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::OutputOptions;

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
