#include "core/registry_service.h"
#include "utils/string_ext.h"
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>

#define REG_PATH_SYS L"SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment"
#define REG_PATH_USER L"Environment"
#define REG_VALUE L"Path"

// 内部辅助函数：加载单个列表
static int load_single_path(HKEY hKeyRoot, const wchar_t *regPath, StringList *list)
{
    clear_string_list(list);

    HKEY hKey;
    LONG res = RegOpenKeyExW(hKeyRoot, regPath, 0, KEY_READ, &hKey);
    if (res != ERROR_SUCCESS)
    {
        return 0; // 失败
    }

    DWORD type, size;
    res = RegQueryValueExW(hKey, REG_VALUE, NULL, &type, NULL, &size);
    if (res == ERROR_SUCCESS)
    {
        wchar_t *buffer = (wchar_t *)malloc(size + 2);
        if (buffer)
        {
            memset(buffer, 0, size + 2);
            if (RegQueryValueExW(hKey, REG_VALUE, NULL, &type, (LPBYTE)buffer, &size) == ERROR_SUCCESS)
            {
                wchar_t *current = buffer;
                wchar_t *next_semicolon = NULL;

                while (*current)
                {
                    next_semicolon = wcschr(current, L';');
                    if (next_semicolon)
                        *next_semicolon = L'\0';

                    if (wcslen(current) > 0)
                    {
                        char *utf8_str = wide_to_utf8(current);
                        if (utf8_str)
                        {
                            add_string_list(list, utf8_str);
                            free(utf8_str);
                        }
                    }

                    if (next_semicolon)
                        current = next_semicolon + 1;
                    else
                        break;
                }
            }
            free(buffer);
        }
    }
    RegCloseKey(hKey);
    return 1; // 成功
}

// 加载系统环境变量路径
int load_system_paths(StringList *list)
{
    return load_single_path(HKEY_LOCAL_MACHINE, REG_PATH_SYS, list);
}

// 加载用户环境变量路径
int load_user_paths(StringList *list)
{
    return load_single_path(HKEY_CURRENT_USER, REG_PATH_USER, list);
}

// 内部辅助函数：保存单个列表
static int save_single_path(HKEY hKeyRoot, const wchar_t *regPath, const StringList *list)
{
    if (!list) return 0;

    // 计算大小
    size_t total_len = 0;
    for (int i = 0; i < list->count; i++)
    {
        if (list->items[i])
        {
            wchar_t *witem = utf8_to_wide(list->items[i]);
            if (witem)
            {
                total_len += wcslen(witem) + 1;
                free(witem);
            }
        }
    }
    total_len += 1;

    wchar_t *buffer = (wchar_t *)malloc(total_len * sizeof(wchar_t));
    if (!buffer)
        return 0;

    buffer[0] = L'\0';
    for (int i = 0; i < list->count; i++)
    {
        if (list->items[i])
        {
            wchar_t *witem = utf8_to_wide(list->items[i]);
            if (witem)
            {
                wcscat(buffer, witem);
                if (i < list->count - 1)
                    wcscat(buffer, L";");
                free(witem);
            }
        }
    }

    HKEY hKey;
    int success = 0;
    if (RegOpenKeyExW(hKeyRoot, regPath, 0, KEY_WRITE, &hKey) == ERROR_SUCCESS)
    {
        DWORD size = (DWORD)((wcslen(buffer) + 1) * sizeof(wchar_t));
        if (RegSetValueExW(hKey, REG_VALUE, 0, REG_EXPAND_SZ, (LPBYTE)buffer, size) == ERROR_SUCCESS)
        {
            success = 1;
        }
        RegCloseKey(hKey);
    }

    free(buffer);
    return success;
}

// 保存系统环境变量路径
int save_system_paths(const StringList *list)
{
    return save_single_path(HKEY_LOCAL_MACHINE, REG_PATH_SYS, list);
}

// 保存用户环境变量路径
int save_user_paths(const StringList *list)
{
    return save_single_path(HKEY_CURRENT_USER, REG_PATH_USER, list);
}