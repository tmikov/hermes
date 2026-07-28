/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// `DebuggerStatement` joined `visit_node`'s override-free generic arm in S2 T7
// (the `CheckImplicitReturn` arm at CheckImplicitReturn.cpp:175 could not be
// tested otherwise), but no corpus file contained a `debugger;` — the S2 T8
// sweep found it was the ONLY node kind the resolver handles with zero corpus
// occurrences. This file closes that gap.
//
// `DebuggerStatement` appears nowhere in `lib/Sema/` outside that one arm: no
// `SemanticResolver::visit` override (SemanticResolver.h:200-304), no
// `DeclCollector` override, and it is an `ESTREE_NODE_0_ARGS` kind
// (ESTree.def:171). So the dump-visible claim being pinned here is a triple
// negative: the statement must appear in the AST dump, must create NO scope of
// its own, and must not disturb the enclosing scope's decls — in every
// statement position a `debugger` can occupy.
//
// `mayReachImplicitReturn` (the reason the kind was whitelisted) is NOT
// dump-visible: `SemContextDumper::printFunction` (SemContext.cpp:449-480)
// prints only `Func strict`/`loose` + scopes + decls + hoisted functions, and
// `FunctionInfo::mayReachImplicitReturn` (SemContext.h:354) is read by the
// FlowChecker and IRGen, neither of which `-dump-sema` runs. That flag's
// regression net is `tests/check_implicit_return.rs`, not this corpus.

debugger;

function f() {
  debugger;
  {
    let x = 1;
    debugger;
    return x;
  }
}

for (var i = 0; i < 1; ++i) debugger;

class C {
  static {
    debugger;
  }
  m() {
    debugger;
  }
}

try {
  debugger;
} catch (e) {
  debugger;
}
