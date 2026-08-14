/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! `remap`: the mechanical repair of citations whose C++ moved.
//!
//! ## What it is for
//!
//! A cherry-pick that inserts fifteen lines near the top of a C++ file leaves
//! every citation below the insertion naming the wrong lines. Nothing about
//! that is a judgement call: the cited *text* is unchanged, it simply lives
//! fifteen lines lower. `check` names those sites; `remap` moves the digits.
//!
//! ## Hypothesis and proof
//!
//! The line map is a **hypothesis** and the hash is the **proof**.
//!
//! The hypothesis comes from `git diff -U0 <blessed commit> -- <file>`, whose
//! hunk headers say which old lines became which new lines. It cannot be
//! trusted on its own: `Snapshot::cpp_commit` is `HEAD` as of the moment
//! `bless` ran, so a committed snapshot names the *parent* of the commit that
//! carries it, and it says nothing about C++ that was uncommitted at the
//! time. The content the blessed hashes came from may therefore differ from
//! that commit's content.
//!
//! So every site is checked twice against the hash that *is* the record:
//!
//! 1. **Base check.** The blessed hash must reproduce at the site's base
//!    coordinates in the blessed commit's content. If it does not, the base
//!    the line map is computed from is not the content that was blessed, and
//!    the map means nothing here — decline.
//! 2. **Destination check.** The blessed hash must reproduce at the mapped
//!    coordinates in the working tree. If it does not, the cited text
//!    *changed* rather than moved, which is a semantic question for a human —
//!    decline.
//!
//! Only a site that passes both is rewritten. This is the property that makes
//! `remap` safe to run without reading its output, and it is not to be
//! weakened: a citation is a claim about specific C++ text, so a repair that
//! cannot show the text is the same is a guess.
//!
//! ## Base coordinates, and why remapping is reversible
//!
//! The map always runs from the blessed commit to the working tree, so the
//! coordinates it maps *from* must be the ones the blessed commit uses.
//! Those are `SnapshotSite::base_start`/`base_end`, which `bless` sets and
//! `remap` deliberately leaves alone. A remapped site therefore records both
//! where it points today (`start`/`end`, which match what the Rust comment
//! now says) and where its text sat at the base (`base_start`/`base_end`).
//!
//! That is what makes repeated remaps well behaved. Remap after a second
//! cherry-pick maps from the same origin through the now-larger diff, instead
//! of compounding one offset on another. Remap after the C++ change is
//! *reverted* sees an empty diff, maps the base coordinates to themselves,
//! and puts the citation back exactly where it started.
//!
//! ## What it declines, and why it never chases
//!
//! | shape | what happens |
//! |---|---|
//! | the cited lines were edited, or deleted, between the base and now | the map has no image for them — declined |
//! | the cited construct moved to a *different file* | its lines read as deleted here; remap never looks in another file, because which file a citation names comes from `citations.toml`, not from a search |
//! | the cited text changed in place | maps cleanly, fails the destination check — declined |
//! | the file shrank past the mapped range | no hash at the destination — declined |
//! | the snapshot's base is not the blessed commit's content | fails the base check — declined, with a note to re-bless |
//! | a citation the config now resolves to another file | not a line shift at all; left alone |
//! | a structurally invalid citation (a reversed range) | never becomes a site, so remap cannot see it, let alone "fix" it into something plausible |
//!
//! A declined site is reported, never silently skipped, and `remap` exits
//! non-zero while anything is left for a human.
//!
//! ## Citations whose file shrank under them
//!
//! One shape needs care in the other direction. A citation naming lines the
//! file no longer has is a hard *error* for `check` and `bless` — there is no
//! span to hash — so it never becomes a [`super::Site`], and a remap that
//! only looked at sites would be unable to repair it. That is not a corner
//! case: it is exactly what reverting an upstream insertion does to citations
//! this tool moved down, and it was measured (5 of the 508 in the proof run).
//! So the scan lists them separately as well, as [`super::Dangling`], and
//! they go through the same two hash proofs by way of the snapshot.
//!
//! ## A citation's text is also its name in `citations.toml`
//!
//! Site keys in the config are `<rust path>#<citation text>`, so changing the
//! digits changes the name of any `[site_override]` entry that resolves the
//! citation. Leaving those behind silently un-overrides the site — measured,
//! not guessed: remapping without this step left the six `[site_override]`
//! sites of `SemanticResolver.cpp` resolving through their module's `[bare]`
//! entry and failing the past-end-of-file check. A repair therefore renames
//! the config keys that name the citations it moved, and refuses to write
//! anything if a rename would collide with a key that already exists.
//!
//! ## What it writes
//!
//! Only digits. The rewrite splices the byte ranges the scanner recorded for
//! each citation's start and end numbers, so a wrapped citation
//! (`SemanticResolver.cpp:` ⏎ `2276-2366`) and the comma-continuation shape
//! (`cpp:86-88, 160-245`) need no special case — each number is its own byte
//! range. Nothing else on the line is touched, and the rewritten file is
//! re-scanned in memory before it is written, to confirm that it still holds
//! the same citations and that each repaired one now reads the intended
//! numbers.

use std::collections::HashMap;
use std::path::Path;

