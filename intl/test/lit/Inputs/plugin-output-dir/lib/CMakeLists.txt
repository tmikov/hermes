# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

add_library(hermesvm SHARED empty.c)
if(FAKE_FRAMEWORK)
  set_target_properties(hermesvm PROPERTIES FRAMEWORK TRUE)
endif()
if(VM_DIR)
  set_target_properties(hermesvm PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${VM_DIR})
endif()
if(VM_RELEASE_DIR)
  set_target_properties(hermesvm PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY_RELEASE ${CMAKE_BINARY_DIR}/${VM_RELEASE_DIR})
endif()
