//! Line-start index for a source buffer: maps byte offsets to 1-based
//! (line, column) and back. Column is the byte distance from the line start,
//! matching LLVH `SourceMgr` so that caret rendering is byte-compatible.

/// Cached table of line-start byte offsets for one buffer.
pub struct LineIndex {
    /// `line_starts[i]` = byte offset of the start of (1-based) line `i + 1`.
    /// Always begins with 0.
    line_starts: Vec<u32>,
}

impl LineIndex {
    /// Build the index over `bytes` (the buffer contents, excluding the NUL
    /// terminator). A line starts at offset 0 and after each `\n`.
    pub fn build(bytes: &[u8]) -> LineIndex {
        let mut line_starts = Vec::with_capacity(64);
        line_starts.push(0u32);
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        LineIndex { line_starts }
    }

    /// Return 1-based `(line, col)` for `offset`. `col` is 1-based byte distance
    /// from the line start.
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        // Largest line whose start is <= offset.
        let line0 = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let col = offset - self.line_starts[line0] + 1;
        ((line0 + 1) as u32, col)
    }

    /// Return the slice of `bytes` for 1-based `line`, including its trailing
    /// EOL if present. The final line may have no EOL.
    pub fn line_ref<'a>(&self, bytes: &'a [u8], line: u32) -> &'a [u8] {
        let start = self.line_starts[(line - 1) as usize] as usize;
        let end = self
            .line_starts
            .get(line as usize)
            .map(|&e| e as usize)
            .unwrap_or(bytes.len());
        &bytes[start..end]
    }

    /// Number of lines.
    #[allow(dead_code)]
    pub fn line_count(&self) -> u32 {
        self.line_starts.len() as u32
    }

    /// Byte offset where 1-based `line` starts (= `line_starts[line - 1]`).
    /// Panics if `line` is out of range; callers should guard with `line_count()`.
    pub fn line_start(&self, line: u32) -> u32 {
        self.line_starts[(line - 1) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lf_line_and_col() {
        // "ab\ncde\nf"  offsets: a0 b1 \n2 c3 d4 e5 \n6 f7
        let idx = LineIndex::build(b"ab\ncde\nf");
        assert_eq!(idx.line_col(0), (1, 1)); // 'a'
        assert_eq!(idx.line_col(1), (1, 2)); // 'b'
        assert_eq!(idx.line_col(3), (2, 1)); // 'c'
        assert_eq!(idx.line_col(5), (2, 3)); // 'e'
        assert_eq!(idx.line_col(7), (3, 1)); // 'f' (last line, no EOL)
    }

    #[test]
    fn crlf_column_counts_bytes() {
        // "a\r\nb": a0 \r1 \n2 b3 -> 'b' is line 2 col 1
        let idx = LineIndex::build(b"a\r\nb");
        assert_eq!(idx.line_col(0), (1, 1));
        assert_eq!(idx.line_col(3), (2, 1));
    }

    #[test]
    fn line_ref_excludes_nothing_before_eol() {
        let bytes = b"ab\ncde\nf";
        let idx = LineIndex::build(bytes);
        // 1-based line number -> the line slice including its EOL if present.
        assert_eq!(idx.line_ref(bytes, 1), b"ab\n");
        assert_eq!(idx.line_ref(bytes, 3), b"f");
    }

    #[test]
    fn empty_buffer_has_one_line() {
        let idx = LineIndex::build(b"");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.line_col(0), (1, 1));
        assert_eq!(idx.line_ref(b"", 1), b"");
    }

    #[test]
    fn trailing_newline_starts_an_empty_last_line() {
        // A trailing '\n' terminates line 1; line 2 exists but is empty.
        // Matches LLVH SourceMgr's line counting.
        let bytes = b"a\n";
        let idx = LineIndex::build(bytes);
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.line_ref(bytes, 1), b"a\n");
        assert_eq!(idx.line_ref(bytes, 2), b"");
        assert_eq!(idx.line_col(2), (2, 1));
    }
}
