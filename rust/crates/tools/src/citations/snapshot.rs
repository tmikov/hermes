/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The blessed snapshot: what each citation pointed at when it was last
//! reviewed.
//!
//! Stored as JSON (`crates/tools/citations.snapshot.json`), one site per
//! line so a `git diff` of a reblessing reads as a list of citations rather
//! than a reflowed blob. It is written by hand here — the field order and the
//! one-line-per-site layout are the point — and read back with the port's own
//! `JSONParser`, which validates it.

use bumpalo::Bump;
use hermes_atom_table::AtomTable;
use hermes_parser::json::{JSONFactory, JSONParser};
use hermes_support::manager::SourceErrorManager;

/// One blessed citation site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotSite {
    /// Rust file, relative to `rust/`.
    pub rust: String,
    /// 1-based line of the citation in the Rust file, when blessed. Advisory:
    /// sites are matched by `(rust, text)`, so editing Rust prose above a
    /// citation does not invalidate the snapshot.
    pub line: u32,
    /// The citation as written (whitespace collapsed to single spaces).
    pub text: String,
    /// Repo-relative C++ path the citation resolved to.
    pub cpp: String,
    /// First cited line, 1-based.
    pub start: u32,
    /// Last cited line, inclusive; equal to `start` for a single-line cite.
    pub end: u32,
    /// Where the blessed span sat in the *base* content — the C++ file as of
    /// [`Snapshot::cpp_commit`] — when this site was last blessed.
    ///
    /// A freshly blessed site has `base_start == start`, and the field is not
    /// written to the file at all; the two differ only after a `remap` moved
    /// the citation without a re-bless. Keeping the base fixed is what makes
    /// `remap` idempotent and reversible: the line map always runs from the
    /// same origin (the blessed commit) to the working tree, so remapping
    /// twice, or remapping after the C++ change is reverted, lands on the
    /// right line rather than compounding an offset.
    ///
    /// It is a *claim* about the base, not a fact: `remap` re-hashes the
    /// blessed span at these coordinates in the base content before it trusts
    /// any line map derived from that commit, and declines the site when the
    /// claim does not hold.
    pub base_start: u32,
    /// Last line of the blessed span in the base content; see
    /// [`SnapshotSite::base_start`].
    pub base_end: u32,
    /// FNV-1a 64 of the cited span's exact bytes, lowercase hex.
    pub hash: String,
}

impl SnapshotSite {
    /// True when the site still sits where it was blessed, i.e. no `remap`
    /// has moved it since. Such a site writes no `base_*` fields.
    fn base_is_current(&self) -> bool {
        self.base_start == self.start && self.base_end == self.end
    }
}

/// A whole snapshot file.
#[derive(Debug)]
pub struct Snapshot {
    /// `git rev-parse HEAD` when `bless` ran, so the remap has a base to diff
    /// from.
    ///
    /// Note what this is *not*: it is HEAD at bless time, so once the
    /// snapshot is committed it names the **parent** of the commit carrying
    /// it, and it says nothing about uncommitted C++ in the working tree. The
    /// hashes, not this field, are the record of what was blessed — a remap
    /// must verify against them and treat a diff from this commit as a
    /// hypothesis about where lines went, never as ground truth. That is
    /// exactly what `remap` does: it re-hashes each blessed span in this
    /// commit's content before believing the line map, and again at the
    /// mapped location before writing anything.
    ///
    /// `remap` never changes this field: the base is only re-chosen by
    /// `bless`, which re-records the hashes at the same time.
    pub cpp_commit: String,
    /// Every checked site, in scan order.
    pub sites: Vec<SnapshotSite>,
}

impl Snapshot {
    /// Render the snapshot file's exact bytes.
    pub fn to_json(&self) -> String {
        let mut out = String::with_capacity(self.sites.len() * 160);
        out.push_str("{\n");
        out.push_str(&format!(
            "  \"cpp_commit\": {},\n",
            json_string(&self.cpp_commit)
        ));
        out.push_str(&format!("  \"site_count\": {},\n", self.sites.len()));
        out.push_str("  \"sites\": [\n");
        for (i, s) in self.sites.iter().enumerate() {
            // The base coordinates are written only when a `remap` has moved
            // the site since it was blessed, so a blessed snapshot keeps the
            // shape it has always had and a remap's diff shows exactly the
            // sites it touched.
            let base = if s.base_is_current() {
                String::new()
            } else {
                format!(
                    ", \"base_start\": {}, \"base_end\": {}",
                    s.base_start, s.base_end
                )
            };
            out.push_str(&format!(
                "    {{\"rust\": {}, \"line\": {}, \"text\": {}, \"cpp\": {}, \
                 \"start\": {}, \"end\": {}{}, \"hash\": {}}}{}\n",
                json_string(&s.rust),
                s.line,
                json_string(&s.text),
                json_string(&s.cpp),
                s.start,
                s.end,
                base,
                json_string(&s.hash),
                if i + 1 == self.sites.len() { "" } else { "," }
            ));
        }
        out.push_str("  ]\n}\n");
        out
    }

