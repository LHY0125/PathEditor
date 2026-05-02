/*
 * string_ext.c 单元测试
 * 测试字符串扩展函数和 StringList 操作
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include <stdlib.h>
#include "utils/string_ext.h"
#include "mock_windows.h"

/* ==================== Mock 计数器 ==================== */
int mock_MultiByteToWideChar_call_count = 0;
int mock_WideCharToMultiByte_call_count = 0;
static int mock_MB2WC_return = 0;
static int mock_WC2MB_return = 0;

void mock_set_MultiByteToWideChar_return(int ret) {
    mock_MB2WC_return = ret;
}

void mock_set_WideCharToMultiByte_return(int ret) {
    mock_WC2MB_return = ret;
}

/* ==================== Mock 实现 ==================== */

int mock_MultiByteToWideChar(
    UINT CodePage,
    DWORD dwFlags,
    LPCSTR lpMultiByteStr,
    int cbMultiByte,
    LPWSTR lpWideCharStr,
    int cchWideChar)
{
    (void)CodePage;
    (void)dwFlags;
    mock_MultiByteToWideChar_call_count++;

    if (!lpMultiByteStr || cbMultiByte == 0)
        return 0;

    int len = (cbMultiByte == -1) ? strlen(lpMultiByteStr) : cbMultiByte;

    /* 简单 ASCII 转宽字符 */
    if (lpWideCharStr && cchWideChar > 0) {
        int copy_len = (cchWideChar < len + 1) ? cchWideChar - 1 : len;
        for (int i = 0; i < copy_len; i++) {
            lpWideCharStr[i] = (wchar_t)lpMultiByteStr[i];
        }
        lpWideCharStr[copy_len] = L'\0';
    }

    return mock_MB2WC_return > 0 ? mock_MB2WC_return : (len + 1);
}

int mock_WideCharToMultiByte(
    UINT CodePage,
    DWORD dwFlags,
    LPCWSTR lpWideCharStr,
    int cchWideChar,
    LPSTR lpMultiByteStr,
    int cbMultiByte,
    LPCSTR lpDefaultChar,
    LPBOOL lpUsedDefaultChar)
{
    (void)CodePage;
    (void)dwFlags;
    (void)lpDefaultChar;
    (void)lpUsedDefaultChar;
    mock_WideCharToMultiByte_call_count++;

    if (!lpWideCharStr || cchWideChar == 0)
        return 0;

    int len = (cchWideChar == -1) ? wcslen(lpWideCharStr) : cchWideChar;

    if (lpMultiByteStr && cbMultiByte > 0) {
        int copy_len = (cbMultiByte < len + 1) ? cbMultiByte - 1 : len;
        for (int i = 0; i < copy_len; i++) {
            lpMultiByteStr[i] = (char)lpWideCharStr[i];
        }
        lpMultiByteStr[copy_len] = '\0';
    }

    return mock_WC2MB_return > 0 ? mock_WC2MB_return : (len + 1);
}

/* ==================== StringList 测试 ==================== */

static void test_init_string_list(void **state)
{
    (void)state;
    StringList list;
    list.items = (void *)0x1234;  /* 初始化前设置垃圾值 */
    list.count = 999;
    list.capacity = 999;

    init_string_list(&list);

    /* init_string_list 将 items 设置为 NULL，count 和 capacity 设为 0 */
    assert_null(list.items);
    assert_int_equal(list.count, 0);
    assert_int_equal(list.capacity, 0);

    /* clear_string_list 对 NULL items 应该安全处理 */
    clear_string_list(&list);
}

static void test_add_string_list_single(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "C:\\Windows");

    assert_int_equal(list.count, 1);
    assert_string_equal(string_list_get(&list, 0), "C:\\Windows");

    clear_string_list(&list);
}

static void test_add_string_list_multiple(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "C:\\Windows");
    add_string_list(&list, "C:\\Program Files");
    add_string_list(&list, "D:\\Tools");

    assert_int_equal(list.count, 3);
    assert_string_equal(string_list_get(&list, 0), "C:\\Windows");
    assert_string_equal(string_list_get(&list, 1), "C:\\Program Files");
    assert_string_equal(string_list_get(&list, 2), "D:\\Tools");

    clear_string_list(&list);
}

