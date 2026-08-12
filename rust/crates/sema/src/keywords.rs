/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::Keywords` (`include/hermes/AST/Context.h:168`,
//! `include/hermes/AST/Keywords.def`): a struct of pre-interned identifier
//! atoms for strings sema compares against often (`"arguments"`, `"eval"`,
//! `"use strict"`, operator spellings like `"+"`, …), so those comparisons
//! are atom equality rather than byte comparison.
//!
//! `Keywords.def` currently has 133 `HERMES_KEYWORD` entries (not 136); see
//! [`Keywords::COUNT`] and the accompanying test. All entries are
//! unconditional here: the `#if HERMES_PARSE_FLOW` / `#if HERMES_PARSE_TS`
//! guards in the .def only strip entries for the mobile build
//! (`include/hermes/AST/Config.h`), and the non-mobile default — the one
//! this port targets — enables both, so every entry is always present.
//!
//! Field type: `hermes_atom_table::AtomBytes`, the same type as `hermes_ast::node_child`'s
//! `NodeLabel`/`NodeString` (the type `Identifier.name` and directive
//! prologue strings actually store), interned the same way the parser
//! interns identifier atoms — via `GCLock::atom_bytes` — so that
//! `directive == kw.ident_use_strict`-style atom comparisons need no
//! conversion.

use hermes_ast::context::GCLock;
use hermes_atom_table::AtomBytes;

/// Declares [`Keywords`] with one `pub $field: AtomBytes` per entry, and
/// `Keywords::new` which interns every string via `GCLock::atom_bytes`. The
/// list is written once here and used to generate both the struct and the
/// constructor, mirroring `Keywords.def`'s `HERMES_KEYWORD(name, string)`
/// list used the same way on the C++ side.
macro_rules! declare_keywords {
    ($(($field:ident, $string:literal),)*) => {
        /// Convenient storage of "keyword" identifier atoms used by sema.
        /// Port of `hermes::Keywords` (AST/Context.h:168, Keywords.def).
        pub struct Keywords {
            $(pub $field: AtomBytes,)*
        }

        impl Keywords {
            /// Interns every keyword string into `gc`'s atom table.
            pub fn new(gc: &GCLock) -> Keywords {
                Keywords {
                    $($field: gc.atom_bytes($string),)*
                }
            }

            /// Number of `HERMES_KEYWORD` entries transcribed from
            /// `Keywords.def` (see the module doc comment: currently 133,
            /// not the nominal 136).
            pub const COUNT: usize = [$(stringify!($field)),*].len();
        }
    };
}

