/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Finding citation tokens in a Rust source file.
//!
//! ## The shapes in the wild
//!
//! ```text
//!   cpp:891                     bare, single line
//!   cpp:891-907                 bare, range
//!   SemanticResolver.cpp:891    qualified by basename
//!   flow.cpp:1232               qualified by a shorthand basename
//!   lib/Parser/JSONParser.cpp:202-211    qualified with a path prefix
//!   ESTree.def:697-750          .def files too
//!   cpp:86-88, 160-245          a continuation: `160-245` inherits the file
//!   ESTree.def:697-750 ... :677 an implicit file: `:677` inherits it too
//!   C++ 4890-4896               the parser port's spelling of `cpp:4890-4896`
//!   2886 in JSParserImpl-flow.cpp   the section-banner spelling, no colon
//! ```
//!
//! `C++ NNN[-MMM]` resolves exactly like `cpp:NNN[-MMM]` — through the
//! `[bare]` table — and is by far the commonest form in `crates/parser/src/js`
//! (1278 of them). A `C++` followed by a number that a dash then continues
//! into a word is prose, not a citation: "the C++ 3-arg `setLocation`
//! overload".
//!
//! `NNN[-MMM] in <basename>` is how the parser port spells the banner above
//! each ported function — `// parseReturnTypeAnnotationFlow — 2886 in
//! JSParserImpl-flow.cpp` — 137 of them, and the one shape with no colon at
//! all. It has no shorthand: the file must be named right there and it
//! resolves through `[qualified]` like any other named file, so nothing about
//! it is guessed. Because English can put a number in front of the word "in",
//! the shape is deliberately tight: `in` must be a word of its own, the
//! number must not be glued to what precedes it (so `C++17 in Foo.cpp` and
//! `v2 in Foo.cpp` are not citations), and what follows `in` must be a
//! `.cpp`/`.h`/`.def` basename — there is no bare-number variant that would
//! let a file named earlier be inherited. A sentence that satisfies all of
//! that anyway ("… returns 5 in Foo.cpp") would be read as a citation; it
//! cannot corrupt anything silently, because a citation nobody blessed is
//! reported as `unblessed` and fails the standing test.
//!
//! The `:NNN` shape is the only guessed one. Prose that has already named a
//! file often goes on citing it with a bare `:NNN`, and the file it means is
//! the one named nearest before it *on the same logical line*. That is a
//! heuristic, so it is kept narrow: the digits must touch the colon, the
//! colon must follow a space or an opening bracket or backtick, and a colon
//! bound to some *other* file (`resolver/mod.rs:1601`) cancels the context
//! rather than lending it. Where prose defeats the rule anyway, the site
//! gets a `[site_override]` in `citations.toml` — there are four, all found
//! by reading every such site once.
//!
//! A citation may also **wrap across two consecutive comment lines**, in any
//! of four places — after the directory (`` `lib/Parser/ `` ⏎ `JSONParser.cpp:202-211`),
//! after the colon (`SemanticResolver.cpp:` ⏎ `2276-2366`), after the range
//! dash (`CompilerDriver.cpp:2105-` ⏎ `2080`), or after a continuation comma.
//! To catch all of them the file is first turned into a *joined view*: a run
//! of consecutive line comments becomes one logical line, with each `//`,
//! `///` or `//!` marker (and the newline before it) replaced by a single
//! space — or by nothing when the previous line ended in `/`, so that a
//! wrapped directory prefix rejoins as `lib/Parser/JSONParser.cpp`. Lines
//! that are not comments are copied verbatim, so citations inside string
//! literals (there are a few, in `assert!` messages) are found too, while a
//! real newline still separates them and cannot be spanned by a token.
//!
//! Byte offsets are carried back to the *file* through a segment map, so a
//! caller (the remap of task 2) can rewrite exactly the digits it must.

use hermes_support::line_index::LineIndex;

/// Which C++ file a citation names, before resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileRef {
    /// A bare `cpp:NNN`; the file comes from the config's `[bare]` table.
    Bare,
    /// A citation naming a file, e.g. `lib/Parser/JSONParser.cpp:202-211`.
    Qualified {
        /// The directory prefix as written, `""` when there was none. Kept so
        /// the checker can flag a prefix that contradicts the config.
        prefix: String,
        /// The basename as written, e.g. `flow.cpp`.
        basename: String,
    },
}