use super::snapshot::Snapshot;
use super::{
    check, compare, config_path, load_config, occurrence_key, scan, scan_tree, skip_report,
    snapshot_path, Cited, CppFile, Verdict,
};

/// Run `git` in `root` and return its stdout, or a message naming what failed.
pub(crate) fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run `git {}`: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

/// One hunk of a `git diff -U0`: old lines `old_start..old_start+old_count`
/// became new lines `new_start..new_start+new_count`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hunk {
    old_start: u32,
    old_count: u32,
    new_start: u32,
    new_count: u32,
}

/// A file's old-line to new-line map, built from `git diff -U0`.
#[derive(Debug, Default)]
struct LineMap {
    /// In increasing `old_start` order, as git emits them.
    hunks: Vec<Hunk>,
}

impl LineMap {
    /// Parse a `git diff -U0` for a single file.
    ///
    /// Only hunk headers matter; with `-U0` there is no context, so a header
    /// says exactly which old lines were replaced by which new ones. A `@@`
    /// that is not a well-formed header is an error rather than something to
    /// skip past: a misparsed diff would produce a confidently wrong map.
    fn parse(diff: &str) -> Result<LineMap, String> {
        let mut hunks: Vec<Hunk> = Vec::new();
        for line in diff.lines() {
            // Content lines always carry a ' ', '+' or '-' prefix, so a bare
            // `@@` at the start of a line is always a header.
            if !line.starts_with("@@") {
                continue;
            }
            let hunk = parse_hunk_header(line)
                .ok_or_else(|| format!("cannot parse a git diff hunk header: {line:?}"))?;
            if let Some(prev) = hunks.last() {
                if hunk.old_start < prev.old_start {
                    return Err(format!(
                        "git diff hunks are not in increasing order: {:?} then {hunk:?}",
                        prev
                    ));
                }
            }
            hunks.push(hunk);
        }
        Ok(LineMap { hunks })
    }

    /// Where old line `old` ended up, or `None` when it has no image: it was
    /// inside a region the diff replaced or deleted.
    ///
    /// A line inside a hunk has no honest answer even when the hunk replaced
    /// as many lines as it removed — the text there is not the text that was
    /// blessed — so this refuses rather than guesses, and the caller declines
    /// the site.
    fn map(&self, old: u32) -> Option<u32> {
        let mut delta: i64 = 0;
        for h in &self.hunks {
            if h.old_count == 0 {
                // A pure insertion `-l,0 +m,t`: t new lines appear after old
                // line l, so only lines strictly after l move.
                if old > h.old_start {
                    delta += i64::from(h.new_count);
                } else {
                    break;
                }
            } else if old < h.old_start {
                break;
            } else if old < h.old_start + h.old_count {
                return None;
            } else {
                delta += i64::from(h.new_count) - i64::from(h.old_count);
            }
        }
        let mapped = i64::from(old) + delta;
        // Only reachable for a corrupt diff; the caller treats it as "no
        // image", which is the safe reading.
        u32::try_from(mapped).ok().filter(|&n| n >= 1)
    }
}

