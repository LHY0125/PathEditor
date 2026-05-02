# error_code 单元测试
add_executable(test_error_code test_error_code.c
    ${CMAKE_SOURCE_DIR}/src/utils/error_code.c
)

target_link_libraries(test_error_code cmocka)

target_include_directories(test_error_code PRIVATE
    ${CMAKE_SOURCE_DIR}/include
)

add_custom_command(TARGET test_error_code POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
    ${CMAKE_BINARY_DIR}/_deps/cmocka-build/src/cmocka.dll
    $<TARGET_FILE_DIR:test_error_code>
)

add_test(NAME error_code_test COMMAND test_error_code)
