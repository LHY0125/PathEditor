/*
 * path_manager.c 单元测试
 * 测试路径管理函数
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include <stdlib.h>
#include "core/path_manager.h"

/* ==================== Mock 函数 ==================== */

#ifdef TESTING

/* Mock is_path_valid - 默认返回 1（有效）*/
int is_path_valid_mock_enabled = 0;
int is_path_valid_mock_return = 1;

int is_path_valid(const char *path)
{
    (void)path;
    if (is_path_valid_mock_enabled) {
        return is_path_valid_mock_return;
    }
    return 1;  /* 默认认为路径有效 */
}

/* Mock 日志函数 - 避免链接日志文件依赖 */
int log_info_enabled = 0;
int log_debug_enabled = 0;
int log_warn_enabled = 0;
int log_error_enabled = 0;

void log_info(const char *fmt, ...)
{
    (void)fmt;
    log_info_enabled++;
}

void log_debug(const char *fmt, ...)
{
    (void)fmt;
    log_debug_enabled++;
}

void log_warn(const char *fmt, ...)
{
    (void)fmt;
    log_warn_enabled++;
}

void log_error(const char *fmt, ...)
{
    (void)fmt;
    log_error_enabled++;
}

#endif /* TESTING */

/* ==================== path_manager_remove_at 测试 ==================== */

static void test_remove_at_normal(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");
    add_string_list(&list, "path2");
    add_string_list(&list, "path3");

    ErrorCode result = path_manager_remove_at(&list, 1);

    assert_int_equal(result, ERR_OK);
    assert_int_equal(list.count, 2);
    assert_string_equal(string_list_get(&list, 0), "path1");
    assert_string_equal(string_list_get(&list, 1), "path3");

    clear_string_list(&list);
}

static void test_remove_at_first(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");
    add_string_list(&list, "path2");

    ErrorCode result = path_manager_remove_at(&list, 0);

    assert_int_equal(result, ERR_OK);
    assert_int_equal(list.count, 1);
    assert_string_equal(string_list_get(&list, 0), "path2");

    clear_string_list(&list);
}

static void test_remove_at_last(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");
    add_string_list(&list, "path2");

    ErrorCode result = path_manager_remove_at(&list, 1);

    assert_int_equal(result, ERR_OK);
    assert_int_equal(list.count, 1);
    assert_string_equal(string_list_get(&list, 0), "path1");

    clear_string_list(&list);
}

static void test_remove_at_invalid_index(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");

    ErrorCode result = path_manager_remove_at(&list, 5);  /* 越界 */

    assert_int_not_equal(result, ERR_OK);

    clear_string_list(&list);
}

static void test_remove_at_null(void **state)
{
    (void)state;
    ErrorCode result = path_manager_remove_at(NULL, 0);
    assert_int_equal(result, ERR_NULL_PTR);
}

/* ==================== path_manager_move_up 测试 ==================== */

static void test_move_up_normal(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");
    add_string_list(&list, "path2");
    add_string_list(&list, "path3");

    ErrorCode result = path_manager_move_up(&list, 2);

    assert_int_equal(result, ERR_OK);
    assert_string_equal(string_list_get(&list, 0), "path1");
    assert_string_equal(string_list_get(&list, 1), "path3");
    assert_string_equal(string_list_get(&list, 2), "path2");

    clear_string_list(&list);
}

static void test_move_up_first_element(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");
    add_string_list(&list, "path2");

    ErrorCode result = path_manager_move_up(&list, 0);  /* 第一个元素无法上移 */

    assert_int_equal(result, ERR_INVALID_INDEX);

    clear_string_list(&list);
}

static void test_move_up_invalid_index(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");
    add_string_list(&list, "path2");

    ErrorCode result = path_manager_move_up(&list, 5);  /* 越界 */

    assert_int_not_equal(result, ERR_OK);

    clear_string_list(&list);
}

static void test_move_up_single_element(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");

    ErrorCode result = path_manager_move_up(&list, 0);

    assert_int_equal(result, ERR_INVALID_INDEX);
    assert_string_equal(string_list_get(&list, 0), "path1");

    clear_string_list(&list);
}

