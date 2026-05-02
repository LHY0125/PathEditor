/*
 * error_code.c 单元测试
 * 测试错误码字符串映射
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "utils/error_code.h"

/* ==================== error_code_to_string 测试 ==================== */

static void test_err_ok(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_OK), "Success");
}

static void test_err_failed(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_FAILED), "Operation failed");
}

static void test_err_null_ptr(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_NULL_PTR), "Null pointer error");
}

static void test_err_out_of_memory(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_OUT_OF_MEMORY), "Out of memory");
}

static void test_err_file_not_found(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_FILE_NOT_FOUND), "File not found");
}

static void test_err_permission_denied(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_PERMISSION_DENIED), "Permission denied");
}

static void test_err_invalid_format(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_INVALID_FORMAT), "Invalid format");
}

static void test_err_registry_failed(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_REGISTRY_FAILED), "Registry operation failed");
}

static void test_err_not_found(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_NOT_FOUND), "Item not found");
}

static void test_err_exists(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_EXISTS), "Item already exists");
}

static void test_err_invalid_index(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string(ERR_INVALID_INDEX), "Invalid index");
}

static void test_unknown_error_code(void **state)
{
    (void)state;
    assert_string_equal(error_code_to_string((ErrorCode)9999), "Unknown error");
    assert_string_equal(error_code_to_string((ErrorCode)-99), "Unknown error");
}

/* ==================== 主函数 ==================== */

int main(void)
{
    const struct CMUnitTest tests[] = {
        cmocka_unit_test(test_err_ok),
        cmocka_unit_test(test_err_failed),
        cmocka_unit_test(test_err_null_ptr),
        cmocka_unit_test(test_err_out_of_memory),
        cmocka_unit_test(test_err_file_not_found),
        cmocka_unit_test(test_err_permission_denied),
        cmocka_unit_test(test_err_invalid_format),
        cmocka_unit_test(test_err_registry_failed),
        cmocka_unit_test(test_err_not_found),
        cmocka_unit_test(test_err_exists),
        cmocka_unit_test(test_err_invalid_index),
        cmocka_unit_test(test_unknown_error_code),
    };

    return cmocka_run_group_tests(tests, NULL, NULL);
}
