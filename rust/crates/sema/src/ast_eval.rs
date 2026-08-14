/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::sema::astFoldBinaryExpression` /
//! `astFoldUnaryExpression` (`lib/Sema/ASTEval.h`, `lib/Sema/ASTEval.cpp`,
//! 95 lines total — read in full). Untyped constant folding: fold a binary
//! or unary expression whose operand(s) are already `NumericLiteral`s into a
//! single `NumericLiteral`, computing the exact operations the C++ performs
//! and nothing else.
//!
//! **Stability: advanced / port-internal.** This module is `pub` because the
//! port's own tests (`tests/ast_eval.rs`) drive it directly, not because it
//! is a settled API; on the compile path the resolver calls it for you. It
//! may change, or be demoted to `pub(crate)`, in a 0.x release. The stable
//! surface is the crate-root façade plus [`mod@crate::resolve`],
//! [`crate::sem_context`] and [`crate::ids`] — see the crate doc.
//!
//! ## Node identity and location handling
//!
//! C++ *reuses* the operand's already-allocated literal node: it mutates
//! `leftNum`/`num` in place (new `_value`, `setSourceRange(BE|UE's range)`)
//! and writes that same pointer through `*ppNode`
//! (ASTEval.cpp:58-63,87-92). Crucially `setSourceRange` only overwrites the
//! source range — `debugLoc_` and `parens_` are untouched, so they survive
//! from the reused literal's own parse (`ESTree.h:89-92` vs `:107-111`,
//! `:78-83`).
//!
//! This port's AST is immutable in its structural fields (see
//! `resolver/mod.rs`'s module doc: "a rebuilt node is a new allocation with
//! a fresh `NodeId`"), so a fold here always allocates a fresh
//! `NumericLiteral` via `gc.alloc` rather than mutating the operand node.
//! To reproduce the same *observable* result — range taken from the
//! compound expression, `debug_loc`/`parens` carried over unchanged from the
//! reused-in-C++ operand — the fresh node's metadata is built from the
//! operand's metadata (`debug_loc`, `parens` copied as-is) with only `range`
//! overridden to the compound expression's own range.
//!
//! ## Fold table
//!
//! Binary (`astFoldBinaryExpression`, both operands must already be
//! `NumericLiteral`, else declines unconditionally regardless of operator):
//!
//! | operator | result (`double`)                                          |
//! |----------|-------------------------------------------------------------|
//! | `+`      | `left + right`                                               |
//! | `-`      | `left - right`                                               |
//! | `*`      | `left * right`                                               |
//! | `/`      | `left / right`                                               |
//! | `%`      | `fmod(left, right)`                                          |
//! | `&`      | `ToInt32(left) & ToInt32(right)`                             |
//! | `^`      | `ToInt32(left) ^ ToInt32(right)`                             |
//! | `\|`     | `ToInt32(left) \| ToInt32(right)`                            |
//! | `<<`     | `(int32)(ToUInt32(left) << (ToUInt32(right) & 0x1f))`        |
//! | `>>`     | `ToInt32(left) >> (ToUInt32(right) & 0x1f)` (arithmetic)     |
//! | `>>>`    | `ToUInt32(left) >> (ToUInt32(right) & 0x1f)` (logical)       |
//! | other    | declines (`None`)                                            |
//!
//! Unary (`astFoldUnaryExpression`, the argument must already be a
//! `NumericLiteral`, else declines unconditionally regardless of operator):
//!
//! | operator | result (`double`)      |
//! |----------|------------------------|
//! | `+`      | `val`                  |
//! | `-`      | `-val`                 |
//! | `~`      | `~ToInt32(val)`        |
//! | other    | declines (`None`)      |
//!
//! `ToInt32`/`ToUInt32` are ES5.1 9.5/9.6, ported below as
//! `truncate_to_i32`/`truncate_to_u32` from
//! `include/hermes/Support/Conversions.h:37-80` and
//! `lib/Support/Conversions.cpp:147-178` (`truncateToInt32`/
//! `truncateToInt32SlowPath`/`truncateToUInt32`) — pure bit manipulation on
//! the IEEE-754 representation, so it is exact for every double (including
//! NaN/Infinity, which always truncate to 0) with no floating-point
//! rounding involved.

use hermes_ast::context::GCLock;
use hermes_ast::node::{BinaryExpression, Node, NumericLiteral, UnaryExpression};
use hermes_ast::node_child::NodeMetadata;
use hermes_support::location::SMRange;

use crate::keywords::Keywords;

/// Port of `hermes::truncateToInt32` / `truncateToInt32SlowPath`
/// (`include/hermes/Support/Conversions.h:37-80`,
/// `lib/Support/Conversions.cpp:147-178`): ES5.1 9.5 ToInt32. Zero and
/// denormals (biased exponent 0) convert to 0 via the `exp_field == 0`
/// early-out below; NaN and +/-Infinity (biased exponent 0x7FF, the
/// maximum) fall through to the general path but always convert to 0 there
/// too, because their debiased `exp` ends up far larger than 31. The rest
/// of the numbers are converted to a (conceptually) infinite-width integer
/// and the low 32 bits of that integer are returned.
///
/// This only ports the "slow path": the C++ fast paths
/// (`HERMES_BUILTIN_CONSTANT_P`/`sh_tryfast_f64_to_i64`) are pure perf
/// shortcuts documented to agree with the slow path on every input, so
/// porting only the slow path is bit-exact for all inputs.
fn truncate_to_i32(d: f64) -> i32 {
    let bits = d.to_bits();
    let exp_field = ((bits >> 52) & 0x7FF) as i32;
    // Denormalized exponent (0): bail out early. NaN/Infinity are handled
    // below by falling into the `exp > 31` branch instead (their biased
    // exponent is 0x7FF, the maximum, so after debiasing `exp` is always
    // far larger than 31).
    if exp_field == 0 {
        return 0;
    }
    // A negative sign is turned into 2, a positive into 0; subtracting from
    // 1 gives us what we need. Matches the C++ bit trick exactly: shifting
    // the full 64-bit pattern right by 62 (arithmetic, i.e. sign-extending)
    // leaves only the sign bit's influence after `& 2`.
    let sign: i64 = 1 - (((bits as i64) >> 62) & 2);
    let mut m: u64 = bits & 0x000F_FFFF_FFFF_FFFF;

    // Subtract the IEEE bias (1023). Additionally, move the decimal point
    // to the right of the mantissa by further decreasing the exponent by
    // 52.
    let exp = exp_field - (1023 + 52);
    // Add the implied leading 1 bit.
    m |= 1u64 << 52;

    if exp >= 0 {
        // Check if the shift would push all bits out. Additionally this
        // catches Infinity and NaN (whose debiased `exp` is always > 31).
        if exp <= 31 {
            let shifted = (m << (exp as u32)) as i64;
            sign.wrapping_mul(shifted) as i32
        } else {
            0
        }
    } else {
        // Check if the shift would push out the entire mantissa.
        if exp > -53 {
            let shifted = (m >> ((-exp) as u32)) as i64;
            sign.wrapping_mul(shifted) as i32
        } else {
            0
        }
    }
}

/// Port of `hermes::truncateToUInt32` (`include/hermes/Support/
/// Conversions.h:78-80`): ES5.1 9.6 ToUInt32, same bit pattern as
/// [`truncate_to_i32`].
fn truncate_to_u32(d: f64) -> u32 {
    truncate_to_i32(d) as u32
}

/// Build the metadata for a folded literal: `range` is the compound
/// expression's own range (port of `leftNum->setSourceRange(BE->
/// getSourceRange())` / `num->setSourceRange(UE->getSourceRange())`,
/// ASTEval.cpp:61,90), while `debug_loc`/`parens` are copied unchanged from
/// the reused operand's metadata (`setSourceRange` only ever touches
/// `sourceRange_`, ESTree.h:89-91).
fn folded_metadata<'gc>(
    range: SMRange,
    operand_metadata: &NodeMetadata<'gc>,
) -> NodeMetadata<'gc> {
    let md =
        NodeMetadata::new_with_debug(range, operand_metadata.debug_loc.get());
    md.parens.set(operand_metadata.parens.get());
    md
}

