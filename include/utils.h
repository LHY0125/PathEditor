#ifndef UTILS_H
#define UTILS_H

#include <windows.h>
#include <wchar.h>

// 宽字符转UTF-8
char* wide_to_utf8(const wchar_t* wstr);

// UTF-8转宽字符
wchar_t* utf8_to_wide(const char* str);

// 检查管理员权限
int check_admin();

#endif // UTILS_H