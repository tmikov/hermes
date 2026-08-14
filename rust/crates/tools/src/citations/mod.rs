/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The `cpp:NNN` citation checker.
//!
//! `README.md` beside this file is the same story for someone who has never
//! met the convention, plus the resolution config, the workflow, and the
//! known citation debt.
//!
//! ## What a citation is
//!
//! This repository holds the C++ Hermes front end and a faithful, bug-for-bug
//! Rust port of it. Because faithfulness is the premise, the Rust sources cite
//! the exact C++ lines each piece was ported from:
//!
//! ```text
//! //! Ports `SemanticResolver::visit(ClassDeclarationNode *)` (cpp:891-907),
//! ```
//!
//! Those citations are how a reader answers "why does the Rust do this odd
//! thing?", and how a reviewer checks that a mirror is complete rather than
//! approximate.
//!
//! ## Why they need a checker
//!
//! They are line numbers into a moving file. Cherry-picking an upstream commit
//! shifts C++ lines, and every citation below the edit silently starts naming
//! the wrong code — nothing fails to compile, the comment just quietly lies.
//! That has happened three times across two plans.
//!
//! So: a snapshot records, per citation, the resolved C++ path, the cited
//! range, and a hash of that span's exact bytes, plus the C++ commit it was
//! blessed against. [`check`] re-hashes every site against the working tree
//! and names the ones whose span moved; the standing test in
//! `crates/tools/tests/citations.rs` runs it.
//!
//! ## Using it
//!
//! ```text
//! cargo run -p tools --bin citations -- check   # verify (what the test runs)
//! cargo run -p tools --bin citations -- remap   # repair what merely shifted
//! cargo run -p tools --bin citations -- bless   # re-record the current tree
//! ```
//!
//! [`remap()`] is the mechanical repair, and the first thing to reach for after
//! a cherry-pick: it moves the digits of every citation whose C++ text only
//! changed *position*, accepting a new location solely when the text there
//! still hashes to what was blessed, and declining — by name, with a reason —
//! every citation whose text changed. See that module for the safety
//! argument.
//!
//! `bless` is for after a *reviewed* change, including whatever `remap`
//! declined: it accepts whatever the citations currently point at. It does
//! not check that a citation is *right*, only that it has not drifted since a
//! human last looked.
//!
//! ## Resolving a citation to a file
//!
//! Which C++ file a citation names comes from `crates/tools/citations.toml`,
//! never from the prose of a module header — see that file's comments. A
//! citation the config cannot resolve is reported, never silently dropped.

pub mod config;
pub mod remap;
pub mod scan;
pub mod snapshot;

pub use remap::{remap, RemapReport};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use config::Config;
use hermes_support::line_index::LineIndex;
use scan::FileRef;
use snapshot::{hash_bytes, Snapshot, SnapshotSite};

/// One resolved, validated citation site.
#[derive(Debug)]
pub struct Site {
    /// Rust file, relative to `rust/`.
    pub rust: String,
    /// 1-based line of the citation's first digit.
    pub line: u32,
    /// The citation as written, whitespace collapsed to single spaces.
    pub text: String,
    /// Repo-relative path of the cited C++ file.
    pub cpp: String,
    /// First cited line, 1-based.
    pub start: u32,
    /// Last cited line, inclusive (equal to `start` if not a range).
    pub end: u32,
    /// Byte range of the start number's digits in the Rust file, for the
    /// mechanical remap (task 2).
    pub start_digits: (u32, u32),
    /// Byte range of the end number's digits, if the citation is a range.
    pub end_digits: Option<(u32, u32)>,
    /// FNV-1a 64 of the cited span, as of the working tree.
    pub hash: String,
}

impl Site {
    /// `crates/x.rs#cpp:891-907` — the key the config and the snapshot use.
    pub fn key(&self) -> String {
        format!("{}#{}", self.rust, self.text)
    }

    /// `crates/x.rs:12 cites lib/Sema/SemanticResolver.cpp:891-907`.
    pub fn describe(&self) -> String {
        self.cited().describe()
    }

    /// The part of this site a rewrite needs.
    pub fn cited(&self) -> Cited<'_> {
        Cited {
            rust: &self.rust,
            line: self.line,
            text: &self.text,
            cpp: &self.cpp,
            start: self.start,
            end: self.end,
            start_digits: self.start_digits,
            end_digits: self.end_digits,
        }
    }
}

