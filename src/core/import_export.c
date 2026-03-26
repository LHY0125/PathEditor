#include "core/import_export.h"
#include "utils/os_env.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

// 获取当前日期时间
static void get_current_datetime(char *buffer, int size)
{
    time_t now = time(NULL);
    struct tm *tm_info = localtime(&now);
    strftime(buffer, size, "%Y-%m-%d %H:%M:%S", tm_info);
}

// 转义 JSON 字符串中的特殊字符
static char *escape_json_string(const char *str)
{
    if (!str)
        return NULL;

    int len = strlen(str);
    char *result = (char *)malloc(len * 2 + 1);
    if (!result)
        return NULL;

    char *p = result;
    for (int i = 0; i < len; i++)
    {
        switch (str[i])
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
        default:
            *p++ = str[i];
            break;
        }
    }
    *p = '\0';
    return result;
}

// 导出路径数据到 JSON 文件
int export_paths_to_file(const ExportData *data, const char *filepath)
{
    if (!data || !filepath)
        return -1;

    FILE *fp = fopen(filepath, "w");
    if (!fp)
        return -1;

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
    return 0;
}

// 移除字符串首尾的空格、制表符、换行符和回车符
static void trim_whitespace(char *str)
{
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
    return ext && strcasecmp(ext, ".json") == 0;
}

// 从文件导入 PATH (返回是否包含全部格式)
int import_paths_from_file(const char *filepath, ExportData *data)
{
    if (!filepath || !data)
        return -1;

    init_string_list(&data->system);
    init_string_list(&data->user);

    if (!is_json_file(filepath))
    {
        FILE *fp = fopen(filepath, "rb");
        if (!fp)
            return -1;

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
        return 0;
    }

    FILE *fp = fopen(filepath, "rb");
    if (!fp)
        return -1;

    char buffer[8192];
    int in_system = 0;
    int in_user = 0;
    int depth = 0;
    int in_string = 0;
    char path_buffer[4096];
    int path_len = 0;

    while (fgets(buffer, sizeof(buffer), fp))
    {
        char *p = buffer;
        while (*p)
        {
            if (*p == '"' && (p == buffer || *(p - 1) != '\\'))
            {
                in_string = !in_string;
            }
            else if (in_string && *p == '\\')
            {
                p++;
                if (*p)
                {
                    if (*p == 'n')
                        *p = '\n';
                    else if (*p == 'r')
                        *p = '\r';
                    else if (*p == 't')
                        *p = '\t';
                }
            }
            else if (!in_string)
            {
                if (*p == '{' || *p == '[')
                    depth++;
                else if (*p == '}' || *p == ']')
                    depth--;
                else if (depth == 1 && *p == '"')
                {
                    if (strncmp(p, "\"system\"", 8) == 0)
                    {
                        in_system = 1;
                        in_user = 0;
                    }
                    else if (strncmp(p, "\"user\"", 6) == 0)
                    {
                        in_user = 1;
                        in_system = 0;
                    }
                }
                else if (in_system && depth == 2 && *p == '"')
                {
                    path_len = 0;
                    p++;
                    while (*p && path_len < (int)sizeof(path_buffer) - 1)
                    {
                        if (*p == '"' && *(p - 1) != '\\')
                            break;
                        if (*p == '\\' && *(p + 1))
                        {
                            p++;
                            if (*p == 'n')
                                *p = '\n';
                            else if (*p == 'r')
                                *p = '\r';
                            else if (*p == 't')
                                *p = '\t';
                        }
                        path_buffer[path_len++] = *p++;
                    }
                    if (path_len > 0)
                    {
                        path_buffer[path_len] = '\0';
                        add_string_list(&data->system, path_buffer);
                    }
                }
                else if (in_user && depth == 2 && *p == '"')
                {
                    path_len = 0;
                    p++;
                    while (*p && path_len < (int)sizeof(path_buffer) - 1)
                    {
                        if (*p == '"' && *(p - 1) != '\\')
                            break;
                        if (*p == '\\' && *(p + 1))
                        {
                            p++;
                            if (*p == 'n')
                                *p = '\n';
                            else if (*p == 'r')
                                *p = '\r';
                            else if (*p == 't')
                                *p = '\t';
                        }
                        path_buffer[path_len++] = *p++;
                    }
                    if (path_len > 0)
                    {
                        path_buffer[path_len] = '\0';
                        add_string_list(&data->user, path_buffer);
                    }
                }
            }
            p++;
        }
    }

    fclose(fp);
    return 0;
}