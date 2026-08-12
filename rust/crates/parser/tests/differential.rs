//! Live token differential: run the Rust lexer and the real C++ `js-lexer-dump`
//! oracle over a punctuator/whitespace/comment corpus and assert the dumps are
//! byte-for-byte equal.
//!
//! The lexer covers punctuators, whitespace, comments, identifiers, numbers,
//! string literals, private identifiers, template literals (no-substitution and
//! head forms) and regular-expression literals. The harness is parameterized by
//! grammar context: the `--context=div` corpus lexes `/` as `slash`/`slashequal`
//! (Rust: `GrammarContext::AllowDiv`); the `--context=regexp` corpus lexes `/` as
//! a regexp literal (Rust: `GrammarContext::AllowRegExp`); the `--context=type`
//! corpus lexes the Flow type grammar (`{|`/`|}`, `%checks`, `@`-prefixed
//! identifiers, individual `<`/`>`, no `??`) (Rust: `GrammarContext::Type`). Both
//! sides are driven with the matching context so the dumps compare byte-for-byte.
//!
//! The harness is also parameterized by strict mode. The default corpora run in
//! strict mode (the lexer default); the `differential_nonstrict` corpus drives
//! both sides in non-strict mode (Rust: `set_strict_mode(false)`; C++:
//! `--non-strict`), exercising the future-reserved-word downgrade
//! (`static`/`yield`/… → `identifier`) and the legacy octal / leading-zero
//! numeric and octal-escape paths that strict mode rejects.

use std::io::Write;
use std::process::{Command, Stdio};

use hermes_atom_table::AtomTable;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_parser::token_kinds::TokenKind;
use hermes_support::manager::SourceErrorManager;

/// The C++ `--context=` flag value matching a Rust `GrammarContext`.
fn context_flag(ctx: GrammarContext) -> &'static str {
    match ctx {
        GrammarContext::AllowDiv => "--context=div",
        GrammarContext::AllowRegExp => "--context=regexp",
        GrammarContext::Type => "--context=type",
        GrammarContext::AllowJSXIdentifier => "--context=jsx",
    }
}

/// Produce the Rust lexer dump for `src` under `ctx` (one `dump_token` line per
/// token, including the final `eof`, each terminated by '\n'). When `strict` is
/// false the lexer is switched to non-strict mode before the first token,
/// matching the C++ oracle's `--non-strict` flag.
fn rust_dump(src: &str, ctx: GrammarContext, strict: bool) -> String {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("t", src);
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, ctx);
    if !strict {
        lex.set_strict_mode(false);
    }
    let mut out = String::new();
    loop {
        let k = lex.advance(ctx).kind();
        lex.dump_token(&mut out);
        out.push('\n');
        if k == TokenKind::eof {
            break;
        }
    }
    out
}

/// Absolute path to the C++ `js-lexer-dump` oracle, resolved relative to the
/// repo root. `CARGO_MANIFEST_DIR` is `<repo>/rust/crates/parser`, so three `..`
/// hops reach the repo root. Resolving relative to the manifest dir (rather than
/// the cwd) is essential: `cargo test` runs integration-test binaries with cwd =
/// the crate manifest dir, not the repo root, so a relative path here would
/// silently never exist and the differential would skip every assertion.
fn js_lexer_dump_bin() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../cmake-build-asan/bin/js-lexer-dump")
}

