/*
 * import_export.c 单元测试
 * 测试 is_valid_path_format 和文件导入导出
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include "core/import_export.h"
#include "utils/string_ext.h"

/* ==================== Mock 函数 ==================== */

#ifdef TESTING

void log_info(const char *fmt, ...) { (void)fmt; }
void log_debug(const char *fmt, ...) { (void)fmt; }
void log_warn(const char *fmt, ...) { (void)fmt; }
void log_error(const char *fmt, ...) { (void)fmt; }

#endif

/* ==================== is_valid_path_format 测试 ==================== */

static void test_valid_drive_path(void **state)
{
    (void)state;
    assert_int_equal(is_valid_path_format("C:\\Windows"), 1);
    assert_int_equal(is_valid_path_format("D:\\Program Files"), 1);
    assert_int_equal(is_valid_path_format("c:\\test"), 1);
}

static void test_valid_unc_path(void **state)
{
    (void)state;
    assert_int_equal(is_valid_path_format("\\\\server\\share"), 1);
}

static void test_valid_env_var(void **state)
{
    (void)state;
    assert_int_equal(is_valid_path_format("%JAVA_HOME%\\bin"), 1);
    assert_int_equal(is_valid_path_format("%PATH%"), 1);
}

static void test_valid_relative_path(void **state)
{
    (void)state;
    assert_int_equal(is_valid_path_format(".\\subdir"), 1);
    assert_int_equal(is_valid_path_format("subdir\\file"), 1);
    assert_int_equal(is_valid_path_format("subdir/file"), 1);
}

static void test_null_path(void **state)
{
    (void)state;
    assert_int_equal(is_valid_path_format(NULL), 0);
}

static void test_empty_path(void **state)
{
    (void)state;
    assert_int_equal(is_valid_path_format(""), 0);
}

static void test_invalid_path(void **state)
{
    (void)state;
    /* 没有分隔符、没有环境变量、不是 UNC 路径 */
    assert_int_equal(is_valid_path_format("justaname"), 0);
}

static void test_drive_only(void **state)
{
    (void)state;
    /* C: 后面没有路径分隔符 */
    assert_int_equal(is_valid_path_format("C:"), 1);  /* colon后是 \0 */
}

/* ==================== export/import 文件测试 ==================== */

static void test_export_json(void **state)
{
    (void)state;
    ExportData data;
    init_string_list(&data.system);
    init_string_list(&data.user);
    add_string_list(&data.system, "C:\\Windows");
    add_string_list(&data.user, "C:\\Users\\test");

    const char *tmpfile = "test_export.json";
    ErrorCode result = export_paths_to_file(&data, tmpfile);
    assert_int_equal(result, ERR_OK);

    /* 验证文件内容 */
    FILE *fp = fopen(tmpfile, "r");
    assert_non_null(fp);
    char buffer[4096];
    fread(buffer, 1, sizeof(buffer) - 1, fp);
    fclose(fp);

    assert_non_null(strstr(buffer, "\"version\""));
    assert_non_null(strstr(buffer, "\"system\""));
    assert_non_null(strstr(buffer, "\"user\""));
    assert_non_null(strstr(buffer, "C:\\\\Windows"));

    remove(tmpfile);
    clear_string_list(&data.system);
    clear_string_list(&data.user);
}

static void test_export_csv(void **state)
{
    (void)state;
    ExportData data;
    init_string_list(&data.system);
    init_string_list(&data.user);
    add_string_list(&data.system, "C:\\Windows");

    const char *tmpfile = "test_export.csv";
    ErrorCode result = export_paths_to_format(&data, tmpfile, EXPORT_CSV);
    assert_int_equal(result, ERR_OK);

    FILE *fp = fopen(tmpfile, "r");
    assert_non_null(fp);
    char buffer[4096];
    fread(buffer, 1, sizeof(buffer) - 1, fp);
    fclose(fp);

    assert_non_null(strstr(buffer, "type,path"));
    assert_non_null(strstr(buffer, "system,"));

    remove(tmpfile);
    clear_string_list(&data.system);
    clear_string_list(&data.user);
}

static void test_export_null_data(void **state)
{
    (void)state;
    ErrorCode result = export_paths_to_file(NULL, "test.json");
    assert_int_equal(result, ERR_NULL_PTR);
}

static void test_export_null_path(void **state)
{
    (void)state;
    ExportData data;
    init_string_list(&data.system);
    ErrorCode result = export_paths_to_file(&data, NULL);
    assert_int_equal(result, ERR_NULL_PTR);
    clear_string_list(&data.system);
}

static void test_import_txt(void **state)
{
    (void)state;
    /* 创建临时 TXT 文件（二进制模式避免 \r\n 转换） */
    const char *tmpfile = "test_import.txt";
    FILE *fp = fopen(tmpfile, "wb");
    assert_non_null(fp);
    fprintf(fp, "C:\\Path1\n");
    fprintf(fp, "C:\\Path2\n");
    fprintf(fp, "# comment\n");
    fprintf(fp, "\n");
    fprintf(fp, "C:\\Path3\n");
    fclose(fp);

    ExportData data;
    ErrorCode result = import_paths_from_file(tmpfile, &data);
    assert_int_equal(result, ERR_OK);
    assert_int_equal(data.system.count, 3);
    assert_string_equal(string_list_get(&data.system, 0), "C:\\Path1");
    assert_string_equal(string_list_get(&data.system, 1), "C:\\Path2");
    assert_string_equal(string_list_get(&data.system, 2), "C:\\Path3");

    remove(tmpfile);
    clear_string_list(&data.system);
}