static void test_string_list_get_out_of_bounds(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "test");

    assert_null(string_list_get(&list, -1));
    assert_null(string_list_get(&list, 1));
    assert_null(string_list_get(&list, 100));

    clear_string_list(&list);
}

static void test_string_list_get_null_list(void **state)
{
    (void)state;
    assert_null(string_list_get(NULL, 0));
}

static void test_string_list_set_normal(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "original");
    int result = string_list_set(&list, 0, "modified");

    assert_int_equal(result, 0);
    assert_string_equal(string_list_get(&list, 0), "modified");

    clear_string_list(&list);
}

static void test_string_list_set_out_of_bounds(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "test");
    int result = string_list_set(&list, 5, "modified");

    assert_int_equal(result, -1);

    clear_string_list(&list);
}

static void test_string_list_set_null_list(void **state)
{
    (void)state;
    int result = string_list_set(NULL, 0, "test");
    assert_int_equal(result, -1);
}

static void test_clear_string_list(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "item1");
    add_string_list(&list, "item2");
    add_string_list(&list, "item3");

    clear_string_list(&list);

    assert_int_equal(list.count, 0);
    assert_null(list.items);
}

/* ==================== string_list_insert_at 测试 ==================== */

static void test_insert_at_beginning(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "B");
    add_string_list(&list, "C");

    int result = string_list_insert_at(&list, 0, "A");

    assert_int_equal(result, 0);
    assert_int_equal(list.count, 3);
    assert_string_equal(string_list_get(&list, 0), "A");
    assert_string_equal(string_list_get(&list, 1), "B");
    assert_string_equal(string_list_get(&list, 2), "C");

    clear_string_list(&list);
}

static void test_insert_at_middle(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "A");
    add_string_list(&list, "C");

    int result = string_list_insert_at(&list, 1, "B");

    assert_int_equal(result, 0);
    assert_int_equal(list.count, 3);
    assert_string_equal(string_list_get(&list, 0), "A");
    assert_string_equal(string_list_get(&list, 1), "B");
    assert_string_equal(string_list_get(&list, 2), "C");

    clear_string_list(&list);
}

static void test_insert_at_end(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "A");

    int result = string_list_insert_at(&list, 1, "B");

    assert_int_equal(result, 0);
    assert_int_equal(list.count, 2);
    assert_string_equal(string_list_get(&list, 0), "A");
    assert_string_equal(string_list_get(&list, 1), "B");

    clear_string_list(&list);
}

static void test_insert_at_empty_list(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    int result = string_list_insert_at(&list, 0, "A");

    assert_int_equal(result, 0);
    assert_int_equal(list.count, 1);
    assert_string_equal(string_list_get(&list, 0), "A");

    clear_string_list(&list);
}

static void test_insert_at_invalid_index(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "A");

    assert_int_equal(string_list_insert_at(&list, -1, "B"), -1);
    assert_int_equal(string_list_insert_at(&list, 5, "B"), -1);
    assert_int_equal(list.count, 1);

    clear_string_list(&list);
}

static void test_insert_at_null(void **state)
{
    (void)state;
    assert_int_equal(string_list_insert_at(NULL, 0, "A"), -1);

    StringList list;
    init_string_list(&list);
    assert_int_equal(string_list_insert_at(&list, 0, NULL), -1);

    clear_string_list(&list);
}

/* ==================== string_list_contains 测试 ==================== */

static void test_contains_found(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "C:\\Windows");
    add_string_list(&list, "C:\\Program Files");

    assert_int_equal(string_list_contains(&list, "C:\\Windows"), 1);
    assert_int_equal(string_list_contains(&list, "c:\\windows"), 1);  /* 不区分大小写 */

    clear_string_list(&list);
}

static void test_contains_not_found(void **state)
{
    (void)state;
    StringList list;
    init_string_list(&list);

    add_string_list(&list, "C:\\Windows");

    assert_int_equal(string_list_contains(&list, "D:\\Tools"), 0);

    clear_string_list(&list);
}

static void test_contains_null(void **state)
{
    (void)state;
    assert_int_equal(string_list_contains(NULL, "test"), 0);

    StringList list;
    init_string_list(&list);
    assert_int_equal(string_list_contains(&list, NULL), 0);

    clear_string_list(&list);
}

