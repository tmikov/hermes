//! Live token differential: run the Rust lexer and the real C++ `js-lexer-dump`
//! oracle over a punctuator/whitespace/comment corpus and assert the dumps are
//! byte-for-byte equal.
//!
//! Phase 1a lexes ONLY punctuators, whitespace, and comments — identifiers,
//! numbers, strings, templates, regexp and private identifiers are stubbed — so
//! the corpus below contains no such tokens. The oracle is driven with
//! `--context=div`, so `/` lexes as `slash`/`slashequal` (regexp is a later
//! phase); the Rust lexer is driven with `GrammarContext::AllowDiv` to match.

use std::io::Write;
use std::process::{Command, Stdio};

use atom_table::AtomTable;
use parser::lexer::{GrammarContext, JSLexer};
use parser::token_kinds::TokenKind;
use support::manager::SourceErrorManager;

/// Produce the Rust lexer dump for `src` (one `dump_token` line per token,
/// including the final `eof`, each terminated by '\n').
fn rust_dump(src: &str) -> String {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("t", src);
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    let mut out = String::new();
    loop {
        let k = lex.advance(GrammarContext::AllowDiv).kind();
        lex.dump_token(&mut out);
        out.push('\n');
        if k == TokenKind::eof {
            break;
        }
    }
    out
}

/// Run the C++ `js-lexer-dump --context=div -` oracle on `src` via stdin and
/// return its stdout. Returns `None` if the binary is not built (so CI without
/// a build can skip), mirroring how the support crate's golden tests skip.
fn cpp_dump(src: &str) -> Option<String> {
    let bin = "cmake-build-asan/bin/js-lexer-dump";
    if !std::path::Path::new(bin).exists() {
        return None;
    }
    let mut child = Command::new(bin)
        .arg("--context=div")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(src.as_bytes())
        .ok()?;
    let out = child.wait_with_output().ok()?;
    Some(String::from_utf8(out.stdout).unwrap())
}

#[test]
fn differential_punctuators_and_trivia() {
    let corpus = [
        // All single-/multi-char punctuators.
        "{ } ( ) [ ] ; , ~ : @",
        "= == === => != !== ! < <= << <<= > >= >> >>> >>= >>>=",
        "+ ++ += - -- -= * *= ** **= / /= % %= & && &= | || |= ^ ^= ? ?? ?. ??= ...",
        // Newline-flag tracking with punctuator-only tokens.
        ";;;\n;",
        ";;\n\t ;; \n\n ;",
        // Comments.
        "; /* block\ncomment */ ;",
        "; // line comment\n;",
        "; /* no newline */ ;",
        // BOM / no-break-space skipping.
        "\u{feff}; ;",
        "\u{00a0}; ;",
        // Line/paragraph separators set the newline flag.
        "; \u{2028} ;",
        "; \u{2029} ;",
        // Adjacent optional-chaining / numeric-lookahead edge: `?.` vs `? .`
        // (no digit, so `?.`).
        "?.;",
        // `...` spread and `.` period runs.
        ". .. ... ....",
        // Tight whitespace runs and tabs.
        "\t\t;  \t ;",
        // Empty input -> just eof.
        "",
        // Trailing line comment with no newline at EOF.
        "; // tail",
        // Hashbang at the very start of the buffer.
        "#!/usr/bin/env hermes\n;",
    ];
    for src in corpus {
        let Some(cpp) = cpp_dump(src) else {
            eprintln!("skip: js-lexer-dump not built");
            return;
        };
        assert_eq!(rust_dump(src), cpp, "mismatch for {src:?}");
    }
}