static void test_import_null_filepath(void **state)
{
    (void)state;
    ExportData data;
    ErrorCode result = import_paths_from_file(NULL, &data);
    assert_int_equal(result, ERR_NULL_PTR);
}

static void test_import_null_data(void **state)
{
    (void)state;
    ErrorCode result = import_paths_from_file("test.txt", NULL);
    assert_int_equal(result, ERR_NULL_PTR);
}

static void test_import_nonexistent_file(void **state)
{
    (void)state;
    ExportData data;
    ErrorCode result = import_paths_from_file("nonexistent_file_12345.txt", &data);
    assert_int_equal(result, ERR_FILE_NOT_FOUND);
}

static void test_import_csv(void **state)
{
    (void)state;
    /* 创建临时 CSV 文件 */
    const char *tmpfile = "test_import.csv";
    FILE *fp = fopen(tmpfile, "wb");
    assert_non_null(fp);
    fprintf(fp, "\xEF\xBB\xBF");  /* UTF-8 BOM */
    fprintf(fp, "type,path\n");
    fprintf(fp, "system,C:\\Windows\n");
    fprintf(fp, "system,C:\\Program Files\n");
    fprintf(fp, "user,C:\\Users\\test\n");
    fclose(fp);

    ExportData data;
    ErrorCode result = import_paths_from_file(tmpfile, &data);
    assert_int_equal(result, ERR_OK);
    assert_int_equal(data.system.count, 2);
    assert_int_equal(data.user.count, 1);
    assert_string_equal(string_list_get(&data.system, 0), "C:\\Windows");
    assert_string_equal(string_list_get(&data.system, 1), "C:\\Program Files");
    assert_string_equal(string_list_get(&data.user, 0), "C:\\Users\\test");

    remove(tmpfile);
    clear_string_list(&data.system);
    clear_string_list(&data.user);
}

static void test_import_csv_quoted(void **state)
{
    (void)state;
    /* 创建带引号字段的 CSV 文件 */
    const char *tmpfile = "test_import_quoted.csv";
    FILE *fp = fopen(tmpfile, "wb");
    assert_non_null(fp);
    fprintf(fp, "type,path\n");
    fprintf(fp, "system,\"C:\\Program Files\"\n");
    fprintf(fp, "user,\"C:\\My\"\"Path\"\n");  /* 转义引号 */
    fclose(fp);

    ExportData data;
    ErrorCode result = import_paths_from_file(tmpfile, &data);
    assert_int_equal(result, ERR_OK);
    assert_int_equal(data.system.count, 1);
    assert_int_equal(data.user.count, 1);
    assert_string_equal(string_list_get(&data.system, 0), "C:\\Program Files");
    assert_string_equal(string_list_get(&data.user, 0), "C:\\My\"Path");

    remove(tmpfile);
    clear_string_list(&data.system);
    clear_string_list(&data.user);
}

static void test_export_csv_format(void **state)
{
    (void)state;
    ExportData data;
    init_string_list(&data.system);
    init_string_list(&data.user);
    add_string_list(&data.system, "C:\\Windows");
    add_string_list(&data.user, "C:\\Users");

    const char *tmpfile = "test_csv_format.csv";
    ErrorCode result = export_paths_to_format(&data, tmpfile, EXPORT_CSV);
    assert_int_equal(result, ERR_OK);

    /* 验证 CSV 格式：type,path（不带双重引号） */
    FILE *fp = fopen(tmpfile, "rb");
    assert_non_null(fp);
    char buffer[4096];
    fread(buffer, 1, sizeof(buffer) - 1, fp);
    fclose(fp);

    assert_non_null(strstr(buffer, "system,\"C:\\Windows\""));
    assert_non_null(strstr(buffer, "user,\"C:\\Users\""));
    /* 不应该有双重引号 */
    assert_null(strstr(buffer, "\"\""));

    remove(tmpfile);
    clear_string_list(&data.system);
    clear_string_list(&data.user);
}

/* ==================== 主函数 ==================== */

int main(void)
{
    const struct CMUnitTest tests[] = {
        /* is_valid_path_format */
        cmocka_unit_test(test_valid_drive_path),
        cmocka_unit_test(test_valid_unc_path),
        cmocka_unit_test(test_valid_env_var),
        cmocka_unit_test(test_valid_relative_path),
        cmocka_unit_test(test_null_path),
        cmocka_unit_test(test_empty_path),
        cmocka_unit_test(test_invalid_path),
        cmocka_unit_test(test_drive_only),

        /* export */
        cmocka_unit_test(test_export_json),
        cmocka_unit_test(test_export_csv),
        cmocka_unit_test(test_export_null_data),
        cmocka_unit_test(test_export_null_path),

        /* import */
        cmocka_unit_test(test_import_txt),
        cmocka_unit_test(test_import_null_filepath),
        cmocka_unit_test(test_import_null_data),
        cmocka_unit_test(test_import_nonexistent_file),
        cmocka_unit_test(test_import_csv),
        cmocka_unit_test(test_import_csv_quoted),

        /* CSV format */
        cmocka_unit_test(test_export_csv_format),
    };

    return cmocka_run_group_tests(tests, NULL, NULL);
}
