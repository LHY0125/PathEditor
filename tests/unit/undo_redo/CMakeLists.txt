# undo_redo 单元测试
add_executable(test_undo_redo test_undo_redo.c
    ${CMAKE_SOURCE_DIR}/src/core/undo_redo.c
    ${CMAKE_SOURCE_DIR}/src/core/path_manager.c
    ${CMAKE_SOURCE_DIR}/src/utils/string_ext.c
    ${CMAKE_SOURCE_DIR}/src/utils/safe_string.c
    ${CMAKE_SOURCE_DIR}/src/utils/error_code.c
)

target_link_libraries(test_undo_redo cmocka)

target_include_directories(test_undo_redo PRIVATE
    ${CMAKE_SOURCE_DIR}/include
)

target_compile_definitions(test_undo_redo PRIVATE TESTING)

add_custom_command(TARGET test_undo_redo POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
    ${CMAKE_BINARY_DIR}/_deps/cmocka-build/src/cmocka.dll
    $<TARGET_FILE_DIR:test_undo_redo>
)

add_test(NAME undo_redo_test COMMAND test_undo_redo)
