/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! JSONFactory: owns the arena, uniques strings/numbers, shares hidden classes.

use std::cell::RefCell;
use std::collections::HashMap;

use atom_table::{AtomBytes, AtomTable};
use bumpalo::Bump;

use super::{JSONHiddenClass, JSONValue};

/// Owns all JSON nodes (in the arena), uniques strings/numbers, and shares
/// hidden classes. Port of `JSONFactory` (JSONParser.h:524). Accessors take
/// `&self`; the returned `&'a JSONValue` lives in the arena, independent of the
/// transient `RefCell` borrow, so a caller may interleave factory use with
/// parsing (as `JSONParserTest::SmokeTest2` does).
pub struct JSONFactory<'a> {
    arena: &'a Bump,
    atoms: &'a AtomTable,
    strings: RefCell<HashMap<AtomBytes, &'a JSONValue<'a>>>,
    numbers: RefCell<HashMap<u64, &'a JSONValue<'a>>>,
    #[allow(dead_code)] // used in B3 (hidden classes)
    classes: RefCell<HashMap<Box<[AtomBytes]>, &'a JSONHiddenClass<'a>>>,
    null_v: &'a JSONValue<'a>,
    true_v: &'a JSONValue<'a>,
    false_v: &'a JSONValue<'a>,
}

impl<'a> JSONFactory<'a> {
    /// Create a new factory backed by `arena` and `atoms`.
    pub fn new(arena: &'a Bump, atoms: &'a AtomTable) -> JSONFactory<'a> {
        JSONFactory {
            arena,
            atoms,
            strings: RefCell::new(HashMap::new()),
            numbers: RefCell::new(HashMap::new()),
            classes: RefCell::new(HashMap::new()),
            null_v: arena.alloc(JSONValue::Null),
            true_v: arena.alloc(JSONValue::Boolean(true)),
            false_v: arena.alloc(JSONValue::Boolean(false)),
        }
    }

    /// Returns the arena backing this factory.
    pub fn arena(&self) -> &'a Bump {
        self.arena
    }

    /// Returns the atom table used by this factory.
    pub fn atoms(&self) -> &'a AtomTable {
        self.atoms
    }

    /// Returns the singleton null value.
    pub fn get_null(&self) -> &'a JSONValue<'a> {
        self.null_v
    }

    /// Returns the singleton boolean value for `v`.
    pub fn get_boolean(&self, v: bool) -> &'a JSONValue<'a> {
        if v {
            self.true_v
        } else {
            self.false_v
        }
    }

    /// JSONParser.cpp:79 — unique a string by its interned handle.
    pub fn get_string(&self, lit: AtomBytes) -> &'a JSONValue<'a> {
        if let Some(found) = self.strings.borrow().get(&lit) {
            return found;
        }
        let node: &'a JSONValue<'a> = self.arena.alloc(JSONValue::String(lit));
        self.strings.borrow_mut().insert(lit, node);
        node
    }

    /// JSONParser.cpp:92 — intern `str` then unique.
    pub fn get_string_str(&self, s: &str) -> &'a JSONValue<'a> {
        self.get_string(self.atoms.atom_bytes(s))
    }

    /// JSONParser.cpp:96 — unique a number by its bit pattern (so -0.0 != 0.0,
    /// matching `JSONNumber::Profile` using DoubleToBits).
    pub fn get_number(&self, value: f64) -> &'a JSONValue<'a> {
        let bits = value.to_bits();
        if let Some(found) = self.numbers.borrow().get(&bits) {
            return found;
        }
        let node: &'a JSONValue<'a> = self.arena.alloc(JSONValue::Number(value));
        self.numbers.borrow_mut().insert(bits, node);
        node
    }
}

#[cfg(test)]
mod factory_tests {
    use super::super::*;
    use atom_table::AtomTable;
    use bumpalo::Bump;

    #[test]
    fn uniquing_and_singletons() {
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let f = JSONFactory::new(&arena, &atoms);

        // Strings unique by content.
        let a = f.get_string_str("key2");
        let b = f.get_string_str("key2");
        assert!(std::ptr::eq(a, b));
        assert_eq!(a.as_string().map(|h| atoms.bytes(h).to_vec()), Some(b"key2".to_vec()));

        // Numbers unique; -0.0 distinct from 0.0 (NegativeNumbers).
        assert!(std::ptr::eq(f.get_number(1.0), f.get_number(1.0)));
        assert!(!std::ptr::eq(f.get_number(0.0), f.get_number(-0.0)));

        // Singletons.
        assert!(std::ptr::eq(f.get_null(), f.get_null()));
        assert!(std::ptr::eq(f.get_boolean(true), f.get_boolean(true)));
        assert!(!std::ptr::eq(f.get_boolean(true), f.get_boolean(false)));
    }
}
