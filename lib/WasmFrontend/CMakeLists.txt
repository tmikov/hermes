# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

add_hermes_library(hermesWasmFrontend
    WasmCompile.cpp
    WasmModuleInfo.cpp
    BinaryReaderHermesIRGen.cpp
    LINK_OBJLIBS
    hermesWasmIRGen
    hermesFrontend
    hermesOptimizer
    hermesHBCBackend
    hermesBackend
    hermesSupport
    LINK_LIBS
    wabt
)

# The lean counterpart of hermesWasmFrontend, mirroring the
# hermesHBCBackend/hermesHBCBackendLean pair. It provides only the entry points
# that the VM's WebAssembly.cpp calls, so that the lean VM archive can link
# without dragging in the Wasm compiler. Exactly one of these two libraries
# must be present in any given archive; see WasmCompileStub.cpp for why weak
# symbols cannot be used instead.
add_hermes_library(hermesWasmFrontendLean
    WasmCompileStub.cpp
)
