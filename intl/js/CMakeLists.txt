# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Compiles intl/js to Hermes bytecode and dumps it as IntlBytecode.inc for
# intl/jsi/IntlBytecode.cpp. Same scheme as API/hermes/extensions.

if(IMPORT_HOST_COMPILERS)
  file(TO_CMAKE_PATH "${IMPORT_HOST_COMPILERS}" IMPORT_HOST_COMPILERS_CMAKE)
  include(${IMPORT_HOST_COMPILERS_CMAKE})
  set(hermesc_EXE imported-hermesc)
else()
  set(hermesc_EXE ${HERMES_TOOLS_OUTPUT_DIR}/hermesc)
endif()

# 00-header.js first and 99-footer.js last; name order between them.
file(GLOB intl_js_sources CONFIGURE_DEPENDS
  ${CMAKE_CURRENT_SOURCE_DIR}/[0-9][0-9]-*.js)
list(SORT intl_js_sources)

add_custom_command(
  OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.js
  COMMAND ${CMAKE_COMMAND} -E cat ${intl_js_sources}
    > ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.js
  DEPENDS ${intl_js_sources}
  VERBATIM)

add_custom_command(
  OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.hbc
  COMMAND ${hermesc_EXE} -O -Wno-undefined-variable -g0 -emit-binary
    -out=${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.hbc
    ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.js
  DEPENDS ${hermesc_EXE} ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.js
  VERBATIM)

add_custom_command(
  OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.inc
  COMMAND ${Python_EXECUTABLE}
    ${PROJECT_SOURCE_DIR}/lib/InternalJavaScript/xxd.py
    ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.hbc
    > ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.inc
  DEPENDS ${PROJECT_SOURCE_DIR}/lib/InternalJavaScript/xxd.py
    ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.hbc
  VERBATIM)

add_custom_target(hermesIntlBytecode
  DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.inc)
# The custom command depends on hermesc by path; make the target order
# explicit too, for generators that do not track files across targets.
if(NOT IMPORT_HOST_COMPILERS)
  add_dependencies(hermesIntlBytecode hermesc)
endif()
