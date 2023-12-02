include(FetchContent)

FetchContent_Declare(
        Corrosion
        GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
        GIT_TAG v0.4.5 # Optionally specify a commit hash, version tag or branch here
)
# Set any global configuration variables such as `Rust_TOOLCHAIN` before this line!
FetchContent_MakeAvailable(Corrosion)

corrosion_import_crate(
        MANIFEST_PATH Cargo.toml
)
target_include_directories(laz-rs-c INTERFACE "${CMAKE_CURRENT_LIST_DIR}/include")

option(PARALLEL_FEATURE "Build laz-rs with parallel capabilities" OFF)

if (PARALLEL_FEATURE)
    corrosion_set_features(laz-rs-c
        FEATURES
            PARALLEL
    )
endif()