/// Parse `@@ -l[,s] +m[,t] @@ ...` into a [`Hunk`].
fn parse_hunk_header(line: &str) -> Option<Hunk> {
    let rest = line.strip_prefix("@@ ")?;
    let body = &rest[..rest.find(" @@")?];
    let (old, new) = body.split_once(' ')?;
    let pair = |s: &str, sign: char| -> Option<(u32, u32)> {
        let s = s.strip_prefix(sign)?;
        match s.split_once(',') {
            // `@@ -5 +5,2 @@`: a missing count means one line.
            None => Some((s.parse().ok()?, 1)),
            Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
        }
    };
    let (old_start, old_count) = pair(old, '-')?;
    let (new_start, new_count) = pair(new, '+')?;
    Some(Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

/// What one `remap` run did.
#[derive(Debug, Default)]
pub struct RemapReport {
    /// True when nothing was written.
    pub dry_run: bool,
    /// One line per citation whose digits were (or would be) rewritten.
    pub repaired: Vec<String>,
    /// One line per stale citation the mechanical repair refused, with why.
    /// These need a human: the cited text changed, not just its position.
    pub declined: Vec<String>,
    /// Problems that are not line drift at all — a re-pointed citation, one
    /// missing from the snapshot, an unresolvable one — reported so a run
    /// never looks cleaner than the tree is.
    pub untouched: Vec<String>,
    /// Rust files rewritten.
    pub files_written: usize,
    /// `citations.toml` site keys renamed to follow a repaired citation.
    pub config_keys_updated: usize,
    /// The `[unresolved]` skips, named on every run exactly as `check` names
    /// them: the two structurally invalid citations are in there, and remap
    /// must be seen not to have touched them.
    pub skipped: String,
    /// One-line totals.
    pub summary: String,
    /// `check`'s summary line after the rewrite, so a run says plainly what
    /// state it left the tree in.
    pub after: String,
}

impl RemapReport {
    /// True when nothing is left for a human to look at.
    pub fn is_clean(&self) -> bool {
        self.declined.is_empty() && self.untouched.is_empty()
    }

    /// The whole report, for stdout.
    pub fn text(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.summary);
        out.push('\n');
        for (title, items) in [
            ("repaired", &self.repaired),
            ("declined — needs a human", &self.declined),
            ("not a line shift, left alone", &self.untouched),
        ] {
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
        out.push_str(&self.after);
        out.push('\n');
        out.push_str(&self.skipped);
        if !self.is_clean() {
            out.push_str(
                "\nWhat remains is a semantic question: the cited C++ text changed, so read \
                 each citation against the C++, re-point it by hand, and re-record with:\n  \
                 cargo run -p tools --bin citations -- bless\n",
            );
        }
        out
    }
}

/// Why a stale citation could not be repaired mechanically.
///
/// Every variant means the same thing to the caller — leave the citation
/// alone and tell a human — but they are distinct because the *reason* is
/// what the human needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decline {
    /// The blessed text is not at the site's base coordinates in the base
    /// content, so a line map computed from that commit maps between the
    /// wrong two things.
    BaseMismatch,
    /// The cited lines were edited or deleted between the base and now, so
    /// they have no image in today's file. This is also what a construct that
    /// moved to a *different* file looks like from here.
    NoImage,
    /// The mapped range covers a different number of lines than was blessed.
    Length(u32, u32),
    /// The mapped range is not inside the file any more (it shrank).
    PastEnd(u32, u32),
    /// The mapped range exists, but the text there is not the blessed text:
    /// the cited C++ changed rather than moved.
    TextChanged(u32, u32),
}

impl Decline {
    /// The sentence that follows "`crates/x.rs:12 cites F:891-907` — ".
    fn explain(self, cited: &Cited<'_>, b: &super::snapshot::SnapshotSite, commit: &str) -> String {
        let cpp = &b.cpp;
        let (bs, be) = (b.base_start, b.base_end);
        match self {
            Decline::BaseMismatch => format!(
                "the blessed text is not at {cpp}:{bs}-{be} in {commit}, so no line map from \
                 that commit can be trusted; the snapshot was blessed against a different \
                 content (uncommitted C++, most likely). Re-verify by hand and `bless`."
            ),
            Decline::NoImage => format!(
                "{cpp}:{bs}-{be} was edited or deleted since {commit}, so the cited lines have \
                 no image in the file today; if the code was rewritten, or moved to another \
                 file, re-point the citation by hand"
            ),
            Decline::Length(s, e) => format!(
                "{cpp}:{bs}-{be} maps to {s}-{e}, a range of a different length: the span \
                 itself changed"
            ),
            Decline::PastEnd(s, e) => format!(
                "{cpp}:{bs}-{be} maps to {s}-{e}, which is past the end of the file as it is \
                 now; re-point the citation by hand"
            ),
            Decline::TextChanged(s, e) => {
                let what = if (s, e) == (cited.start, cited.end) {
                    "the citation already names those lines, and the C++ at them changed"
                } else {
                    "the text there is not the text that was blessed"
                };
                format!(
                    "the line map says {cpp}:{bs}-{be} is now {s}-{e}, but {what}. A changed \
                     citation is a semantic question, not a mechanical one: read it against \
                     the C++."
                )
            }
        }
    }
}

/// Where a blessed citation's text lives in the working tree, or why that
/// cannot be settled mechanically.
///
/// This is the whole safety argument in one function, which is why it takes
/// content rather than paths: the two hash comparisons here are what stand
/// between "the lines moved" and "somebody rewrote the code", and they are
/// unit-tested directly.
fn locate(
    blessed: &super::snapshot::SnapshotSite,
    base: &CppFile,
    work: &CppFile,
    map: &LineMap,
) -> Result<(u32, u32), Decline> {
    let (bs, be) = (blessed.base_start, blessed.base_end);
    // Proof #1: the base really is the content this hash came from.
    if base.hash_span(bs, be).as_deref() != Some(blessed.hash.as_str()) {
        return Err(Decline::BaseMismatch);
    }
    let (Some(start), Some(end)) = (map.map(bs), map.map(be)) else {
        return Err(Decline::NoImage);
    };
    if end - start != be - bs {
        return Err(Decline::Length(start, end));
    }
    // Proof #2: the blessed text is what is actually at the destination.
    match work.hash_span(start, end) {
        None => Err(Decline::PastEnd(start, end)),
        Some(h) if h != blessed.hash => Err(Decline::TextChanged(start, end)),
        Some(_) => Ok((start, end)),
    }
}

/// One accepted repair: a scanned citation, its snapshot entry, and where its
/// text moved to.
struct Repair<'a> {
    /// Index into `Snapshot::sites`.
    snap_idx: usize,
    /// The citation as scanned today, which carries the digits' byte ranges.
    cited: Cited<'a>,
    /// The mapped, hash-verified destination.
    new_start: u32,
    new_end: u32,
}