/// A citation that resolves to a real C++ file but does not name lines that
/// exist in it.
///
/// It is a hard error for `check` and `bless` — it is in [`Scan::errors`] as
/// well — because a citation past the end of its file is exactly the breakage
/// this tool exists to catch. It is listed separately as well because
/// [`remap()`] can often repair it: the snapshot still knows what text the
/// citation named and where that text sat, and the digits' byte ranges say
/// what to rewrite. This is the shape a file that *shrank* leaves behind,
/// which is what a revert of an upstream insertion looks like.
#[derive(Debug)]
pub struct Dangling {
    /// Rust file, relative to `rust/`.
    pub rust: String,
    /// 1-based line of the citation's first digit.
    pub line: u32,
    /// The citation as written, whitespace collapsed.
    pub text: String,
    /// The C++ file it resolved to.
    pub cpp: String,
    /// First cited line, as written.
    pub start: u32,
    /// Last cited line, as written.
    pub end: u32,
    /// Byte range of the start number's digits in the Rust file.
    pub start_digits: (u32, u32),
    /// Byte range of the end number's digits, if the citation is a range.
    pub end_digits: Option<(u32, u32)>,
    /// The exact line this citation contributed to [`Scan::errors`], so a
    /// caller that handles the citation itself can drop the duplicate.
    pub message: String,
}

impl Dangling {
    /// The part of this citation a rewrite needs.
    pub fn cited(&self) -> Cited<'_> {
        Cited {
            rust: &self.rust,
            line: self.line,
            text: &self.text,
            cpp: &self.cpp,
            start: self.start,
            end: self.end,
            start_digits: self.start_digits,
            end_digits: self.end_digits,
        }
    }
}

/// Where a citation is written and what it currently says: the view [`remap()`]
/// works from, so that a resolved [`Site`] and a [`Dangling`] citation can be
/// repaired by the same code.
#[derive(Clone, Copy, Debug)]
pub struct Cited<'a> {
    /// Rust file, relative to `rust/`.
    pub rust: &'a str,
    /// 1-based line of the citation's first digit.
    pub line: u32,
    /// The citation as written, whitespace collapsed.
    pub text: &'a str,
    /// The C++ file it resolves to.
    pub cpp: &'a str,
    /// First cited line, as written.
    pub start: u32,
    /// Last cited line, as written.
    pub end: u32,
    /// Byte range of the start number's digits in the Rust file.
    pub start_digits: (u32, u32),
    /// Byte range of the end number's digits, if the citation is a range.
    pub end_digits: Option<(u32, u32)>,
}

impl Cited<'_> {
    /// `crates/x.rs#cpp:891-907` — the key the config and the snapshot use.
    pub fn key(&self) -> String {
        format!("{}#{}", self.rust, self.text)
    }

    /// `crates/x.rs:12 cites lib/Sema/SemanticResolver.cpp:891-907`.
    pub fn describe(&self) -> String {
        let range = if self.start == self.end {
            self.start.to_string()
        } else {
            format!("{}-{}", self.start, self.end)
        };
        format!("{}:{} cites {}:{}", self.rust, self.line, self.cpp, range)
    }
}

/// A citation that was deliberately not checked.
#[derive(Debug)]
pub struct Skipped {
    /// The site key.
    pub key: String,
    /// The `[unresolved]` reason from the config.
    pub reason: String,
}

/// The result of walking the Rust tree.
#[derive(Debug)]
pub struct Scan {
    /// Every resolved, in-range citation.
    pub sites: Vec<Site>,
    /// Resolved citations that name lines the file does not have. Each is in
    /// [`Scan::errors`] too — this list exists so `remap` can try to repair
    /// them; see [`Dangling`].
    pub dangling: Vec<Dangling>,
    /// Citations the config says to skip, with why.
    pub skipped: Vec<Skipped>,
    /// Citations that could not be resolved or do not name real lines. These
    /// are hard failures for both `check` and `bless`: a citation whose range
    /// runs past the end of the file is exactly the breakage this tool exists
    /// to catch, so it must never be quietly ignored.
    pub errors: Vec<String>,
}

impl Scan {
    /// Total citations found, however they were classified.
    /// A dangling citation is counted once, as an error.
    pub fn total(&self) -> usize {
        self.sites.len() + self.skipped.len() + self.errors.len()
    }
}

