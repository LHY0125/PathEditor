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

// 访问器函数 - 安全访问内部数据
// 获取指定索引的字符串（只读），越界返回 NULL
const char *string_list_get(const StringList *list, int index);
// 设置指定索引的字符串（会复制新字符串并释放旧字符串），越界返回 -1，成功返回 0
int string_list_set(StringList *list, int index, const char *str);

// 字符串转换函数
char *wide_to_utf8(const wchar_t *wstr);
wchar_t *utf8_to_wide(const char *str);
char *stristr(const char *haystack, const char *needle);

#endif // STRING_EXT_H
