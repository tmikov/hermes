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
