set(CMAKE_LEGACY_CYGWIN_WIN32 0)
cmake_minimum_required(VERSION 3.5)

project(cJSON
    VERSION 1.7.19
    LANGUAGES C)

cmake_policy(SET CMP0054 NEW)  # set CMP0054 policy

include(GNUInstallDirs)

set(CJSON_VERSION_SO 1)
set(CJSON_UTILS_VERSION_SO 1)

set(custom_compiler_flags)

include(CheckCCompilerFlag)
option(ENABLE_CUSTOM_COMPILER_FLAGS "Enables custom compiler flags" ON)
if (ENABLE_CUSTOM_COMPILER_FLAGS)
    if (("${CMAKE_C_COMPILER_ID}" STREQUAL "Clang") OR ("${CMAKE_C_COMPILER_ID}" STREQUAL "GNU"))
        list(APPEND custom_compiler_flags
            -std=c89
            -pedantic
            -Wall
            -Wextra
            -Werror
            -Wstrict-prototypes
            -Wwrite-strings
            -Wshadow
            -Winit-self
            -Wcast-align
            -Wformat=2
            -Wmissing-prototypes
            -Wstrict-overflow=2
            -Wcast-qual
            -Wundef
            -Wswitch-default
            -Wconversion
            -Wc++-compat
            -fstack-protector-strong
            -Wcomma
            -Wdouble-promotion
            -Wparentheses
            -Wformat-overflow
            -Wunused-macros
            -Wmissing-variable-declarations
            -Wused-but-marked-unused
            -Wswitch-enum
        )
    elseif("${CMAKE_C_COMPILER_ID}" STREQUAL "MSVC")
        # Disable warning c4001 - nonstandard extension 'single line comment' was used
        # Define _CRT_SECURE_NO_WARNINGS to disable deprecation warnings for "insecure" C library functions
        list(APPEND custom_compiler_flags
            /GS
            /Za
            /sdl
            /W4
            /wd4001
            /D_CRT_SECURE_NO_WARNINGS
        )
    endif()
endif()

option(ENABLE_SANITIZERS "Enables AddressSanitizer and UndefinedBehaviorSanitizer." OFF)
if (ENABLE_SANITIZERS)
    list(APPEND custom_compiler_flags
        -fno-omit-frame-pointer
        -fsanitize=address
        -fsanitize=undefined
        -fsanitize=float-cast-overflow
        -fsanitize-address-use-after-scope
        -fsanitize=integer
        -fno-sanitize-recover
        )
endif()

option(ENABLE_SAFE_STACK "Enables the SafeStack instrumentation pass by the Code Pointer Integrity Project" OFF)
if (ENABLE_SAFE_STACK)
    if (ENABLE_SANITIZERS)
        message(FATAL_ERROR "ENABLE_SAFE_STACK cannot be used in combination with ENABLE_SANITIZERS")
    endif()
    list(APPEND custom_compiler_flags
        -fsanitize=safe-stack
        )
endif()

option(ENABLE_PUBLIC_SYMBOLS "Export library symbols." On)
if (ENABLE_PUBLIC_SYMBOLS)
    list(APPEND custom_compiler_flags -fvisibility=hidden)
    add_definitions(-DCJSON_EXPORT_SYMBOLS -DCJSON_API_VISIBILITY)
endif()
option(ENABLE_HIDDEN_SYMBOLS "Hide library symbols." Off)
if (ENABLE_HIDDEN_SYMBOLS)
    add_definitions(-DCJSON_HIDE_SYMBOLS -UCJSON_API_VISIBILITY)
endif()

# apply custom compiler flags
foreach(compiler_flag ${custom_compiler_flags})
    #remove problematic characters
    string(REGEX REPLACE "[^a-zA-Z0-9]" "" current_variable ${compiler_flag})

    CHECK_C_COMPILER_FLAG(${compiler_flag} "FLAG_SUPPORTED_${current_variable}")
    if (FLAG_SUPPORTED_${current_variable})
        set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} ${compiler_flag}")
    endif()
endforeach()

option(BUILD_SHARED_LIBS "Build shared libraries" ON)
option(ENABLE_TARGET_EXPORT "Enable exporting of CMake targets. Disable when it causes problems!" ON)

#cJSON
set(CJSON_LIB cjson)

file(GLOB HEADERS cJSON.h)
set(SOURCES cJSON.c)

