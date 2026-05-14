#ifndef STRING_EXT_H
#define STRING_EXT_H

#include <wchar.h>

// 简单字符串列表结构
typedef struct
{
    char **items;
    int count;
    int capacity;
} StringList;

// 字符串列表
void init_string_list(StringList *list);
void add_string_list(StringList *list, const char *str);
void clear_string_list(StringList *list);

// 字符串转换函数
char *wide_to_utf8(const wchar_t *wstr);
wchar_t *utf8_to_wide(const char *str);
char *stristr(const char *haystack, const char *needle);

#endif // STRING_EXT_H
