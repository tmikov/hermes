/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Faithful Rust port of Hermes' JSONParser (include/hermes/Parser/JSONParser.h,
//! lib/Parser/JSONParser.cpp): the JSON value model, the uniquing/hidden-class
//! `JSONFactory`, and the recursive-descent `JSONParser` over `JSLexer`.

pub mod factory;
pub mod parser;

pub use factory::{JSONFactory, Prop};

use atom_table::AtomBytes;

/// Port of `JSONKind` (JSONParser.h:36).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JSONKind {
    Object,
    Array,
    String,
    Number,
    Boolean,
    Null,
}

/// Port of `JSONKindToString` (JSONParser.cpp:21).
pub fn kind_to_string(kind: JSONKind) -> &'static str {
    match kind {
        JSONKind::Object => "Object",
        JSONKind::Array => "Array",
        JSONKind::String => "String",
        JSONKind::Number => "Number",
        JSONKind::Boolean => "Boolean",
        JSONKind::Null => "Null",
    }
}

/// A descriptor with a sorted list of names; objects of the same shape share one
/// (JSONParser.h:180 `JSONHiddenClass`). `keys` are sorted by string content.
pub struct JSONHiddenClass<'a> {
    pub(crate) keys: &'a [AtomBytes],
}

impl<'a> JSONHiddenClass<'a> {
    /// Returns the number of keys in the hidden class.
    pub fn size(&self) -> usize {
        self.keys.len()
    }

    /// Returns the sorted slice of key atoms.
    pub fn keys(&self) -> &'a [AtomBytes] {
        self.keys
    }

    /// JSONParser.h:225 — binary-search the sorted keys for `name` (compared by
    /// bytes); return its index. `atoms` resolves AtomBytes -> bytes.
    pub fn find(&self, name: &[u8], atoms: &atom_table::AtomTable) -> Option<usize> {
        self.keys
            .binary_search_by(|k| atoms.bytes(*k).cmp(name))
            .ok()
    }
}

/// The base type for all JSON values (JSONParser.h:49). `&'a JSONValue<'a>` IS
/// the C++ `JSONValue*`: nodes live in a `bumpalo` arena; the variant replaces
/// the kind tag + LLVM RTTI; arena identity gives pointer equality.
pub enum JSONValue<'a> {
    Null,
    Boolean(bool),
    Number(f64),
    String(AtomBytes),
    Array(&'a [&'a JSONValue<'a>]),
    Object(&'a JSONHiddenClass<'a>, &'a [&'a JSONValue<'a>]),
}

impl<'a> JSONValue<'a> {
    /// Returns the `JSONKind` tag for this value.
    pub fn kind(&self) -> JSONKind {
        match self {
            JSONValue::Null => JSONKind::Null,
            JSONValue::Boolean(_) => JSONKind::Boolean,
            JSONValue::Number(_) => JSONKind::Number,
            JSONValue::String(_) => JSONKind::String,
            JSONValue::Array(_) => JSONKind::Array,
            JSONValue::Object(..) => JSONKind::Object,
        }
    }

    /// Returns `Some(f)` if this is a `Number`, else `None`.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            JSONValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns `Some(b)` if this is a `Boolean`, else `None`.
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            JSONValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns the interned handle (resolve bytes via the AtomTable).
    pub fn as_string(&self) -> Option<AtomBytes> {
        match self {
            JSONValue::String(a) => Some(*a),
            _ => None,
        }
    }

    /// Returns an `ArrayView` if this is an `Array`, else `None`.
    pub fn as_array(&self) -> Option<ArrayView<'a>> {
        match self {
            JSONValue::Array(v) => Some(ArrayView { values: v }),
            _ => None,
        }
    }

    /// Returns an `ObjectView` if this is an `Object`, else `None`.
    pub fn as_object(&self) -> Option<ObjectView<'a>> {
        match self {
            JSONValue::Object(c, v) => Some(ObjectView { class: c, values: v }),
            _ => None,
        }
    }

    /// Port of `JSONValue::emitInto` (JSONParser.cpp:39). `atoms` resolves
    /// interned string handles to bytes. Strings must be valid UTF-8.
    pub fn emit_into(
        &self,
        emitter: &mut support::json_emitter::JSONEmitter,
        atoms: &atom_table::AtomTable,
    ) {
        match self {
            JSONValue::Object(class, values) => {
                emitter.open_dict();
                for (k, v) in class.keys.iter().copied().zip(values.iter().copied()) {
                    let key =
                        std::str::from_utf8(atoms.bytes(k)).expect("valid UTF-8 key");
                    emitter.emit_key(key);
                    v.emit_into(emitter, atoms);
                }
                emitter.close_dict();
            }
            JSONValue::Array(values) => {
                emitter.open_array();
                for v in values.iter().copied() {
                    v.emit_into(emitter, atoms);
                }
                emitter.close_array();
            }
            JSONValue::String(a) => {
                let s =
                    std::str::from_utf8(atoms.bytes(*a)).expect("valid UTF-8 string");
                emitter.emit_str(s);
            }
            JSONValue::Number(n) => emitter.emit_f64(*n),
            JSONValue::Boolean(b) => emitter.emit_bool(*b),
            JSONValue::Null => emitter.emit_null_value(),
        }
    }
}