/// Run the C++ `js-lexer-dump` oracle on `src` via stdin with an explicit list
/// of flags (before the trailing `-`) and return its stdout. The caller is
/// responsible for checking the binary exists (see `js_lexer_dump_bin`); this
/// always attempts to run it.
fn cpp_dump_flags(bin: &std::path::Path, src: &str, flags: &[&str]) -> Option<String> {
    let mut child = Command::new(bin)
        .args(flags)
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
        // Identifiers and reserved words (harness default = strict mode).
        "foo bar baz",
        "_x $y a1 Z9",
        // strict-mode reswords
        "function for while return yield static implements",
        // a sampling of the rest of the reserved words
        "if in var break continue switch this true false null case catch const",
        "debugger default delete do else finally instanceof new throw try typeof",
        "void with export import class extends super enum interface package",
        "private protected public",
        // not reserved words: lex as identifiers
        "let async await of as from get set",
        // unicode identifiers
        "caf\u{00e9} \u{4e2d}\u{6587} na\u{00ef}ve",
        // unicode-only identifier start via the non-ASCII default arm
        "\u{00e9}tude \u{03b1}\u{03b2}\u{03b3}",
        // Lead-byte fall-through cases (the C++ `goto default_label` from the
        // c2/e2/ef special-byte arms). `ª`(c2 aa) and `ﬀ`(ef ac 80) are
        // Unicode-only id-starts -> identifiers; `«`(c2 ab)/`»`(c2 bb) are
        // neither id-start nor space -> unrecognized-character errors (to
        // stderr) that the lexer recovers from, so stdout still matches.
        "\u{00aa} \u{00ab}\u{00bb} \u{fb00} x",
        // e2-led byte that is NOT a line/paragraph separator (`…` U+2026) and
        // is neither id-start nor space -> unrecognized error + recovery.
        "a\u{2026} b",
        // No-break space (c2 a0) immediately followed by a c2-led id-start.
        "\u{00a0}\u{00aa}",
        // escaped identifiers (start + part)
        "\\u0041\\u0042 ab\\u0063",
        // braced unicode escape inside an identifier
        "x\\u{1F600}y",
        // newline flag between idents
        "x;y\nz",
        // ---- numeric literals (valid forms only; error/NaN literals would go
        // to stderr and a NaN bit pattern could differ, so they are kept out) --
        // decimal integers, incl. the >9-digit / >2^53 paths.
        "0 1 42 1000000000 9007199254740993",
        // fractions, exponents, separators.
        "0.1 .5 3.14159 1e10 2E-3 6.022e23 1_000_000",
        // hex / octal / binary, mixed case + separators.
        "0xff 0xDEAD_BEEF 0o17 0b1010 0XAB 0O7 0B11",
        // BigInt: decimal + hex.
        "10n 0xffn 255n 0n",
        // '.' number vs. period: `0 .5` is numeric `.5`, not a member access.
        "0 .5 .25",
        // number immediately followed by a punctuator.
        "1+2 3*4 5;6",
        // legacy octal. In strict mode (the harness default) these report an
        // error to stderr, but the lexer still recovers and emits the octal
        // numeric_literal value on stdout, so the dumps match byte-for-byte.
        "0123 010 07",
        // the <=9-digit decimal fast-path boundary (9 digits, 10 digits, and
        // all-nines at the boundary).
        "123456789 1234567890 999999999",
        // trailing dot and leading dot, incl. `0.`, and a dot+exponent form.
        "5. .5 0. 1.e3",
        // ---- string literals --------------------------------------------------
        // plain strings, both quote styles.
        "'a' \"b\" 'hello world'",
        // escapes -> escapes=1 (tab, newline, CR, backslash, escaped quote).
        "'a\\tb' \"x\\ny\" '\\r\\\\\\''",
        // hex (\xHH) + unicode (\uHHHH) escapes.
        "'\\x41\\x7e' '\\u00e9\\u4e2d'",
        // raw unicode inside strings -> re-encoded as WTF-8 \xHH in the dump.
        "'caf\u{00e9}' \"\u{4e2d}\u{6587}\"",
        // NUL escape, octal \101='A', embedded NUL. Octal escapes error in
        // strict mode (the harness default), but the cooked value is still
        // emitted, like legacy-octal numbers.
        "'\\0' '\\101' '\\x00end'",
        // escaped line continuations (LF and CRLF) -> skipped, escapes=1.
        "'a\\\nb' 'line\\\r\ncont'",
        // ---- private identifiers ----------------------------------------------
        // private ids (incl. a member-like `x.#priv`).
        "#foo #_bar x.#priv",
        // mixed: string, private id, number, punctuator.
        "'a' #b 5 ;",
        // ---- template literals ------------------------------------------------
        // The corpus is restricted to forms a plain `advance` loop lexes cleanly:
        // no_substitution_template (ends in `` ` ``) and template_head (ends in
        // `${`). A `}`-continuation would start a new template scan in a plain
        // loop (the parser drives `rescanRBraceInTemplateLiteral`), so it is out.
        // no_substitution_template.
        "`hello` `a b c`",
        // template_head (then EOF).
        "`a${",
        // multiple heads + a no-substitution at the end.
        "`x${ `y${ `done`",
        // escapes: cooked vs raw differ.
        "`tab\\tnl\\n` `raw\\u00e9`",
        // NotEscapeSequence -> cooked=null.
        "`not\\9esc`",
        // CR -> LF in cooked+raw.
        "`cr\rlf`",
        // raw unicode -> WTF-8 in cooked+raw (incl. a supplementary-plane char).
        "`uni\u{4e2d}` `astral\u{1f600}`",
    ];
    run_differential("div", &corpus, GrammarContext::AllowDiv, true);
}

