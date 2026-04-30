# path_manager 单元测试
add_executable(test_path_manager test_path_manager.c
    ${CMAKE_SOURCE_DIR}/src/core/path_manager.c
    ${CMAKE_SOURCE_DIR}/src/utils/string_ext.c
    ${CMAKE_SOURCE_DIR}/src/utils/safe_string.c
    ${CMAKE_SOURCE_DIR}/src/utils/error_code.c
)

target_link_libraries(test_path_manager cmocka)

target_include_directories(test_path_manager PRIVATE
    ${CMAKE_SOURCE_DIR}/src/core
    ${CMAKE_SOURCE_DIR}/src/utils
)

# 定义 TESTING 宏以启用 mock
target_compile_definitions(test_path_manager PRIVATE TESTING)

# 添加测试
add_test(NAME path_manager_test COMMAND test_path_manager)
