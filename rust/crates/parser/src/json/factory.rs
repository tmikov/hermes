/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! JSONFactory: owns the arena, uniques strings/numbers, shares hidden classes.

use std::cell::RefCell;
use std::collections::HashMap;

use bumpalo::Bump;
use hermes_atom_table::{AtomBytes, AtomTable};

use super::{JSONHiddenClass, JSONValue};

/// A single property: (key, value). The key is a `JSONValue::String`.
pub type Prop<'a> = (&'a JSONValue<'a>, &'a JSONValue<'a>);

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

    fn key_bytes(&self, key: &'a JSONValue<'a>) -> AtomBytes {
        key.as_string().expect("object key must be a JSON string")
    }

    /// JSONParser.cpp:120 — sort props by key content; return the first
    /// duplicate key (by interned identity) if any, else None.
    pub fn sort_props(&self, props: &mut [Prop<'a>]) -> Option<AtomBytes> {
        props.sort_by(|a, b| {
            self.atoms
                .bytes(self.key_bytes(a.0))
                .cmp(self.atoms.bytes(self.key_bytes(b.0)))
        });
        let mut last: Option<AtomBytes> = None;
        for p in props.iter() {
            let kb = self.key_bytes(p.0);
            if last == Some(kb) {
                return Some(kb);
            }
            last = Some(kb);
        }
        None
    }

    /// JSONParser.cpp:109 — look up or create the shared hidden class for `keys`
    /// (already content-sorted).
    pub fn get_hidden_class(&self, keys: &[AtomBytes]) -> &'a JSONHiddenClass<'a> {
        if let Some(found) = self.classes.borrow().get(keys) {
            return found;
        }
        let arena_keys: &'a [AtomBytes] = self.arena.alloc_slice_copy(keys);
        let cls: &'a JSONHiddenClass<'a> =
            self.arena.alloc(JSONHiddenClass { keys: arena_keys });
        self.classes.borrow_mut().insert(keys.into(), cls);
        cls
    }

    /// JSONParser.cpp:138 — create an object from props. Sorts + dedups; returns
    /// None on a duplicate key.
    pub fn new_object(&self, props: &mut [Prop<'a>]) -> Option<&'a JSONValue<'a>> {
        if self.sort_props(props).is_some() {
            return None;
        }
        let keys: Vec<AtomBytes> = props.iter().map(|p| self.key_bytes(p.0)).collect();
        let cls = self.get_hidden_class(&keys);
        let values: Vec<&'a JSONValue<'a>> = props.iter().map(|p| p.1).collect();
        let values: &'a [&'a JSONValue<'a>] = self.arena.alloc_slice_copy(&values);
        Some(self.arena.alloc(JSONValue::Object(cls, values)))
    }

    /// JSONParser.cpp:138 with propsAreSorted=true — props already sorted and
    /// dup-checked.
    pub fn new_object_sorted(&self, props: &[Prop<'a>]) -> Option<&'a JSONValue<'a>> {
        let keys: Vec<AtomBytes> = props.iter().map(|p| self.key_bytes(p.0)).collect();
        let cls = self.get_hidden_class(&keys);
        let values: Vec<&'a JSONValue<'a>> = props.iter().map(|p| p.1).collect();
        let values: &'a [&'a JSONValue<'a>] = self.arena.alloc_slice_copy(&values);
        Some(self.arena.alloc(JSONValue::Object(cls, values)))
    }

    /// JSONParser.h:617 — create an array from values.
    pub fn new_array(&self, values: &[&'a JSONValue<'a>]) -> &'a JSONValue<'a> {
        let values: &'a [&'a JSONValue<'a>] = self.arena.alloc_slice_copy(values);
        self.arena.alloc(JSONValue::Array(values))
    }
}

#[cfg(test)]
mod factory_tests {
    use super::super::*;
    use bumpalo::Bump;
    use hermes_atom_table::AtomTable;

    #[test]
    fn objects_arrays_and_hidden_class_sharing() {
        use super::super::JSONFactory;
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let f = JSONFactory::new(&arena, &atoms);

        fn mk<'b>(f: &'b JSONFactory<'b>, k1: f64, k2: f64) -> &'b super::super::JSONValue<'b> {
            // object {'key1': k1, 'key2': k2} via unsorted props
            let p1 = (f.get_string_str("key1"), f.get_number(k1));
            let p2 = (f.get_string_str("key2"), f.get_number(k2));
            f.new_object(&mut [p2, p1]).unwrap() // intentionally unsorted
        }
        let o1 = mk(&f, 1.0, 2.0);
        let o3 = mk(&f, 20.0, 10.0);

        let v1 = o1.as_object().unwrap();
        assert_eq!(v1.size(), 2);
        // shared hidden class for same-shape objects (HiddenClassTest).
        assert!(std::ptr::eq(
            v1.get_hidden_class(),
            o3.as_object().unwrap().get_hidden_class()
        ));
        // lookups
        assert_eq!(v1.count("key1", &atoms), 1);
        assert_eq!(v1.count("zzz", &atoms), 0);
        assert_eq!(v1.get("key1", &atoms).and_then(|v| v.as_number()), Some(1.0));
        // duplicate keys -> error
        let dup = (f.get_string_str("k"), f.get_number(1.0));
        assert!(f.new_object(&mut [dup, dup]).is_none());

        // arrays
        let a = f.new_array(&[f.get_number(5.0), f.get_null()]);
        let av = a.as_array().unwrap();
        assert_eq!(av.len(), 2);
        assert_eq!(av.at(0).as_number(), Some(5.0));
    }

    #[test]
    fn object_positional_accessors_and_iter_sorted_order() {
        use super::super::JSONFactory;
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let f = JSONFactory::new(&arena, &atoms);
        // Insert keys out of order; hidden class / iteration is SORTED order.
        let obj = f
            .new_object(&mut [
                (f.get_string_str("b"), f.get_number(2.0)),
                (f.get_string_str("a"), f.get_number(1.0)),
                (f.get_string_str("c"), f.get_number(3.0)),
            ])
            .unwrap();
        let o = obj.as_object().unwrap();
        assert_eq!(o.size(), 3);
        // value_at / key_at follow sorted key order: a,b,c
        assert_eq!(atoms.bytes(o.key_at(0)), b"a");
        assert_eq!(o.value_at(0).as_number(), Some(1.0));
        assert_eq!(atoms.bytes(o.key_at(2)), b"c");
        assert_eq!(o.value_at(2).as_number(), Some(3.0));
        // iter yields (key, value) pairs in sorted order.
        let collected: Vec<(Vec<u8>, f64)> = o
            .iter()
            .map(|(k, v)| (atoms.bytes(k).to_vec(), v.as_number().unwrap()))
            .collect();
        assert_eq!(
            collected,
            vec![(b"a".to_vec(), 1.0), (b"b".to_vec(), 2.0), (b"c".to_vec(), 3.0)]
        );
    }

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
