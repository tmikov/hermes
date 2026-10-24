# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by osx-architectures.js: hermes_intl_osx_arch_mismatch on each case.

include(${CMAKE_CURRENT_LIST_DIR}/../../../../cmake/modules/HermesIntl.cmake)

set(arm_host "cargo 1.90.0\nrelease: 1.90.0\nhost: aarch64-apple-darwin\n")
set(intel_host "cargo 1.90.0\nrelease: 1.90.0\nhost: x86_64-apple-darwin\n")

# Print the case and the result.
function(check archs cargo_vv)
  hermes_intl_osx_arch_mismatch("${archs}" "${cargo_vv}" mismatch)
  string(REGEX MATCH "host: [^\n]*" host "${cargo_vv}")
  message("[${archs}] [${host}]: ${mismatch}")
endfunction()

check("arm64" "${arm_host}")
check("x86_64" "${arm_host}")
check("arm64" "${intel_host}")
check("x86_64" "${intel_host}")
check("arm64;x86_64" "${arm_host}")
check("" "${arm_host}")
check("arm64" "")
