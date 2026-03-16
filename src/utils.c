#include "utils.h"
#include <stdio.h>
#include <stdlib.h>
#include <iup.h>

// 宽字符转UTF-8 (用于IUP显示)
char *wide_to_utf8(const wchar_t *wstr)
{
    if (!wstr)
        return NULL;
    int size_needed = WideCharToMultiByte(CP_UTF8, 0, wstr, -1, NULL, 0, NULL, NULL);
    char *str = (char *)malloc(size_needed);
    WideCharToMultiByte(CP_UTF8, 0, wstr, -1, str, size_needed, NULL, NULL);
    return str;
}

// UTF-8转宽字符 (用于Windows API)
wchar_t *utf8_to_wide(const char *str)
{
    if (!str)
        return NULL;
    int size_needed = MultiByteToWideChar(CP_UTF8, 0, str, -1, NULL, 0);
    wchar_t *wstr = (wchar_t *)malloc(size_needed * sizeof(wchar_t));
    MultiByteToWideChar(CP_UTF8, 0, str, -1, wstr, size_needed);
    return wstr;
}

// 检查管理员权限
int check_admin()
{
    HKEY hKey;
    LONG result = RegOpenKeyExW(HKEY_LOCAL_MACHINE, L"SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment", 0, KEY_WRITE, &hKey);
    if (result == ERROR_SUCCESS)
    {
        RegCloseKey(hKey);
        return 1;
    }
    return 0;
}