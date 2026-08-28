# Configure wabt build options.
set(BUILD_TESTS OFF)
set(BUILD_LIBWASM OFF)
set(USE_INTERNAL_SHA256 ON)

# wabt is linked into libhermesvm.so (WebAssembly.validate() uses wabt's
# semantic validator), so its objects must be position-independent.
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

add_subdirectory(wabt EXCLUDE_FROM_ALL)
