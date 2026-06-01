//! Unicode character properties for the JS lexer, ported from
//! include/hermes/Platform/Unicode/CharacterProperties.{h,cpp}. The range tables
//! in `tables` are generated from lib/Platform/Unicode/UnicodeData.inc by
//! gen_tables.py and pinned to Hermes's Unicode version (17.0.0). RegExp
//! canonicalization / property escapes are intentionally not ported here.

mod tables;
