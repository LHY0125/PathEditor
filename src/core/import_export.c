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

// 导出 PATH 到 JSON 文件
int export_paths_to_file(const StringList *list, const char *filepath, int is_system)
{
    if (!list || !filepath)
        return -1;

    FILE *fp = fopen(filepath, "w, ccs=UTF-8");
    if (!fp)
        return -1;

    char datetime[64];
    get_current_datetime(datetime, sizeof(datetime));

    fprintf(fp, "{\n");
    fprintf(fp, "  \"version\": \"%s\",\n", EXPORT_VERSION);
    fprintf(fp, "  \"type\": \"%s\",\n", is_system ? "SYSTEM" : "USER");
    fprintf(fp, "  \"exported\": \"%s\",\n", datetime);
    fprintf(fp, "  \"paths\": [\n");

    for (int i = 0; i < list->count; i++)
    {
        if (list->items[i])
        {
            char *escaped = escape_json_string(list->items[i]);
            if (escaped)
            {
                fprintf(fp, "    \"%s\"%s\n", escaped, (i < list->count - 1) ? "," : "");
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

// 检查是否为注释行或空行
static int is_comment_or_empty(const char *line)
{
    while (*line == ' ' || *line == '\t')
        line++;

    if (*line == '#' || *line == '\0')
        return 1;

    return 0;
}

// 检查是否为 JSON 文件
static int is_json_file(const char *filepath)
{
    const char *ext = strrchr(filepath, '.');
    return ext && strcasecmp(ext, ".json") == 0;
}

// 从文件导入 PATH
int import_paths_from_file(const char *filepath, StringList *list)
{
    if (!filepath || !list)
        return -1;

    if (is_json_file(filepath))
    {
        FILE *fp = fopen(filepath, "r, ccs=UTF-8");
        if (!fp)
            return -1;

        clear_string_list(list);

        char buffer[8192];
        int in_paths = 0;
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
                    else if (*p == ':')
                        in_paths = (depth == 1);
                    else if (in_paths && depth == 2 && *p == '"')
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
                            add_string_list(list, path_buffer);
                        }
                    }
                }
                p++;
            }
        }

        fclose(fp);
        return 0;
    }
    else
    {
        FILE *fp = fopen(filepath, "r, ccs=UTF-8");
        if (!fp)
            return -1;

        clear_string_list(list);

        char line[4096];
        while (fgets(line, sizeof(line), fp))
        {
            trim_whitespace(line);

            if (is_comment_or_empty(line))
                continue;

            add_string_list(list, line);
        }

        fclose(fp);
        return 0;
    }
}