/// Repair every stale citation whose cited text can be found, unchanged, at a
/// mapped location in the working tree; report everything else.
///
/// With `dry_run` nothing is written — neither the Rust sources nor the
/// snapshot — and the report says what would have happened.
pub fn remap(root: &Path, dry_run: bool) -> Result<RemapReport, String> {
    let cfg = load_config(root)?;
    let scan = scan_tree(root, &cfg)?;
    let path = snapshot_path(root);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e} (run `bless` first)", path.display()))?;
    let mut snap = Snapshot::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))?;

    let mut report = RemapReport {
        dry_run,
        skipped: skip_report(&scan),
        ..Default::default()
    };

    // Pair scanned sites with snapshot entries exactly as `check` does, so
    // remap acts on precisely the sites the standing test complains about.
    let mut blessed: HashMap<String, usize> = HashMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (i, s) in snap.sites.iter().enumerate() {
        let key = occurrence_key(&mut counts, &format!("{}#{}", s.rust, s.text));
        blessed.insert(key, i);
    }
    let mut stale: Vec<(usize, Cited)> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for site in &scan.sites {
        let key = occurrence_key(&mut counts, &site.key());
        match blessed.get(&key) {
            None => report.untouched.push(format!(
                "{} — not in the snapshot, so there is no blessed text to look for; \
                 review it and `bless`",
                site.describe()
            )),
            Some(&i) => match compare(&snap.sites[i], site) {
                Verdict::Same => {}
                Verdict::RePointed => report.untouched.push(format!(
                    "{} — the snapshot has {}:{}-{}; a resolution change, not a line shift",
                    site.describe(),
                    snap.sites[i].cpp,
                    snap.sites[i].start,
                    snap.sites[i].end
                )),
                Verdict::Stale => stale.push((i, site.cited())),
            },
        }
    }

    // Citations that now name lines their file does not have are errors, but
    // repairable ones: a file that shrank under a citation is exactly what a
    // reverted upstream insertion looks like, and the blessed text may well
    // still be in the file. They cannot be compared (there is no span to
    // hash), so they are paired with the snapshot by key alone and the same
    // two hash proofs decide.
    let mut counts: HashMap<String, usize> = HashMap::new();
    for d in &scan.dangling {
        let cited = d.cited();
        let key = occurrence_key(&mut counts, &cited.key());
        match blessed.get(&key) {
            None => report.untouched.push(format!(
                "{} — past the end of {}, and not in the snapshot either; repair it by hand",
                cited.describe(),
                d.cpp
            )),
            Some(&i) if snap.sites[i].cpp != d.cpp => report.untouched.push(format!(
                "{} — past the end of the file, and the snapshot has it in {}; a resolution \
                 change, not a line shift",
                cited.describe(),
                snap.sites[i].cpp
            )),
            Some(&i) => stale.push((i, cited)),
        }
    }

    // Every other unresolvable citation is out of remap's reach.
    let handled: std::collections::HashSet<&str> =
        scan.dangling.iter().map(|d| d.message.as_str()).collect();
    for e in scan.errors.iter().filter(|e| !handled.contains(e.as_str())) {
        report
            .untouched
            .push(format!("{e} — unresolvable, so not remappable"));
    }

    if stale.is_empty() {
        report.summary = format!(
            "no citation is stale: nothing to remap ({} sites checked against the snapshot \
             blessed at C++ commit {})",
            scan.sites.len(),
            super::short(&snap.cpp_commit)
        );
        report.after = check_summary(root)?;
        return Ok(report);
    }

    // The map's origin must exist before any of this means anything.
    let commit_exists = format!("{}^{{commit}}", snap.cpp_commit);
    if let Err(e) = git(root, &["cat-file", "-e", &commit_exists]) {
        return Err(format!(
            "remap cannot run: the snapshot's blessed C++ commit {} is not in this \
             repository, so there is no base to compute a line map from ({e}). This \
             happens after a history rewrite or in a shallow clone. Check the {} stale \
             citation(s) by hand and re-record them with `bless`.",
            super::short(&snap.cpp_commit),
            stale.len()
        ));
    }

    // Per C++ file: its content at the base commit, the line map to the
    // working tree, and its content now. Each is loaded once and may fail on
    // its own terms, which declines that file's sites rather than the run.
    let mut bases: HashMap<String, Result<CppFile, String>> = HashMap::new();
    let mut maps: HashMap<String, Result<LineMap, String>> = HashMap::new();
    let mut working: HashMap<String, Result<CppFile, String>> = HashMap::new();

    let mut repairs: Vec<Repair> = Vec::new();
    for (idx, cited) in stale {
        let b = &snap.sites[idx];
        let cpp = b.cpp.clone();
        let base = bases
            .entry(cpp.clone())
            .or_insert_with(|| file_at_commit(root, &snap.cpp_commit, &cpp));
        let base = match base {
            Ok(f) => f,
            Err(e) => {
                report.declined.push(format!(
                    "{} — cannot read {cpp} as of {}: {e}",
                    cited.describe(),
                    super::short(&snap.cpp_commit)
                ));
                continue;
            }
        };

        let map = maps
            .entry(cpp.clone())
            .or_insert_with(|| diff_map(root, &snap.cpp_commit, &cpp));
        let map = match map {
            Ok(m) => m,
            Err(e) => {
                report.declined.push(format!(
                    "{} — cannot compute a line map for {cpp}: {e}",
                    cited.describe()
                ));
                continue;
            }
        };
        let work = working
            .entry(cpp.clone())
            .or_insert_with(|| read_cpp(root, &cpp));
        let work = match work {
            Ok(f) => f,
            Err(e) => {
                report
                    .declined
                    .push(format!("{} — cannot read {cpp}: {e}", cited.describe()));
                continue;
            }
        };

        let (new_start, new_end) = match locate(b, base, work, map) {
            Ok(range) => range,
            Err(why) => {
                report.declined.push(format!(
                    "{} — {}",
                    cited.describe(),
                    why.explain(&cited, b, super::short(&snap.cpp_commit))
                ));
                continue;
            }
        };

        report.repaired.push(format!(
            "{}:{} cites {cpp}:{} -> {}",
            cited.rust,
            cited.line,
            range(cited.start, cited.end),
            range(new_start, new_end)
        ));
        repairs.push(Repair {
            snap_idx: idx,
            cited,
            new_start,
            new_end,
        });
    }
    report.repaired.sort();
    report.declined.sort();
    report.untouched.sort();

    // Rewrite every affected Rust file in memory and verify each result
    // before anything touches the disk, so a failed verification leaves the
    // tree exactly as it was.
    let mut by_file: HashMap<&str, Vec<&Repair>> = HashMap::new();
    for r in &repairs {
        by_file.entry(r.cited.rust).or_default().push(r);
    }
    let mut files: Vec<&str> = by_file.keys().copied().collect();
    files.sort();
    let mut writes: Vec<(std::path::PathBuf, String)> = Vec::new();
    let mut new_texts: HashMap<usize, String> = HashMap::new();
    for rust in files {
        let path = root.join("rust").join(rust);
        let old = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let (new, tokens) = rewrite_file(&old, &by_file[rust], rust)?;
        for (r, token) in by_file[rust].iter().zip(tokens) {
            new_texts.insert(r.snap_idx, token);
        }
        writes.push((path, new));
    }

    // The config names sites by their citation text, so any key that named a
    // repaired citation has to move with it.
    let renames: HashMap<String, String> = repairs
        .iter()
        .map(|r| {
            (
                r.cited.key(),
                format!("{}#{}", r.cited.rust, new_texts[&r.snap_idx]),
            )
        })
        .collect();
    let cfg_path = config_path(root);
    let cfg_text = std::fs::read_to_string(&cfg_path)
        .map_err(|e| format!("cannot read {}: {e}", cfg_path.display()))?;
    let (new_cfg, renamed) = rewrite_config_keys(&cfg_text, &renames)
        .map_err(|e| format!("{}: {e}", cfg_path.display()))?;
    report.config_keys_updated = renamed;

    report.summary = format!(
        "{} citation(s) stale: {} repaired{}, {} declined; {} other problem(s) left alone; \
         {} citations.toml key(s) follow the repaired citations",
        report.repaired.len() + report.declined.len(),
        report.repaired.len(),
        if dry_run { " (dry run)" } else { "" },
        report.declined.len(),
        report.untouched.len(),
        renamed,
    );

    if dry_run {
        report.after =
            "dry run: nothing written; the citations, the config and the snapshot are unchanged"
                .to_string();
        return Ok(report);
    }
    report.files_written = writes.len();
    if renamed != 0 {
        writes.push((cfg_path, new_cfg));
    }

    for (path, text) in &writes {
        std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }

    // The snapshot follows the citation: same text at a new place, so the
    // hash is untouched and the base stays where it was blessed.
    for r in &repairs {
        let entry = &mut snap.sites[r.snap_idx];
        entry.start = r.new_start;
        entry.end = r.new_end;
        entry.text = new_texts
            .remove(&r.snap_idx)
            .expect("every repair recorded its rewritten citation text");
    }
    std::fs::write(&path, snap.to_json())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;

    report.after = format!(
        "wrote {} Rust file(s), renamed {} citations.toml key(s), re-recorded the snapshot\n{}",
        report.files_written,
        report.config_keys_updated,
        check_summary(root)?
    );
    Ok(report)
}

