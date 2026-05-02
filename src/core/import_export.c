#include "core/import_export.h"
#include "utils/os_env.h"
#include "utils/error_code.h"
#include "utils/logger.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <time.h>

// 获取当前日期时间
static void get_current_datetime(char *buffer, int size)
{
    time_t now = time(NULL);
    struct tm tm_info;
    localtime_s(&tm_info, &now);
    strftime(buffer, size, "%Y-%m-%d %H:%M:%S", &tm_info);
}

// 转义 JSON 字符串中的特殊字符（符合 RFC 8259 规范）
static char *escape_json_string(const char *str)
{
    if (!str)
        return NULL;

    int len = strlen(str);
    // 最坏情况：每个字符都需要 \uXXXX 转义（6字节）
    char *result = (char *)malloc(len * 6 + 1);
    if (!result)
        return NULL;

    char *p = result;
    for (int i = 0; i < len; i++)
    {
        unsigned char c = (unsigned char)str[i];
        switch (c)
        {
        case '\\':
            *p++ = '\\';
            *p++ = '\\';
            break;
        case '"':
            *p++ = '\\';
            *p++ = '"';
            break;
        case '\n':
            *p++ = '\\';
            *p++ = 'n';
            break;
        case '\r':
            *p++ = '\\';
            *p++ = 'r';
            break;
        case '\t':
            *p++ = '\\';
            *p++ = 't';
            break;
        case '\b':
            *p++ = '\\';
            *p++ = 'b';
            break;
        case '\f':
            *p++ = '\\';
            *p++ = 'f';
            break;
        default:
            if (c < 0x20) // 其他控制字符 (0x00-0x1F)
            {
                sprintf(p, "\\u%04x", c);
                p += 6;
            }
            else
            {
                *p++ = str[i];
            }
            break;
        }
    }
    *p = '\0';
    return result;
}

// 转义 CSV 字段中的特殊字符
static char *escape_csv_field(const char *str)
{
    if (!str)
        return NULL;

    int len = strlen(str);
    // 需要转义双引号和包含逗号、引号、换行的字段
    char *result = (char *)malloc(len * 2 + 3);
    if (!result)
        return NULL;

    char *p = result;
    *p++ = '"';

    for (int i = 0; i < len; i++)
    {
        unsigned char c = (unsigned char)str[i];
        switch (c)
        {
        case '"':
            *p++ = '"';
            *p++ = '"';
            break;
        default:
            *p++ = str[i];
            break;
        }
    }

    *p++ = '"';
    *p = '\0';
    return result;
}

// 导出 PATH 到 JSON 文件
static ErrorCode export_paths_to_json(const ExportData *data, FILE *fp)
{
    char datetime[64];
    get_current_datetime(datetime, sizeof(datetime));

    fprintf(fp, "{\n");
    fprintf(fp, "  \"version\": \"%s\",\n", EXPORT_VERSION);
    fprintf(fp, "  \"type\": \"ALL\",\n");
    fprintf(fp, "  \"exported\": \"%s\",\n", datetime);

    fprintf(fp, "  \"system\": [\n");
    for (int i = 0; i < data->system.count; i++)
    {
        if (data->system.items[i])
        {
            char *escaped = escape_json_string(data->system.items[i]);
            if (escaped)
            {
                fprintf(fp, "    \"%s\"%s\n", escaped, (i < data->system.count - 1) ? "," : "");
                free(escaped);
            }
        }
    }
    fprintf(fp, "  ],\n");

    fprintf(fp, "  \"user\": [\n");
    for (int i = 0; i < data->user.count; i++)
    {
        if (data->user.items[i])
        {
            char *escaped = escape_json_string(data->user.items[i]);
            if (escaped)
            {
                fprintf(fp, "    \"%s\"%s\n", escaped, (i < data->user.count - 1) ? "," : "");
                free(escaped);
            }
        }
    }
    fprintf(fp, "  ]\n");

    fprintf(fp, "}\n");
    return ERR_OK;
}

