# string_ext 单元测试
add_executable(test_string_ext test_string_ext.c
    ${CMAKE_SOURCE_DIR}/src/utils/string_ext.c
    ${CMAKE_SOURCE_DIR}/src/utils/safe_string.c
)

target_link_libraries(test_string_ext cmocka)

target_include_directories(test_string_ext PRIVATE
    ${CMAKE_SOURCE_DIR}/src/utils
    ${CMAKE_SOURCE_DIR}/tests/mocks
)

# 定义 TESTING 宏和 REPLACE_WINDOWS_API 以启用 mock
target_compile_definitions(test_string_ext PRIVATE TESTING REPLACE_WINDOWS_API)

# 添加测试
add_test(NAME string_ext_test COMMAND test_string_ext)
