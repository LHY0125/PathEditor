#ifndef UTILS_H
#define UTILS_H

#include <windows.h>
#include <wchar.h>
#include <iup.h>

// 宽字符转UTF-8
char* wide_to_utf8(const wchar_t* wstr);

// UTF-8转宽字符
wchar_t* utf8_to_wide(const char* str);

// 检查管理员权限
int check_admin();

// 展开环境变量
char* expand_env_vars(const char* path);

// 检查路径是否有效（存在且为目录）
int is_path_valid(const char *path);

// 刷新列表样式（斑马纹）
void refresh_list_style();
void refresh_single_list_style(Ihandle *list);

// 备份注册表
void backup_registry();

// 不区分大小写的字符串查找
char *stristr(const char *haystack, const char *needle);

#endif // UTILS_H