#[test]
fn differential_regexp() {
    let corpus = [
        // regexp corpus (driven with --context=regexp on both sides).
        "/abc/g /x/ /[a-z]+/gi",
        "/[/]/ /a\\/b/ /\\d+/", // '/' in class, escaped '/', escape
        "/foo/gimsuy",          // all flags
        "/\u{4e2d}/u",          // unicode in body -> WTF-8
        "x = /re/g",            // div context would differ; here regexp follows '='
    ];
    run_differential("regexp", &corpus, GrammarContext::AllowRegExp, true);
}

#[test]
fn differential_type() {
    let corpus = [
        // Flow type corpus (driven with --context=type on both sides).
        // object-type braces `{| |}` and a union `|`.
        "{| a: number |} | string",
        // individual `<`/`>`, `>>`/`<<` as separate tokens, no `??`, %checks.
        "<T> >> << ?? %checks",
        // `@`-prefixed Flow identifiers.
        "@flow @decorator a b",
        // generics: `<`/`>` as individual tokens.
        "Array<string> Map<K, V>",
        // unions / intersections.
        "x | y & z",
        // plain punctuators still work in Type context.
        "{ a: 1 } [1, 2]",
    ];
    run_differential("type", &corpus, GrammarContext::Type, true);
}

#[test]
fn differential_jsx() {
    let corpus = [
        // JSX-identifier corpus (regular `advance` under --context=jsx). The
        // `-` is an identifier part and `>` lexes as `greater`.
        "<div-foo a-b>",
        "<my-element data-x>",
        "x-y-z foo-bar",
        // `<`/`>` punctuation around JSX-flavored identifiers.
        "< a-b > < /c-d >",
    ];
    run_differential("jsx", &corpus, GrammarContext::AllowJSXIdentifier, true);
}

#[test]
fn differential_nonstrict() {
    let corpus = [
        // The eight future reserved words: in non-strict mode each lexes as a
        // plain `identifier` rather than `rw_*` (the meaningful stdout
        // divergence from strict mode).
        "implements interface package private protected public static yield",
        // `yield` on its own (the most common one), plus a mix with words that
        // stay reserved in BOTH modes (`function`/`for`/`return`/`var`) to
        // confirm only the future-reserved set downgrades.
        "function yield for static return var public",
        // Future reserved words used as ordinary identifiers in real code.
        "let static = 1; var private = yield;",
        // ---- legacy octal / leading-zero numerics (strict mode errors on
        // these to stderr; non-strict accepts them silently). stdout already
        // matches in strict mode, but this locks in the non-strict path too. --
        "0123 010 07 0o17",
        // leading-zero decimals (`08`/`09`): NOT octal (contain 8/9), parsed as
        // decimal; rejected in strict mode, accepted in non-strict.
        "08 09 00 019",
        // ---- legacy octal string escapes (`\07`): error in strict, accepted
        // in non-strict; the cooked value is identical either way. -----------
        "'\\07' '\\101' '\\0'",
        // a mix exercising several non-strict paths in one buffer.
        "static 0123 '\\77' yield",
    ];
    run_differential("nonstrict", &corpus, GrammarContext::AllowDiv, false);
}