/// `check`'s one-line summary, for the tail of a remap report.
fn check_summary(root: &Path) -> Result<String, String> {
    Ok(format!("after: {}", check(root)?.summary))
}

/// `891` or `891-907`, the way a citation writes a range.
fn range(start: u32, end: u32) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

/// One C++ file's content as of `commit`.
fn file_at_commit(root: &Path, commit: &str, cpp: &str) -> Result<CppFile, String> {
    let bytes = git(root, &["show", &format!("{commit}:{cpp}")])?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{cpp} is not UTF-8 at {commit}"))?;
    Ok(CppFile::new(text))
}

/// One C++ file's content in the working tree.
fn read_cpp(root: &Path, cpp: &str) -> Result<CppFile, String> {
    std::fs::read_to_string(root.join(cpp))
        .map(CppFile::new)
        .map_err(|e| e.to_string())
}

/// The line map from `commit`'s version of `cpp` to the working tree's.
fn diff_map(root: &Path, commit: &str, cpp: &str) -> Result<LineMap, String> {
    let out = git(
        root,
        &[
            "diff",
            "-U0",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            commit,
            "--",
            cpp,
        ],
    )?;
    let text = String::from_utf8(out).map_err(|_| format!("the diff of {cpp} is not UTF-8"))?;
    LineMap::parse(&text)
}

