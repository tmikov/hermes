/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Tests for [`OptDesc::opt_value`] — several registered options sharing one
//! [`OptValue`] storage. Exercised through the public API only.

use std::rc::Rc;

use hermes_command_line::{CommandLine, CommandLineIntent, Opt, OptDesc, OptValue};

fn argv(s: &str) -> Vec<String> {
    s.split_whitespace().map(|s| s.to_string()).collect()
}

/// Two list options sharing one storage accumulate into it in command-line
/// order, and every handle onto that storage reads the combined result.
#[test]
fn shared_list_accumulates_from_both_options() {
    let mut cl = CommandLine::new("t");
    let shared: Rc<OptValue<String>> = Rc::new(Default::default());

    let define = Opt::<String>::new_list(
        &mut cl,
        OptDesc {
            short: Some("D"),
            opt_value: Some(shared.clone()),
            ..Default::default()
        },
    );
    let undefine = Opt::<String>::new_list(
        &mut cl,
        OptDesc {
            short: Some("U"),
            opt_value: Some(shared.clone()),
            ..Default::default()
        },
    );

    assert_eq!(cl.parse(&argv("t -D A -U B -D C")), Ok(CommandLineIntent::Normal));

    let expected = ["A".to_string(), "B".to_string(), "C".to_string()];
    // Readable through either option handle...
    assert_eq!(define.values().as_slice(), expected);
    assert_eq!(undefine.values().as_slice(), expected);
    // ...and through the shared storage the caller kept.
    assert_eq!(shared.values().as_slice(), expected);
    assert_eq!(shared.value(), "A");

    // Occurrence counts stay per-option even though the storage is shared.
    assert_eq!(define.occurrences(), 2);
    assert_eq!(undefine.occurrences(), 1);
}

/// Making `finish()` idempotent must not weaken the other guard: storing into
/// an [`OptValue`] after it has been frozen is still an error. Reachable by
/// sharing one storage across two [`CommandLine`]s and registering against the
/// second after the first has been parsed.
#[test]
#[should_panic(expected = "value cannot be modified after parsing")]
fn storing_after_parse_still_panics() {
    let shared: Rc<OptValue<u32>> = Rc::new(Default::default());

    let mut first = CommandLine::new("t");
    Opt::<u32>::new(
        &mut first,
        OptDesc {
            long: Some("count"),
            opt_value: Some(shared.clone()),
            ..Default::default()
        },
    );
    assert_eq!(
        first.parse(&argv("t --count=1")),
        Ok(CommandLineIntent::Normal)
    );

    let mut second = CommandLine::new("t");
    Opt::<u32>::new(
        &mut second,
        OptDesc {
            long: Some("count"),
            opt_value: Some(shared.clone()),
            ..Default::default()
        },
    );
}

/// The LLVM `CLFlag` pattern: a positive and a negative spelling of the same
/// boolean sharing one storage, so the last occurrence on the command line
/// wins and neither spelling needs a separate merge step.
#[test]
fn shared_flag_last_occurrence_wins() {
    fn parse(args: &str) -> bool {
        let mut cl = CommandLine::new("t");
        let shared: Rc<OptValue<bool>> = Rc::new(Default::default());

        let yes = Opt::<bool>::new_flag(
            &mut cl,
            OptDesc {
                long: Some("fstd-globals"),
                init: Some(true),
                def_value: Some(true),
                opt_value: Some(shared.clone()),
                ..Default::default()
            },
        );
        Opt::<bool>::new_flag(
            &mut cl,
            OptDesc {
                long: Some("fno-std-globals"),
                init: Some(true),
                def_value: Some(false),
                opt_value: Some(shared.clone()),
                ..Default::default()
            },
        );

        assert_eq!(cl.parse(&argv(args)), Ok(CommandLineIntent::Normal));
        // Both handles and the shared storage agree.
        assert_eq!(*yes, *shared.value());
        *shared.value()
    }

    // Neither given: the shared init value.
    assert!(parse("t"));
    assert!(!parse("t --fno-std-globals"));
    assert!(parse("t --fstd-globals"));
    // Last one on the command line wins, in both orders.
    assert!(parse("t --fno-std-globals --fstd-globals"));
    assert!(!parse("t --fstd-globals --fno-std-globals"));
}