/// One citation token found in one Rust file.
#[derive(Debug)]
pub struct RawCitation {
    /// 1-based line of the citation's first digit.
    pub line: u32,
    /// The token as written, with runs of whitespace (including a wrap
    /// across comment lines) collapsed to one space.
    pub text: String,
    /// The file the token names.
    pub file_ref: FileRef,
    /// First cited line, 1-based.
    pub start: u32,
    /// Last cited line for a range; `None` for a single-line citation.
    pub end: Option<u32>,
    /// Byte range of the start number's digits in the Rust file.
    pub start_digits: (u32, u32),
    /// Byte range of the end number's digits in the Rust file, if any.
    pub end_digits: Option<(u32, u32)>,
}

/// The joined view of a file plus the map back to file offsets.
struct Joined {
    text: String,
    /// `(joined offset, file offset, length)`, in increasing order.
    segments: Vec<(usize, usize, usize)>,
}

impl Joined {
    /// File offset for a joined offset. Panics only on an offset outside
    /// every segment, which cannot happen for an offset inside a token.
    fn file_offset(&self, joined: usize) -> usize {
        let i = match self.segments.binary_search_by_key(&joined, |&(j, _, _)| j) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let (j, f, _) = self.segments[i];
        f + (joined - j)
    }
}

/// Where a line comment's content starts, if the line is a line comment.
/// `// x` -> 2, `/// x` and `//! x` -> 3 (the marker is not content).
fn comment_content_start(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start().len();
    let body = &line[indent..];
    if !body.starts_with("//") {
        return None;
    }
    let third = body.as_bytes().get(2);
    let marker = if matches!(third, Some(b'/') | Some(b'!')) {
        3
    } else {
        2
    };
    Some(indent + marker)
}

/// Build the joined view described in the module doc.
fn join(text: &str) -> Joined {
    let mut joined = String::with_capacity(text.len());
    let mut segments = Vec::new();
    let mut prev_was_comment = false;
    let mut file_off = 0usize;
    for line in text.split('\n') {
        let content_start = comment_content_start(line);
        match content_start {
            Some(start) if prev_was_comment => {
                // Continue the previous logical comment line. The
                // continuation's own indentation is dropped, so a wrapped
                // directory prefix rejoins as `lib/Parser/JSONParser.cpp`
                // rather than `lib/Parser/ JSONParser.cpp`.
                let content = &line[start..];
                let indent = content.len() - content.trim_start().len();
                if !joined.ends_with('/') {
                    joined.push(' ');
                }
                segments.push((
                    joined.len(),
                    file_off + start + indent,
                    content.len() - indent,
                ));
                joined.push_str(&content[indent..]);
            }
            _ => {
                if !joined.is_empty() {
                    joined.push('\n');
                }
                segments.push((joined.len(), file_off, line.len()));
                joined.push_str(line);
            }
        }
        prev_was_comment = content_start.is_some();
        file_off += line.len() + 1;
    }
    Joined {
        text: joined,
        segments,
    }
}

/// True for the characters that can appear in the file part of a citation.
fn is_ref_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'/')
}

/// True for a word that names some file, whatever its kind — `mod.rs`,
/// `Foo.cpp`. Used to tell "this colon belongs to another file" from "this
/// colon belongs to prose".
fn looks_like_a_file(word: &str) -> bool {
    match word.rsplit_once('.') {
        None => false,
        Some((stem, ext)) => {
            !stem.is_empty()
                && (1..=4).contains(&ext.len())
                && ext.bytes().all(|b| b.is_ascii_alphabetic())
        }
    }
}

/// True when the character before a lone `:` is one that a citation can
/// follow — a space, an opening bracket, or a backtick.
fn opens_a_citation(bytes: &[u8], colon: usize) -> bool {
    match colon.checked_sub(1).map(|i| bytes[i]) {
        None => true,
        Some(b) => b.is_ascii_whitespace() || matches!(b, b'(' | b'[' | b'`'),
    }
}

/// Start of the run of file-reference characters ending at `end`.
fn read_ref_back(bytes: &[u8], end: usize) -> usize {
    let mut start = end;
    while start > 0 && is_ref_char(bytes[start - 1]) {
        start -= 1;
    }
    start
}

/// End of the run of file-reference characters starting at `start`, with any
/// trailing `.` dropped so a sentence-ending period is not read as part of the
/// name (`in JSParserImpl.cpp.`).
fn read_ref_forward(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < bytes.len() && is_ref_char(bytes[end]) {
        end += 1;
    }
    while end > start && bytes[end - 1] == b'.' {
        end -= 1;
    }
    end
}

