/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Library half of the unpublished `tools` package.
//!
//! The package's reason for existing is its binaries (`ast-dump`,
//! `sema-dump`, ...), but two things need to be *shared* between a binary and
//! an integration test — and `tools` is `publish = false`, so putting them
//! here cannot affect any published crate's tarball or its verify build. That
//! is the same reason `tests/common_copies_identical.rs` lives here.

pub mod citations;
