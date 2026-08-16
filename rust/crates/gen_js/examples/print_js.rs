/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Parse a source string and print the regenerated JS in both [`Pretty`]
//! modes, to show what `Pretty::Yes` adds (indentation, readability spaces)
//! and does not (it is not a formatter — see the crate docs).
//!
//! ```text
//! cargo run -p hermes-gen-js --example print_js
//! ```

use hermes_gen_js::{to_js, Opt, Pretty};
use hermes_parser::{parse, ParseFlags};

const SOURCE: &str = "let x=1;function f(y){if(y>x){return y-x}return x-y}";

fn main() {
    let mut parsed = parse(SOURCE, ParseFlags::default()).unwrap_or_else(|e| {
        eprintln!("print_js: parse error: {e}");
        std::process::exit(1);
    });

    for pretty in [Pretty::No, Pretty::Yes] {
        let opt = Opt {
            pretty,
            ..Opt::default()
        };
        let js = to_js(&mut parsed, opt).unwrap_or_else(|e| {
            eprintln!("print_js: generation error: {e}");
            std::process::exit(1);
        });
        println!("--- Pretty::{pretty:?} ---\n{js}");
    }
}