/// Rename the `citations.toml` keys that name a repaired citation.
///
/// Returns the new config text and how many keys moved. Only the key part of
/// a `"key" = "value"` line is touched — comments, order and values are left
/// exactly as they were, because this file is read by humans far more often
/// than by the tool.
///
/// A rename onto a key that already exists is refused rather than merged: two
/// entries would then be one, and which of the two reasons or overrides
/// survived would be a coin toss.
fn rewrite_config_keys(
    text: &str,
    renames: &HashMap<String, String>,
) -> Result<(String, usize), String> {
    // The config's grammar is `"key" = "value"`, so a key is what sits
    // between the first two quotes of an entry line; `config.rs` rejects a
    // key containing a quote or a backslash, so this cannot mis-split one.
    let key_span = |line: &str| -> Option<(usize, usize)> {
        if !line.trim_start().starts_with('"') {
            return None;
        }
        let open = line.find('"')?;
        let close = open + 1 + line[open + 1..].find('"')?;
        Some((open + 1, close))
    };
    let existing: std::collections::HashSet<&str> = text
        .lines()
        .filter_map(|l| key_span(l).map(|(a, b)| &l[a..b]))
        .collect();
    let mut targets: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (old, new) in renames {
        // A key that is itself being renamed away is not a collision: the
        // renames all apply at once.
        if existing.contains(new.as_str()) && !renames.contains_key(new.as_str()) {
            return Err(format!(
                "renaming the key {old:?} to {new:?} would collide with an entry that \
                 already exists; resolve it by hand"
            ));
        }
        if !targets.insert(new.as_str()) {
            return Err(format!(
                "two citations would both be renamed to the key {new:?}; resolve it by hand"
            ));
        }
    }

    let mut out = String::with_capacity(text.len());
    let mut renamed = 0usize;
    for (i, line) in text.split('\n').enumerate() {
        if i != 0 {
            out.push('\n');
        }
        match key_span(line).and_then(|(a, b)| renames.get(&line[a..b]).map(|new| (a, b, new))) {
            None => out.push_str(line),
            Some((a, b, new)) => {
                out.push_str(&line[..a]);
                out.push_str(new);
                out.push_str(&line[b..]);
                renamed += 1;
            }
        }
    }
    Ok((out, renamed))
}

