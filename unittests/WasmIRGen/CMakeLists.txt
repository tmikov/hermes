# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

set(WasmIRGenSources
  WasmIRGenTest.cpp
  )

add_hermes_unittest(WasmIRGenTest
  ${WasmIRGenSources}
  )

target_link_libraries(WasmIRGenTest
  hermesWasmIRGen
  )