/// Evaluate a binary expression if possible and fold it into a single
/// literal node. Port of `hermes::sema::astFoldBinaryExpression`
/// (`lib/Sema/ASTEval.cpp:15-64`).
///
/// \return `Some(folded_literal)` where the C++ returns `true` and writes
/// `*ppNode`; `None` where the C++ returns `false` (either operand isn't a
/// `NumericLiteral`, or the operator isn't one of the eleven folded ones).
pub fn ast_fold_binary_expression<'gc>(
    gc: &'gc GCLock<'_, '_>,
    kw: &Keywords,
    be: &BinaryExpression<'gc>,
) -> Option<&'gc Node<'gc>> {
    // For now only fold numeric constants.
    let Node::NumericLiteral(left_num) = be.left else {
        return None;
    };
    let Node::NumericLiteral(right_num) = be.right else {
        return None;
    };
    let left = left_num.value.get();
    let right = right_num.value.get();

    // Check for common operators.
    let op = be.operator.get();
    let res: f64 = if op == kw.ident_plus {
        left + right
    } else if op == kw.ident_minus {
        left - right
    } else if op == kw.ident_star {
        left * right
    } else if op == kw.ident_slash {
        left / right
    } else if op == kw.ident_percent {
        left % right
    } else if op == kw.ident_amp {
        (truncate_to_i32(left) & truncate_to_i32(right)) as f64
    } else if op == kw.ident_caret {
        (truncate_to_i32(left) ^ truncate_to_i32(right)) as f64
    } else if op == kw.ident_pipe {
        (truncate_to_i32(left) | truncate_to_i32(right)) as f64
    } else if op == kw.ident_less_less {
        let shifted =
            truncate_to_u32(left).wrapping_shl(truncate_to_u32(right) & 0x1f);
        (shifted as i32) as f64
    } else if op == kw.ident_greater_greater {
        (truncate_to_i32(left) >> (truncate_to_u32(right) & 0x1f)) as f64
    } else if op == kw.ident_greater_greater_greater {
        (truncate_to_u32(left) >> (truncate_to_u32(right) & 0x1f)) as f64
    } else {
        return None;
    };

    // Reuse the left node's debug_loc/parens; range comes from `be`.
    let md = folded_metadata(be.metadata.range.get(), &left_num.metadata);
    Some(gc.alloc(Node::NumericLiteral(NumericLiteral::new(md, res))))
}

