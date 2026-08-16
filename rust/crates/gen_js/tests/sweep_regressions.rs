/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Regression tests for the generator defects the **Tier 2 wide sweep** found.
//!
//! The sweep (`crates/tools/src/bin/gen_js_sweep.rs`) round-trips every `.js`
//! file in the C++ lit test tree — 1934 files, 1782 of them round-trippable,
//! 3564 round trips — under parse flags read out of each file's lit `RUN:`
//! line. It is a development-time tool rather than a standing test because it
//! reads `test/` at the repository root, which a published crate cannot
//! assume exists; this file is the part that *can* stand, one named test per
//! defect, each pinned to a minimal reproducer distilled from the lit file
//! that exposed it.
//!
//! Every test here asserts two things: that the round trip is now clean (the
//! same oracle the Tier 1 corpus gate uses — the ESTree dump without `"raw"`
//! and without locations, compared byte for byte), **and** what the generated
//! text actually says. The second assertion is what makes these tests fail
//! for the original reason rather than for some later, unrelated one: a
//! round-trip assertion alone would also pass if a future change made the
//! input parse differently.

use hermes_ast::dump::{ESTreeDumpMode, ESTreeRawProp, LocationDumpMode};
use hermes_gen_js::{generate, Opt, Pretty};
use hermes_parser::{ParseFlags, ParsedJS};

/// Plain JavaScript.
const PLAIN: ParseFlags = ParseFlags {
    parse_flow: false,
    parse_flow_component_syntax: false,
    parse_flow_records: false,
    parse_flow_match: false,
    parse_ts: false,
    parse_jsx: false,
    strict_mode: false,
};

/// `-parse-flow`.
const FLOW: ParseFlags = ParseFlags {
    parse_flow: true,
    ..PLAIN
};

/// The round-trip oracle: the ESTree dump with `"raw"` and locations dropped.
/// Identical to `tests/corpus.rs`'s `ast_json` — see that file's module doc
/// comment for why those two, and only those two, are normalized away.
fn ast_json(parsed: &mut ParsedJS) -> String {
    parsed.to_estree_json_with(
        true,
        ESTreeDumpMode::HideEmpty,
        LocationDumpMode::None,
        ESTreeRawProp::Exclude,
    )
}

/// Generate `parsed`'s program under `pretty`.
fn gen(parsed: &mut ParsedJS, pretty: Pretty) -> String {
    let mut out = Vec::new();
    parsed
        .with_program(|gc, root| {
            generate(
                &mut out,
                gc,
                root,
                Opt {
                    pretty,
                    ..Opt::default()
                },
            )
        })
        .expect("generation succeeds");
    String::from_utf8(out).expect("the generator emits UTF-8")
}

/// Parse `src` under `flags`, regenerate it in both [`Pretty`] modes, reparse
/// each, and require the ESTree dumps to be identical. Returns the
/// `Pretty::No` text, without its trailing newline, so the caller can assert
/// on the spelling.
///
/// Panics with the first differing dump line, the same way the corpus gate
/// reports, rather than with two multi-thousand-line blobs.
fn round_trip(src: &str, flags: ParseFlags) -> String {
    let mut parsed = hermes_parser::parse(src, flags)
        .unwrap_or_else(|e| panic!("the reproducer must parse: {src:?}: {e:?}"));
    let before = ast_json(&mut parsed);

    let mut compact = None;
    for pretty in [Pretty::Yes, Pretty::No] {
        let js = gen(&mut parsed, pretty);
        let mut reparsed = hermes_parser::parse(&js, flags).unwrap_or_else(|e| {
            panic!("regenerated source does not parse [{pretty:?}]:\n{js}\n{e:?}")
        });
        let after = ast_json(&mut reparsed);
        assert_eq!(
            first_difference(&before, &after),
            None,
            "[{pretty:?}] {src:?} regenerated as {js:?} reparses to a different AST"
        );
        if pretty == Pretty::No {
            // The generator always ends a program with a newline; it carries
            // no information for these assertions.
            compact = Some(js.trim_end_matches('\n').to_string());
        }
    }
    compact.expect("Pretty::No ran")
}

/// The first differing line of two dumps, as `(line number, original,
/// regenerated)`, or `None` when they are identical.
fn first_difference(before: &str, after: &str) -> Option<(usize, String, String)> {
    let (mut b, mut a) = (before.lines(), after.lines());
    let mut i = 0usize;
    loop {
        i += 1;
        match (b.next(), a.next()) {
            (Some(x), Some(y)) if x == y => continue,
            (Some(x), Some(y)) => return Some((i, x.trim().into(), y.trim().into())),
            (Some(x), None) => return Some((i, x.trim().into(), "<end of dump>".into())),
            (None, Some(y)) => return Some((i, "<end of dump>".into(), y.trim().into())),
            (None, None) => return None,
        }
    }
}