/// Borrowed view over an array (JSONParser.h:458 `JSONArray`).
pub struct ArrayView<'a> {
    values: &'a [&'a JSONValue<'a>],
}

impl<'a> ArrayView<'a> {
    /// Returns the number of elements in the array.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if the array is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Element at `pos`; panics if out of bounds (faithful to C++ `JSONArray::at`).
    pub fn at(&self, pos: usize) -> &'a JSONValue<'a> {
        self.values[pos]
    }

    /// Returns an iterator over `&'a JSONValue<'a>` references.
    pub fn iter(&self) -> impl Iterator<Item = &'a JSONValue<'a>> + '_ {
        self.values.iter().copied()
    }
}

/// Borrowed view over an object (JSONParser.h:239 `JSONObject`). Grown in B3
/// with name lookups + iteration.
pub struct ObjectView<'a> {
    pub(crate) class: &'a JSONHiddenClass<'a>,
    pub(crate) values: &'a [&'a JSONValue<'a>],
}

impl<'a> ObjectView<'a> {
    /// Number of members (faithful to C++ `JSONObject::size`).
    pub fn size(&self) -> usize {
        self.values.len()
    }

    /// Returns the hidden class descriptor shared among same-shape objects.
    pub fn get_hidden_class(&self) -> &'a JSONHiddenClass<'a> {
        self.class
    }

    /// JSONParser.h:286 — value for `name`, or None.
    pub fn get(&self, name: &str, atoms: &atom_table::AtomTable) -> Option<&'a JSONValue<'a>> {
        self.class.find(name.as_bytes(), atoms).map(|i| self.values[i])
    }

    /// JSONParser.h:295 — value for `name`; panics if absent (C++ asserts).
    pub fn at(&self, name: &str, atoms: &atom_table::AtomTable) -> &'a JSONValue<'a> {
        self.get(name, atoms).expect("name not found")
    }

    /// JSONParser.h:323 — 1 if present else 0.
    pub fn count(&self, name: &str, atoms: &atom_table::AtomTable) -> usize {
        if self.class.find(name.as_bytes(), atoms).is_some() {
            1
        } else {
            0
        }
    }

    /// Value by position (0..size). Panics if out of range.
    pub fn value_at(&self, index: usize) -> &'a JSONValue<'a> {
        self.values[index]
    }

    /// Key (interned handle) by position. Panics if out of range.
    pub fn key_at(&self, index: usize) -> atom_table::AtomBytes {
        self.class.keys[index]
    }

