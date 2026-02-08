# Configure wabt build options.
set(BUILD_TESTS OFF)
set(BUILD_LIBWASM OFF)
set(USE_INTERNAL_SHA256 ON)

add_subdirectory(wabt EXCLUDE_FROM_ALL)
