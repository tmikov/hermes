/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! An LLVM-`cl`-style command-line option parser.
//!
//! Options are declared as typed handles registered against a [`CommandLine`];
//! each declaration returns an [`Opt`] that dereferences to the parsed value
//! once parsing has finished. Supported out of the box: long and short names,
//! positional arguments, list-valued options, enum options (a set of mutually
//! exclusive flags) and enum-valued options (`--opt=name`), minimum/maximum
//! occurrence counts, values shared between several options
//! ([`OptDesc::opt_value`]), help categories and hidden options. `--help` is
//! generated from the declarations.
//!
//! # Provenance
//!
//! This crate is Meta-authored and was copied from
//! `unsupported/juno/crates/command_line` in the Hermes repository. It is
//! *styled* after LLVM's `cl` library — it matches that library's
//! command-line syntax and help layout — but it is not derived from LLVM
//! source; the references to LLVM in the comments describe behavior being
//! matched.
//!
//! Three behavioral changes have been made since the copy:
//!
//! - A single leading dash is accepted as a synonym for a double dash when it
//!   matches an option's full long name (`-parse-flow` == `--parse-flow`),
//!   with single-character grouping (`-i32`, `-m 10`) as the fallback tried
//!   only when no long name matches. Aligns with LLVM `cl` and `hermesc`.
//! - [`CommandLine::parse_env_args()`] exits with status 1, not 0, on a
//!   command-line usage error. Aligns with LLVM `cl` and `hermesc`.
//! - [`OptDesc::opt_value`] sharing works. In juno the end-of-parse sweep
//!   froze the shared storage once per sharing option and the second freeze
//!   asserted, so any two options sharing a value panicked; freezing is now
//!   idempotent, and [`OptValue`] gained the two accessors needed to read a
//!   shared storage directly.
//!
//! # Example
//!
//! ```
//! use hermes_command_line::{CommandLine, CommandLineIntent, Opt, OptDesc};
//!
//! let mut cl = CommandLine::new("demo -- an example tool");
//!
//! let count = Opt::<u32>::new(
//!     &mut cl,
//!     OptDesc {
//!         long: Some("count"),
//!         short: Some("c"),
//!         desc: Some("How many times to repeat"),
//!         value_desc: Some("number"),
//!         ..Default::default()
//!     },
//! );
//! // No long or short name: a positional argument. `new_list` accumulates.
//! let files = Opt::<String>::new_list(
//!     &mut cl,
//!     OptDesc {
//!         value_desc: Some("file"),
//!         ..Default::default()
//!     },
//! );
//!
//! let args: Vec<String> = ["demo", "-c", "3", "a.js", "b.js"]
//!     .iter()
//!     .map(|s| s.to_string())
//!     .collect();
//! assert_eq!(cl.parse(&args), Ok(CommandLineIntent::Normal));
//!
//! // Values are readable only after parsing has finished.
//! assert_eq!(*count, 3);
//! assert_eq!(files.num_values(), 2);
//! assert_eq!(files[0], "a.js");
//! assert_eq!(files[1], "b.js");
//! ```

#![warn(missing_docs)]

macro_rules! cond {
    ($condition: expr, $_true: expr, $_false: expr) => {
        if $condition { $_true } else { $_false }
    };
}

mod cl;
mod opt;
mod parser;

pub use cl::CommandLine;
pub use opt::EnumDesc;
pub use opt::ExpectedValue;
pub use opt::Hidden;
pub use opt::Opt;
pub use opt::OptDesc;
pub use opt::OptHolder;
pub use opt::OptValue;
pub use opt::parse_bool;
pub use opt::parse_disallowed;
pub use parser::CommandLineIntent;