option(BUILD_SHARED_AND_STATIC_LIBS "Build both shared and static libraries" Off)
option(CJSON_OVERRIDE_BUILD_SHARED_LIBS "Override BUILD_SHARED_LIBS with CJSON_BUILD_SHARED_LIBS" OFF)
option(CJSON_BUILD_SHARED_LIBS "Overrides BUILD_SHARED_LIBS if CJSON_OVERRIDE_BUILD_SHARED_LIBS is enabled" ON)
option(ENABLE_CJSON_VERSION_SO "Enables cJSON so version" ON)

# Rust drop-in: cmake -DENABLE_RUST_CJSON=ON (default when cargo is on PATH)
#   cargo build --release  ->  target/release/libcjson.so and libcjson.a
#   cJSON.c is still compiled as cjson_oracle for optional differential tests.
#   ENABLE_RUST_CJSON=OFF compiles cJSON.c as libcjson, same as upstream.
#   cargo test  # Rust unit tests (hex4/BOM pass; parse/print golden tests are ignored)
find_program(CARGO_EXECUTABLE cargo)
if(CARGO_EXECUTABLE)
    set(_ENABLE_RUST_CJSON_DEFAULT ON)
else()
    set(_ENABLE_RUST_CJSON_DEFAULT OFF)
endif()
option(ENABLE_RUST_CJSON
    "Build libcjson from the Rust crate (cargo build --release) instead of cJSON.c"
    ${_ENABLE_RUST_CJSON_DEFAULT})

if ((CJSON_OVERRIDE_BUILD_SHARED_LIBS AND CJSON_BUILD_SHARED_LIBS) OR ((NOT CJSON_OVERRIDE_BUILD_SHARED_LIBS) AND BUILD_SHARED_LIBS))
    set(CJSON_LIBRARY_TYPE SHARED)
else()
    set(CJSON_LIBRARY_TYPE STATIC)
endif()

