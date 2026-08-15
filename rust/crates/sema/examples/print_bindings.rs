/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Walk a resolved AST and print every identifier with the binding it names.
//!
//! ```text
//! cargo run -p hermes-sema --example print_bindings -- file.js
//! ```
//!
//! With no argument it uses a built-in snippet that exercises several binding
//! kinds. This is the canonical use of the two crates together — parse,
//! resolve, walk, ask "what does this name mean?" — and it demonstrates the
//! two things that are otherwise easy to get wrong:
//!
//! 1. turning an atom into a string, with the generated `name_str` accessor
//!    (see [`hermes_parser::ast::node::Identifier::name_str`]);
//! 2. holding a `&GCLock` inside a [`Visitor`] without tripping over the
//!    lock's invariance — see the comment on `BindingPrinter` below.

use hermes_parser::ast::context::GCLock;
use hermes_parser::ast::node::Node;
use hermes_parser::ast::visitor::Visitor;
use hermes_parser::{parse_named, ParseFlags};
use hermes_sema::sem_context::SemContext;
use hermes_sema::{resolve_for_compile, CompileOptions};

/// Prints one line per identifier: its name, whether it declares or uses a
/// binding, and the binding's kind.
///
/// **The lifetimes are the point.** `GCLock<'ast, 'ctx>` holds a
/// `&'ctx mut Context<'ast>`, and `&mut T` is invariant in `T`, so the lock is
/// invariant in `'ast`: a `&GCLock<'static, '_>` cannot be coerced to a
/// `&GCLock<'shorter, '_>`. `with_program` hands the closure exactly a
/// `&GCLock<'static, '_>` together with nodes borrowed for a higher-ranked
/// `'gc`, so the natural first attempt — reusing the visitor's `'gc` for the
/// lock, i.e.
///
/// ```text
/// struct BindingPrinter<'gc> { gc: &'gc GCLock<'gc, 'gc>, ... }
/// impl<'gc> Visitor<'gc> for BindingPrinter<'gc> { ... }
/// ```
///
/// — fails, because it demands `'ast == 'gc` and invariance refuses to shorten
/// `'static` to get there:
///
/// ```text
/// error: lifetime may not live long enough
///    |         let mut v = BindingPrinter { gc, sem };
///    |                                      ^^ this usage requires that
///    |                                         `'1` must outlive `'2`
///    = note: the struct `GCLock<'ast, 'ctx>` is invariant over the
///            parameter `'ast`
/// ```
///
/// The working pattern is below: give the lock its **own** lifetime
/// parameters and leave them unconstrained by the visitor's `'gc`, which the
/// `impl` does by writing them as `'_`. `'gc` then does only its real job —
/// tying `node` to the tree — and never has to equal the arena's `'ast`.
struct BindingPrinter<'a, 'ast, 'ctx> {
    /// The lock, for turning atoms into strings. Its `'ast`/`'ctx` are
    /// deliberately independent of the `'gc` of the `Visitor` impl.
    gc: &'a GCLock<'ast, 'ctx>,
    /// The resolution results: which declaration each identifier names.
    sem: &'a SemContext,
    /// Rows collected during the walk; printing happens after the lock is
    /// released, though printing inside the walk would work equally well.
    rows: Vec<(String, &'static str, String)>,
}

// Note the `'_`s: the lock's lifetimes are *not* the visitor's `'gc`.
impl<'gc> Visitor<'gc> for BindingPrinter<'_, '_, '_> {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        if let Node::Identifier(id) = node {
            // The atom → string path. `name` is a `Cell<NodeLabel>`, i.e. an
            // index into the arena's atom table; `name_str` reads the cell and
            // borrows the table's bytes as UTF-8. Use `gc.bytes(id.name.get())`
            // instead when the exact (possibly WTF-8) bytes matter.
            let name = id.name_str(self.gc).to_string();

            let (role, binding) = if let Some(d) =
                self.sem.get_declaration_decl(id)
            {
                ("decl", format!("{:?}", self.sem.decl(d).kind))
            } else if id.unresolvable.get() {
                // An enclosing `eval` or `with` can capture the name at run
                // time, so the resolver refuses to commit to a declaration.
                ("use", "(unresolvable: inside eval or with)".to_string())
            } else if let Some(d) = self.sem.get_expression_decl(id) {
                ("use", format!("{:?}", self.sem.decl(d).kind))
            } else {
                // Property keys, member accesses and label names are
                // identifiers too, and none of them names a binding.
                ("-", "(not a variable reference)".to_string())
            };

            self.rows.push((name, role, binding));
        }
        // The default `visit_node` does exactly this; recursion is ours to
        // control, so a visitor can prune subtrees by not calling it.
        node.visit_children(self);
    }
}

const SOURCE: &str = r#"
class Counter {
  #n = 0;
  step(by = 1) {
    const before = this.#n;
    this.#n += by;
    return before;
  }
}

let counter = new Counter();
var total = 0;
for (const step of [1, 2, 3]) {
  total += counter.step(step);
}
console.log(total);
"#;

fn main() {
    let (name, source) = match std::env::args().nth(1) {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(s) => (path, s),
            Err(e) => {
                eprintln!("print_bindings: cannot read '{path}': {e}");
                std::process::exit(1);
            }
        },
        None => ("<builtin>".to_string(), SOURCE.to_string()),
    };

    // Step 1: parse. `ParseFlags::default()` is plain ECMAScript.
    let parsed = match parse_named(&source, &name, ParseFlags::default()) {
        Ok(parsed) => parsed,
        Err(e) => {
            // `messages()` strings are already newline-terminated.
            for m in e.messages() {
                eprint!("{m}");
            }
            std::process::exit(2);
        }
    };

    // Step 2: resolve. The compile path, so the standard globals exist and an
    // undeclared `console` comes back as `UndeclaredGlobalProperty` rather
    // than as nothing at all; `hermes_sema::resolve` is the parser path.
    let mut resolved =
        match resolve_for_compile(parsed, &CompileOptions::default()) {
            Ok(resolved) => resolved,
            Err(e) => {
                for m in e.messages() {
                    eprint!("{m}");
                }
                std::process::exit(2);
            }
        };

    // Step 3: walk. References into the arena die with the lock, so the
    // visitor collects owned `String`s and hands them back out.
    let rows = resolved.with_program(|gc, root, sem| {
        let mut printer = BindingPrinter {
            gc,
            sem,
            rows: Vec::new(),
        };
        printer.visit_node(root);
        printer.rows
    });

    println!("{}: {} identifiers", name, rows.len());
    for (name, role, binding) in rows {
        println!("  {name:<12} {role:<5} {binding}");
    }
}