    /// Parse a snapshot file. `Err` describes what was wrong with it.
    pub fn from_json(text: &str) -> Result<Snapshot, String> {
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let factory = JSONFactory::new(&arena, &atoms);
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer_bytes("citations.snapshot.json", text.as_bytes());
        let (parsed, errors) = {
            let mut p = JSONParser::new(&factory, id, &mut sm, &atoms, false);
            let v = p.parse();
            let e = p.error_count();
            (v, e)
        };
        if errors != 0 {
            return Err(format!("snapshot is not valid JSON ({errors} errors)"));
        }
        let root = parsed.ok_or_else(|| "snapshot is empty".to_string())?;
        let obj = root
            .as_object()
            .ok_or_else(|| "snapshot root is not an object".to_string())?;
        let cpp_commit = obj
            .find("cpp_commit", &atoms)
            .and_then(|i| obj.value_at(i).as_string())
            .map(|a| String::from_utf8_lossy(atoms.bytes(a)).into_owned())
            .ok_or_else(|| "snapshot has no string `cpp_commit`".to_string())?;
        let arr = obj
            .find("sites", &atoms)
            .and_then(|i| obj.value_at(i).as_array())
            .ok_or_else(|| "snapshot has no `sites` array".to_string())?;
        let mut sites = Vec::with_capacity(arr.len());
        for i in 0..arr.len() {
            let o = arr
                .at(i)
                .as_object()
                .ok_or_else(|| format!("snapshot site {i} is not an object"))?;
            let string = |name: &str| -> Result<String, String> {
                o.find(name, &atoms)
                    .and_then(|j| o.value_at(j).as_string())
                    .map(|a| String::from_utf8_lossy(atoms.bytes(a)).into_owned())
                    .ok_or_else(|| format!("snapshot site {i} has no string `{name}`"))
            };
            let number = |name: &str| -> Result<u32, String> {
                o.find(name, &atoms)
                    .and_then(|j| o.value_at(j).as_number())
                    .map(|n| n as u32)
                    .ok_or_else(|| format!("snapshot site {i} has no number `{name}`"))
            };
            // Absent base coordinates mean "still where it was blessed"; see
            // `SnapshotSite::base_start`.
            let optional_number = |name: &str| -> Option<u32> {
                o.find(name, &atoms)
                    .and_then(|j| o.value_at(j).as_number())
                    .map(|n| n as u32)
            };
            let start = number("start")?;
            let end = number("end")?;
            sites.push(SnapshotSite {
                rust: string("rust")?,
                line: number("line")?,
                text: string("text")?,
                cpp: string("cpp")?,
                start,
                end,
                base_start: optional_number("base_start").unwrap_or(start),
                base_end: optional_number("base_end").unwrap_or(end),
                hash: string("hash")?,
            });
        }
        Ok(Snapshot { cpp_commit, sites })
    }
}

/// Quote a string as JSON. Citation text, paths and hashes are plain ASCII in
/// practice; the escapes are here so a stray quote cannot corrupt the file.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// FNV-1a 64 of `bytes`, as 16 lowercase hex digits.
///
/// A snapshot hash only has to notice that a cited span's text changed, and
/// it must be stable across toolchain versions — which rules out
/// `DefaultHasher`, whose output std explicitly does not promise to keep.
/// FNV-1a is small enough to read, has no dependency, and the chance that an
/// edited span reproduces its old hash is 2^-64.
pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Snapshot {
        Snapshot {
            cpp_commit: "fab819f1caf767234bedf958715fa315b75faf9e".into(),
            sites: vec![
                SnapshotSite {
                    rust: "crates/sema/src/resolver/classes.rs".into(),
                    line: 19,
                    text: "cpp:891-907".into(),
                    cpp: "lib/Sema/SemanticResolver.cpp".into(),
                    start: 891,
                    end: 907,
                    base_start: 891,
                    base_end: 907,
                    hash: hash_bytes(b"whatever"),
                },
                SnapshotSite {
                    rust: "crates/parser/src/json/parser.rs".into(),
                    line: 11,
                    text: "lib/Parser/JSONParser.cpp:202-211".into(),
                    cpp: "lib/Parser/JSONParser.cpp".into(),
                    start: 202,
                    end: 211,
                    // A site a `remap` has moved: the base stays where it was
                    // blessed, so the round trip must carry both pairs.
                    base_start: 200,
                    base_end: 209,
                    hash: hash_bytes(b"other"),
                },
            ],
        }
    }

    #[test]
    fn round_trips_through_the_ports_own_json_parser() {
        let s = sample();
        let back = Snapshot::from_json(&s.to_json()).expect("round trip");
        assert_eq!(back.cpp_commit, s.cpp_commit);
        assert_eq!(back.sites, s.sites);
    }

    #[test]
    fn one_site_per_line() {
        let json = sample().to_json();
        assert_eq!(json.lines().filter(|l| l.contains("\"rust\":")).count(), 2);
    }

    /// A site that has not been remapped writes no `base_*` fields, and a
    /// snapshot without them reads back with the base equal to the citation.
    #[test]
    fn the_base_is_written_only_when_it_differs() {
        let json = sample().to_json();
        assert_eq!(json.matches("\"base_start\"").count(), 1);
        assert!(json.contains("\"base_start\": 200, \"base_end\": 209"));

        let old = "{\"cpp_commit\": \"x\", \"sites\": [{\"rust\": \"a.rs\", \"line\": 1, \
                   \"text\": \"cpp:5\", \"cpp\": \"b.cpp\", \"start\": 5, \"end\": 5, \
                   \"hash\": \"0\"}]}";
        let snap = Snapshot::from_json(old).expect("a pre-remap snapshot still reads");
        assert_eq!((snap.sites[0].base_start, snap.sites[0].base_end), (5, 5));
    }

    #[test]
    fn malformed_snapshots_are_errors_not_silence() {
        assert!(Snapshot::from_json("{").is_err());
        assert!(Snapshot::from_json("{}").is_err());
        assert!(Snapshot::from_json("{\"cpp_commit\": \"x\"}").is_err());
    }

    #[test]
    fn the_hash_is_the_documented_fnv1a() {
        // Reference vectors for FNV-1a 64.
        assert_eq!(hash_bytes(b""), "cbf29ce484222325");
        assert_eq!(hash_bytes(b"a"), "af63dc4c8601ec8c");
        assert_ne!(hash_bytes(b"abc"), hash_bytes(b"abd"));
    }
}