if(ENABLE_RUST_CJSON)
    if(NOT CARGO_EXECUTABLE)
        message(FATAL_ERROR "ENABLE_RUST_CJSON=ON requires cargo on PATH. Use -DENABLE_RUST_CJSON=OFF for the original C-only build.")
    endif()

    if(DEFINED ENV{CARGO_TARGET_DIR} AND NOT "$ENV{CARGO_TARGET_DIR}" STREQUAL "")
        set(CJSON_RUST_TARGET_DIR "$ENV{CARGO_TARGET_DIR}/release")
    else()
        set(CJSON_RUST_TARGET_DIR "${CMAKE_CURRENT_SOURCE_DIR}/target/release")
    endif()

    if(WIN32)
        set(CJSON_RUST_STATIC_LIB "${CJSON_RUST_TARGET_DIR}/cjson.lib")
        set(CJSON_RUST_SHARED_LIB "${CJSON_RUST_TARGET_DIR}/cjson.dll")
        set(CJSON_RUST_INSTALL_SHARED "cjson.dll")
        set(CJSON_RUST_INSTALL_STATIC "cjson.lib")
    elseif(APPLE)
        set(CJSON_RUST_STATIC_LIB "${CJSON_RUST_TARGET_DIR}/libcjson.a")
        set(CJSON_RUST_SHARED_LIB "${CJSON_RUST_TARGET_DIR}/libcjson.dylib")
        set(CJSON_RUST_INSTALL_SHARED "libcjson.dylib")
        set(CJSON_RUST_INSTALL_STATIC "libcjson.a")
    else()
        set(CJSON_RUST_STATIC_LIB "${CJSON_RUST_TARGET_DIR}/libcjson.a")
        set(CJSON_RUST_SHARED_LIB "${CJSON_RUST_TARGET_DIR}/libcjson.so")
        set(CJSON_RUST_INSTALL_SHARED "libcjson.so")
        set(CJSON_RUST_INSTALL_STATIC "libcjson.a")
    endif()

    add_custom_target(cjson_rust ALL
        COMMAND "${CARGO_EXECUTABLE}" build --release --manifest-path "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
        WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
        BYPRODUCTS "${CJSON_RUST_STATIC_LIB}" "${CJSON_RUST_SHARED_LIB}"
        COMMENT "Building Rust cJSON drop-in (cargo build --release)"
        VERBATIM
        USES_TERMINAL
    )
    if(UNIX AND NOT APPLE)
        add_custom_command(TARGET cjson_rust POST_BUILD
            COMMAND "${CMAKE_COMMAND}" -E create_symlink "libcjson.so" "${CJSON_RUST_TARGET_DIR}/libcjson.so.1"
            COMMENT "Symlink libcjson.so.1 -> libcjson.so (cdylib soname)"
        )
    endif()

    if(NOT BUILD_SHARED_AND_STATIC_LIBS)
        add_library("${CJSON_LIB}" "${CJSON_LIBRARY_TYPE}" IMPORTED GLOBAL)
        if(CJSON_LIBRARY_TYPE STREQUAL "STATIC")
            set_target_properties("${CJSON_LIB}" PROPERTIES IMPORTED_LOCATION "${CJSON_RUST_STATIC_LIB}")
        else()
            set_target_properties("${CJSON_LIB}" PROPERTIES IMPORTED_LOCATION "${CJSON_RUST_SHARED_LIB}")
        endif()
    else()
        add_library("${CJSON_LIB}" SHARED IMPORTED GLOBAL)
        set_target_properties("${CJSON_LIB}" PROPERTIES IMPORTED_LOCATION "${CJSON_RUST_SHARED_LIB}")
        add_library("${CJSON_LIB}-static" STATIC IMPORTED GLOBAL)
        set_target_properties("${CJSON_LIB}-static" PROPERTIES
            IMPORTED_LOCATION "${CJSON_RUST_STATIC_LIB}"
            OUTPUT_NAME "${CJSON_LIB}"
            PREFIX "lib")
        add_dependencies("${CJSON_LIB}-static" cjson_rust)
    endif()

    set_target_properties("${CJSON_LIB}" PROPERTIES
        INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}")
    add_dependencies("${CJSON_LIB}" cjson_rust)

    if(NOT WIN32)
        find_package(Threads REQUIRED)
        set(_rust_static_syslibs Threads::Threads ${CMAKE_DL_LIBS} m)
        if(CJSON_LIBRARY_TYPE STREQUAL "STATIC")
            set_property(TARGET "${CJSON_LIB}" APPEND PROPERTY INTERFACE_LINK_LIBRARIES ${_rust_static_syslibs})
        endif()
        if(BUILD_SHARED_AND_STATIC_LIBS)
            set_property(TARGET "${CJSON_LIB}-static" APPEND PROPERTY
                INTERFACE_LINK_LIBRARIES ${_rust_static_syslibs})
        endif()
    endif()

    # So test binaries find the cdylib before install.
    set(CMAKE_BUILD_RPATH "${CJSON_RUST_TARGET_DIR}")

    # Original C implementation kept as an oracle (do not delete cJSON.c).
    add_library(cjson_oracle STATIC "${HEADERS}" "${SOURCES}")
    if(NOT WIN32)
        target_link_libraries(cjson_oracle m)
    endif()
    target_include_directories(cjson_oracle PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
else()
    if (NOT BUILD_SHARED_AND_STATIC_LIBS)
        add_library("${CJSON_LIB}" "${CJSON_LIBRARY_TYPE}" "${HEADERS}" "${SOURCES}")
    else()
        # See https://cmake.org/Wiki/CMake_FAQ#How_do_I_make_my_shared_and_static_libraries_have_the_same_root_name.2C_but_different_suffixes.3F
        add_library("${CJSON_LIB}" SHARED "${HEADERS}" "${SOURCES}")
        add_library("${CJSON_LIB}-static" STATIC "${HEADERS}" "${SOURCES}")
        set_target_properties("${CJSON_LIB}-static" PROPERTIES OUTPUT_NAME "${CJSON_LIB}")
        set_target_properties("${CJSON_LIB}-static" PROPERTIES PREFIX "lib")
    endif()
    if (NOT WIN32)
        target_link_libraries("${CJSON_LIB}" m)
    endif()
endif()

configure_file("${CMAKE_CURRENT_SOURCE_DIR}/library_config/libcjson.pc.in"
    "${CMAKE_CURRENT_BINARY_DIR}/libcjson.pc" @ONLY)

install(FILES cJSON.h DESTINATION "${CMAKE_INSTALL_FULL_INCLUDEDIR}/cjson")
install (FILES "${CMAKE_CURRENT_BINARY_DIR}/libcjson.pc" DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}/pkgconfig")
if(ENABLE_RUST_CJSON)
    # IMPORTED rust artifacts: install the files cargo produced. Header is still cJSON.h.
    if(CJSON_LIBRARY_TYPE STREQUAL "SHARED")
        install(FILES "${CJSON_RUST_SHARED_LIB}" DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}")
        if(UNIX AND NOT APPLE)
            install(FILES "${CJSON_RUST_TARGET_DIR}/libcjson.so.1" DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}" OPTIONAL)
        endif()
        set(_CJSON_RUST_EXPORTED_LOCATION "${CMAKE_INSTALL_FULL_LIBDIR}/${CJSON_RUST_INSTALL_SHARED}")
    else()
        install(FILES "${CJSON_RUST_STATIC_LIB}" DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}")
        set(_CJSON_RUST_EXPORTED_LOCATION "${CMAKE_INSTALL_FULL_LIBDIR}/${CJSON_RUST_INSTALL_STATIC}")
    endif()
    if(BUILD_SHARED_AND_STATIC_LIBS)
        install(FILES "${CJSON_RUST_STATIC_LIB}" DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}")
    endif()
    if(ENABLE_TARGET_EXPORT)
        file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/cjson.cmake"
"if(NOT TARGET cjson)
  add_library(cjson ${CJSON_LIBRARY_TYPE} IMPORTED)
  set_target_properties(cjson PROPERTIES
    IMPORTED_LOCATION \"${_CJSON_RUST_EXPORTED_LOCATION}\"
    INTERFACE_INCLUDE_DIRECTORIES \"${CMAKE_INSTALL_FULL_INCLUDEDIR};${CMAKE_INSTALL_FULL_INCLUDEDIR}/cjson\")
endif()
")
        install(FILES "${CMAKE_CURRENT_BINARY_DIR}/cjson.cmake"
            DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}/cmake/cJSON")
    endif()
else()
    install(TARGETS "${CJSON_LIB}"
        EXPORT "${CJSON_LIB}"
        ARCHIVE DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}"
        LIBRARY DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}"
        RUNTIME DESTINATION "${CMAKE_INSTALL_FULL_BINDIR}"
        INCLUDES DESTINATION "${CMAKE_INSTALL_FULL_INCLUDEDIR}"
    )
    if (BUILD_SHARED_AND_STATIC_LIBS)
        install(TARGETS "${CJSON_LIB}-static"
        EXPORT "${CJSON_LIB}"
        ARCHIVE DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}"
        INCLUDES DESTINATION "${CMAKE_INSTALL_FULL_INCLUDEDIR}"
    )
    endif()
    if(ENABLE_TARGET_EXPORT)
        # export library information for CMake projects
        install(EXPORT "${CJSON_LIB}" DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}/cmake/cJSON")
    endif()

    if(ENABLE_CJSON_VERSION_SO)
        set_target_properties("${CJSON_LIB}"
            PROPERTIES
                SOVERSION "${CJSON_VERSION_SO}"
                VERSION "${PROJECT_VERSION}")
    endif()
