/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of unittests/Parser/JSONParserTest.cpp (5 cases).

use bumpalo::Bump;
use hermes_atom_table::AtomTable;
use hermes_parser::json::{JSONFactory, JSONParser, JSONValue};
use hermes_support::manager::SourceErrorManager;

/// Mirror of the C++ setup: factory in the arena, parse a source string.
fn parse<'a>(
    arena: &'a Bump,
    atoms: &'a AtomTable,
    sm: &'a mut SourceErrorManager,
    src: &str,
) -> (Option<&'a JSONValue<'a>>, u32) {
    let f = arena.alloc(JSONFactory::new(arena, atoms));
    let id = sm.add_buffer("json", src);
    let mut p = JSONParser::new(f, id, sm, atoms, false);
    let r = p.parse();
    let errs = p.error_count();
    (r, errs)
}

#[test]
fn smoke_test_1() {
    // JSONParserTest::SmokeTest1
    let src = "{\n  '6': null,\n  '1': null,\n  '2': null,\n  '3': null,\n  '4': null,\n  '5': null\n}";
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let mut sm = SourceErrorManager::new();
    let (parsed, _) = parse(&arena, &atoms, &mut sm, src);
    assert!(parsed.is_some());
}

#[test]
fn smoke_test_2() {
    // JSONParserTest::SmokeTest2 — parse + accessors + uniquing.
    let src = "{ 'key1' : 1, 'key2' : 'value2', 'key3' : {'nested1': true}, \"key4\" : [false, null, 'value2']}";
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let f = arena.alloc(JSONFactory::new(&arena, &atoms));
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("json", src);
    // Pre-parse factory sanity (C++ JSONParserTest.cpp:53-59).
    assert_eq!(atoms.bytes(f.get_string_str("key4").as_string().unwrap()), b"key4");
    assert_eq!(atoms.bytes(f.get_string_str("key3").as_string().unwrap()), b"key3");
    assert_eq!(atoms.bytes(f.get_string_str("key2").as_string().unwrap()), b"key2");
    assert_eq!(atoms.bytes(f.get_string_str("key1").as_string().unwrap()), b"key1");
    assert_eq!(atoms.bytes(f.get_string_str("key0").as_string().unwrap()), b"key0");
    assert_eq!(f.get_number(1.0).as_number(), Some(1.0));
    assert!(std::ptr::eq(f.get_string_str("key2"), f.get_string_str("key2"))); // uniquing
    // End p's borrow of sm before using f for pointer comparisons.
    let t1 = {
        let mut p = JSONParser::new(f, id, &mut sm, &atoms, false);
        p.parse().unwrap()
    };
    let o1 = t1.as_object().unwrap();
    assert_eq!(o1.size(), 4);
    assert_eq!(o1.count("key0", &atoms), 0);
    assert!(o1.get("key0", &atoms).is_none());
    assert_eq!(o1.count("key1", &atoms), 1);
    assert!(std::ptr::eq(o1.get("key1", &atoms).unwrap(), f.get_number(1.0)));
    let value2 = o1.get("key2", &atoms).unwrap();
    assert!(std::ptr::eq(value2, f.get_string_str("value2"))); // uniquing
    // nested object
    let o2 = o1.get("key3", &atoms).unwrap().as_object().unwrap();
    assert!(std::ptr::eq(o2.get("nested1", &atoms).unwrap(), f.get_boolean(true)));
    assert_eq!(o1.count("key3", &atoms), 1);
    assert_eq!(o1.count("key4", &atoms), 1);
    // Keys iterate in (sorted) order key1,key2,key3,key4 (C++ lines 91-105).
    let keys: Vec<Vec<u8>> = o1.iter().map(|(k, _)| atoms.bytes(k).to_vec()).collect();
    assert_eq!(
        keys,
        vec![
            b"key1".to_vec(),
            b"key2".to_vec(),
            b"key3".to_vec(),
            b"key4".to_vec()
        ]
    );
    // array, incl. shared 'value2' node
    let a1 = o1.get("key4", &atoms).unwrap().as_array().unwrap();
    assert_eq!(a1.len(), 3);
    assert!(std::ptr::eq(a1.at(0), f.get_boolean(false)));
    assert!(std::ptr::eq(a1.at(1), f.get_null()));
    assert!(std::ptr::eq(a1.at(2), value2));
}

#[test]
fn negative_numbers() {
    // JSONParserTest::NegativeNumbers
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let mut sm = SourceErrorManager::new();
    let (t1, _) = parse(&arena, &atoms, &mut sm, "[-1.0, -1, -0]");
    let a1 = t1.unwrap().as_array().unwrap();
    let expected = [-1.0f64, -1.0, -0.0];
    assert_eq!(a1.len(), expected.len());
    for (i, &e) in expected.iter().enumerate() {
        let actual = a1.at(i).as_number().unwrap();
        // distinguish -0.0 from 0.0 via bit pattern, as the C++ ASSERT_EQ does.
        assert_eq!(actual.to_bits(), e.to_bits(), "elem {i}");
    }
    // lone "-" -> failure, error count 1 (fresh manager).
    let mut sm2 = SourceErrorManager::new();
    let (t2, errs) = parse(&arena, &atoms, &mut sm2, "-");
    assert!(t2.is_none());
    assert_eq!(errs, 1);
}

#[test]
fn hidden_class_test() {
    // JSONParserTest::HiddenClassTest — same-shape objects share one class.
    let src = "[ {'key1': 1, 'key2': {'key2': 5, 'key1': 6}}, {'key2': 10, 'key1': 20}]";
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let mut sm = SourceErrorManager::new();
    let (t1, _) = parse(&arena, &atoms, &mut sm, src);
    let array = t1.unwrap().as_array().unwrap();
    assert_eq!(array.len(), 2);
    let o1 = array.at(0).as_object().unwrap();
    let o2 = o1.get("key2", &atoms).unwrap().as_object().unwrap();
    assert!(std::ptr::eq(o1.get_hidden_class(), o2.get_hidden_class()));
    let o3 = array.at(1).as_object().unwrap();
    assert!(std::ptr::eq(o1.get_hidden_class(), o3.get_hidden_class()));
}

#[test]
fn emit_test() {
    // JSONParserTest::EmitTest — parse then emit, compare bytes.
    use hermes_support::json_emitter::JSONEmitter;
    let src = "{ 'key1' : 1, 'key2' : 'value2', 'key3' : {'nested1': true}, \"key4\" : [false, null, 'value2']}";
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let mut sm = SourceErrorManager::new();
    let (t1, _) = parse(&arena, &atoms, &mut sm, src);
    let mut s = String::new();
    {
        let mut e = JSONEmitter::new(&mut s, false);
        t1.unwrap().emit_into(&mut e, &atoms);
    }
    assert_eq!(
        s,
        r#"{"key1":1,"key2":"value2","key3":{"nested1":true},"key4":[false,null,"value2"]}"#
    );
}
