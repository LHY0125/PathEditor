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
int string_list_insert_at(StringList *list, int index, const char *str);
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

// 检查字符串列表中是否存在指定路径（不区分大小写）
int string_list_contains(const StringList *list, const char *str);

// 展开环境变量（如 %JAVA_HOME%\bin → C:\Java\bin）
// 返回 malloc 分配的字符串，调用者负责释放；无变量返回 NULL
char *expand_env_vars(const char *path);

#endif // STRING_EXT_H
