/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Table-driven tests for `sema::ast_eval`, ported switch-arm-for-switch-arm
//! from `lib/Sema/ASTEval.cpp` (95 lines, both functions). Every operator x
//! operand-kind combination the C++ folds is asserted to produce the exact
//! `f64` bits and node kind; every combination it declines is asserted to
//! return `None`.

use ast::context::{Context, GCLock};
use ast::node::{
    BinaryExpression, BooleanLiteral, Identifier, Node, NumericLiteral,
    UnaryExpression,
};
use ast::node_child::NodeMetadata;
use atom_table::AtomBytes;
use sema::ast_eval::{ast_fold_binary_expression, ast_fold_unary_expression};
use sema::keywords::Keywords;
use support::location::{SMLoc, SMRange, SourceId};

fn loc(offset: u32) -> SMLoc {
    SMLoc {
        source: SourceId::from_index(0),
        offset,
    }
}

fn rng(start: u32, end: u32) -> SMRange {
    SMRange {
        start: loc(start),
        end: loc(end),
    }
}

/// A `NumericLiteral` whose `range`, `debug_loc`, and `parens` are all
/// independently distinguishable, so tests can tell exactly which of the
/// operand's metadata fields the fold carries over versus overwrites (see
/// the `ast_eval` module doc: only `range` is overwritten, `debug_loc`/
/// `parens` survive from the reused-in-C++ operand).
fn num_lit<'gc>(
    gc: &'gc GCLock<'_, '_>,
    value: f64,
    range: SMRange,
    debug_loc: SMLoc,
    parens: u8,
) -> &'gc Node<'gc> {
    let md = NodeMetadata::new_with_debug(range, debug_loc);
    md.parens.set(parens);
    gc.alloc(Node::NumericLiteral(NumericLiteral::new(md, value)))
}

/// A plain numeric literal (default debug_loc/parens) for cases where the
/// carryover isn't under test.
fn simple_num<'gc>(
    gc: &'gc GCLock<'_, '_>,
    value: f64,
    range: SMRange,
) -> &'gc Node<'gc> {
    gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(range),
        value,
    )))
}

fn ident_node<'gc>(
    gc: &'gc GCLock<'_, '_>,
    name: &str,
    range: SMRange,
) -> &'gc Node<'gc> {
    gc.alloc(Node::Identifier(Identifier::new(
        NodeMetadata::new(range),
        gc.atom_bytes(name),
        None,
        false,
    )))
}

fn bool_lit<'gc>(
    gc: &'gc GCLock<'_, '_>,
    value: bool,
    range: SMRange,
) -> &'gc Node<'gc> {
    gc.alloc(Node::BooleanLiteral(BooleanLiteral::new(
        NodeMetadata::new(range),
        value,
    )))
}

fn bin<'gc>(
    op: AtomBytes,
    left: &'gc Node<'gc>,
    right: &'gc Node<'gc>,
    range: SMRange,
) -> BinaryExpression<'gc> {
    BinaryExpression::new(NodeMetadata::new(range), left, right, op)
}

fn un<'gc>(
    op: AtomBytes,
    argument: &'gc Node<'gc>,
    range: SMRange,
) -> UnaryExpression<'gc> {
    UnaryExpression::new(NodeMetadata::new(range), op, argument, true)
}