/* ==================== path_manager_move_down 测试 ==================== */

static void test_move_down_normal(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");
    add_string_list(&list, "path2");
    add_string_list(&list, "path3");

    ErrorCode result = path_manager_move_down(&list, 1);

    assert_int_equal(result, ERR_OK);
    assert_string_equal(string_list_get(&list, 0), "path1");
    assert_string_equal(string_list_get(&list, 1), "path3");
    assert_string_equal(string_list_get(&list, 2), "path2");

    clear_string_list(&list);
}

static void test_move_down_last_element(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");
    add_string_list(&list, "path2");

    ErrorCode result = path_manager_move_down(&list, 1);  /* 最后一个元素无法下移 */

    assert_int_equal(result, ERR_INVALID_INDEX);

    clear_string_list(&list);
}

static void test_move_down_invalid_index(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");
    add_string_list(&list, "path2");

    ErrorCode result = path_manager_move_down(&list, 5);  /* 越界 */

    assert_int_not_equal(result, ERR_OK);

    clear_string_list(&list);
}

static void test_move_down_single_element(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);
    add_string_list(&list, "path1");

    ErrorCode result = path_manager_move_down(&list, 0);

    assert_int_equal(result, ERR_INVALID_INDEX);
    assert_string_equal(string_list_get(&list, 0), "path1");

    clear_string_list(&list);
}

/* ==================== path_manager_clean 测试 ==================== */

static void test_clean_no_invalid(void **state)
{
    (void)state;
    is_path_valid_mock_enabled = 1;
    is_path_valid_mock_return = 1;

    StringList list;
    init_string_list(&list);
    add_string_list(&list, "C:\\Windows");
    add_string_list(&list, "C:\\Program Files");

    int before_count = list.count;
    ErrorCode result = path_manager_clean(&list);

    assert_int_equal(result, ERR_OK);
    assert_int_equal(list.count, before_count);  /* 没有无效路径，不应删除 */

    is_path_valid_mock_enabled = 0;
    clear_string_list(&list);
}

static void test_clean_with_invalid(void **state)
{
    (void)state;
    is_path_valid_mock_enabled = 1;
    is_path_valid_mock_return = 0;  /* 所有路径都无效 */

    StringList list;
    init_string_list(&list);
    add_string_list(&list, "C:\\Invalid1");
    add_string_list(&list, "C:\\Invalid2");
    add_string_list(&list, "C:\\Invalid3");

    ErrorCode result = path_manager_clean(&list);

    assert_int_equal(result, ERR_OK);
    assert_int_equal(list.count, 0);  /* 所有路径都被删除 */

    is_path_valid_mock_enabled = 0;
    clear_string_list(&list);
}

static void test_clean_empty_list(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    ErrorCode result = path_manager_clean(&list);

    assert_int_equal(result, ERR_OK);
    assert_int_equal(list.count, 0);

    clear_string_list(&list);
}

/* ==================== 主函数 ==================== */

int main(void)
{
    const struct CMUnitTest tests[] = {
        /* remove_at 测试 */
        cmocka_unit_test(test_remove_at_normal),
        cmocka_unit_test(test_remove_at_first),
        cmocka_unit_test(test_remove_at_last),
        cmocka_unit_test(test_remove_at_invalid_index),
        cmocka_unit_test(test_remove_at_null),

        /* move_up 测试 */
        cmocka_unit_test(test_move_up_normal),
        cmocka_unit_test(test_move_up_first_element),
        cmocka_unit_test(test_move_up_invalid_index),
        cmocka_unit_test(test_move_up_single_element),

        /* move_down 测试 */
        cmocka_unit_test(test_move_down_normal),
        cmocka_unit_test(test_move_down_last_element),
        cmocka_unit_test(test_move_down_invalid_index),
        cmocka_unit_test(test_move_down_single_element),

        /* clean 测试 */
        cmocka_unit_test(test_clean_no_invalid),
        cmocka_unit_test(test_clean_with_invalid),
        cmocka_unit_test(test_clean_empty_list),
    };

    return cmocka_run_group_tests(tests, NULL, NULL);
}
