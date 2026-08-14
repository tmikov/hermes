/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The resolution config (`crates/tools/citations.toml`) and its tiny TOML
//! reader.
//!
//! The file is authored by hand and read only here, so instead of taking on a
//! TOML dependency (the workspace has none beyond `bumpalo`) this parses the
//! subset the file actually uses: `# comments`, `[table]` headers, and
//! `"key" = "value"` entries whose key and value are both double-quoted
//! strings containing neither `"` nor `\`. Anything else is a hard error, so
//! a typo cannot be silently ignored.

use std::collections::HashMap;

/// Parsed `citations.toml`.
pub struct Config {
    /// Cited basename (`SemanticResolver.cpp`) -> repo-relative path.
    qualified: HashMap<String, String>,
    /// Rust-path glob -> C++ file that a bare `cpp:` means there. Ordered:
    /// the first matching entry wins, so specific globs come first.
    bare: Vec<(String, String)>,
    /// Site key -> the C++ file that one citation really means.
    site_override: HashMap<String, String>,
    /// Key (site key, basename, or Rust-path glob) -> why it is not checked.
    /// Ordered so the report is stable and reads like the file.
    unresolved: Vec<(String, String)>,
}

impl Config {
    /// Parse the config text. `Err` carries a `line N: ...` message.
    pub fn parse(text: &str) -> Result<Config, String> {
        let mut qualified = HashMap::new();
        let mut bare = Vec::new();
        let mut site_override = HashMap::new();
        let mut unresolved = Vec::new();
        let mut table = String::new();

        for (i, raw) in text.lines().enumerate() {
            let lineno = i + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix('[') {
                let name = rest
                    .strip_suffix(']')
                    .ok_or_else(|| format!("line {lineno}: unterminated table header: {line}"))?;
                table = name.to_string();
                match table.as_str() {
                    "qualified" | "bare" | "site_override" | "unresolved" => {}
                    other => {
                        return Err(format!("line {lineno}: unknown table [{other}]"));
                    }
                }
                continue;
            }
            let (key, value) = parse_entry(line)
                .ok_or_else(|| format!("line {lineno}: expected `\"key\" = \"value\"`: {line}"))?;
            match table.as_str() {
                "qualified" => {
                    if qualified.insert(key.clone(), value).is_some() {
                        return Err(format!("line {lineno}: duplicate [qualified] key {key:?}"));
                    }
                }
                "bare" => {
                    if bare.iter().any(|(k, _): &(String, String)| *k == key) {
                        return Err(format!("line {lineno}: duplicate [bare] key {key:?}"));
                    }
                    bare.push((key, value));
                }
                "site_override" => {
                    if site_override.insert(key.clone(), value).is_some() {
                        return Err(format!(
                            "line {lineno}: duplicate [site_override] key {key:?}"
                        ));
                    }
                }
                "unresolved" => {
                    if unresolved.iter().any(|(k, _): &(String, String)| *k == key) {
                        return Err(format!("line {lineno}: duplicate [unresolved] key {key:?}"));
                    }
                    unresolved.push((key, value));
                }
                "" => return Err(format!("line {lineno}: entry before any [table] header")),
                _ => unreachable!("table names are validated above"),
            }
        }
        Ok(Config {
            qualified,
            bare,
            site_override,
            unresolved,
        })
    }

    /// The C++ path for a cited basename, if `[qualified]` has one.
    pub fn qualified(&self, basename: &str) -> Option<&str> {
        self.qualified.get(basename).map(String::as_str)
    }

    /// The C++ path a bare `cpp:` means in `rust_path` (relative to `rust/`),
    /// per the first matching `[bare]` glob.
    pub fn bare(&self, rust_path: &str) -> Option<&str> {
        self.bare
            .iter()
            .find(|(glob, _)| glob_match(glob, rust_path))
            .map(|(_, path)| path.as_str())
    }

    /// The C++ path forced for one exact site key, if any.
    pub fn site_override(&self, site_key: &str) -> Option<&str> {
        self.site_override.get(site_key).map(String::as_str)
    }

    /// The reason `key` is deliberately unchecked, if it is listed. `key` may
    /// be a site key, a cited basename, or (matched as a glob) a Rust path.
    pub fn unresolved_reason(&self, site_key: &str, basename: Option<&str>) -> Option<&str> {
        let rust_path = site_key.split('#').next().unwrap_or(site_key);
        self.unresolved
            .iter()
            .find(|(k, _)| {
                k == site_key || Some(k.as_str()) == basename || glob_match(k, rust_path)
            })
            .map(|(_, reason)| reason.as_str())
    }
}