/// Extract `(value_bits, range, debug_loc, parens)` from a folded
/// `NumericLiteral`, panicking if the node isn't one (the fold must always
/// produce a `NumericLiteral` when it succeeds).
fn literal_parts<'gc>(node: &'gc Node<'gc>) -> (u64, SMRange, SMLoc, u8) {
    match node {
        Node::NumericLiteral(n) => (
            n.value.get().to_bits(),
            n.metadata.range.get(),
            n.metadata.debug_loc.get(),
            n.metadata.parens.get(),
        ),
        other => panic!("expected NumericLiteral, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Binary fold table.
// ---------------------------------------------------------------------

struct BinCase {
    name: &'static str,
    op: fn(&Keywords) -> AtomBytes,
    left: f64,
    right: f64,
    expected: f64,
}

fn bin_cases() -> Vec<BinCase> {
    vec![
        BinCase {
            name: "+",
            op: |kw| kw.ident_plus,
            left: 1.5,
            right: 2.5,
            expected: 4.0,
        },
        BinCase {
            name: "-",
            op: |kw| kw.ident_minus,
            left: 5.0,
            right: 2.0,
            expected: 3.0,
        },
        BinCase {
            name: "*",
            op: |kw| kw.ident_star,
            left: 3.0,
            right: 4.0,
            expected: 12.0,
        },
        BinCase {
            name: "/",
            op: |kw| kw.ident_slash,
            left: 7.0,
            right: 2.0,
            expected: 3.5,
        },
        BinCase {
            name: "% positive",
            op: |kw| kw.ident_percent,
            left: 7.0,
            right: 3.0,
            expected: 1.0,
        },
        // fmod's result takes the sign of the dividend.
        BinCase {
            name: "% negative dividend",
            op: |kw| kw.ident_percent,
            left: -7.0,
            right: 3.0,
            expected: -1.0,
        },
        // `&`/`^`/`|` truncate both operands to int32 first (ToInt32),
        // discarding any fractional part.
        BinCase {
            name: "&",
            op: |kw| kw.ident_amp,
            left: 6.7,
            right: 3.2,
            expected: 2.0,
        },
        BinCase {
            name: "^",
            op: |kw| kw.ident_caret,
            left: 6.0,
            right: 3.0,
            expected: 5.0,
        },
        BinCase {
            name: "|",
            op: |kw| kw.ident_pipe,
            left: 6.0,
            right: 3.0,
            expected: 7.0,
        },
        BinCase {
            name: "<<",
            op: |kw| kw.ident_less_less,
            left: 1.0,
            right: 3.0,
            expected: 8.0,
        },
        // Shift count is masked with 0x1f: 33 & 0x1f == 1.
        BinCase {
            name: "<< masked shift count",
            op: |kw| kw.ident_less_less,
            left: 1.0,
            right: 33.0,
            expected: 2.0,
        },
        // `<<` result is reinterpreted as a signed int32: bit 31 set means
        // negative.
        BinCase {
            name: "<< sign wraparound",
            op: |kw| kw.ident_less_less,
            left: 0x4000_0000 as f64,
            right: 1.0,
            expected: i32::MIN as f64,
        },
        // `>>` is arithmetic (sign-propagating).
        BinCase {
            name: ">> arithmetic",
            op: |kw| kw.ident_greater_greater,
            left: -8.0,
            right: 1.0,
            expected: -4.0,
        },
        // `>>>` is logical (zero-filling): -1 as uint32 is 2^32-1.
        BinCase {
            name: ">>> logical",
            op: |kw| kw.ident_greater_greater_greater,
            left: -1.0,
            right: 0.0,
            expected: 4294967295.0,
        },
        BinCase {
            name: ">>> logical negative shifted",
            op: |kw| kw.ident_greater_greater_greater,
            left: -8.0,
            right: 1.0,
            expected: 2147483644.0,
        },
    ]
}

#[test]
fn binary_fold_table() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);

    let left_range = rng(0, 1);
    let left_debug = loc(50);
    let right_range = rng(2, 3);
    let compound_range = rng(100, 110);

    for case in bin_cases() {
        let left = num_lit(&gc, case.left, left_range, left_debug, 1);
        let right = num_lit(&gc, case.right, right_range, loc(60), 2);
        let be = bin((case.op)(&kw), left, right, compound_range);

        let folded = ast_fold_binary_expression(&gc, &kw, &be)
            .unwrap_or_else(|| panic!("case {:?}: expected a fold", case.name));
        let (bits, range, debug_loc, parens) = literal_parts(folded);

        assert_eq!(
            bits,
            case.expected.to_bits(),
            "case {:?}: value {} bits (expected {})",
            case.name,
            f64::from_bits(bits),
            case.expected
        );
        // Range comes from the compound expression (setSourceRange).
        assert_eq!(range, compound_range, "case {:?}: range", case.name);
        // debug_loc/parens are carried over unchanged from the reused-in-C++
        // left operand, not reset to the compound's.
        assert_eq!(debug_loc, left_debug, "case {:?}: debug_loc", case.name);
        assert_eq!(parens, 1, "case {:?}: parens", case.name);
    }
}