/// True when the character before a `NNN in File.cpp` citation's first digit
/// lets the number stand as a token of its own. A letter, digit, `_`, `+`,
/// `.` or `/` glues it to something else, and `C++17 in Foo.cpp` or
/// `v2 in Foo.cpp` is then not a citation of line 17 or line 2.
fn number_stands_alone(bytes: &[u8], first_digit: usize) -> bool {
    match first_digit.checked_sub(1).map(|i| bytes[i]) {
        None => true,
        Some(b) => !(b.is_ascii_alphanumeric() || matches!(b, b'_' | b'+' | b'.' | b'/')),
    }
}

/// A `NNN[-MMM] in <basename>` citation, parsed by its anchor search so the
/// main loop can push it without re-reading the token.
#[derive(Clone, Debug)]
struct InCitation {
    /// Offset of the first digit: the token's start, and its anchor.
    text_start: usize,
    /// Offset just past the basename: the token's end.
    text_end: usize,
    /// `(value, offset just past the last digit)` of the first number.
    start_num: (u32, usize),
    /// The same for a range's second number.
    end_num: Option<(u32, usize)>,
    /// The file the token names, always [`FileRef::Qualified`].
    file_ref: FileRef,
}

/// Scan one Rust file's text for citation tokens, in source order.
pub fn scan_text(text: &str) -> Vec<RawCitation> {
    let joined = join(text);
    let index = LineIndex::build(text.as_bytes());
    let bytes = joined.text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    // The file named most recently on the current logical line, for the
    // `:NNN` shape that leaves it implicit. Reset at every real newline.
    let mut last_file: Option<FileRef> = None;
    let mut scanned_to = 0usize;

    // A citation is anchored at the `:` that precedes its first digit, at the
    // `C++` of the `C++ NNN-MMM` spelling, or at the first digit of the
    // `NNN in File.cpp` spelling. The three anchor characters are disjoint —
    // `:`, `C`, a digit — so whichever comes first identifies its own shape.
    // All three searches are cached and only re-run once passed, so the scan
    // stays linear in the file's length.
    let mut next_colon = joined.text.find(':');
    let mut next_cxx = find_cxx_anchor(&joined.text, bytes, 0);
    let mut next_in = find_in_anchor(&joined.text, bytes, 0);
    loop {
        if next_colon.is_some_and(|c| c < i) {
            next_colon = joined.text[i..].find(':').map(|r| i + r);
        }
        if next_cxx.is_some_and(|c| c < i) {
            next_cxx = find_cxx_anchor(&joined.text, bytes, i);
        }
        if next_in.as_ref().is_some_and(|c| c.text_start < i) {
            next_in = find_in_anchor(&joined.text, bytes, i);
        }
        let Some(colon) = [next_colon, next_cxx, next_in.as_ref().map(|c| c.text_start)]
            .into_iter()
            .flatten()
            .min()
        else {
            break;
        };
        i = colon + 1;
        if joined.text[scanned_to..colon].contains('\n') {
            last_file = None;
        }
        scanned_to = colon;

        // `NNN[-MMM] in File.cpp`: the section-banner spelling. The file is
        // always written out, so nothing is inherited and nothing is guessed;
        // `last_file` is deliberately left alone, since a shape with no colon
        // is a poor thing to lend a colon's context to.
        if next_in.as_ref().is_some_and(|c| c.text_start == colon) {
            let c = next_in.as_ref().expect("just matched");
            push_site(
                &mut out,
                &joined,
                &index,
                c.text_start,
                c.text_end,
                c.start_num,
                c.end_num,
                c.file_ref.clone(),
                c.start_num.0,
                c.end_num.map(|e| e.0),
            );
            i = c.text_end;
            continue;
        }

        // `C++ NNN-MMM`: the same citation as `cpp:NNN-MMM`, spelled the way
        // the parser port's inline comments spell it. The file comes from the
        // `[bare]` table exactly as for `cpp:`.
        // (A `:` and a `C` can never be at the same offset, so this is exact.)
        if Some(colon) == next_cxx {
            let num_start = skip_blanks(bytes, colon + 3);
            let start_num = digits_at(bytes, num_start).expect("the anchor checked for digits");
            let (end_num, after) = parse_range_end(bytes, start_num.1);
            let text_end = end_num.map(|e| e.1).unwrap_or(start_num.1);
            // `the C++ 3-arg setLocation overload` is prose, not a citation:
            // a dash that is not the start of a range gives it away.
            if bytes.get(text_end) == Some(&b'-') {
                continue;
            }
            push_site(
                &mut out,
                &joined,
                &index,
                colon,
                text_end,
                start_num,
                end_num,
                FileRef::Bare,
                start_num.0,
                end_num.map(|e| e.0),
            );
            last_file = Some(FileRef::Bare);
            i = scan_continuations(&mut out, &joined, &index, bytes, after);
            continue;
        }

        // The word before the colon, if any.
        let word_end = skip_blanks_back(bytes, colon);
        let mut word_start = read_ref_back(bytes, word_end);
        let mut named = classify(&joined.text[word_start..word_end]);
        // A closing backtick may sit between the file and the colon, as in
        // ``Ported from `lib/Parser/JSLexer.cpp`:``. Only look past it when
        // what turns up is a C++ file, so that the unrelated identifier in
        // ``see `MatchStatementCase` :677`` is not mistaken for one.
        if named.is_none() && word_start == word_end && word_end > 0 && bytes[word_end - 1] == b'`'
        {
            let quoted_end = word_end - 1;
            let quoted_start = read_ref_back(bytes, quoted_end);
            if let Some(file) = classify(&joined.text[quoted_start..quoted_end]) {
                named = Some(file);
                word_start = quoted_start;
            }
        }
        if named.is_none()
            && word_start != word_end
            && looks_like_a_file(&joined.text[word_start..word_end])
        {
            // A different file is bound to this colon
            // (`resolver/mod.rs:1601`): a later `:NNN` on this line would now
            // mean *that* file, which is not one we can resolve, so drop the
            // context rather than let it inherit the wrong file.
            last_file = None;
            continue;
        }

        // The number after the colon (spaces and tabs may intervene: a wrap
        // between the colon and the digits joins as `cpp:  891`).
        let num_start = skip_blanks(bytes, colon + 1);
        let Some(start_num) = digits_at(bytes, num_start) else {
            // No line number, but a file was named: `JSLexer.cpp:` followed
            // by a list of `:NNN-MMM` bullets. Remember it for them.
            if named.is_some() {
                last_file = named;
            }
            continue;
        };

        let (file_ref, text_start) = match named {
            Some(named) => {
                last_file = Some(named.clone());
                (named, word_start)
            }
            // A `:NNN` with no file of its own inherits the one named earlier
            // on this logical line. This is deliberately narrow — the digits
            // must touch the colon and the colon must follow an opener or a
            // space — because the alternative reading of a lone colon is
            // ordinary prose (`<unknown>:0:`, `(ESTree.h:1464-1477): 2000`).
            None => match (
                &last_file,
                num_start == colon + 1 && opens_a_citation(bytes, colon),
            ) {
                (Some(f), true) => (f.clone(), colon),
                _ => continue,
            },
        };

        // Optional `-MMM`.
        let (end_num, after) = parse_range_end(bytes, start_num.1);
        let text_end = end_num.map(|e| e.1).unwrap_or(start_num.1);
        let (start, end) = (start_num.0, end_num.map(|e| e.0));

        push_site(
            &mut out, &joined, &index, text_start, text_end, start_num, end_num, file_ref, start,
            end,
        );
        i = scan_continuations(&mut out, &joined, &index, bytes, after);
    }
    out
}