/* ==================== 编码转换测试 ==================== */

static void test_utf8_to_wide_normal(void **state)
{
    (void)state;
    const char *utf8_str = "Hello";

    wchar_t *result = utf8_to_wide(utf8_str);

    if (result) {
        assert_true(wcscmp(result, L"Hello") == 0);
        free(result);
    }
}

static void test_utf8_to_wide_null(void **state)
{
    (void)state;
    wchar_t *result = utf8_to_wide(NULL);
    assert_null(result);
}

static void test_wide_to_utf8_normal(void **state)
{
    (void)state;
    const wchar_t *wide_str = L"World";

    char *result = wide_to_utf8(wide_str);

    if (result) {
        assert_string_equal(result, "World");
        free(result);
    }
}

static void test_wide_to_utf8_null(void **state)
{
    (void)state;
    char *result = wide_to_utf8(NULL);
    assert_null(result);
}

/* ==================== stristr 测试 ==================== */

static void test_stristr_found(void **state)
{
    (void)state;
    const char *haystack = "The quick brown fox";
    const char *needle = "quick";

    char *result = stristr(haystack, needle);

    assert_non_null(result);
    assert_ptr_equal(result, haystack + 4);  /* "quick" 在 "The " 之后 */
}

static void test_stristr_not_found(void **state)
{
    (void)state;
    const char *haystack = "The quick brown fox";
    const char *needle = "jumps";

    char *result = stristr(haystack, needle);

    assert_null(result);
}

static void test_stristr_case_insensitive(void **state)
{
    (void)state;
    const char *haystack = "The QUICK brown fox";
    const char *needle = "quick";

    char *result = stristr(haystack, needle);

    assert_non_null(result);
}

static void test_stristr_null_haystack(void **state)
{
    (void)state;
    char *result = stristr(NULL, "test");
    assert_null(result);
}

static void test_stristr_null_needle(void **state)
{
    (void)state;
    char *result = stristr("test", NULL);
    assert_null(result);
}

static void test_stristr_empty_needle(void **state)
{
    (void)state;
    const char *haystack = "The quick brown fox";
    const char *needle = "";

    char *result = stristr(haystack, needle);

    /* 空字符串应该返回原字符串首地址 */
    assert_non_null(result);
    assert_ptr_equal(result, haystack);
}

/* ==================== 主函数 ==================== */

int main(void)
{
    const struct CMUnitTest tests[] = {
        /* StringList 测试 */
        cmocka_unit_test(test_init_string_list),
        cmocka_unit_test(test_add_string_list_single),
        cmocka_unit_test(test_add_string_list_multiple),
        cmocka_unit_test(test_string_list_get_out_of_bounds),
        cmocka_unit_test(test_string_list_get_null_list),
        cmocka_unit_test(test_string_list_set_normal),
        cmocka_unit_test(test_string_list_set_out_of_bounds),
        cmocka_unit_test(test_string_list_set_null_list),
        cmocka_unit_test(test_clear_string_list),

        /* insert_at 测试 */
        cmocka_unit_test(test_insert_at_beginning),
        cmocka_unit_test(test_insert_at_middle),
        cmocka_unit_test(test_insert_at_end),
        cmocka_unit_test(test_insert_at_empty_list),
        cmocka_unit_test(test_insert_at_invalid_index),
        cmocka_unit_test(test_insert_at_null),

        /* contains 测试 */
        cmocka_unit_test(test_contains_found),
        cmocka_unit_test(test_contains_not_found),
        cmocka_unit_test(test_contains_null),

        /* 编码转换测试 */
        cmocka_unit_test(test_utf8_to_wide_normal),
        cmocka_unit_test(test_utf8_to_wide_null),
        cmocka_unit_test(test_wide_to_utf8_normal),
        cmocka_unit_test(test_wide_to_utf8_null),

        /* stristr 测试 */
        cmocka_unit_test(test_stristr_found),
        cmocka_unit_test(test_stristr_not_found),
        cmocka_unit_test(test_stristr_case_insensitive),
        cmocka_unit_test(test_stristr_null_haystack),
        cmocka_unit_test(test_stristr_null_needle),
        cmocka_unit_test(test_stristr_empty_needle),
    };

    return cmocka_run_group_tests(tests, NULL, NULL);
}
