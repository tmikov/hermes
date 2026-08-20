//! Rust mirror of `tools/parse-bench/parse-bench.cpp` (default mode).
//!
//! Deliberately the *same* measurement loop as the C++ tool so the two
//! numbers are comparable: one untimed warm-up, then `--iters` timed
//! iterations, median reported as ms and MiB/s. Like the C++ tool, each timed
//! iteration includes context setup, the parse, and teardown — the C++
//! `parseOnce` destroys its `Context` and parser before the clock is read, and
//! dropping `ParsedJS` here frees the arena the same way.
//!
//! Run one file per process (the isolation rule in
//! `rust/crates/comparison/BENCH-RESULTS.md`): the port's throughput is
//! sensitive to what ran earlier in the same process.
//!
//!   cargo run --release --manifest-path rust/crates/comparison/Cargo.toml \
//!     --example port_parse_bench -- --iters=30 <file.js>

use std::hint::black_box;
use std::time::Instant;

fn main() {
    let mut iters: usize = 30;
    let mut files: Vec<String> = Vec::new();
    for a in std::env::args().skip(1) {
        match a.strip_prefix("--iters=") {
            Some(n) => iters = n.parse().expect("--iters=N"),
            None => files.push(a),
        }
    }
    if files.is_empty() {
        eprintln!("usage: port_parse_bench [--iters=N] file1.js [file2.js ...]");
        std::process::exit(1);
    }

    for path in &files {
        let src = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let bytes = src.len();

        // Warm-up, not timed — matches the C++ tool.
        let warm = hermes_parser::parse(&src, hermes_parser::ParseFlags::default());
        let ok = warm.is_ok();
        drop(warm);

        let mut times: Vec<f64> = Vec::with_capacity(iters);
        for _ in 0..iters {
            let t0 = Instant::now();
            let parsed = hermes_parser::parse(
                black_box(&src),
                hermes_parser::ParseFlags::default(),
            );
            black_box(&parsed);
            drop(parsed); // teardown inside the timed region, as in C++
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
        }

        times.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        let med = times[times.len() / 2];
        let min = times[0];
        let max = times[times.len() - 1];
        let mib = bytes as f64 / (med / 1000.0) / (1024.0 * 1024.0);
        let name = path.rsplit('/').next().unwrap_or(path);
        println!(
            "{name}  size={bytes}  median={med:.3} ms  {mib:.1} MiB/s  \
             (min={min:.3} max={max:.3}, parsed_ok={ok})"
        );
    }
}
