#include "core/import_export.h"
#include "utils/os_env.h"
#include "utils/error_code.h"
#include "utils/logger.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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

// 导出路径数据到 JSON 文件
ErrorCode export_paths_to_file(const ExportData *data, const char *filepath)
{
    if (!data || !filepath)
        return ERR_NULL_PTR;

    FILE *fp = fopen(filepath, "w");
    if (!fp)
    {
        log_error("Failed to open file for export: %s", filepath);
        return ERR_FILE_NOT_FOUND;
    }

    fprintf(fp, "\xEF\xBB\xBF");

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

    fclose(fp);
    log_info("Exported paths to file: sys=%d, user=%d, file=%s",
             data->system.count, data->user.count, filepath);
    return ERR_OK;
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
    while (end > start && (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r'))
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