/// The repository root, derived from this crate's location at compile time so
/// the tool does not depend on the current directory.
pub fn repo_root() -> PathBuf {
    // <root>/rust/crates/tools -> <root>
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("crates/tools is three levels below the repo root")
        .to_path_buf()
}

/// Path of the resolution config.
pub fn config_path(root: &Path) -> PathBuf {
    root.join("rust/crates/tools/citations.toml")
}

/// Path of the blessed snapshot.
pub fn snapshot_path(root: &Path) -> PathBuf {
    root.join("rust/crates/tools/citations.snapshot.json")
}

/// Load and parse the resolution config.
pub fn load_config(root: &Path) -> Result<Config, String> {
    let path = config_path(root);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Config::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Every Rust source file the checker looks at: `crates/*/{src,tests,examples}`
/// under `rust/`, recursively, sorted for determinism.
pub fn rust_files(root: &Path) -> Result<Vec<String>, String> {
    let crates = root.join("rust/crates");
    let mut crate_dirs: Vec<PathBuf> = read_dir_sorted(&crates)?
        .into_iter()
        .filter(|p| p.is_dir())
        .collect();
    crate_dirs.sort();
    let mut out = Vec::new();
    for dir in crate_dirs {
        for sub in ["src", "tests", "examples"] {
            let d = dir.join(sub);
            if d.is_dir() {
                collect_rs(&d, &crates.parent().unwrap().to_path_buf(), &mut out)?;
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Recursively collect `*.rs` under `dir`, as paths relative to `rust/`.
fn collect_rs(dir: &Path, rust_root: &Path, out: &mut Vec<String>) -> Result<(), String> {
    for entry in read_dir_sorted(dir)? {
        if entry.is_dir() {
            collect_rs(&entry, rust_root, out)?;
        } else if entry.extension().is_some_and(|e| e == "rs") {
            let rel = entry
                .strip_prefix(rust_root)
                .map_err(|e| format!("{}: {e}", entry.display()))?;
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// `read_dir` with a sorted, owned result and a useful error message.
fn read_dir_sorted(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory {}: {e}", dir.display()))?
        .map(|e| {
            e.map(|e| e.path())
                .map_err(|e| format!("{}: {e}", dir.display()))
        })
        .collect::<Result<_, _>>()?;
    v.sort();
    Ok(v)
}

/// A C++ file's bytes plus its line index, loaded once per run.
///
/// The content need not come from the working tree — [`remap`] builds one
/// from a file as of a git commit — so this holds the bytes rather than a
/// path.
pub(crate) struct CppFile {
    text: String,
    index: LineIndex,
}

impl CppFile {
    /// Index `text` for span lookups.
    pub(crate) fn new(text: String) -> CppFile {
        let index = LineIndex::build(text.as_bytes());
        CppFile { text, index }
    }

    /// Last line that has any content, i.e. the file's line count ignoring
    /// the empty "line" that a trailing newline creates.
    pub(crate) fn last_real_line(&self) -> u32 {
        let count = self.index.line_count();
        if count > 1 && self.text.ends_with('\n') {
            count - 1
        } else {
            count
        }
    }

    /// The exact bytes of lines `start..=end`, including the last line's
    /// newline when it has one. This is what gets hashed.
    fn span_bytes(&self, start: u32, end: u32) -> &[u8] {
        let bytes = self.text.as_bytes();
        let from = self.index.line_start(start) as usize;
        let to = if end < self.index.line_count() {
            self.index.line_start(end + 1) as usize
        } else {
            bytes.len()
        };
        &bytes[from..to]
    }

    /// Hash of lines `start..=end`, or `None` when that range is not entirely
    /// inside the file — which is the answer a remap needs when a line map
    /// sends a citation past the end of a file that shrank.
    pub(crate) fn hash_span(&self, start: u32, end: u32) -> Option<String> {
        if start < 1 || end < start || end > self.last_real_line() {
            return None;
        }
        Some(hash_bytes(self.span_bytes(start, end)))
    }
}

/// Walk the Rust tree, resolve every citation through `cfg`, and hash the
/// cited spans of the C++ tree at `root`.
pub fn scan_tree(root: &Path, cfg: &Config) -> Result<Scan, String> {
    let mut cpp_cache: HashMap<String, Option<CppFile>> = HashMap::new();
    let mut sites = Vec::new();
    let mut dangling = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();

    for rust in rust_files(root)? {
        let path = root.join("rust").join(&rust);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        for raw in scan::scan_text(&text) {
            let key = format!("{}#{}", rust, raw.text);
            let basename = match &raw.file_ref {
                FileRef::Bare => None,
                FileRef::Qualified { basename, .. } => Some(basename.as_str()),
            };
            if let Some(reason) = cfg.unresolved_reason(&key, basename) {
                skipped.push(Skipped {
                    key,
                    reason: reason.to_string(),
                });
                continue;
            }
            // Resolve: an explicit per-site override wins, then the table for
            // the citation's form.
            let cpp = match cfg.site_override(&key) {
                Some(path) => path.to_string(),
                None => match &raw.file_ref {
                    FileRef::Qualified { prefix, basename } => {
                        let Some(path) = cfg.qualified(basename) else {
                            errors.push(format!(
                                "{rust}:{} cites {} — no [qualified] entry for {basename:?}; \
                                 add one to citations.toml",
                                raw.line, raw.text
                            ));
                            continue;
                        };
                        // A written-out directory must agree with the config,
                        // or one of the two is about a different file.
                        if !prefix.is_empty() && !path.ends_with(&format!("{prefix}{basename}")) {
                            errors.push(format!(
                                "{rust}:{} cites {} — path prefix {prefix:?} contradicts \
                                 [qualified] {basename:?} = {path:?}",
                                raw.line, raw.text
                            ));
                            continue;
                        }
                        path.to_string()
                    }
                    FileRef::Bare => {
                        let Some(path) = cfg.bare(&rust) else {
                            errors.push(format!(
                                "{rust}:{} cites {} — no [bare] entry covers this file; add \
                                 one to citations.toml (and verify it against the C++ first)",
                                raw.line, raw.text
                            ));
                            continue;
                        };
                        path.to_string()
                    }
                },
            };

            let file = cpp_cache.entry(cpp.clone()).or_insert_with(|| {
                let p = root.join(&cpp);
                std::fs::read_to_string(&p).ok().map(CppFile::new)
            });
            let Some(file) = file else {
                errors.push(format!(
                    "{rust}:{} cites {} — C++ file {cpp} does not exist",
                    raw.line, raw.text
                ));
                continue;
            };

            let end = raw.end.unwrap_or(raw.start);
            if end < raw.start {
                errors.push(format!(
                    "{rust}:{} cites {cpp}:{}-{} — reversed range (end < start)",
                    raw.line, raw.start, end
                ));
                continue;
            }
            // `line_count()` counts the empty line after a trailing newline.
            let last_line = file.last_real_line();
            if raw.start < 1 || end > last_line {
                let message = format!(
                    "{rust}:{} cites {cpp}:{}-{} — past end of file ({cpp} has {last_line} lines)",
                    raw.line, raw.start, end
                );
                errors.push(message.clone());
                // Still an error, but a repairable one: recorded so `remap`
                // can look for the blessed text in the file that shrank.
                dangling.push(Dangling {
                    rust: rust.clone(),
                    line: raw.line,
                    text: raw.text,
                    cpp,
                    start: raw.start,
                    end,
                    start_digits: raw.start_digits,
                    end_digits: raw.end_digits,
                    message,
                });
                continue;
            }

            let hash = file
                .hash_span(raw.start, end)
                .expect("the range was just checked against the file");
            sites.push(Site {
                rust: rust.clone(),
                line: raw.line,
                text: raw.text,
                cpp,
                start: raw.start,
                end,
                start_digits: raw.start_digits,
                end_digits: raw.end_digits,
                hash,
            });
        }
    }
    Ok(Scan {
        sites,
        dangling,
        skipped,
        errors,
    })
}

/// `git rev-parse HEAD` for the C++ tree, so the remap has a base commit.
fn head_commit(root: &Path) -> Result<String, String> {
    let out = remap::git(root, &["rev-parse", "HEAD"])?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// Rescan the tree and rewrite the snapshot. Returns the human report.
pub fn bless(root: &Path) -> Result<String, String> {
    let cfg = load_config(root)?;
    let scan = scan_tree(root, &cfg)?;
    if !scan.errors.is_empty() {
        return Err(error_report(&scan));
    }
    let snap = Snapshot {
        cpp_commit: head_commit(root)?,
        sites: scan
            .sites
            .iter()
            .map(|s| SnapshotSite {
                rust: s.rust.clone(),
                line: s.line,
                text: s.text.clone(),
                cpp: s.cpp.clone(),
                start: s.start,
                end: s.end,
                // Blessing re-chooses the base: the citation as written now is
                // where the hash just recorded came from.
                base_start: s.start,
                base_end: s.end,
                hash: s.hash.clone(),
            })
            .collect(),
    };
    let path = snapshot_path(root);
    std::fs::write(&path, snap.to_json())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(format!(
        "blessed {} citation sites against C++ commit {}\n{}",
        snap.sites.len(),
        snap.cpp_commit,
        skip_report(&scan)
    ))
}

/// What `check` found.
#[derive(Debug, Default)]
pub struct CheckReport {
    /// Sites whose cited span no longer hashes to the blessed value.
    pub stale: Vec<String>,
    /// Sites the config now resolves to a different file or range than the
    /// snapshot recorded — a `citations.toml` edit, not C++ drift. Kept apart
    /// from `stale` because "the span changed" would misdescribe it.
    pub repointed: Vec<String>,
    /// Sites in the tree that the snapshot does not have (Rust-side edits).
    pub unblessed: Vec<String>,
    /// Snapshot entries with no site in the tree (Rust-side deletions).
    pub missing: Vec<String>,
    /// Resolution/range failures; see [`Scan::errors`].
    pub errors: Vec<String>,
    /// The `[unresolved]` skips, so every run names them and not just their
    /// number: a config entry that silences a citation must stay visible.
    pub skipped: String,
    /// One-line totals, always printed.
    pub summary: String,
}

impl CheckReport {
    /// True when nothing at all is wrong. Skips are not failures — they are
    /// reported, not counted against the check.
    pub fn is_ok(&self) -> bool {
        self.stale.is_empty()
            && self.repointed.is_empty()
            && self.unblessed.is_empty()
            && self.missing.is_empty()
            && self.errors.is_empty()
    }

    /// Summary plus the skip identities: what a passing run should print.
    pub fn success_text(&self) -> String {
        format!("{}\n{}", self.summary, self.skipped)
    }

    /// The failure text for the standing test: every problem, one short line
    /// each, then what to do about it.
    pub fn failure_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.summary);
        out.push('\n');
        for group in [
            ("citation span changed", &self.stale),
            ("resolved elsewhere than the snapshot says", &self.repointed),
            ("not in the snapshot", &self.unblessed),
            ("in the snapshot but no longer in the source", &self.missing),
            ("could not be checked", &self.errors),
        ] {
            let (title, items) = group;
            if items.is_empty() {
                continue;
            }
            out.push_str(&format!("\n{} ({}):\n", title, items.len()));
            for item in items {
                out.push_str("  ");
                out.push_str(item);
                out.push('\n');
            }
        }
        out.push('\n');
        out.push_str(&self.skipped);
        out.push_str(
            "\nThe C++ lines a comment cites moved, or the C++ at them changed. Try the \
             mechanical repair first — it only accepts a new location whose text still \
             hashes to what was blessed, so it cannot invent a plausible-but-wrong \
             citation:\n  \
             cargo run -p tools --bin citations -- remap --dry-run   # what it would do\n  \
             cargo run -p tools --bin citations -- remap\n\
             Whatever it declines is a semantic question, not a mechanical one: read those \
             citations against the C++, re-point them by hand, and re-record with:\n  \
             cargo run -p tools --bin citations -- bless\n",
        );
        out
    }
}

/// Rescan the tree and compare it against the blessed snapshot.
///
/// Sites are matched by `(rust file, citation text, occurrence)`, not by line,
/// so editing prose around a citation does not make the check fail.
pub fn check(root: &Path) -> Result<CheckReport, String> {
    let cfg = load_config(root)?;
    let scan = scan_tree(root, &cfg)?;
    let path = snapshot_path(root);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e} (run `bless` first)", path.display()))?;
    let snap = Snapshot::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))?;

    let mut blessed: HashMap<String, &SnapshotSite> = HashMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for s in &snap.sites {
        let key = occurrence_key(&mut counts, &format!("{}#{}", s.rust, s.text));
        blessed.insert(key, s);
    }

    let mut report = CheckReport {
        errors: scan.errors.clone(),
        skipped: skip_report(&scan),
        ..Default::default()
    };
    let mut seen = std::collections::HashSet::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for site in &scan.sites {
        let key = occurrence_key(&mut counts, &site.key());
        match blessed.get(&key) {
            None => report.unblessed.push(site.describe()),
            Some(b) => {
                seen.insert(key);
                match compare(b, site) {
                    Verdict::Same => {}
                    Verdict::RePointed => report.repointed.push(format!(
                        "{} — the snapshot has {}:{}-{}; a resolution change, not a C++ edit",
                        site.describe(),
                        b.cpp,
                        b.start,
                        b.end
                    )),
                    Verdict::Stale => report
                        .stale
                        .push(format!("{} — span changed", site.describe())),
                }
            }
        }
    }
    for (key, b) in &blessed {
        if !seen.contains(key) {
            report.missing.push(format!(
                "{}:{} cites {}:{}-{}",
                b.rust, b.line, b.cpp, b.start, b.end
            ));
        }
    }
    report.stale.sort();
    report.repointed.sort();
    report.unblessed.sort();
    report.missing.sort();
    report.summary = format!(
        "{} citation sites checked against {} blessed at C++ commit {}; \
         {} stale, {} re-pointed, {} unblessed, {} missing, {} unresolvable, \
         {} skipped by config",
        scan.sites.len(),
        snap.sites.len(),
        short(&snap.cpp_commit),
        report.stale.len(),
        report.repointed.len(),
        report.unblessed.len(),
        report.missing.len(),
        report.errors.len(),
        scan.skipped.len(),
    );
    Ok(report)
}

/// How a scanned site compares with the snapshot's record of it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// Same file, same lines, same bytes.
    Same,
    /// The config resolves the citation somewhere else now.
    RePointed,
    /// Same file and lines, different bytes: the C++ moved or changed.
    Stale,
}

/// Compare one blessed record with the site scanned today.
///
/// Where the citation *points* is compared before what is there. A
/// `citations.toml` edit that re-points a glob also changes the hash, and
/// reporting that as "span changed" would send the reader hunting through the
/// C++ for an edit that never happened.
pub(crate) fn compare(blessed: &SnapshotSite, site: &Site) -> Verdict {
    if (blessed.cpp.as_str(), blessed.start, blessed.end)
        != (site.cpp.as_str(), site.start, site.end)
    {
        Verdict::RePointed
    } else if blessed.hash != site.hash {
        Verdict::Stale
    } else {
        Verdict::Same
    }
}

/// Make a key unique per occurrence: `k`, `k#2`, `k#3`, ...
pub(crate) fn occurrence_key(counts: &mut HashMap<String, usize>, key: &str) -> String {
    let n = counts.entry(key.to_string()).or_insert(0);
    *n += 1;
    if *n == 1 {
        key.to_string()
    } else {
        format!("{key}#{n}")
    }
}

/// First 12 characters of a commit hash.
pub(crate) fn short(commit: &str) -> &str {
    &commit[..commit.len().min(12)]
}

/// The `[unresolved]` skips, for the tail of a `bless`/`check` report,
/// grouped by reason so a blanket exclusion is one line rather than thirty.
pub fn skip_report(scan: &Scan) -> String {
    if scan.skipped.is_empty() {
        return "no citations skipped by [unresolved]".to_string();
    }
    let mut order: Vec<&str> = Vec::new();
    let mut groups: HashMap<&str, Vec<&str>> = HashMap::new();
    for s in &scan.skipped {
        let entry = groups.entry(&s.reason).or_default();
        if entry.is_empty() {
            order.push(&s.reason);
        }
        entry.push(&s.key);
    }
    let mut out = format!(
        "{} citation(s) skipped by [unresolved]:\n",
        scan.skipped.len()
    );
    for reason in order {
        let keys = &groups[reason];
        out.push_str(&format!("  {} site(s): {reason}\n", keys.len()));
        for key in keys.iter().take(3) {
            out.push_str(&format!("    {key}\n"));
        }
        if keys.len() > 3 {
            out.push_str(&format!("    ... and {} more\n", keys.len() - 3));
        }
    }
    out
}

/// The failure text when a scan could not resolve or range-check citations.
fn error_report(scan: &Scan) -> String {
    let mut out = format!("{} citation(s) could not be checked:\n", scan.errors.len());
    for e in &scan.errors {
        out.push_str("  ");
        out.push_str(e);
        out.push('\n');
    }
    out.push_str(
        "Each must either be corrected in the Rust source, given a [site_override] or \
         [qualified]/[bare] entry, or listed in [unresolved] with a reason, in \
         crates/tools/citations.toml.\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tree itself must resolve: this is the property that makes the
    /// snapshot meaningful, so it is asserted rather than reported.
    #[test]
    fn the_whole_tree_resolves() {
        let root = repo_root();
        let cfg = load_config(&root).expect("config loads");
        let scan = scan_tree(&root, &cfg).expect("scan succeeds");
        assert!(scan.errors.is_empty(), "{}", error_report(&scan));
        // A floor just under today's count, to catch a scanner or config
        // change that stops finding citations *here*, where no snapshot is
        // consulted. It is a smoke alarm, not the guarantee: losing sites is
        // caught precisely by `check`, which names every snapshot entry that
        // no longer has a site. Raise this when the corpus grows; lower it
        // only for a deliberate, explained shrink.
        const FLOOR: usize = 3000;
        assert!(
            scan.sites.len() >= FLOOR,
            "only {} citations resolved, below the floor of {FLOOR}; if that drop is \
             intentional, update the floor — otherwise the scanner or citations.toml \
             just stopped seeing citations",
            scan.sites.len()
        );
    }

    #[test]
    fn span_bytes_covers_the_inclusive_range() {
        let file = CppFile::new("a\nbb\nccc\ndddd\n".to_string());
        assert_eq!(file.span_bytes(1, 1), b"a\n");
        assert_eq!(file.span_bytes(2, 3), b"bb\nccc\n");
        assert_eq!(file.span_bytes(4, 4), b"dddd\n");
        assert_eq!(file.last_real_line(), 4);
        assert_eq!(file.hash_span(2, 3), Some(hash_bytes(b"bb\nccc\n")));
        // A range the file does not contain has no hash, rather than a hash
        // of whatever happens to be there: a remap must be able to tell that
        // a line map sent it past the end of a file that shrank.
        assert_eq!(file.hash_span(4, 5), None);
        assert_eq!(file.hash_span(0, 1), None);
        assert_eq!(file.hash_span(3, 2), None);
    }

    #[test]
    fn a_re_pointed_citation_is_not_reported_as_drift() {
        let blessed = SnapshotSite {
            rust: "crates/support/src/manager.rs".into(),
            line: 439,
            text: "cpp:109-117".into(),
            cpp: "lib/Support/SourceErrorManager.cpp".into(),
            start: 109,
            end: 117,
            base_start: 109,
            base_end: 117,
            hash: "0123456789abcdef".into(),
        };
        let site = |cpp: &str, hash: &str| Site {
            rust: blessed.rust.clone(),
            line: 439,
            text: blessed.text.clone(),
            cpp: cpp.to_string(),
            start: 109,
            end: 117,
            start_digits: (0, 0),
            end_digits: None,
            hash: hash.to_string(),
        };
        assert_eq!(
            compare(&blessed, &site(&blessed.cpp, &blessed.hash)),
            Verdict::Same
        );
        // Same lines, different bytes: the C++ changed under the citation.
        assert_eq!(
            compare(&blessed, &site(&blessed.cpp, "ffffffffffffffff")),
            Verdict::Stale
        );
        // Another file entirely: a config change, whatever the hash says.
        assert_eq!(
            compare(
                &blessed,
                &site("lib/Parser/JSONParser.cpp", "ffffffffffffffff")
            ),
            Verdict::RePointed
        );
        assert_eq!(
            compare(&blessed, &site("lib/Parser/JSONParser.cpp", &blessed.hash)),
            Verdict::RePointed
        );
        // And a re-point must fail the check, not slip through.
        let report = CheckReport {
            repointed: vec!["x".into()],
            ..Default::default()
        };
        assert!(!report.is_ok());
    }

    #[test]
    fn occurrence_keys_disambiguate_repeats() {
        let mut counts = HashMap::new();
        assert_eq!(occurrence_key(&mut counts, "a#cpp:1"), "a#cpp:1");
        assert_eq!(occurrence_key(&mut counts, "a#cpp:1"), "a#cpp:1#2");
        assert_eq!(occurrence_key(&mut counts, "b#cpp:1"), "b#cpp:1");
    }
}
