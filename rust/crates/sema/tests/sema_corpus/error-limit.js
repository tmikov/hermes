/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// THE ERROR LIMIT. `hermesc`'s driver sets one — `-ferror-limit`, `init(20)`
// (CompilerDriver.cpp:555-559) applied by
// `context->getSourceErrorManager().setErrorLimit(cl::ErrorLimit)`
// (CompilerDriver.cpp:1223) — while a bare `SourceErrorManager` starts
// unlimited (`errorLimit_` = `UINT32_MAX`, SourceErrorManager.h). The S2 T8
// sweep found `sema-dump` was never applying the driver's limit, so any input
// with more than 20 errors diverged; the corpus had never noticed because its
// noisiest other file (`error-private-load-store.js`) stops at 15.
//
// This file has 26 errors, so it pins all four observable halves of
// `countAndGenMessage` (SourceErrorManager.cpp:124-136) + `message`
// (:172-190):
//
//  1. exactly the first 20 GENERATED errors are reported;
//  2. GENERATED, not first-by-location: the 20th survivor is the
//     `Identifier 'redeclaredAfterTheLimit' is already declared` error from
//     the LAST line of the file, because redeclaration errors come from the
//     declaration-collection pass that runs before the statement walk
//     (`processDeclarations`/`validateAndDeclareIdentifier`) — so only 19 of
//     the 25 `break`s get in, even though every one of them is at an earlier
//     source location. The location sort then puts that error back where it
//     was written — line 77, last of the 20 — when the buffer is flushed;
//  3. `<unknown>:0: error: too many errors emitted` is appended exactly once,
//     with an invalid location, and the buffered flush's comparator keeps it
//     LAST regardless of that location sort (SourceErrorManager.cpp:61-71);
//  4. once `errorLimitReached_` is set, `message()` drops EVERYTHING — the
//     six `break`s past the cut and the `undeclaredGlobalAfterTheLimit`
//     warning. Its surviving `note: previous declaration` is the contrast: a
//     note is attached to its primary message when that message is buffered
//     (`doGenMessage`, :138-155), so it rides along with an error that DID
//     make the cut.
//
// The sentinel is not counted as a message (it goes straight to
// `doGenMessage`), which is why the driver epilogue says
// `Emitted 20 errors. exiting.` and not 21.
//
// NOTE: 20 is only the DEFAULT — `sema-dump` now takes `--ferror-limit` like
// hermesc's `-ferror-limit` (0 = unlimited), but the corpus has no per-file
// flag mechanism, so what this file pins is that shared default.

break;
break;
break;
break;
break;
break;
break;
break;
break;
break;
break;
break;
break;
break;
break;
break;
break;
break;
break;
// ^ those 19 are the survivors; the 20th is the redeclaration error at the
// bottom of the file, so every message from here on is suppressed.
break;
break;
break;
break;
break;
break;
undeclaredGlobalAfterTheLimit;
let redeclaredAfterTheLimit;
let redeclaredAfterTheLimit;