declare_keywords! {
    (ident_arguments, "arguments"),
    (ident_eval, "eval"),
    (ident_delete, "delete"),
    (ident_this, "this"),
    (ident_use_strict, "use strict"),
    (ident_show_source, "show source"),
    (ident_hide_source, "hide source"),
    (ident_sensitive, "sensitive"),
    (ident_inline, "inline"),
    (ident_no_inline, "noinline"),
    (ident_builtin, "builtin"),
    (ident_var, "var"),
    (ident_let, "let"),
    (ident_const, "const"),
    (ident_await, "await"),
    (ident_using, "using"),
    (ident_await_using, "await using"),
    (ident_method, "method"),
    (ident_get, "get"),
    (ident_set, "set"),
    (ident_bang, "!"),
    (ident_equal, "="),
    (ident_plus, "+"),
    (ident_plus_plus, "++"),
    (ident_minus, "-"),
    (ident_minus_minus, "--"),
    (ident_star, "*"),
    (ident_slash, "/"),
    (ident_percent, "%"),
    (ident_amp, "&"),
    (ident_caret, "^"),
    (ident_pipe, "|"),
    (ident_less_less, "<<"),
    (ident_greater_greater, ">>"),
    (ident_greater_greater_greater, ">>>"),
    (ident_tilde, "~"),
    (ident_assign, "="),
    (ident_logical_or, "||"),
    (ident_logical_and, "&&"),
    (ident_nullish_coalesce, "??"),
    (ident_new, "new"),
    (ident_target, "target"),
    (ident_typeof, "typeof"),
    (ident_constructor, "constructor"),
    (ident_length, "length"),
    (ident_push, "push"),
    (ident_prototype, "prototype"),
    (ident_underscore_proto, "__proto__"),
    (ident_undefined, "undefined"),
    (ident_infinity, "Infinity"),
    (ident_na_n, "NaN"),
    (ident_sh_builtin, "$SHBuiltin"),
    (ident_call, "call"),
    (ident_extern_c, "extern_c"),
    (ident_init, "init"),
    (ident_in, "in"),
    (ident_c_null, "c_null"),
    (ident_c_native_runtime, "c_native_runtime"),
    (ident_priv_fast_array_push, "?fastArrayPush"),
    (ident_fast_array_pop, "fastArrayPop"),
    (ident_fast_array_length, "fastArrayLength"),
    (ident_hermes, "Hermes"),
    (ident_final, "final"),
    (ident_overload, "overload"),
    (ident_array, "array"),
    (ident_decorate, "decorate"),
    (ident_module_factory, "moduleFactory"),
    (ident_export, "export"),
    (ident_import, "import"),
    (ident_object, "object"),
    (ident_string, "string"),
    (ident_number, "number"),
    (ident_boolean, "boolean"),
    (ident_symbol, "symbol"),
    (ident_bigint, "bigint"),
    (ident_function, "function"),
    (ident_empty_string, ""),
    (ident_of, "of"),
    (ident_from, "from"),
    (ident_as, "as"),
    (ident_implements, "implements"),
    (ident_interface, "interface"),
    (ident_package, "package"),
    (ident_private, "private"),
    (ident_protected, "protected"),
    (ident_public, "public"),
    (ident_static, "static"),
    (ident_yield, "yield"),
    (ident_meta, "meta"),
    (ident_value, "value"),
    (ident_type, "type"),
    (ident_async, "async"),
    (ident_assert, "assert"),
    (ident_use_static_builtin, "use static builtin"),
    (ident_keyof, "keyof"),
    (ident_declare, "declare"),
    (ident_proto, "proto"),
    (ident_opaque, "opaque"),
    (ident_flow_plus, "plus"),
    (ident_flow_minus, "minus"),
    (ident_module, "module"),
    (ident_exports, "exports"),
    (ident_es, "ES"),
    (ident_common_js, "CommonJS"),
    (ident_mixins, "mixins"),
    (ident_any, "any"),
    (ident_mixed, "mixed"),
    (ident_empty, "empty"),
    (ident_void, "void"),
    (ident_null, "null"),
    (ident_bool, "bool"),
    (ident_mapped_type_optional, "Optional"),
    (ident_mapped_type_plus_optional, "PlusOptional"),
    (ident_mapped_type_minus_optional, "MinusOptional"),
    (ident_checks, "%checks"),
    (ident_flow_asserts, "asserts"),
    (ident_implies, "implies"),
    (ident_component, "component"),
    (ident_renders, "renders"),
    (ident_renders_maybe, "renders?"),
    (ident_renders_star, "renders*"),
    (ident_hook, "hook"),
    (ident_match, "match"),
    (ident_underscore, "_"),
    (ident_record, "record"),
    (ident_writeonly, "writeonly"),
    (ident_out, "out"),
    (ident_readonly, "readonly"),
    (ident_never, "never"),
    (ident_unknown, "unknown"),
    (ident_namespace, "namespace"),
    (ident_is, "is"),
    (ident_infer, "infer"),
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_ast::context::Context;

    #[test]
    fn count_is_133() {
        // Keywords.def has 133 HERMES_KEYWORD entries, not the nominal 136;
        // see the module doc comment.
        assert_eq!(Keywords::COUNT, 133);
    }

    #[test]
    fn ident_use_strict_round_trips_to_use_strict_bytes() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let kw = Keywords::new(&gc);
        assert_eq!(gc.bytes(kw.ident_use_strict), b"use strict");
    }

    #[test]
    fn ident_plus_is_the_plus_operator() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let kw = Keywords::new(&gc);
        assert_eq!(gc.bytes(kw.ident_plus), b"+");
    }

    #[test]
    fn same_string_interns_to_the_same_atom() {
        // ident_equal and ident_assign are both "=" in Keywords.def; the
        // atom table dedups strings, so the two fields must compare equal.
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let kw = Keywords::new(&gc);
        assert_eq!(kw.ident_equal, kw.ident_assign);
        // Interning the same bytes independently must also match a keyword
        // atom, so parser-produced identifier atoms compare directly.
        assert_eq!(gc.atom_bytes("use strict"), kw.ident_use_strict);
    }
}
