/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Guards that the committed src/node.rs is exactly what gen_nodes.py produces.
//! Skips if python3 is unavailable, unless REQUIRE_GEN=1 forces a hard failure.

use std::process::Command;

#[test]
fn committed_node_rs_matches_generator() {
    let manifest = env!("CARGO_MANIFEST_DIR"); // .../rust/crates/ast
    let script = format!("{manifest}/gen_nodes.py");
    let committed = std::fs::read_to_string(format!("{manifest}/src/node.rs"))
        .expect("read committed node.rs");

    let out = Command::new("python3")
        .arg(&script)
        .arg("--stdout")
        .output();
    let out = match out {
        Ok(o) if o.status.success() => o,
        other => {
            if std::env::var("REQUIRE_GEN").is_ok() {
                panic!("gen_nodes.py failed/absent but REQUIRE_GEN=1: {other:?}");
            }
            eprintln!(
                "skipping idempotency check (python3 unavailable). \
                 Set REQUIRE_GEN=1 to force."
            );
            return;
        }
    };
    let regenerated = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        committed,
        regenerated,
        "src/node.rs is stale — re-run `python3 rust/crates/ast/gen_nodes.py`"
    );
}