endif()

#cJSON_Utils
option(ENABLE_CJSON_UTILS "Enable building the cJSON_Utils library." OFF)
if(ENABLE_CJSON_UTILS)
    set(CJSON_UTILS_LIB cjson_utils)

    file(GLOB HEADERS_UTILS cJSON_Utils.h)
    set(SOURCES_UTILS cJSON_Utils.c)

    if (NOT BUILD_SHARED_AND_STATIC_LIBS)
        add_library("${CJSON_UTILS_LIB}" "${CJSON_LIBRARY_TYPE}" "${HEADERS_UTILS}" "${SOURCES_UTILS}")
        target_link_libraries("${CJSON_UTILS_LIB}" "${CJSON_LIB}")
    else()
        add_library("${CJSON_UTILS_LIB}" SHARED "${HEADERS_UTILS}" "${SOURCES_UTILS}")
        target_link_libraries("${CJSON_UTILS_LIB}" "${CJSON_LIB}")
        add_library("${CJSON_UTILS_LIB}-static" STATIC "${HEADERS_UTILS}" "${SOURCES_UTILS}")
        target_link_libraries("${CJSON_UTILS_LIB}-static" "${CJSON_LIB}-static")
        set_target_properties("${CJSON_UTILS_LIB}-static" PROPERTIES OUTPUT_NAME "${CJSON_UTILS_LIB}")
        set_target_properties("${CJSON_UTILS_LIB}-static" PROPERTIES PREFIX "lib")
    endif()

    configure_file("${CMAKE_CURRENT_SOURCE_DIR}/library_config/libcjson_utils.pc.in"
        "${CMAKE_CURRENT_BINARY_DIR}/libcjson_utils.pc" @ONLY)

    install(TARGETS "${CJSON_UTILS_LIB}"
        EXPORT "${CJSON_UTILS_LIB}"
        ARCHIVE DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}"
        LIBRARY DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}"
        RUNTIME DESTINATION "${CMAKE_INSTALL_FULL_BINDIR}"
        INCLUDES DESTINATION "${CMAKE_INSTALL_FULL_INCLUDEDIR}"
    )
    if (BUILD_SHARED_AND_STATIC_LIBS)
        install(TARGETS "${CJSON_UTILS_LIB}-static" 
        EXPORT "${CJSON_UTILS_LIB}" 
        ARCHIVE DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}"
        INCLUDES DESTINATION "${CMAKE_INSTALL_FULL_INCLUDEDIR}"
        )
    endif()
    install(FILES cJSON_Utils.h DESTINATION "${CMAKE_INSTALL_FULL_INCLUDEDIR}/cjson")
    install (FILES "${CMAKE_CURRENT_BINARY_DIR}/libcjson_utils.pc" DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}/pkgconfig")
    if(ENABLE_TARGET_EXPORT)
      # export library information for CMake projects
      install(EXPORT "${CJSON_UTILS_LIB}" DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}/cmake/cJSON")
    endif()

    if(ENABLE_CJSON_VERSION_SO)
        set_target_properties("${CJSON_UTILS_LIB}"
            PROPERTIES
                SOVERSION "${CJSON_UTILS_VERSION_SO}"
                VERSION "${PROJECT_VERSION}")
    endif()
