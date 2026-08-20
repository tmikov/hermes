/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! FxHash, the hash rustc uses for its own interner maps.
//!
//! The atom table's `HashMap`s default to SipHash-1-3, which is ~8.7% of the
//! parser's profile on a large file: every identifier occurrence is hashed
//! (384k of them in `typescript.js`), and SipHash is built to resist collision
//! attacks rather than to be fast on short keys. FxHash is a rotate-xor-multiply
//! per word, which is what rustc, swc and oxc all use for exactly this job.
//!
//! Written out rather than pulled in as `rustc-hash`: it is thirty lines, and
//! the crates in this family ship with no third-party dependencies beyond
//! `bumpalo`, which is a property worth more than the thirty lines.
//!
//! **This trades collision resistance for speed, deliberately.** A caller
//! parsing hostile source could craft identifiers that collide and degrade
//! interning toward quadratic. That is the same exposure C++ Hermes has —
//! `llvm::StringMap` is not collision-resistant either — and the same choice
//! every other Rust JavaScript front end makes. It is confined to the atom
//! table: nothing here is exposed to a network attacker choosing keys against
//! a long-lived map.

use std::hash::BuildHasherDefault;
use std::hash::Hasher;

/// A `HashMap` using [`FxHasher`]. `BuildHasherDefault` is `Default`, so
/// `HashMap::default()` and `#[derive(Default)]` keep working unchanged.
pub(crate) type FxHashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<FxHasher>>;

/// Multiplier from rustc's `FxHasher`: the fractional bits of the golden ratio
/// scaled to 64 bits, which spreads the low bits of short keys across the word.
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// Rotate applied before mixing each word in, so that a word's high bits reach
/// the low bits that `HashMap` uses to pick a bucket.
const ROTATE: u32 = 5;

/// The FxHash state: one accumulator, mixed one word at a time.
#[derive(Default, Clone, Copy)]
pub(crate) struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add_to_hash(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(ROTATE) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // Mix the length first. Without it a trailing zero byte is invisible:
        // the 2-byte tail reads `[0x61, 0x00]` as a native-endian `u16`, which
        // is `0x61`, so `"a"` and `"a\0"` produce identical hashes. rustc-hash
        // 1.x has this property too. It is harmless there and reachable here —
        // JS string literals may contain NUL and are interned by these maps —
        // and one extra mix per key is not a price worth arguing about.
        //
        // `Hash for [u8]` already supplies a length prefix, so this is
        // redundant for the byte maps and load-bearing for the `&str` one,
        // whose `write_str` contributes only a `0xff` terminator.
        self.add_to_hash(bytes.len() as u64);

        // Whole words first, then a 4/2/1 tail. Identifiers are short, so the
        // tail is not a rare case to be handled sloppily — most keys are only
        // a word or two long.
        //
        // `chunks_exact` rather than the newer `split_first_chunk`: these
        // crates declare no MSRV, and the const-generic slice APIs would raise
        // the floor to 1.77 for every consumer to save nothing here. The
        // `unwrap`s are on lengths the iterator guarantees and compile away.
        let mut words = bytes.chunks_exact(8);
        for word in &mut words {
            self.add_to_hash(u64::from_ne_bytes(word.try_into().unwrap()));
        }
        let mut rest = words.remainder();
        if rest.len() >= 4 {
            self.add_to_hash(u32::from_ne_bytes(rest[..4].try_into().unwrap()) as u64);
            rest = &rest[4..];
        }
        if rest.len() >= 2 {
            self.add_to_hash(u16::from_ne_bytes(rest[..2].try_into().unwrap()) as u64);
            rest = &rest[2..];
        }
        if let Some(&byte) = rest.first() {
            self.add_to_hash(byte as u64);
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    fn hash_of<T: Hash + ?Sized>(value: &T) -> u64 {
        let mut h = FxHasher::default();
        value.hash(&mut h);
        h.finish()
    }

    /// The property the table actually depends on: equal keys hash equally.
    /// Everything else about a hasher is a performance claim, not a contract.
    #[test]
    fn equal_keys_hash_equally() {
        for s in ["", "a", "ab", "abcd", "abcdefg", "abcdefgh", "abcdefghijklmno"] {
            assert_eq!(hash_of(s), hash_of(&s.to_string().as_str()), "{s:?}");
        }
    }

    /// Every tail width must reach `add_to_hash`. A `write` that dropped its
    /// 4/2/1 tail would still be a *correct* hasher — equal keys would still
    /// collide equally — so only a difference test catches it. Each pair below
    /// differs solely in a byte that one of the tail branches consumes.
    #[test]
    fn tail_bytes_are_not_dropped() {
        let pairs = [
            ("abcdefgh", "abcdefgi"),         // 8: whole-word path
            ("abcdefghi", "abcdefghj"),       // 8 + 1
            ("abcdefghij", "abcdefghik"),     // 8 + 2
            ("abcdefghijkl", "abcdefghijkm"), // 8 + 4
            ("abcdefghijklm", "abcdefghijkln"), // 8 + 4 + 1
            ("abcdefghijklmno", "abcdefghijklmnp"), // 8 + 4 + 2 + 1
        ];
        for (a, b) in pairs {
            assert_ne!(hash_of(a), hash_of(b), "{a:?} vs {b:?}");
        }
    }

    /// Length must participate, or a trailing zero byte vanishes into the
    /// native-endian tail conversion. This failed on the first run against the
    /// straight rustc-hash 1.x `write`, which is why `write` mixes the length.
    #[test]
    fn length_participates() {
        assert_ne!(hash_of("a"), hash_of("a\0"));
        assert_ne!(hash_of(""), hash_of("\0"));
    }

    /// Distinct short identifiers must land in distinct buckets often enough to
    /// be worth using. This is a smoke test on the mixing, not a quality bound:
    /// a hasher that returned a constant would pass every test above.
    #[test]
    fn short_identifiers_spread() {
        let names: Vec<String> = (0..2000).map(|i| format!("v{i}")).collect();
        let mut hashes: Vec<u64> = names.iter().map(|n| hash_of(n.as_str())).collect();
        hashes.sort_unstable();
        hashes.dedup();
        assert_eq!(hashes.len(), names.len(), "distinct names must hash distinctly");
    }
}
