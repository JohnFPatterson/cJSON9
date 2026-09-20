if(ENABLE_CJSON_TEST)
    add_library(unity STATIC unity/src/unity.c)

    # Disable -Werror for Unity
    if (FLAG_SUPPORTED_Werror)
        if ("${CMAKE_VERSION}" VERSION_LESS "2.8.12")
            set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wno-error")
        else()
            target_compile_options(unity PRIVATE "-Wno-error")
        endif()
    endif()
    # Disable -fvisibility=hidden for Unity
    if (FLAG_SUPPORTED_fvisibilityhidden)
        if ("${CMAKE_VERSION}" VERSION_LESS "2.8.12")
            set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fvisibility=default")
        else()
            target_compile_options(unity PRIVATE "-fvisibility=default")
        endif()
    endif()
    # Disable -fsanitize=float-divide-by-zero for Unity (GCC bug on x86 https://gcc.gnu.org/bugzilla/show_bug.cgi?id=80097)
    if (FLAG_SUPPORTED_fsanitizefloatdividebyzero AND (CMAKE_C_COMPILER_ID STREQUAL "GNU"))
        if ("${CMAKE_VERSION}" VERSION_LESS "2.8.12")
            set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fno-sanitize=float-divide-by-zero")
        else()
            target_compile_options(unity PRIVATE "-fno-sanitize=float-divide-by-zero")
        endif()
    endif()
    # Disable -Wswitch-enum for Unity
    if (FLAG_SUPPORTED_Wswitchenum)
        if ("${CMAKE_VERSION}" VERSION_LESS "2.8.12")
            set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wno-switch-enum")
        else()
            target_compile_options(unity PRIVATE "-Wno-switch-enum")
        endif()
    endif()

    #copy test files
    file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/inputs")
    file(GLOB test_files "inputs/*")
    file(COPY ${test_files} DESTINATION "${CMAKE_CURRENT_BINARY_DIR}/inputs/")

    set(unity_public_tests
        parse_examples
        parse_with_opts
        compare_tests
        cjson_add
        readme_examples
        minify_tests
        print_value
        misc_tests
    )

    set(unity_internal_tests
        parse_number
        parse_hex4
        parse_string
        parse_array
        parse_object
        parse_value
        print_string
        print_number
        print_array
        print_object
    )

    # Internal Unity tests compile cJSON.c statics (parse_number, print_string,
    # skip_utf8_bom, ensure, global_hooks). The Rust cdylib does not export those.
    # When ENABLE_RUST_CJSON, only public-API tests are built; they include
    # cJSON.h (CJSON_PUBLIC_TESTS_ONLY) and link the imported rust library.
    if(ENABLE_RUST_CJSON)
        set(unity_tests ${unity_public_tests})
    else()
        set(unity_tests ${unity_public_tests} ${unity_internal_tests})
    endif()

    option(ENABLE_VALGRIND OFF "Enable the valgrind memory checker for the tests.")
    if (ENABLE_VALGRIND)
        find_program(MEMORYCHECK_COMMAND valgrind)
        if ("${MEMORYCHECK_COMMAND}" MATCHES "MEMORYCHECK_COMMAND-NOTFOUND")
            message(WARNING "Valgrind couldn't be found.")
            unset(MEMORYCHECK_COMMAND)
        else()
            set(MEMORYCHECK_COMMAND_OPTIONS --trace-children=yes --leak-check=full --error-exitcode=1 --suppressions=${CMAKE_CURRENT_SOURCE_DIR}/../valgrind.supp)
        endif()
    endif()

    foreach(unity_test ${unity_tests})
        add_executable("${unity_test}" "${unity_test}.c")
        if("${CMAKE_C_COMPILER_ID}" STREQUAL "MSVC")
            target_sources(${unity_test} PRIVATE unity_setup.c)
        endif()
        target_link_libraries("${unity_test}" "${CJSON_LIB}" unity)
        if(ENABLE_RUST_CJSON)
            target_compile_definitions("${unity_test}" PRIVATE CJSON_PUBLIC_TESTS_ONLY)
            add_dependencies("${unity_test}" cjson_rust)
        endif()
        if(MEMORYCHECK_COMMAND)
            add_test(NAME "${unity_test}"
                COMMAND "${MEMORYCHECK_COMMAND}" ${MEMORYCHECK_COMMAND_OPTIONS} "${CMAKE_CURRENT_BINARY_DIR}/${unity_test}")
        else()
            add_test(NAME "${unity_test}"
                COMMAND "./${unity_test}")
        endif()
    endforeach()

    add_dependencies(check ${unity_tests})

    if (ENABLE_CJSON_UTILS)
        #copy test files
        file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/json-patch-tests")
        file(GLOB test_files "json-patch-tests/*")
        file(COPY ${test_files} DESTINATION "${CMAKE_CURRENT_BINARY_DIR}/json-patch-tests/")

        set (cjson_utils_tests
            json_patch_tests
            old_utils_tests
            misc_utils_tests)

        foreach (cjson_utils_test ${cjson_utils_tests})
            add_executable("${cjson_utils_test}" "${cjson_utils_test}.c")
            target_link_libraries("${cjson_utils_test}" "${CJSON_LIB}" "${CJSON_UTILS_LIB}" unity)
            if("${CMAKE_C_COMPILER_ID}" STREQUAL "MSVC")
                target_sources(${cjson_utils_test} PRIVATE unity_setup.c)
            endif()
            if(MEMORYCHECK_COMMAND)
                add_test(NAME "${cjson_utils_test}"
                    COMMAND "${MEMORYCHECK_COMMAND}" ${MEMORYCHECK_COMMAND_OPTIONS} "${CMAKE_CURRENT_BINARY_DIR}/${cjson_utils_test}")
            else()
                add_test(NAME "${cjson_utils_test}"
                    COMMAND "./${cjson_utils_test}")
            endif()
        endforeach()

        add_dependencies(check ${cjson_utils_tests})
    endif()
endif()
