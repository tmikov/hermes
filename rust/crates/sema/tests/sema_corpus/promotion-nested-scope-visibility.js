/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// S3 T2 battery: nested-scope visibility of a `let` blocker
// (`ScopedFunctionPromoter::processDeclarations`, cpp:160-244, and its
// `bindingTable_` scoping via `visitScope`, cpp:141-145). The promoter's
// binding table follows the SAME push/pop discipline as the AST's lexical
// scopes, so a `let` blocks a candidate found ANYWHERE in its descendant
// scopes (however deeply nested), but stops applying the moment its own
// block exits — a candidate in a later SIBLING block is unaffected.
//
// `blockedByOuterLet`: `let x` lives in the function's own top-level block;
// the candidate is TWO scopes deeper (an extra nested block was added on
// purpose so this isn't just `promotion-blocked-by-let.js`'s direct-sibling
// shape) and is still blocked — `return x` resolves to the `Let` decl, and
// the function keeps its local, non-promoted `ScopedFunction` decl.
//
// `notBlockedBySiblingLet`: `let y` is confined to its own block, which
// closes (popping the promoter's blocker AND the resolver's binding-table
// entry) before the sibling block with the candidate opens — `y` promotes
// normally and `return y` resolves to the promoted `Var` decl.

function blockedByOuterLet() {
  let x;
  {
    {
      function x() {}
    }
  }
  return x;
}

function notBlockedBySiblingLet() {
  {
    let y;
  }
  {
    function y() {}
  }
  return y;
}
