# import_export 单元测试
add_executable(test_import_export test_import_export.c
    ${CMAKE_SOURCE_DIR}/src/core/import_export.c
    ${CMAKE_SOURCE_DIR}/src/utils/string_ext.c
    ${CMAKE_SOURCE_DIR}/src/utils/safe_string.c
    ${CMAKE_SOURCE_DIR}/src/utils/error_code.c
)

target_link_libraries(test_import_export cmocka)

target_include_directories(test_import_export PRIVATE
    ${CMAKE_SOURCE_DIR}/include
)

target_compile_definitions(test_import_export PRIVATE TESTING)

add_custom_command(TARGET test_import_export POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
    ${CMAKE_BINARY_DIR}/_deps/cmocka-build/src/cmocka.dll
    $<TARGET_FILE_DIR:test_import_export>
)

add_test(NAME import_export_test COMMAND test_import_export)
