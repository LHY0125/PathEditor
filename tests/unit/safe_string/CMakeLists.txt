# safe_string 单元测试
add_executable(test_safe_string test_safe_string.c
    ${CMAKE_SOURCE_DIR}/src/utils/safe_string.c
)

target_link_libraries(test_safe_string cmocka)

target_include_directories(test_safe_string PRIVATE
    ${CMAKE_SOURCE_DIR}/src/utils
)

# 定义 TESTING 宏以启用 mock
target_compile_definitions(test_safe_string PRIVATE TESTING)

# 添加测试
add_test(NAME safe_string_test COMMAND test_safe_string)