/// Produce the Rust lexer dump for `src` driven through `advance_in_jsx_child`
/// (one `dump_token` line per token, including the final `eof`).
fn rust_dump_jsx_child(src: &str) -> String {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("t", src);
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowJSXIdentifier);
    let mut out = String::new();
    loop {
        let k = lex.advance_in_jsx_child().kind();
        lex.dump_token(&mut out);
        out.push('\n');
        if k == TokenKind::eof {
            break;
        }
    }
    out
}

#[test]
fn differential_jsx_child() {
    let corpus = [
        // jsx-child corpus (driven with --jsx-child on both sides).
        "hello world{",
        "a&amp;b&lt;c{",          // named entities
        "x&#65;&#x42;y<",         // decimal + hex entities
        "text\u{4e2d}more{",      // unicode jsx text -> WTF-8
        "line1\nline2<",          // newlines in jsx text
        // a bare `&` that is not a valid entity stays literal.
        "a&notanentity b{",
        // delimiters back-to-back and at the very start.
        "{<",
        "<{",
        // entity immediately followed by a delimiter.
        "&amp;<",
        // plain text to EOF (no delimiter).
        "just text",
    ];

    let bin = js_lexer_dump_bin();
    if !bin.exists() {
        if std::env::var_os("REQUIRE_DIFFERENTIAL").is_some() {
            panic!("REQUIRE_DIFFERENTIAL=1 but js-lexer-dump not built at {bin:?}");
        }
        eprintln!("skip: js-lexer-dump not built at {bin:?}");
        return;
    }

    let mut compared = 0usize;
    for &src in &corpus {
        let cpp = cpp_dump_flags(&bin, src, &["--context=jsx", "--jsx-child"])
            .expect("js-lexer-dump exists but failed to run");
        assert_eq!(rust_dump_jsx_child(src), cpp, "mismatch for {src:?}");
        compared += 1;
    }
    eprintln!("differential[jsx-child] compared {compared} corpus entries");
}

/// Run the differential over `corpus` under `ctx`, in strict or non-strict mode.
/// The skip is all-or-nothing: if the binary is absent we skip cleanly; if it is
/// present we MUST run every corpus assertion (no early return can bypass an
/// assertion). Set `REQUIRE_DIFFERENTIAL=1` to turn a missing binary into a hard
/// failure, so CI can be configured to require the differential to actually run.
fn run_differential(label: &str, corpus: &[&str], ctx: GrammarContext, strict: bool) {
    let bin = js_lexer_dump_bin();
    if !bin.exists() {
        if std::env::var_os("REQUIRE_DIFFERENTIAL").is_some() {
            panic!("REQUIRE_DIFFERENTIAL=1 but js-lexer-dump not built at {bin:?}");
        }
        eprintln!("skip: js-lexer-dump not built at {bin:?}");
        return;
    }

    // Build the C++ flag list: the grammar context, plus `--non-strict` when the
    // Rust side is driven non-strict, so both lexers are configured identically.
    let mut flags = vec![context_flag(ctx)];
    if !strict {
        flags.push("--non-strict");
    }

    let mut compared = 0usize;
    for &src in corpus {
        let cpp = cpp_dump_flags(&bin, src, &flags).expect("js-lexer-dump exists but failed to run");
        assert_eq!(rust_dump(src, ctx, strict), cpp, "mismatch for {src:?}");
        compared += 1;
    }
    eprintln!("differential[{label}] compared {compared} corpus entries");
}