// 导出 PATH 到 CSV 文件
// 格式：type,path
// type: system 或 user
static ErrorCode export_paths_to_csv(const ExportData *data, FILE *fp)
{
    // 写入 UTF-8 BOM
    fprintf(fp, "\xEF\xBB\xBF");

    // 写入 CSV 标题行
    fprintf(fp, "type,path\n");

    // 写入系统路径
    for (int i = 0; i < data->system.count; i++)
    {
        if (data->system.items[i])
        {
            char *escaped = escape_csv_field(data->system.items[i]);
            if (escaped)
            {
                fprintf(fp, "system,%s\n", escaped);
                free(escaped);
            }
        }
    }

    // 写入用户路径
    for (int i = 0; i < data->user.count; i++)
    {
        if (data->user.items[i])
        {
            char *escaped = escape_csv_field(data->user.items[i]);
            if (escaped)
            {
                fprintf(fp, "user,%s\n", escaped);
                free(escaped);
            }
        }
    }

    return ERR_OK;
}

// 导出 PATH 到文件
ErrorCode export_paths_to_file(const ExportData *data, const char *filepath)
{
    if (!data || !filepath)
        return ERR_NULL_PTR;

    const char *ext = strrchr(filepath, '.');
    if (ext && _stricmp(ext, ".csv") == 0)
    {
        return export_paths_to_format(data, filepath, EXPORT_CSV);
    }
    return export_paths_to_format(data, filepath, EXPORT_JSON);
}

// 导出 PATH 到指定格式的文件
ErrorCode export_paths_to_format(const ExportData *data, const char *filepath, ExportFormat format)
{
    if (!data || !filepath)
        return ERR_NULL_PTR;

    FILE *fp = fopen(filepath, "w");
    if (!fp)
    {
        log_error("Failed to open file for export: %s", filepath);
        return ERR_FILE_NOT_FOUND;
    }

    ErrorCode result;
    if (format == EXPORT_CSV)
        result = export_paths_to_csv(data, fp);
    else
        result = export_paths_to_json(data, fp);

    fclose(fp);

    if (result == ERR_OK)
    {
        log_info("Exported paths to file: sys=%d, user=%d, format=%d, file=%s",
                 data->system.count, data->user.count, format, filepath);
    }
    return result;
}