endif()

# create the other package config files
configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/library_config/cJSONConfig.cmake.in"
    ${PROJECT_BINARY_DIR}/cJSONConfig.cmake @ONLY)
configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/library_config/cJSONConfigVersion.cmake.in"
    ${PROJECT_BINARY_DIR}/cJSONConfigVersion.cmake @ONLY)

if(ENABLE_TARGET_EXPORT)
    # Install package config files
    install(FILES ${PROJECT_BINARY_DIR}/cJSONConfig.cmake
        ${PROJECT_BINARY_DIR}/cJSONConfigVersion.cmake
        DESTINATION "${CMAKE_INSTALL_FULL_LIBDIR}/cmake/cJSON")
endif()

option(ENABLE_CJSON_TEST "Enable building cJSON test" ON)
if(ENABLE_CJSON_TEST)
    enable_testing()

    set(TEST_CJSON cJSON_test)
    add_executable("${TEST_CJSON}" test.c)
    target_link_libraries("${TEST_CJSON}" "${CJSON_LIB}")

    add_test(NAME ${TEST_CJSON} COMMAND "${CMAKE_CURRENT_BINARY_DIR}/${TEST_CJSON}")

    # Disable -fsanitize=float-divide-by-zero for cJSON_test
    if (FLAG_SUPPORTED_fsanitizefloatdividebyzero)
        if ("${CMAKE_VERSION}" VERSION_LESS "2.8.12")
            set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fno-sanitize=float-divide-by-zero")
        else()
            target_compile_options(${TEST_CJSON} PRIVATE "-fno-sanitize=float-divide-by-zero")
        endif()
    endif()

    #"check" target that automatically builds everything and runs the tests
    add_custom_target(check
        COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure
        DEPENDS ${TEST_CJSON})
endif()

#Create the uninstall target
option(ENABLE_CJSON_UNINSTALL "Enable creating uninstall target" ON)
if(ENABLE_CJSON_UNINSTALL)
  add_custom_target(uninstall "${CMAKE_COMMAND}" -P
    "${PROJECT_SOURCE_DIR}/library_config/uninstall.cmake")
endif()

# Enable the use of locales
option(ENABLE_LOCALES "Enable the use of locales" ON)
if(ENABLE_LOCALES)
	add_definitions(-DENABLE_LOCALES)
endif()

add_subdirectory(tests)
add_subdirectory(fuzzing)

# Fuzzing is unchanged (AFL not rewritten). fuzzing/cjson_read_fuzzer.c can
# link the Rust staticlib (target/release/libcjson.a) later the same way
# cJSON_test does when ENABLE_RUST_CJSON is ON.
if(ENABLE_RUST_CJSON)
    if(TARGET "${TEST_CJSON}")
        add_dependencies("${TEST_CJSON}" cjson_rust)
    endif()
    if(TARGET fuzz_main)
        add_dependencies(fuzz_main cjson_rust)
    endif()
    if(TARGET afl-main)
        add_dependencies(afl-main cjson_rust)
    endif()
endif()