/// Consume any `, NNN` / `, NNN-MMM` continuations at `after`, each becoming
/// its own site inheriting the previous one's file. Returns the offset to
/// carry on scanning at.
fn scan_continuations(
    out: &mut Vec<RawCitation>,
    joined: &Joined,
    index: &LineIndex,
    bytes: &[u8],
    mut after: usize,
) -> usize {
    let mut text_end = after;
    loop {
        let after_comma = skip_blanks(bytes, after);
        if bytes.get(after_comma) != Some(&b',') {
            return text_end;
        }
        let n_start = skip_blanks(bytes, after_comma + 1);
        let Some(cont_start) = digits_at(bytes, n_start) else {
            return text_end;
        };
        let (cont_end, next_after) = parse_range_end(bytes, cont_start.1);
        text_end = cont_end.map(|e| e.1).unwrap_or(cont_start.1);
        let file_ref = out[out.len() - 1].file_ref.clone();
        push_site(
            out,
            joined,
            index,
            cont_start.1 - digit_len(cont_start.0),
            text_end,
            cont_start,
            cont_end,
            file_ref,
            cont_start.0,
            cont_end.map(|e| e.0),
        );
        after = next_after;
    }
}

/// Offset of the next `C++ NNN` anchor at or after `from`: a `C++` on a word
/// boundary, followed by blanks and then a digit.
fn find_cxx_anchor(text: &str, bytes: &[u8], from: usize) -> Option<usize> {
    let mut at = from;
    while let Some(rel) = text[at..].find("C++") {
        let pos = at + rel;
        at = pos + 3;
        let before_is_word = pos
            .checked_sub(1)
            .is_some_and(|i| bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_');
        // At least one blank is required, so that the language's own name in
        // `C++11` is not read as a citation of line 11.
        let num_start = skip_blanks(bytes, pos + 3);
        if !before_is_word && num_start > pos + 3 && digits_at(bytes, num_start).is_some() {
            return Some(pos);
        }
    }
    None
}

/// The next `NNN[-MMM] in <basename>` citation whose first digit is at or
/// after `from`, parsed.
///
/// Anchored on the word `in`, which is the only fixed part of the shape. Every
/// other requirement is a guard against reading prose as a citation: `in` must
/// be blank-delimited on both sides, the number must end where the blanks
/// before `in` begin and must not be glued to the word before it, and what
/// follows must be a basename this checker can resolve. There is no
/// bare-number variant — a number followed by `in` and something that is not a
/// source file is prose.
fn find_in_anchor(text: &str, bytes: &[u8], from: usize) -> Option<InCitation> {
    let mut at = from;
    while let Some(rel) = text[at..].find("in") {
        let pos = at + rel;
        at = pos + 2;
        let blank = |b: Option<&u8>| matches!(b, Some(b' ') | Some(b'\t'));
        if !blank(pos.checked_sub(1).map(|i| &bytes[i])) || !blank(bytes.get(pos + 2)) {
            continue;
        }
        // The file, written out in full right after `in`.
        let name_start = skip_blanks(bytes, pos + 2);
        let name_end = read_ref_forward(bytes, name_start);
        let Some(file_ref @ FileRef::Qualified { .. }) = classify(&text[name_start..name_end])
        else {
            continue;
        };
        // The number (or range), immediately before `in`.
        let Some(end_num) = digits_back(bytes, skip_blanks_back(bytes, pos)) else {
            continue;
        };
        let dash = skip_blanks_back(bytes, end_num.1 - digit_len(end_num.0));
        let (start_num, end_num) = match dash.checked_sub(1).map(|i| bytes[i]) {
            Some(b'-') => match digits_back(bytes, skip_blanks_back(bytes, dash - 1)) {
                Some(first) => (first, Some(end_num)),
                // `- MMM` with nothing before the dash is not a range.
                None => continue,
            },
            _ => (end_num, None),
        };
        let text_start = start_num.1 - digit_len(start_num.0);
        if text_start < from || !number_stands_alone(bytes, text_start) {
            continue;
        }
        return Some(InCitation {
            text_start,
            text_end: name_end,
            start_num,
            end_num,
            file_ref,
        });
    }
    None
}

/// Parse the decimal number that ends at `end` (exclusive), scanning
/// backwards. Returns the same `(value, offset just past the last digit)` pair
/// as [`digits_at`], or `None` if `end` is not preceded by a digit.
fn digits_back(bytes: &[u8], end: usize) -> Option<(u32, usize)> {
    let mut start = end;
    while start > 0 && bytes[start - 1].is_ascii_digit() {
        start -= 1;
    }
    if start == end {
        return None;
    }
    // Reuse the forward parse so the overflow rule is stated once.
    digits_at(bytes, start).filter(|&(_, past)| past == end)
}

/// Number of decimal digits in `n` (n >= 1 for every line number we parse).
fn digit_len(n: u32) -> usize {
    n.to_string().len()
}

/// Record one citation. `text_start`/`text_end` bound the token as written.
#[allow(clippy::too_many_arguments)]
fn push_site(
    out: &mut Vec<RawCitation>,
    joined: &Joined,
    index: &LineIndex,
    text_start: usize,
    text_end: usize,
    start_num: (u32, usize),
    end_num: Option<(u32, usize)>,
    file_ref: FileRef,
    start: u32,
    end: Option<u32>,
) {
    let start_digits_begin = joined.file_offset(start_num.1 - digit_len(start_num.0));
    let start_digits_end = joined.file_offset(start_num.1 - 1) + 1;
    let end_digits = end_num.map(|(value, past)| {
        (
            joined.file_offset(past - digit_len(value)) as u32,
            (joined.file_offset(past - 1) + 1) as u32,
        )
    });
    let (line, _) = index.line_col(start_digits_begin as u32);
    out.push(RawCitation {
        line,
        text: collapse_blanks(&joined.text[text_start..text_end]),
        file_ref,
        start,
        end,
        start_digits: (start_digits_begin as u32, start_digits_end as u32),
        end_digits,
    });
}

/// Parse an optional `- MMM` at `pos`. Returns the end number (value and the
/// offset just past its last digit) and the offset to continue scanning at.
fn parse_range_end(bytes: &[u8], pos: usize) -> (Option<(u32, usize)>, usize) {
    let dash = skip_blanks(bytes, pos);
    if bytes.get(dash) != Some(&b'-') {
        return (None, pos);
    }
    let n_start = skip_blanks(bytes, dash + 1);
    match digits_at(bytes, n_start) {
        // A dash not followed by a number is prose ("cpp:891 - the visitor").
        None => (None, pos),
        Some(end) => (Some(end), end.1),
    }
}

/// Classify the word before the colon, or `None` if it is not a file
/// reference at all.
fn classify(word: &str) -> Option<FileRef> {
    if word == "cpp" {
        return Some(FileRef::Bare);
    }
    let (prefix, basename) = match word.rfind('/') {
        Some(i) => (&word[..=i], &word[i + 1..]),
        None => ("", word),
    };
    let is_source =
        basename.ends_with(".cpp") || basename.ends_with(".h") || basename.ends_with(".def");
    if !is_source {
        return None;
    }
    Some(FileRef::Qualified {
        prefix: prefix.to_string(),
        basename: basename.to_string(),
    })
}

/// Parse the decimal number at `pos`; returns `(value, offset past it)`.
fn digits_at(bytes: &[u8], pos: usize) -> Option<(u32, usize)> {
    let mut i = pos;
    let mut value: u64 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        value = value * 10 + u64::from(bytes[i] - b'0');
        // A number longer than a plausible line number is not a citation.
        if value > u64::from(u32::MAX) {
            return None;
        }
        i += 1;
    }
    if i == pos {
        None
    } else {
        Some((value as u32, i))
    }
}

