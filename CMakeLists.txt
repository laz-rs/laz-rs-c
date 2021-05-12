cmake_minimum_required(VERSION 3.17)
project(test_lasrsc)

set(LIB_NAME laz_rs_c)



if (DEFINED ENV{CARGO_TARGET_DIR})
    set(CARGO_TARGET_DIR "$ENV{CARGO_TARGET_DIR}")
else()
    set(CARGO_TARGET_DIR "${CMAKE_CURRENT_LIST_DIR}/target")
endif()
# custom_target's COMMAND doesn't work if I give him a string
# so i don't know how to factorize this
if (CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(CARGO_TARGET "${CARGO_TARGET_DIR}/debug/${LIB_NAME}")
    add_custom_target(
        cargo_build
        ALL
        COMMAND cargo build
        WORKING_DIRECTORY "${CMAKE_CURRENT_LIST_DIR}"
        VERBATIM
    )
else()
    set(CARGO_TARGET "${CARGO_TARGET_DIR}/release/${LIB_NAME}")
    add_custom_target(
        cargo_build
        ALL
        COMMAND cargo build --release
        WORKING_DIRECTORY "${CMAKE_CURRENT_LIST_DIR}"
        VERBATIM
    )
endif()


message("${CARGO_TARGET}")
add_executable(test_lazrsc test_lazrsc.c)
target_include_directories(test_lazrsc PRIVATE "${CMAKE_CURRENT_LIST_DIR}")
target_link_libraries(test_lazrsc PRIVATE "${CARGO_TARGET}.dll.lib")

add_dependencies(test_lazrsc cargo_build)

add_custom_command(
        TARGET test_lazrsc POST_BUILD
        COMMAND "${CMAKE_COMMAND}" -E copy "${CARGO_TARGET}.dll" "$<TARGET_FILE_DIR:test_lazrsc>"
)