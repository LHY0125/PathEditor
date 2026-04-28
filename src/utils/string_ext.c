#include "utils/string_ext.h"
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

// 宽字符转UTF-8 (用于IUP显示)
char *wide_to_utf8(const wchar_t *wstr)
{
    if (!wstr)
        return NULL;
    int size_needed = WideCharToMultiByte(CP_UTF8, 0, wstr, -1, NULL, 0, NULL, NULL);
    char *str = (char *)malloc(size_needed);
    if (str)
    {
        WideCharToMultiByte(CP_UTF8, 0, wstr, -1, str, size_needed, NULL, NULL);
    }
    return str;
}

// UTF-8转宽字符 (用于Windows API)
wchar_t *utf8_to_wide(const char *str)
{
    if (!str)
        return NULL;
    int size_needed = MultiByteToWideChar(CP_UTF8, 0, str, -1, NULL, 0);
    wchar_t *wstr = (wchar_t *)malloc(size_needed * sizeof(wchar_t));
    if (wstr)
    {
        MultiByteToWideChar(CP_UTF8, 0, str, -1, wstr, size_needed);
    }
    return wstr;
}

// 初始化字符串列表
void init_string_list(StringList *list)
{
    if (!list)
        return;
    list->items = NULL;
    list->count = 0;
    list->capacity = 0;
}

// 不区分大小写的字符串查找 (简单实现)
char *stristr(const char *haystack, const char *needle)
{
    if (!haystack || !needle)
        return NULL;
    if (*needle == '\0')
        return (char *)haystack;

    const char *h = haystack;
    const char *n = needle;
    const char *h_start = haystack;

    while (*h)
    {
        h_start = h;
        n = needle;
        while (*h && *n && (tolower((unsigned char)*h) == tolower((unsigned char)*n)))
        {
            h++;
            n++;
        }
        if (*n == '\0')
            return (char *)h_start;
        h = h_start + 1;
    }
    return NULL;
}

// 添加字符串到列表
void add_string_list(StringList *list, const char *str)
{
    if (!list || !str)
        return;
    if (list->count >= list->capacity)
    {
        int new_capacity = (list->capacity == 0) ? 16 : list->capacity * 2;
        char **new_items = (char **)realloc(list->items, new_capacity * sizeof(char *));
        if (!new_items)
            return; // realloc 失败，保留原数据
        list->items = new_items;
        list->capacity = new_capacity;
    }
    char *dup = _strdup(str);
    if (!dup)
        return; // _strdup 失败，不递增 count
    list->items[list->count] = dup;
    list->count++;
}

// 获取指定索引的字符串（只读）
const char *string_list_get(const StringList *list, int index)
{
    if (!list || index < 0 || index >= list->count)
        return NULL;
    return list->items[index];
}

// 设置指定索引的字符串（复制新字符串并释放旧字符串）
int string_list_set(StringList *list, int index, const char *str)
{
    if (!list || index < 0 || index >= list->count || !str)
        return -1;
    char *dup = _strdup(str);
    if (!dup)
        return -1; // _strdup 失败，保留旧数据
    free(list->items[index]);
    list->items[index] = dup;
    return 0;
}

// 清空字符串列表
void clear_string_list(StringList *list)
{
    if (!list)
        return;
    for (int i = 0; i < list->count; i++)
    {
        free(list->items[i]);
    }
    free(list->items);
    list->items = NULL;
    list->count = 0;
    list->capacity = 0;
}