    /// JSONParser.h:330 — (key, value) pairs, in the hidden class's sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (atom_table::AtomBytes, &'a JSONValue<'a>)> + '_ {
        self.class.keys.iter().copied().zip(self.values.iter().copied())
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn kinds_and_scalar_accessors() {
        let arena = Bump::new();
        let n: &JSONValue = arena.alloc(JSONValue::Number(1.5));
        let b: &JSONValue = arena.alloc(JSONValue::Boolean(true));
        assert_eq!(n.kind(), JSONKind::Number);
        assert_eq!(b.kind(), JSONKind::Boolean);
        assert_eq!(n.as_number(), Some(1.5));
        assert_eq!(b.as_boolean(), Some(true));
        assert_eq!(n.as_boolean(), None);
        assert_eq!(JSONValue::Null.kind(), JSONKind::Null);
        assert_eq!(kind_to_string(JSONKind::Array), "Array");
    }

    #[test]
    fn array_accessors() {
        let arena = Bump::new();
        let a = arena.alloc(JSONValue::Number(10.0));
        let b = arena.alloc(JSONValue::Number(20.0));
        let elems: &[&JSONValue] = arena.alloc_slice_copy(&[&*a, &*b]);
        let arr = arena.alloc(JSONValue::Array(elems));
        let view = arr.as_array().unwrap();
        assert_eq!(view.len(), 2);
        assert_eq!(view.at(0).as_number(), Some(10.0));
        assert_eq!(view.iter().count(), 2);
    }

    #[test]
    fn kind_to_string_all_variants() {
        use JSONKind::*;
        let pairs = [
            (Object, "Object"),
            (Array, "Array"),
            (String, "String"),
            (Number, "Number"),
            (Boolean, "Boolean"),
            (Null, "Null"),
        ];
        for (k, s) in pairs {
            assert_eq!(kind_to_string(k), s);
        }
    }

    #[test]
    fn emit_into_round_trip() {
        use super::JSONFactory;
        use atom_table::AtomTable;
        use bumpalo::Bump;
        use support::json_emitter::JSONEmitter;

        let arena = Bump::new();
        let atoms = AtomTable::new();
        let f = JSONFactory::new(&arena, &atoms);

        // {'key1':1,'key2':'value2','key3':{'nested1':true},'key4':[false,null,'value2']}
        let nested = {
            let p = (f.get_string_str("nested1"), f.get_boolean(true));
            f.new_object(&mut [p]).unwrap()
        };
        let arr = f.new_array(&[f.get_boolean(false), f.get_null(), f.get_string_str("value2")]);
        let obj = f.new_object(&mut [
            (f.get_string_str("key1"), f.get_number(1.0)),
            (f.get_string_str("key2"), f.get_string_str("value2")),
            (f.get_string_str("key3"), nested),
            (f.get_string_str("key4"), arr),
        ]).unwrap();

        let mut s = String::new();
        {
            let mut e = JSONEmitter::new(&mut s, false);
            obj.emit_into(&mut e, &atoms);
        }
        // sorted-key order: key1,key2,key3,key4
        assert_eq!(s, r#"{"key1":1,"key2":"value2","key3":{"nested1":true},"key4":[false,null,"value2"]}"#);
    }

    #[test]
    fn string_accessor_and_hidden_class_find() {
        use atom_table::AtomTable;
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let a = atoms.atom_bytes("foo");
        let s = arena.alloc(JSONValue::String(a));
        assert_eq!(s.as_string(), Some(a));
        assert_eq!(s.as_number(), None);

        // sorted keys: "a","b","c" -> find by bytes
        let ka = atoms.atom_bytes("a");
        let kb = atoms.atom_bytes("b");
        let kc = atoms.atom_bytes("c");
        let keys: &[atom_table::AtomBytes] = arena.alloc_slice_copy(&[ka, kb, kc]);
        let hc = JSONHiddenClass { keys };
        assert_eq!(hc.find(b"a", &atoms), Some(0));
        assert_eq!(hc.find(b"b", &atoms), Some(1));
        assert_eq!(hc.find(b"c", &atoms), Some(2));
        assert_eq!(hc.find(b"z", &atoms), None);
    }
}
