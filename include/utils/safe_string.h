#ifndef SAFE_STRING_H
#define SAFE_STRING_H

#include <stddef.h>

// 安全字符串操作函数
char* safe_strcpy(char *dst, size_t dst_size, const char *src);

// 安全字符串拼接函数
char* safe_strcat(char *dst, size_t dst_size, const char *src);

// 安全字符串复制函数
char* safe_strdup(const char *src);

#endif // SAFE_STRING_H