#[test]
fn binary_fold_nan_and_negative_zero_are_bit_exact() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);
    let r = rng(0, 1);

    // 0/0 is NaN.
    let left = simple_num(&gc, 0.0, r);
    let right = simple_num(&gc, 0.0, r);
    let be = bin(kw.ident_slash, left, right, r);
    let folded = ast_fold_binary_expression(&gc, &kw, &be).unwrap();
    let (bits, ..) = literal_parts(folded);
    assert!(f64::from_bits(bits).is_nan());
    // Compare against the hardware's actual 0.0/0.0 result rather than
    // `f64::NAN` (Rust's language-level canonical NaN, which differs from
    // the runtime hardware division result): both the C++ and this fold
    // compute the division on the same IEEE-754 hardware, so the fold must
    // reproduce whatever bit pattern that division yields here (the
    // x86-64 "indefinite" QNaN, sign bit set), not just any NaN.
    // `black_box` keeps the divisor/dividend from being constant-folded by
    // rustc into its own (different) NaN convention.
    let zero = std::hint::black_box(0.0f64);
    assert_eq!(bits, (zero / zero).to_bits());

    // (-0.0) * 1.0 is -0.0, not +0.0: bit pattern must differ from 0.0.
    let left = simple_num(&gc, -0.0, r);
    let right = simple_num(&gc, 1.0, r);
    let be = bin(kw.ident_star, left, right, r);
    let folded = ast_fold_binary_expression(&gc, &kw, &be).unwrap();
    let (bits, ..) = literal_parts(folded);
    assert_eq!(bits, (-0.0f64).to_bits());
    assert_ne!(bits, 0.0f64.to_bits());
}

#[test]
fn binary_fold_declines_when_operand_is_not_numeric_literal() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);
    let r = rng(0, 1);

    let num = simple_num(&gc, 1.0, r);
    let id = ident_node(&gc, "x", r);
    let boolean = bool_lit(&gc, true, r);

    // Left operand not numeric (identifier).
    let be = bin(kw.ident_plus, id, num, r);
    assert!(ast_fold_binary_expression(&gc, &kw, &be).is_none());

    // Right operand not numeric (boolean literal).
    let be = bin(kw.ident_plus, num, boolean, r);
    assert!(ast_fold_binary_expression(&gc, &kw, &be).is_none());

    // Neither operand numeric.
    let be = bin(kw.ident_plus, id, boolean, r);
    assert!(ast_fold_binary_expression(&gc, &kw, &be).is_none());
}

#[test]
fn binary_fold_declines_unsupported_operators() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);
    let r = rng(0, 1);
    let left = simple_num(&gc, 1.0, r);
    let right = simple_num(&gc, 2.0, r);

    // Comparison, equality, `in`/`instanceof`, and exponentiation are all
    // absent from `astFoldBinaryExpression`'s if-chain, so every one of
    // them falls through to `return false` regardless of operand kind.
    let declined_ops = [
        "==",
        "!=",
        "===",
        "!==",
        "<",
        "<=",
        ">",
        ">=",
        "in",
        "instanceof",
        "**",
    ];
    for op_str in declined_ops {
        let op = gc.atom_bytes(op_str);
        let be = bin(op, left, right, r);
        assert!(
            ast_fold_binary_expression(&gc, &kw, &be).is_none(),
            "operator {op_str:?} should decline"
        );
    }
}

// ---------------------------------------------------------------------
// Unary fold table.
// ---------------------------------------------------------------------

struct UnCase {
    name: &'static str,
    op: fn(&Keywords) -> AtomBytes,
    val: f64,
    expected: f64,
}

