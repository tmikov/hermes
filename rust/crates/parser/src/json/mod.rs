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
