add_subdirectory(dtoa)
# The Node-API test suites build addons that bind to the host executable's
# NAPI symbols when loaded; Android's NDK links shared libraries with
# --no-undefined, and the suites do not run there.
if(HERMES_ENABLE_NAPI AND NOT ANDROID)
  add_subdirectory(node-api-cts/implementors/hermes)
  add_subdirectory(node-api-tests/hermes)

  # Combined target to run all NAPI tests (CTS + Node.js).
  add_custom_target(check-napi
    DEPENDS check-napi-cts check-napi-node-tests
    COMMENT "Running all Node-API tests under Hermes"
  )
endif()

add_subdirectory(zip)
if(HERMESVM_ALLOW_JIT)
  add_subdirectory(asmjit)
endif()
add_subdirectory(llvh)
if (HERMES_USE_BOOST_CONTEXT)
  add_subdirectory(boost)
endif()