/// Defect 36 — a numeric literal whose value overflowed to infinity.
///
/// From `test/Parser/extreme-numbers.js`. A literal too large for `f64` is
/// not an error: it parses to `+inf`, and `Number::toString` spells that
/// `Infinity`, which is an *identifier*. `1e999` is a numeric literal whose
/// value is exactly `+inf`.
#[test]
fn numeric_literal_that_overflowed_to_infinity_prints_as_a_literal() {
    let js = round_trip("55e55555555555555555555555555555555555;", PLAIN);
    assert_eq!(js, "1e999;", "an infinite literal must not print as `Infinity`");
}

/// Defect 37 — the parser's Annex-B implicit block.
///
/// From `test/Parser/if-function.js`. `if (x) function f() {}` wraps the
/// declaration in a synthetic `BlockStatement` with `implicit: true`;
/// printing real braces makes the reparse produce an explicit block.
#[test]
fn implicit_block_around_an_if_function_declaration_prints_without_braces() {
    let js = round_trip("if (x) function f() {} else function f() {}", PLAIN);
    assert_eq!(
        js, "if(x)function f(){}else function f(){}",
        "an implicit block must print neither braces nor a run-together `else`"
    );
}

/// Defect 38 — `x as (const)`.
///
/// From `test/Parser/flow/as-const.js`. The parser folds `x as const` into
/// `AsConstExpression` only when the annotation is unparenthesized, so an
/// `AsExpression` over a `GenericTypeAnnotation` named `const` is spellable
/// exactly one way.
#[test]
fn as_expression_whose_type_is_const_keeps_its_parens() {
    let js = round_trip("x as (const);", FLOW);
    assert_eq!(
        js, "x as (const);",
        "dropping these parens turns the node into an AsConstExpression"
    );
}

/// Defect 39 — variance after `proto`/`static`, not before.
///
/// From `test/Parser/flow/proto.js` and `test/Parser/flow/static-property.js`.
/// Emitting `+proto x: T` is a hard reparse failure, not a silent divergence.
#[test]
fn object_type_property_prints_variance_after_proto_and_static() {
    let js = round_trip("declare class B { proto +x: T }", FLOW);
    assert_eq!(js, "declare class B{proto +x:T}");

    let js = round_trip("declare class C { static +foo: string }", FLOW);
    assert_eq!(js, "declare class C{static +foo:string}");
}

/// Defect 31 — a unary left operand of `**`.
///
/// From `test/hermes/bigint-binary-exponentiate.js` and its `shermes` twin.
/// ECMA-262 13.6 only allows an `UpdateExpression` there, so the parens are
/// grammar, not precedence: without them the text does not parse at all.
#[test]
fn exponentiation_keeps_the_parens_around_a_unary_left_operand() {
    let js = round_trip("print((-BigInt(2)) ** BigInt(63));", PLAIN);
    assert_eq!(js, "print(((-BigInt(2))**BigInt(63)));");
    // A prefix UpdateExpression *is* a legal left operand and must not be
    // wrapped: this is the boundary the fix must not overshoot.
    let js = round_trip("--x ** y;", PLAIN);
    assert_eq!(js, "--x**y;");
}

/// Defect 40 — a trailing elision in an array pattern.
///
/// From `test/Parser/es6/arrow-non-simple-params.js`. `n` elements print
/// `n - 1` commas, so a trailing hole disappears; `ArrayExpression` escapes
/// this only because it carries the parser's `trailingComma` flag.
#[test]
fn array_pattern_keeps_a_trailing_elision() {
    let js = round_trip("let bar = ([,,]) => {}", PLAIN);
    assert_eq!(js, "let bar=([,,])=>{};");
    // One hole, for contrast: `[,]` is a one-element pattern.
    let js = round_trip("let f = ([,]) => {}", PLAIN);
    assert_eq!(js, "let f=([,])=>{};");
}

/// Defect 33 — an array/object literal as an assignment target.
///
/// From `test/Parser/es6/reparse-array-destr.js`. `([a, b]) = t` keeps an
/// `ArrayExpression` on the left (for sema to reject); printed bare it
/// reparses as an `ArrayPattern`, silently.
#[test]
fn literal_assignment_target_keeps_its_parens() {
    let js = round_trip("([a, b]) = t;", PLAIN);
    assert_eq!(js, "([a,b])=t;");
    // The valid spelling is unaffected: a real pattern still prints bare.
    let js = round_trip("[a, b] = t;", PLAIN);
    assert_eq!(js, "[a,b]=t;");
}

/// Defect 41 — a parenthesized element inside a destructuring target.
///
/// From `test/Parser/es6/reparse-array-destr.js`. The parens are what stopped
/// the parser rewriting the element into a pattern, so they are the only
/// spelling of the resulting tree.
#[test]
fn parenthesized_pattern_element_keeps_its_parens() {
    let js = round_trip("[(a = 1)] = t;", PLAIN);
    assert_eq!(js, "[(a=1)]=t;");
    let js = round_trip("[([b])] = t;", PLAIN);
    assert_eq!(js, "[([b])]=t;");
    // The unparenthesized spellings are different trees and must stay bare.
    let js = round_trip("[a = 1] = t;", PLAIN);
    assert_eq!(js, "[a=1]=t;");
    let js = round_trip("[[b]] = t;", PLAIN);
    assert_eq!(js, "[[b]]=t;");
}