/// Skip spaces and tabs forward. Never crosses a newline: the joined view has
/// already merged the wraps that a citation may legitimately contain.
fn skip_blanks(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t') {
        pos += 1;
    }
    pos
}

/// Skip spaces and tabs backward from `pos` (exclusive).
fn skip_blanks_back(bytes: &[u8], mut pos: usize) -> usize {
    while pos > 0 && matches!(bytes[pos - 1], b' ' | b'\t') {
        pos -= 1;
    }
    pos
}

/// Collapse runs of whitespace to a single space, so a wrapped citation has
/// one canonical spelling in the snapshot and in config keys.
fn collapse_blanks(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_blank = false;
    for c in s.chars() {
        if c.is_whitespace() {
            in_blank = true;
        } else {
            if in_blank && !out.is_empty() {
                out.push(' ');
            }
            in_blank = false;
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> RawCitation {
        let mut v = scan_text(text);
        assert_eq!(
            v.len(),
            1,
            "expected exactly one citation in {text:?}: {v:?}"
        );
        v.pop().unwrap()
    }

    #[test]
    fn bare_single_and_range() {
        let c = one("// (cpp:891)\n");
        assert_eq!(c.file_ref, FileRef::Bare);
        assert_eq!((c.start, c.end), (891, None));
        assert_eq!(c.text, "cpp:891");
        assert_eq!(c.line, 1);
        let c = one("// (cpp:891-907)\n");
        assert_eq!((c.start, c.end), (891, Some(907)));
        assert_eq!(c.text, "cpp:891-907");
    }

    #[test]
    fn qualified_with_and_without_prefix() {
        let c = one("//! SemContext.h:354\n");
        assert_eq!(
            c.file_ref,
            FileRef::Qualified {
                prefix: String::new(),
                basename: "SemContext.h".into()
            }
        );
        assert_eq!((c.start, c.end), (354, None));
        let c = one("/// see `lib/Parser/JSONParser.cpp:202-211`\n");
        assert_eq!(
            c.file_ref,
            FileRef::Qualified {
                prefix: "lib/Parser/".into(),
                basename: "JSONParser.cpp".into()
            }
        );
        assert_eq!((c.start, c.end), (202, Some(211)));
        let c = one("//! ESTree.def:697-750\n");
        assert_eq!((c.start, c.end), (697, Some(750)));
    }

    #[test]
    fn wrapped_after_the_directory() {
        // The `lib/Parser/` ⏎ `JSONParser.cpp:202-211` shape.
        let src = "//! matching C++ (`lib/Parser/\n//! JSONParser.cpp:202-211`), so\n";
        let c = one(src);
        assert_eq!(
            c.file_ref,
            FileRef::Qualified {
                prefix: "lib/Parser/".into(),
                basename: "JSONParser.cpp".into()
            }
        );
        assert_eq!((c.start, c.end), (202, Some(211)));
        // The digits live on the second line, and the byte span points at them.
        assert_eq!(c.line, 2);
        assert_eq!(
            &src[c.start_digits.0 as usize..c.start_digits.1 as usize],
            "202"
        );
        let e = c.end_digits.expect("range");
        assert_eq!(&src[e.0 as usize..e.1 as usize], "211");
    }

    #[test]
    fn wrapped_after_the_colon_and_after_the_dash() {
        let src = "//! Ports `extractIdentsFromDecl` (SemanticResolver.cpp:\n//! 2276-2366)\n";
        let c = one(src);
        assert_eq!((c.start, c.end), (2276, Some(2366)));
        assert_eq!(c.text, "SemanticResolver.cpp: 2276-2366");
        assert_eq!(
            &src[c.start_digits.0 as usize..c.start_digits.1 as usize],
            "2276"
        );

        let src = "//! (CompilerDriver.cpp:2105-\n//!     2109) is what prints\n";
        let c = one(src);
        assert_eq!((c.start, c.end), (2105, Some(2109)));
        let e = c.end_digits.expect("range");
        assert_eq!(&src[e.0 as usize..e.1 as usize], "2109");
    }

    #[test]
    fn continuations_inherit_the_file() {
        let v = scan_text("/// (cpp:86-88, 160-245).\n");
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].start, v[0].end), (86, Some(88)));
        assert_eq!(v[0].text, "cpp:86-88");
        assert_eq!(v[1].file_ref, FileRef::Bare);
        assert_eq!((v[1].start, v[1].end), (160, Some(245)));
        assert_eq!(v[1].text, "160-245");

        let v = scan_text("/// flow.cpp:1232, 3462, 4856; jsx.cpp:260\n");
        assert_eq!(v.len(), 4);
        for c in &v[..3] {
            assert!(
                matches!(&c.file_ref, FileRef::Qualified { basename, .. } if basename == "flow.cpp")
            );
        }
        assert_eq!((v[1].start, v[2].start), (3462, 4856));
        assert!(
            matches!(&v[3].file_ref, FileRef::Qualified { basename, .. } if basename == "jsx.cpp")
        );
        assert_eq!(v[3].start, 260);

        // A comma followed by prose ends the run.
        let v = scan_text("/// (cpp:243, 261, ...).\n");
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn the_cxx_spelling_is_a_bare_citation() {
        let c = one("// C++ 4890-4896: eat(rw_class, AllowRegExp, ...).\n");
        assert_eq!(c.file_ref, FileRef::Bare);
        assert_eq!((c.start, c.end), (4890, Some(4896)));
        assert_eq!(c.text, "C++ 4890-4896");
        let c = one("// `from` is a contextual identifier. C++ 7306.\n");
        assert_eq!((c.start, c.end), (7306, None));
        // Continuations work here too.
        let v = scan_text("/// (C++ 7133-7137, 7361-7368).\n");
        assert_eq!(v.len(), 2);
        assert_eq!((v[1].start, v[1].end), (7361, Some(7368)));
        assert_eq!(v[1].file_ref, FileRef::Bare);
        // Prose, not a citation.
        assert!(scan_text("/// the C++ 3-arg `setLocation` overload\n").is_empty());
        assert!(scan_text("/// matching the C++11 rules\n").is_empty());
        assert!(scan_text("/// see ANSI-C++ 42\n").len() == 1);
    }

    #[test]
    fn the_in_spelling_names_its_file_outright() {
        let src = "    // parseReturnTypeAnnotationFlow — 2886 in JSParserImpl-flow.cpp\n";
        let c = one(src);
        assert_eq!(
            c.file_ref,
            FileRef::Qualified {
                prefix: String::new(),
                basename: "JSParserImpl-flow.cpp".into()
            }
        );
        assert_eq!((c.start, c.end), (2886, None));
        assert_eq!(c.text, "2886 in JSParserImpl-flow.cpp");
        assert_eq!(c.line, 1);
        assert_eq!(
            &src[c.start_digits.0 as usize..c.start_digits.1 as usize],
            "2886"
        );
        assert!(c.end_digits.is_none());

        // A range, a directory prefix, and a sentence-ending period.
        let src = "//! see 202-211 in lib/Parser/JSONParser.cpp.\n";
        let c = one(src);
        assert_eq!(
            c.file_ref,
            FileRef::Qualified {
                prefix: "lib/Parser/".into(),
                basename: "JSONParser.cpp".into()
            }
        );
        assert_eq!((c.start, c.end), (202, Some(211)));
        assert_eq!(c.text, "202-211 in lib/Parser/JSONParser.cpp");
        let e = c.end_digits.expect("range");
        assert_eq!(&src[e.0 as usize..e.1 as usize], "211");

        // Headers and `.def` files resolve the same way.
        assert_eq!(one("// 354 in SemContext.h\n").start, 354);
        assert_eq!(one("// 697 in ESTree.def\n").start, 697);

        // It coexists with the other shapes on one line, in source order.
        let v = scan_text("// parseFoo — 12 in JSLexer.cpp, ported at cpp:34 and C++ 56\n");
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].text, "12 in JSLexer.cpp");
        assert_eq!(
            (v[1].text.as_str(), v[2].text.as_str()),
            ("cpp:34", "C++ 56")
        );
    }

    #[test]
    fn the_in_spelling_does_not_swallow_prose() {
        // The number must not be glued to the word before it.
        assert!(scan_text("//! the C++17 in Foo.cpp rules\n").is_empty());
        assert!(scan_text("//! the v2 in Foo.cpp rules\n").is_empty());
        // `in` must be a word of its own.
        assert!(scan_text("//! 12 int Foo.cpp\n").is_empty());
        assert!(scan_text("//! 12in Foo.cpp\n").is_empty());
        // No bare-number variant: what follows `in` must be a source file, and
        // a file named earlier on the line is never inherited.
        assert!(scan_text("//! JSLexer.cpp has 12 in total\n").is_empty());
        assert!(scan_text("//! 12 in resolver/mod.rs\n").is_empty());
        assert!(scan_text("//! 12 in the parser\n").is_empty());
        // A number with no file after it at all.
        assert!(scan_text("//! 3 of the 12 in question\n").is_empty());
        // Prose with the file but no number: the `in` shape adds nothing.
        assert!(scan_text("//! the whitelist in SemanticResolver.h\n").is_empty());
        // A citation must not span a real newline between two code lines.
        assert!(scan_text("let a = 12;\nin JSLexer.cpp;\n").is_empty());
    }

    #[test]
    fn a_lone_colon_inherits_the_file_named_before_it() {
        // The file may have been named with line numbers ...
        let v = scan_text("//! (ESTree.def:697-750) plus `MatchStatementCase` :677.\n");
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].text, ":677");
        assert_eq!((v[1].start, v[1].end), (677, None));
        assert!(
            matches!(&v[1].file_ref, FileRef::Qualified { basename, .. } if basename == "ESTree.def")
        );

        // ... or without them, as a heading for a list of bullets.
        let v = scan_text(
            "//! Ported from `lib/Parser/JSLexer.cpp`:\n//! - `lookahead1` (`:1038-1095`)\n",
        );
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].start, v[0].end), (1038, Some(1095)));
        assert!(
            matches!(&v[0].file_ref, FileRef::Qualified { basename, .. } if basename == "JSLexer.cpp")
        );
    }

    #[test]
    fn a_lone_colon_does_not_inherit_from_prose_traps() {
        // A space before the digits means prose punctuation, not a citation.
        assert_eq!(
            scan_text("//! (ESTree.h:1464-1477): 2000 nested assignments\n").len(),
            1
        );
        // A colon glued to a word is that word's, not a citation.
        assert_eq!(
            scan_text("//! (SourceMgr.cpp:238) prints `<unknown>:0: error`\n").len(),
            1
        );
        // A colon bound to another file cancels the context.
        assert_eq!(
            scan_text(
                "//! `SemResolve.cpp:20` — the same one `resolver/mod.rs:1601` and `:1621` use\n"
            )
            .len(),
            1
        );
        // With no file named before it, a lone `:NNN` is not a citation.
        assert!(scan_text("//! the check at (:939-947) is shared\n").is_empty());
        // The context does not survive to the next logical line.
        assert_eq!(
            scan_text("//! see SemResolve.cpp:20\nlet x = 1;\n// (:31)\n").len(),
            1
        );
    }

    #[test]
    fn non_citations_are_ignored() {
        assert!(scan_text("// see https://example.com:8080/x\n").is_empty());
        assert!(scan_text("let x = map[\"k\"]; // ratio 16:9\n").is_empty());
        assert!(scan_text("// cpp:891 - the visitor is elsewhere\n")[0]
            .end
            .is_none());
        assert!(scan_text("// nothing here at all\n").is_empty());
        // A citation must not span a real newline between two code lines.
        assert!(scan_text("let a = foo.h;\n42;\n").is_empty());
    }

    #[test]
    fn citations_in_string_literals_are_found() {
        let v = scan_text("assert!(x, \"cpp:803 passes `false`\");\n");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].start, 803);
    }

    #[test]
    fn line_numbers_and_spans_point_at_the_digits() {
        let src = "//! a\n//! b\n/// see cpp:891-907 here\n";
        let c = one(src);
        assert_eq!(c.line, 3);
        assert_eq!(
            &src[c.start_digits.0 as usize..c.start_digits.1 as usize],
            "891"
        );
        let e = c.end_digits.expect("range");
        assert_eq!(&src[e.0 as usize..e.1 as usize], "907");
    }
}