// 移除字符串首尾的空格、制表符、换行符和回车符
static void trim_whitespace(char *str)
{
    if (!str || *str == '\0')
        return;

    char *start = str;
    while (*start == ' ' || *start == '\t')
        start++;

    char *end = str + strlen(str) - 1;
    while (end >= start && (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r'))
        *end-- = '\0';

    if (start != str)
        memmove(str, start, strlen(start) + 1);
}

// 检查字符串是否为注释行或空行
static int is_comment_or_empty(const char *line)
{
    while (*line == ' ' || *line == '\t')
        line++;

    if (*line == '#' || *line == '\0')
        return 1;

    return 0;
}

// 检查文件是否为 JSON 格式
static int is_json_file(const char *filepath)
{
    const char *ext = strrchr(filepath, '.');
    return ext && _stricmp(ext, ".json") == 0;
}

// 检查文件是否为 CSV 格式（通过扩展名）
static int is_csv_file(const char *filepath)
{
    const char *ext = strrchr(filepath, '.');
    return ext && _stricmp(ext, ".csv") == 0;
}

// 解析 CSV 字段（处理引号包围的字段）
// 返回值：指向下一个字段的指针，或 NULL
static const char *parse_csv_field(const char *line, char *field, int field_size)
{
    if (!line || !field || field_size <= 0)
        return NULL;

    const char *p = line;
    char *out = field;
    char *end = field + field_size - 1;

    if (*p == '"')
    {
        // 引号包围的字段
        p++; // 跳过开始引号
        while (*p && out < end)
        {
            if (*p == '"')
            {
                if (*(p + 1) == '"')
                {
                    // 转义的引号 ""
                    *out++ = '"';
                    p += 2;
                }
                else
                {
                    // 结束引号
                    p++; // 跳过结束引号
                    break;
                }
            }
            else
            {
                *out++ = *p++;
            }
        }
        // 跳过逗号分隔符
        if (*p == ',') p++;
    }
    else
    {
        // 非引号字段（按逗号分隔）
        while (*p && *p != ',' && out < end)
        {
            *out++ = *p++;
        }
        if (*p == ',') p++;
    }

    *out = '\0';
    return p;
}

// 导入 CSV 格式的 PATH 文件
static ErrorCode import_paths_from_csv(const char *filepath, ExportData *data)
{
    FILE *fp = fopen(filepath, "rb");
    if (!fp)
    {
        log_error("Failed to open file for import: %s", filepath);
        return ERR_FILE_NOT_FOUND;
    }

    char line[4096];
    int line_num = 0;
    int header_skipped = 0;

    while (fgets(line, sizeof(line), fp))
    {
        line_num++;
        trim_whitespace(line);

        if (line[0] == '\0')
            continue;

        // 跳过 UTF-8 BOM
        const char *start = line;
        if ((unsigned char)start[0] == 0xEF &&
            (unsigned char)start[1] == 0xBB &&
            (unsigned char)start[2] == 0xBF)
        {
            start += 3;
        }

        // 智能检测标题行：第一行包含 "type" 和 "path" 则视为标题
        if (!header_skipped)
        {
            header_skipped = 1;
            char lower[256];
            strncpy(lower, start, sizeof(lower) - 1);
            lower[sizeof(lower) - 1] = '\0';
            for (char *p = lower; *p; p++) *p = (char)tolower((unsigned char)*p);
            if (strstr(lower, "type") && strstr(lower, "path"))
                continue;
        }

        char type[32] = {0};
        char path[4096] = {0};

        start = parse_csv_field(start, type, sizeof(type));
        if (!start)
            continue;
        parse_csv_field(start, path, sizeof(path));

        if (path[0] == '\0')
            continue;

        if (_stricmp(type, "system") == 0)
            add_string_list(&data->system, path);
        else if (_stricmp(type, "user") == 0)
            add_string_list(&data->user, path);
    }

    fclose(fp);
    log_info("Imported paths from CSV file: sys=%d, user=%d, file=%s",
             data->system.count, data->user.count, filepath);
    return ERR_OK;
}

// 检查引号前是否有奇数个连续反斜杠（奇数个表示引号被转义）
static int is_quote_escaped(const char *quote_pos, const char *line_start)
{
    int backslash_count = 0;
    const char *p = quote_pos - 1;
    while (p >= line_start && *p == '\\')
    {
        backslash_count++;
        p--;
    }
    return (backslash_count % 2) == 1; // 奇数个反斜杠表示转义
}

// 从文件导入 PATH
ErrorCode import_paths_from_file(const char *filepath, ExportData *data)
{
    if (!filepath || !data)
        return ERR_NULL_PTR;

    init_string_list(&data->system);
    init_string_list(&data->user);

    if (is_csv_file(filepath))
    {
        return import_paths_from_csv(filepath, data);
    }

    if (!is_json_file(filepath))
    {
        FILE *fp = fopen(filepath, "rb");
        if (!fp)
        {
            log_error("Failed to open file for import: %s", filepath);
            return ERR_FILE_NOT_FOUND;
        }

        StringList list;
        init_string_list(&list);

        char line[4096];
        while (fgets(line, sizeof(line), fp))
        {
            trim_whitespace(line);
            if (is_comment_or_empty(line))
                continue;
            add_string_list(&list, line);
        }

        fclose(fp);

        data->system = list;
        log_info("Imported paths from TXT file: %d paths, file=%s", list.count, filepath);
        return ERR_OK;
    }

    FILE *fp = fopen(filepath, "rb");
    if (!fp)
    {
        log_error("Failed to open file for import: %s", filepath);
        return ERR_FILE_NOT_FOUND;
    }

    char buffer[8192];
    int in_system = 0;
    int in_user = 0;
    int depth = 0;
    int in_string = 0;
    char key_buffer[256] = {0};
    int key_len = 0;

    while (fgets(buffer, sizeof(buffer), fp))
    {
        char *p = buffer;
        while (*p)
        {
            // 处理字符串开始/结束
            if (*p == '"')
            {
                if (!in_string)
                {
                    // 字符串开始
                    in_string = 1;
                    key_len = 0; // 开始收集键名或字符串内容
                }
                else if (!is_quote_escaped(p, buffer))
                {
                    // 字符串结束（未转义的引号）
                    in_string = 0;

                    // 在 depth 1 时，检查刚结束的字符串是否是键名
                    if (depth == 1)
                    {
                        key_buffer[key_len] = '\0';
                        if (strcmp(key_buffer, "system") == 0)
                        {
                            in_system = 1;
                            in_user = 0;
                        }
                        else if (strcmp(key_buffer, "user") == 0)
                        {
                            in_user = 1;
                            in_system = 0;
                        }
                    }
                    // 在 depth 2 时，如果在 system/user 数组内，提取路径
                    else if (depth == 2 && (in_system || in_user))
                    {
                        key_buffer[key_len] = '\0';
                        if (key_len > 0)
                        {
                            StringList *target = in_system ? &data->system : &data->user;
                            add_string_list(target, key_buffer);
                        }
                    }
                }
                else
                {
                    // 转义的引号，作为内容的一部分
                    if (key_len < (int)sizeof(key_buffer) - 1)
                        key_buffer[key_len++] = *p;
                }
            }
            else if (in_string)
            {
                // 在字符串内，收集内容
                if (*p == '\\' && *(p + 1))
                {
                    // 处理转义序列
                    p++;
                    char ch;
                    switch (*p)
                    {
                    case 'n':  ch = '\n'; break;
                    case 'r':  ch = '\r'; break;
                    case 't':  ch = '\t'; break;
                    case 'b':  ch = '\b'; break;
                    case 'f':  ch = '\f'; break;
                    case '\\': ch = '\\'; break;
                    case '"':  ch = '"';  break;
                    case '/':  ch = '/';  break;
                    default:   ch = *p;   break;
                    }
                    if (key_len < (int)sizeof(key_buffer) - 1)
                        key_buffer[key_len++] = ch;
                }
                else
                {
                    if (key_len < (int)sizeof(key_buffer) - 1)
                        key_buffer[key_len++] = *p;
                }
            }
            else
            {
                // 不在字符串内
                if (*p == '{' || *p == '[')
                    depth++;
                else if (*p == '}' || *p == ']')
                    depth--;
            }
            p++;
        }
    }

    fclose(fp);
    log_info("Imported paths from JSON file: sys=%d, user=%d, file=%s",
             data->system.count, data->user.count, filepath);
    return ERR_OK;
}

// 验证路径格式是否有效
// 有效的 Windows 路径格式：
//   - 绝对路径：C:\path\to\something
//   - UNC 路径：\\server\share
//   - 环境变量：%PATH%
//   - 相对路径（带冒号后面跟着反斜杠或正斜杠的）
int is_valid_path_format(const char *path)
{
    if (!path || *path == '\0')
        return 0;

    // 检查是否包含冒号（驱动器路径）
    const char *colon = strchr(path, ':');

    // 检查是否以 \\ 开头（UNC 路径）
    if (path[0] == '\\' && path[1] == '\\')
        return 1;

    // 检查是否为驱动器路径（如 C:\）
    if (colon && colon - path == 1)
    {
        char drive = path[0];
        if ((drive >= 'A' && drive <= 'Z') || (drive >= 'a' && drive <= 'z'))
        {
            // 检查冒号后面是否是路径分隔符
            const char *after_colon = colon + 1;
            if (*after_colon == '\\' || *after_colon == '/' || *after_colon == '\0')
                return 1;
        }
    }

    // 检查是否包含环境变量（%...%）
    if (strchr(path, '%'))
        return 1;

    // 检查路径是否包含反斜杠或正斜杠（相对路径）
    if (strchr(path, '\\') || strchr(path, '/'))
        return 1;

    return 0;
}