/// Rewrite one Rust file's citation digits.
///
/// Returns the new text and, per repair in the order given, the citation
/// token as it now reads — which is what the snapshot must record, since
/// sites are matched by their text.
///
/// Everything here is checked rather than assumed: the bytes being replaced
/// must be the digits the scan saw, the edits must not overlap, and the
/// rewritten file must scan to the same citations with the intended numbers.
/// A rewrite that cannot be verified aborts the whole run.
fn rewrite_file(
    old: &str,
    repairs: &[&Repair],
    rust: &str,
) -> Result<(String, Vec<String>), String> {
    /// A byte range to replace, and the repair whose start digits it is.
    struct Edit {
        begin: usize,
        end: usize,
        text: String,
        /// `Some(i)` for the start-number edit of `repairs[i]`.
        of_repair: Option<usize>,
    }
    let mut edits: Vec<Edit> = Vec::new();
    for (i, r) in repairs.iter().enumerate() {
        let check_digits = |span: (u32, u32), value: u32| -> Result<(), String> {
            let (b, e) = (span.0 as usize, span.1 as usize);
            match old.get(b..e) {
                Some(s) if s == value.to_string() => Ok(()),
                other => Err(format!(
                    "{rust}: expected the digits {value} at bytes {b}..{e}, found {other:?}; \
                     the file changed under the scan — re-run"
                )),
            }
        };
        check_digits(r.cited.start_digits, r.cited.start)?;
        edits.push(Edit {
            begin: r.cited.start_digits.0 as usize,
            end: r.cited.start_digits.1 as usize,
            text: r.new_start.to_string(),
            of_repair: Some(i),
        });
        match r.cited.end_digits {
            Some(span) => {
                check_digits(span, r.cited.end)?;
                edits.push(Edit {
                    begin: span.0 as usize,
                    end: span.1 as usize,
                    text: r.new_end.to_string(),
                    of_repair: None,
                });
            }
            // A single-line citation writes one number, and a pure line shift
            // keeps it single-line; anything else is a bug in the mapping.
            None if r.new_start == r.new_end => {}
            None => {
                return Err(format!(
                    "{rust}: {} cites one line but would be remapped to {}-{}",
                    r.cited.text, r.new_start, r.new_end
                ))
            }
        }
    }
    edits.sort_by_key(|e| e.begin);

    let mut out = String::with_capacity(old.len());
    let mut at = 0usize;
    // Where each repair's start digits ended up, to find it again in the
    // rewritten file.
    let mut moved: Vec<Option<usize>> = vec![None; repairs.len()];
    for e in &edits {
        if e.begin < at {
            return Err(format!(
                "{rust}: two citation rewrites overlap at byte {}; refusing to write",
                e.begin
            ));
        }
        out.push_str(&old[at..e.begin]);
        if let Some(i) = e.of_repair {
            moved[i] = Some(out.len());
        }
        out.push_str(&e.text);
        at = e.end;
    }
    out.push_str(&old[at..]);

    // Verify against the scanner itself: same citations, same order, and the
    // repaired ones now reading what was intended.
    let before = scan::scan_text(old);
    let after = scan::scan_text(&out);
    if before.len() != after.len() {
        return Err(format!(
            "{rust}: rewriting the digits changed how many citations the file has \
             ({} -> {}); refusing to write",
            before.len(),
            after.len()
        ));
    }
    let mut tokens = Vec::with_capacity(repairs.len());
    for (i, r) in repairs.iter().enumerate() {
        let at = moved[i].expect("every repair rewrites its start digits") as u32;
        let found = after
            .iter()
            .find(|c| c.start_digits.0 == at)
            .ok_or_else(|| {
                format!(
                    "{rust}: the rewritten file has no citation at byte {at}, where {} was \
                     rewritten; refusing to write",
                    r.cited.text
                )
            })?;
        if (found.start, found.end.unwrap_or(found.start)) != (r.new_start, r.new_end) {
            return Err(format!(
                "{rust}: rewriting {} produced {}, not the intended {}; refusing to write",
                r.cited.text,
                found.text,
                range(r.new_start, r.new_end)
            ));
        }
        tokens.push(found.text.clone());
    }
    Ok((out, tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_of(diff: &str) -> LineMap {
        LineMap::parse(diff).expect("the diff parses")
    }

    #[test]
    fn hunk_headers_parse_with_and_without_counts() {
        assert_eq!(
            parse_hunk_header("@@ -30,0 +31,15 @@ void f() {"),
            Some(Hunk {
                old_start: 30,
                old_count: 0,
                new_start: 31,
                new_count: 15
            })
        );
        assert_eq!(
            parse_hunk_header("@@ -5 +5 @@"),
            Some(Hunk {
                old_start: 5,
                old_count: 1,
                new_start: 5,
                new_count: 1
            })
        );
        assert!(parse_hunk_header("@@ nonsense @@").is_none());
        // A malformed header is an error, never a hunk quietly dropped.
        assert!(LineMap::parse("@@ nonsense @@\n").is_err());
    }

    #[test]
    fn an_insertion_shifts_everything_below_it() {
        // 15 lines inserted after old line 30.
        let m = map_of("@@ -30,0 +31,15 @@\n");
        assert_eq!(m.map(1), Some(1));
        assert_eq!(m.map(30), Some(30));
        assert_eq!(m.map(31), Some(46));
        assert_eq!(m.map(891), Some(906));
    }

    #[test]
    fn a_deletion_pulls_up_and_deleted_lines_have_no_image() {
        // Old lines 10-12 deleted.
        let m = map_of("@@ -10,3 +9,0 @@\n");
        assert_eq!(m.map(9), Some(9));
        assert_eq!(m.map(10), None);
        assert_eq!(m.map(12), None);
        assert_eq!(m.map(13), Some(10));
    }

    #[test]
    fn a_replacement_refuses_its_own_lines_even_when_the_count_matches() {
        // One line replaced by one line: the position is unchanged but the
        // text is not, so there is no honest image for it.
        let m = map_of("@@ -502 +502 @@\n");
        assert_eq!(m.map(501), Some(501));
        assert_eq!(m.map(502), None);
        assert_eq!(m.map(503), Some(503));
    }

    #[test]
    fn several_hunks_accumulate() {
        let m = map_of(
            "diff --git a/x b/x\n--- a/x\n+++ b/x\n\
             @@ -30,0 +31,15 @@\n+a\n@@ -100,5 +115,2 @@\n-b\n@@ -200,0 +212,1 @@\n+c\n",
        );
        assert_eq!(m.map(29), Some(29));
        assert_eq!(m.map(50), Some(65));
        assert_eq!(m.map(102), None);
        assert_eq!(m.map(150), Some(162));
        assert_eq!(m.map(250), Some(263));
    }

    /// An empty diff — which is what a remap sees once the C++ change is
    /// reverted — maps every line to itself, so a citation that a previous
    /// remap moved is put back where it started.
    #[test]
    fn an_empty_diff_is_the_identity() {
        let m = map_of("");
        assert_eq!(m.map(1), Some(1));
        assert_eq!(m.map(906), Some(906));
    }

    /// A blessed site for `lines[start-1..end]` of `base`.
    fn blessed(base: &CppFile, start: u32, end: u32) -> super::super::snapshot::SnapshotSite {
        super::super::snapshot::SnapshotSite {
            rust: "crates/x.rs".into(),
            line: 1,
            text: format!("cpp:{start}-{end}"),
            cpp: "lib/Sema/X.cpp".into(),
            start,
            end,
            base_start: start,
            base_end: end,
            hash: base.hash_span(start, end).expect("in range"),
        }
    }

    fn cited_at(start: u32, end: u32) -> Cited<'static> {
        Cited {
            rust: "crates/x.rs",
            line: 1,
            text: "cpp:x",
            cpp: "lib/Sema/X.cpp",
            start,
            end,
            start_digits: (0, 0),
            end_digits: None,
        }
    }

    /// The whole point of the tool: a pure shift is repaired, and every way
    /// the cited *text* can differ is refused instead.
    #[test]
    fn a_shift_is_repaired_and_a_changed_text_is_refused() {
        let base = CppFile::new("a\nb\nc\nd\ne\nf\n".to_string());
        let b = blessed(&base, 3, 4); // "c\nd\n"

        // Two lines inserted above: the text moved, so the citation moves.
        let shifted = CppFile::new("a\nb\nX\nY\nc\nd\ne\nf\n".to_string());
        let m = map_of("@@ -2,0 +3,2 @@\n");
        assert_eq!(locate(&b, &base, &shifted, &m), Ok((5, 6)));

        // The same shift, but the text at the destination was also edited:
        // the map is just as plausible and the answer must still be no.
        let edited = CppFile::new("a\nb\nX\nY\nc!\nd\ne\nf\n".to_string());
        assert_eq!(
            locate(&b, &base, &edited, &m),
            Err(Decline::TextChanged(5, 6))
        );

        // Changed in place, without any shift: an empty map, and the citation
        // already names the lines. Still refused.
        let in_place = CppFile::new("a\nb\nc!\nd\ne\nf\n".to_string());
        assert_eq!(
            locate(&b, &base, &in_place, &map_of("")),
            Err(Decline::TextChanged(3, 4))
        );

        // The cited lines were deleted — which is also what a construct that
        // moved to another file looks like. Remap does not go looking.
        let deleted = CppFile::new("a\nb\ne\nf\n".to_string());
        assert_eq!(
            locate(&b, &base, &deleted, &map_of("@@ -3,2 +2,0 @@\n")),
            Err(Decline::NoImage)
        );

        // The file shrank under a citation a previous remap had moved.
        let mut moved = blessed(&base, 3, 4);
        moved.base_start = 3;
        moved.base_end = 4;
        let short_file = CppFile::new("a\nb\n".to_string());
        assert_eq!(
            locate(&moved, &base, &short_file, &map_of("")),
            Err(Decline::PastEnd(3, 4))
        );

        // And the base itself must be the content the hash came from: a
        // snapshot blessed against uncommitted C++ is refused, not guessed at.
        let wrong_base = CppFile::new("a\nb\nZ\nd\ne\nf\n".to_string());
        assert_eq!(
            locate(&b, &wrong_base, &shifted, &m),
            Err(Decline::BaseMismatch)
        );
    }

    /// A citation whose text is unchanged but whose span grew or shrank is
    /// not a line shift; the hash would refuse it anyway, but the length
    /// check names the real reason.
    #[test]
    fn a_span_of_a_different_length_is_refused() {
        let base = CppFile::new("a\nb\nc\nd\ne\n".to_string());
        let b = blessed(&base, 2, 4);
        // A line inserted inside the cited span: 2 maps to 2, 4 maps to 5.
        let grown = CppFile::new("a\nb\nX\nc\nd\ne\n".to_string());
        assert_eq!(
            locate(&b, &base, &grown, &map_of("@@ -2,0 +3,1 @@\n")),
            Err(Decline::Length(2, 5))
        );
        assert!(Decline::Length(2, 5)
            .explain(&cited_at(2, 4), &b, "abc123")
            .contains("a range of a different length"));
    }

    #[test]
    fn config_keys_follow_a_repaired_citation() {
        let cfg = "# a comment mentioning \"x.rs#cpp:1\"\n[site_override]\n\
                   \"crates/a.rs#cpp:1969-1974\" = \"lib/Sema/SemanticResolver.cpp\"\n\
                   \"crates/b.rs#cpp:12\" = \"lib/Sema/SemContext.cpp\"\n\
                   [qualified]\n\"flow.cpp\" = \"lib/Parser/JSParserImpl-flow.cpp\"\n";
        let renames: HashMap<String, String> = [(
            "crates/a.rs#cpp:1969-1974".to_string(),
            "crates/a.rs#cpp:1984-1989".to_string(),
        )]
        .into_iter()
        .collect();
        let (out, n) = rewrite_config_keys(cfg, &renames).expect("the rename applies");
        assert_eq!(n, 1);
        assert!(out.contains("\"crates/a.rs#cpp:1984-1989\" = \"lib/Sema/SemanticResolver.cpp\""));
        // Everything else, comments included, is byte-identical.
        assert_eq!(
            out.replace("cpp:1984-1989", "cpp:1969-1974"),
            cfg,
            "only the key moved"
        );

        // A rename onto an existing key is refused, not merged.
        let collide: HashMap<String, String> = [(
            "crates/a.rs#cpp:1969-1974".to_string(),
            "crates/b.rs#cpp:12".to_string(),
        )]
        .into_iter()
        .collect();
        assert!(rewrite_config_keys(cfg, &collide).is_err());
    }

    /// The two structurally invalid citations (reversed ranges) are excluded
    /// by `citations.toml`, so they never become sites and remap cannot see
    /// them, let alone "fix" one into something plausible.
    #[test]
    fn structurally_invalid_citations_never_reach_the_remap() {
        let root = super::super::repo_root();
        let cfg = load_config(&root).expect("config loads");
        let scan = scan_tree(&root, &cfg).expect("scan succeeds");
        assert!(
            scan.sites.iter().all(|s| s.start <= s.end),
            "a reversed range reached the site list, which remap would try to map"
        );
        for key in [
            "crates/sema/tests/check_implicit_return.rs#cpp:266-257",
            "crates/tools/src/bin/sema_dump.rs#CompilerDriver.cpp:2105- 2080",
        ] {
            assert!(
                scan.skipped.iter().any(|s| s.key == key),
                "{key} is no longer reported as skipped; remap would start seeing it"
            );
        }
    }
}