/// Parse one `"key" = "value"` line, allowing a trailing `# comment`. Returns
/// `None` if the line does not have exactly that shape.
fn parse_entry(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix('"')?;
    let (key, rest) = rest.split_once('"')?;
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let (value, tail) = rest.split_once('"')?;
    let tail = tail.trim_start();
    if !(tail.is_empty() || tail.starts_with('#')) {
        return None;
    }
    if key.contains('\\') || value.contains('\\') {
        return None;
    }
    Some((key.to_string(), value.to_string()))
}

/// Glob match for the config's path patterns. `*` matches any run of
/// characters other than `/`; everything else is literal. Deliberately no
/// `**`: a mapping should name the directory it applies to.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == path,
        Some((prefix, suffix)) => {
            if !path.starts_with(prefix) {
                return false;
            }
            let rest = &path[prefix.len()..];
            // The `*` cannot cross a `/`, so only the first segment of `rest`
            // is a candidate for it; the rest must match `suffix`'s pattern.
            let seg_end = rest.find('/').unwrap_or(rest.len());
            (0..=seg_end).any(|i| glob_match(suffix, &rest[i..]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_star_does_not_cross_slash() {
        assert!(glob_match(
            "crates/sema/src/resolver/*.rs",
            "crates/sema/src/resolver/mod.rs"
        ));
        assert!(!glob_match(
            "crates/sema/src/resolver/*.rs",
            "crates/sema/src/resolver/sub/mod.rs"
        ));
        assert!(!glob_match(
            "crates/parser/src/js/*.rs",
            "crates/parser/src/js/flow/mod.rs"
        ));
        assert!(glob_match("a/b.rs", "a/b.rs"));
        assert!(!glob_match("a/b.rs", "a/bb.rs"));
        assert!(glob_match("a/*", "a/b"));
        assert!(glob_match("*.rs", "b.rs"));
    }

    #[test]
    fn parses_the_four_tables_and_keeps_bare_order() {
        let cfg = Config::parse(
            r##"
# a comment
[qualified]
"flow.cpp" = "lib/Parser/JSParserImpl-flow.cpp"  # trailing comment

[bare]
"crates/sema/src/resolver/promoter.rs" = "lib/Sema/ScopedFunctionPromoter.cpp"
"crates/sema/src/resolver/*.rs" = "lib/Sema/SemanticResolver.cpp"

[site_override]
"crates/x.rs#cpp:1" = "lib/Sema/SemanticResolver.cpp"

[unresolved]
"crates/y.rs#cpp:2-1" = "reversed range"
"##,
        )
        .expect("config parses");
        assert_eq!(
            cfg.qualified("flow.cpp"),
            Some("lib/Parser/JSParserImpl-flow.cpp")
        );
        assert_eq!(cfg.qualified("nope.cpp"), None);
        // Specific glob first: the promoter must not fall into the wildcard.
        assert_eq!(
            cfg.bare("crates/sema/src/resolver/promoter.rs"),
            Some("lib/Sema/ScopedFunctionPromoter.cpp")
        );
        assert_eq!(
            cfg.bare("crates/sema/src/resolver/classes.rs"),
            Some("lib/Sema/SemanticResolver.cpp")
        );
        assert_eq!(cfg.bare("crates/parser/src/js/mod.rs"), None);
        assert_eq!(
            cfg.site_override("crates/x.rs#cpp:1"),
            Some("lib/Sema/SemanticResolver.cpp")
        );
        assert_eq!(
            cfg.unresolved_reason("crates/y.rs#cpp:2-1", None),
            Some("reversed range")
        );
        assert_eq!(cfg.unresolved_reason("crates/y.rs#cpp:9", None), None);
    }

    #[test]
    fn a_hash_inside_a_key_is_not_a_comment() {
        let cfg = Config::parse("[unresolved]\n\"crates/a.rs#cpp:5\" = \"why # not\"\n")
            .expect("config parses");
        assert_eq!(
            cfg.unresolved_reason("crates/a.rs#cpp:5", None),
            Some("why # not")
        );
    }

    #[test]
    fn malformed_lines_are_errors() {
        assert!(Config::parse("[qualified]\nflow.cpp = \"x\"\n").is_err());
        assert!(Config::parse("[nope]\n").is_err());
        assert!(Config::parse("\"a\" = \"b\"\n").is_err());
        assert!(Config::parse("[bare]\n\"a\" = \"b\"\n\"a\" = \"c\"\n").is_err());
    }
}
