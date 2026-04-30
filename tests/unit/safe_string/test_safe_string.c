/*
 * safe_string.c 单元测试
 * 测试安全字符串操作函数
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include <stdlib.h>
#include "utils/safe_string.h"

/* ==================== safe_strcpy 测试 ==================== */

static void test_safe_strcpy_normal(void **state)
{
    (void)state;
    char dst[32];
    const char *src = "Hello, World!";
    char *result = safe_strcpy(dst, sizeof(dst), src);

    assert_non_null(result);
    assert_string_equal(dst, src);
}

static void test_safe_strcpy_truncation(void **state)
{
    (void)state;
    char dst[8];
    const char *src = "This is a long string";
    char *result = safe_strcpy(dst, sizeof(dst), src);

    assert_non_null(result);
    assert_int_equal(strlen(dst), 7);  /* 截断到 7 字符 */
    assert_memory_equal(dst, src, 7);  /* 前 7 字符相同 */
    assert_int_equal(dst[7], '\0');
}

static void test_safe_strcpy_null_dst(void **state)
{
    (void)state;
    const char *src = "test";
    char *result = safe_strcpy(NULL, 10, src);
    assert_null(result);
}

static void test_safe_strcpy_null_src(void **state)
{
    (void)state;
    char dst[32];
    char *result = safe_strcpy(dst, sizeof(dst), NULL);
    assert_null(result);
}

static void test_safe_strcpy_zero_size(void **state)
{
    (void)state;
    char dst[32];
    char *result = safe_strcpy(dst, 0, "test");
    assert_null(result);
}

static void test_safe_strcpy_exact_fit(void **state)
{
    (void)state;
    char dst[6];
    const char *src = "12345";  /* 5字符 + 1终止符 = 6 */
    char *result = safe_strcpy(dst, sizeof(dst), src);

    assert_non_null(result);
    assert_string_equal(dst, "12345");
}

/* ==================== safe_strcat 测试 ==================== */

static void test_safe_strcat_normal(void **state)
{
    (void)state;
    char dst[32] = "Hello";
    const char *src = ", World!";
    char *result = safe_strcat(dst, sizeof(dst), src);

    assert_non_null(result);
    assert_string_equal(dst, "Hello, World!");
}

static void test_safe_strcat_truncation(void **state)
{
    (void)state;
    char dst[12] = "Hello";
    const char *src = ", World!";  /* 总长 12，但最后一个位置要放 \0 */
    char *result = safe_strcat(dst, sizeof(dst), src);

    assert_non_null(result);
    /* dst 有 11 可用位置 (12-1)，"Hello" 占 5，还剩 6 */
    /* ", World!" 有 9 字符，只能放 6 个字符 + \0 */
    assert_true(strlen(dst) <= 11);
}

static void test_safe_strcat_null_dst(void **state)
{
    (void)state;
    char dst[32] = "test";
    char *result = safe_strcat(NULL, sizeof(dst), "src");
    assert_null(result);
}

static void test_safe_strcat_null_src(void **state)
{
    (void)state;
    char dst[32] = "test";
    char *result = safe_strcat(dst, sizeof(dst), NULL);
    /* src 为 NULL 时函数返回 NULL */
    assert_null(result);
}

static void test_safe_strcat_empty_dst(void **state)
{
    (void)state;
    char dst[32] = "";
    const char *src = "Hello";
    char *result = safe_strcat(dst, sizeof(dst), src);

    assert_non_null(result);
    assert_string_equal(dst, "Hello");
}

/* ==================== safe_strdup 测试 ==================== */

static void test_safe_strdup_normal(void **state)
{
    (void)state;
    const char *src = "Hello, World!";
    char *result = safe_strdup(src);

    assert_non_null(result);
    assert_string_equal(result, src);
    assert_ptr_not_equal(result, src);  /* 必须是新分配的内存 */

    free(result);
}

static void test_safe_strdup_null(void **state)
{
    (void)state;
    char *result = safe_strdup(NULL);
    assert_null(result);
}

static void test_safe_strdup_empty_string(void **state)
{
    (void)state;
    const char *src = "";
    char *result = safe_strdup(src);

    assert_non_null(result);
    assert_string_equal(result, "");

    free(result);
}

static void test_safe_strdup_long_string(void **state)
{
    (void)state;
    /* 构造一个 510 字符的字符串 */
    char src[511];
    for (int i = 0; i < 510; i++) {
        src[i] = 'A';
    }
    src[510] = '\0';

    char *result = safe_strdup(src);

    assert_non_null(result);
    assert_int_equal(strlen(result), 510);
    assert_memory_equal(result, src, 510);

    free(result);
}

/* ==================== 主函数 ==================== */

int main(void)
{
    const struct CMUnitTest tests[] = {
        /* safe_strcpy 测试 */
        cmocka_unit_test(test_safe_strcpy_normal),
        cmocka_unit_test(test_safe_strcpy_truncation),
        cmocka_unit_test(test_safe_strcpy_null_dst),
        cmocka_unit_test(test_safe_strcpy_null_src),
        cmocka_unit_test(test_safe_strcpy_zero_size),
        cmocka_unit_test(test_safe_strcpy_exact_fit),

        /* safe_strcat 测试 */
        cmocka_unit_test(test_safe_strcat_normal),
        cmocka_unit_test(test_safe_strcat_truncation),
        cmocka_unit_test(test_safe_strcat_null_dst),
        cmocka_unit_test(test_safe_strcat_null_src),
        cmocka_unit_test(test_safe_strcat_empty_dst),

        /* safe_strdup 测试 */
        cmocka_unit_test(test_safe_strdup_normal),
        cmocka_unit_test(test_safe_strdup_null),
        cmocka_unit_test(test_safe_strdup_empty_string),
        cmocka_unit_test(test_safe_strdup_long_string),
    };

    return cmocka_run_group_tests(tests, NULL, NULL);
}