/// Evaluate a unary expression if possible and fold it into a single
/// literal node. Port of `hermes::sema::astFoldUnaryExpression`
/// (`lib/Sema/ASTEval.cpp:66-93`).
///
/// \return `Some(folded_literal)` where the C++ returns `true` and writes
/// `*ppNode`; `None` where the C++ returns `false` (the argument isn't a
/// `NumericLiteral`, or the operator isn't one of the three folded ones).
pub fn ast_fold_unary_expression<'gc>(
    gc: &'gc GCLock<'_, '_>,
    kw: &Keywords,
    ue: &UnaryExpression<'gc>,
) -> Option<&'gc Node<'gc>> {
    // For now only fold numeric constants.
    let Node::NumericLiteral(num) = ue.argument else {
        return None;
    };
    let val = num.value.get();

    // Check for common operators.
    let op = ue.operator.get();
    let res: f64 = if op == kw.ident_plus {
        val
    } else if op == kw.ident_minus {
        -val
    } else if op == kw.ident_tilde {
        (!truncate_to_i32(val)) as f64
    } else {
        return None;
    };

    // Reuse the argument node's debug_loc/parens; range comes from `ue`.
    let md = folded_metadata(ue.metadata.range.get(), &num.metadata);
    Some(gc.alloc(Node::NumericLiteral(NumericLiteral::new(md, res))))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Sanity checks for the ToInt32/ToUInt32 port against well-known
    // ECMAScript conversion results, independent of the fold tests in
    // `tests/ast_eval.rs` (which exercise it only indirectly).
    #[test]
    fn truncate_to_i32_known_values() {
        assert_eq!(truncate_to_i32(0.0), 0);
        assert_eq!(truncate_to_i32(-0.0), 0);
        assert_eq!(truncate_to_i32(f64::NAN), 0);
        assert_eq!(truncate_to_i32(f64::INFINITY), 0);
        assert_eq!(truncate_to_i32(f64::NEG_INFINITY), 0);
        assert_eq!(truncate_to_i32(3.7), 3);
        assert_eq!(truncate_to_i32(-3.7), -3);
        assert_eq!(truncate_to_i32(4294967297.0), 1); // 2^32 + 1
        assert_eq!(truncate_to_i32(-1.0), -1);
        assert_eq!(truncate_to_i32(2147483648.0), i32::MIN); // 2^31
        assert_eq!(truncate_to_i32(4294967295.0), -1); // 2^32 - 1
        assert_eq!(truncate_to_i32(1e300), 0);
    }

    #[test]
    fn truncate_to_u32_known_values() {
        assert_eq!(truncate_to_u32(-1.0), u32::MAX);
        assert_eq!(truncate_to_u32(4294967295.0), u32::MAX);
        assert_eq!(truncate_to_u32(0.0), 0);
        assert_eq!(truncate_to_u32(f64::NAN), 0);
    }
}
