add_library(minilas)

target_include_directories(minilas PUBLIC "${CMAKE_CURRENT_LIST_DIR}/include")

target_sources(minilas PRIVATE
        "${CMAKE_CURRENT_LIST_DIR}/src/las.c"
        "${CMAKE_CURRENT_LIST_DIR}/include/minilas/las.h"
        )