fn un_cases() -> Vec<UnCase> {
    vec![
        UnCase {
            name: "unary + is identity",
            op: |kw| kw.ident_plus,
            val: 5.0,
            expected: 5.0,
        },
        UnCase {
            name: "unary - negates",
            op: |kw| kw.ident_minus,
            val: 5.0,
            expected: -5.0,
        },
        UnCase {
            name: "unary ~ bitwise-not",
            op: |kw| kw.ident_tilde,
            val: 5.0,
            expected: -6.0,
        },
        UnCase {
            name: "unary ~ of -1 is 0",
            op: |kw| kw.ident_tilde,
            val: -1.0,
            expected: 0.0,
        },
        // `~` truncates to int32 (ToInt32) first, discarding the fraction.
        UnCase {
            name: "unary ~ truncates fraction",
            op: |kw| kw.ident_tilde,
            val: 0.5,
            expected: -1.0,
        },
    ]
}

#[test]
fn unary_fold_table() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);

    let arg_range = rng(0, 1);
    let arg_debug = loc(50);
    let compound_range = rng(100, 110);

    for case in un_cases() {
        let arg = num_lit(&gc, case.val, arg_range, arg_debug, 1);
        let ue = un((case.op)(&kw), arg, compound_range);

        let folded = ast_fold_unary_expression(&gc, &kw, &ue)
            .unwrap_or_else(|| panic!("case {:?}: expected a fold", case.name));
        let (bits, range, debug_loc, parens) = literal_parts(folded);

        assert_eq!(
            bits,
            case.expected.to_bits(),
            "case {:?}: value {} bits (expected {})",
            case.name,
            f64::from_bits(bits),
            case.expected
        );
        assert_eq!(range, compound_range, "case {:?}: range", case.name);
        assert_eq!(debug_loc, arg_debug, "case {:?}: debug_loc", case.name);
        assert_eq!(parens, 1, "case {:?}: parens", case.name);
    }
}

#[test]
fn unary_fold_negative_zero_is_bit_exact() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);
    let r = rng(0, 1);

    // -(0.0) is -0.0.
    let arg = simple_num(&gc, 0.0, r);
    let ue = un(kw.ident_minus, arg, r);
    let folded = ast_fold_unary_expression(&gc, &kw, &ue).unwrap();
    let (bits, ..) = literal_parts(folded);
    assert_eq!(bits, (-0.0f64).to_bits());

    // -(-0.0) is +0.0.
    let arg = simple_num(&gc, -0.0, r);
    let ue = un(kw.ident_minus, arg, r);
    let folded = ast_fold_unary_expression(&gc, &kw, &ue).unwrap();
    let (bits, ..) = literal_parts(folded);
    assert_eq!(bits, 0.0f64.to_bits());

    // Unary + is an identity fold: -0.0 stays -0.0.
    let arg = simple_num(&gc, -0.0, r);
    let ue = un(kw.ident_plus, arg, r);
    let folded = ast_fold_unary_expression(&gc, &kw, &ue).unwrap();
    let (bits, ..) = literal_parts(folded);
    assert_eq!(bits, (-0.0f64).to_bits());
}

#[test]
fn unary_fold_declines_when_argument_is_not_numeric_literal() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);
    let r = rng(0, 1);

    let id = ident_node(&gc, "x", r);
    let ue = un(kw.ident_minus, id, r);
    assert!(ast_fold_unary_expression(&gc, &kw, &ue).is_none());

    let boolean = bool_lit(&gc, true, r);
    let ue = un(kw.ident_minus, boolean, r);
    assert!(ast_fold_unary_expression(&gc, &kw, &ue).is_none());
}

#[test]
fn unary_fold_declines_unsupported_operators() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);
    let r = rng(0, 1);
    let num = simple_num(&gc, 1.0, r);

    // `!`, `typeof`, `void`, `delete` are all absent from
    // `astFoldUnaryExpression`'s if-chain.
    for op_str in ["!", "typeof", "void", "delete"] {
        let op = gc.atom_bytes(op_str);
        let ue = un(op, num, r);
        assert!(
            ast_fold_unary_expression(&gc, &kw, &ue).is_none(),
            "operator {op_str:?} should decline"
        );
    }
}
