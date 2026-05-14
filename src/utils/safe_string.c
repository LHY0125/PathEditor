#include "utils/safe_string.h"
#include <string.h>
#include <stdlib.h>

// 安全字符串复制
char *safe_strcpy(char *dst, size_t dst_size, const char *src)
{
    if (!dst || !src || dst_size == 0)
        return NULL;

    size_t len = strlen(src);
    if (len >= dst_size)
    {
        memcpy(dst, src, dst_size - 1);
        dst[dst_size - 1] = '\0';
    }
    else
    {
        strcpy(dst, src);
    }
    return dst;
}

// 安全字符串拼接
char *safe_strcat(char *dst, size_t dst_size, const char *src)
{
    if (!dst || !src || dst_size == 0)
        return NULL;

    size_t dst_len = strlen(dst);
    if (dst_len >= dst_size)
        return NULL;

    return safe_strcpy(dst + dst_len, dst_size - dst_len, src);
}

// 安全字符串复制（返回新分配的内存）
char *safe_strdup(const char *src)
{
    if (!src)
        return NULL;

    size_t len = strlen(src);
    char *result = (char *)malloc(len + 1);
    if (result)
        strcpy(result, src);

    return result;
}