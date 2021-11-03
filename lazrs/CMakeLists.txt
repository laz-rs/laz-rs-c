if(WIN32)
    set(LIB_NAME laz_rs_c)
else()
    set(LIB_NAME liblaz_rs_c)
endif()

if(DEFINED ENV{CARGO_TARGET_DIR})
    set(CARGO_TARGET_DIR "$ENV{CARGO_TARGET_DIR}")
else()
    set(CARGO_TARGET_DIR "${CMAKE_CURRENT_LIST_DIR}/target")
endif()

set(CARGO_COMMAND cargo build)

if(CMAKE_CONFIGURATION_TYPES)
    message(SEND_ERROR "multi-config generators are not supported; use make, ninja or nmake")
else()
    if(CMAKE_BUILD_TYPE STREQUAL "Debug")
        set(CARGO_TARGET "${CARGO_TARGET_DIR}/debug/${LIB_NAME}")
    else()
        set(CARGO_TARGET "${CARGO_TARGET_DIR}/release/${LIB_NAME}")
        list(APPEND CARGO_COMMAND --release)
    endif()
endif()

add_custom_target(
        cargo_build
        ALL
        COMMAND ${CARGO_COMMAND}
        WORKING_DIRECTORY "${CMAKE_CURRENT_LIST_DIR}"
        VERBATIM
)

add_library(lazrs SHARED IMPORTED GLOBAL)
add_dependencies(lazrs cargo_build)
target_include_directories(lazrs INTERFACE "${CMAKE_CURRENT_LIST_DIR}/include")
if(WIN32)
    set(CARGO_LIB "${CARGO_TARGET}.dll.lib")
    set_target_properties(lazrs PROPERTIES IMPORTED_LOCATION "${CARGO_TARGET}.dll")
    set_target_properties(lazrs PROPERTIES IMPORTED_IMPLIB "${CARGO_LIB}")
    set(CARGO_LIB_DLL "${CARGO_TARGET}.dll" PARENT_SCOPE)
else()
    set(CARGO_LIB "${CARGO_TARGET}.so")
    set_target_properties(lazrs PROPERTIES IMPORTED_LOCATION "${CARGO_LIB}")
endif()