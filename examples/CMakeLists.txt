add_executable(c_decompressor c_decompressor.c)
target_link_libraries(c_decompressor lazrs minilas)

add_executable(cpp_decompressor cpp_decompressor.cpp)
target_link_libraries(cpp_decompressor lazrs_cpp minilas)

add_executable(cpp_pardecompressor cpp_pardecompressor.cpp)
target_link_libraries(cpp_pardecompressor lazrs_cpp minilas)

if (WIN32)
    add_custom_command(
            TARGET c_decompressor POST_BUILD
            COMMAND "${CMAKE_COMMAND}" -E copy "${CARGO_LIB_DLL}" "$<TARGET_FILE_DIR:c_decompressor>"
    )
endif ()
