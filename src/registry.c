#include "registry.h"
#include "globals.h"
#include "utils.h"
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>

// 内部辅助函数：加载单个列表
static void load_single_path(HKEY hKeyRoot, const wchar_t *regPath, Ihandle *list, StringList *cache)
{
    // 清空旧缓存
    clear_string_list(cache);

    HKEY hKey;
    LONG res = RegOpenKeyExW(hKeyRoot, regPath, 0, KEY_READ, &hKey);
    if (res != ERROR_SUCCESS)
    {
        // 只有 HKLM 失败才提示需要管理员，HKCU 失败可能是其他原因
        if (hKeyRoot == HKEY_LOCAL_MACHINE)
        {
            char msg[512];
            snprintf(msg, sizeof(msg), "无法打开注册表键 (HKLM)。\n路径: %ls\n错误码: %ld\n\n请尝试右键点击程序 -> '以管理员身份运行'。", regPath, res);
            IupMessage("错误", msg);
        }
        return;
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
                int count = 0;

                IupSetAttribute(list, "REMOVEITEM", "ALL");

                while (*current)
                {
                    next_semicolon = wcschr(current, L';');
                    if (next_semicolon)
                        *next_semicolon = L'\0';

                    if (wcslen(current) > 0)
                    {
                        char *utf8_str = wide_to_utf8(current);
                        
                        // 添加到列表
                        count++;
                        IupSetAttributeId(list, "", count, utf8_str);
                        
                        // 添加到缓存
                        add_string_list(cache, utf8_str);

                        free(utf8_str);
                    }

                    if (next_semicolon)
                        current = next_semicolon + 1;
                    else
                        break;
                }

                IupSetInt(list, "COUNT", count);
                IupSetInt(list, "VALUE", 1);
            }
            free(buffer);
        }
    }
    RegCloseKey(hKey);
}

// 加载所有PATH
void load_all_paths()
{
    load_single_path(HKEY_LOCAL_MACHINE, REG_PATH_SYS, list_sys, &raw_sys_paths);
    load_single_path(HKEY_CURRENT_USER, REG_PATH_USER, list_user, &raw_user_paths);

    refresh_list_style();
    IupSetAttribute(lbl_status, "TITLE", "状态: 已加载系统和用户变量");
}

// 内部辅助函数：保存单个列表
static int save_single_path(HKEY hKeyRoot, const wchar_t *regPath, Ihandle *list)
{
    int count = IupGetInt(list, "COUNT");

    // 计算大小
    size_t total_len = 0;
    for (int i = 1; i <= count; i++)
    {
        char *item = IupGetAttributeId(list, "", i);
        if (item)
        {
            wchar_t *witem = utf8_to_wide(item);
            total_len += wcslen(witem) + 1;
            free(witem);
        }
    }
    total_len += 1;

    wchar_t *buffer = (wchar_t *)malloc(total_len * sizeof(wchar_t));
    if (!buffer)
        return 0;

    buffer[0] = L'\0';
    for (int i = 1; i <= count; i++)
    {
        char *item = IupGetAttributeId(list, "", i);
        if (item)
        {
            wchar_t *witem = utf8_to_wide(item);
            wcscat(buffer, witem);
            if (i < count)
                wcscat(buffer, L";");
            free(witem);
        }
    }

    HKEY hKey;
    int success = 0;
    if (RegOpenKeyExW(hKeyRoot, regPath, 0, KEY_WRITE, &hKey) == ERROR_SUCCESS)
    {
        DWORD size = (wcslen(buffer) + 1) * sizeof(wchar_t);
        if (RegSetValueExW(hKey, REG_VALUE, 0, REG_EXPAND_SZ, (LPBYTE)buffer, size) == ERROR_SUCCESS)
        {
            success = 1;
        }
        RegCloseKey(hKey);
    }

    free(buffer);
    return success;
}

// 保存所有PATH
void save_all_paths()
{
    if (!check_admin())
    {
        IupMessage("错误", "需要管理员权限才能保存更改！");
        return;
    }

    // 备份
    backup_registry();

    int sys_ok = save_single_path(HKEY_LOCAL_MACHINE, REG_PATH_SYS, list_sys);
    int user_ok = save_single_path(HKEY_CURRENT_USER, REG_PATH_USER, list_user);

    if (sys_ok && user_ok)
    {
        SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, (LPARAM)L"Environment", SMTO_ABORTIFHUNG, 5000, NULL);
        IupMessage("成功", "系统和用户 PATH 环境变量均已更新！");
        IupSetAttribute(lbl_status, "TITLE", "状态: 全部保存成功");
    }
    else if (sys_ok)
    {
        IupMessage("提示", "系统变量保存成功，但用户变量保存失败。");
    }
    else if (user_ok)
    {
        IupMessage("提示", "用户变量保存成功，但系统变量保存失败。");
    }
    else
    {
        IupMessage("错误", "保存失败！");
        IupSetAttribute(lbl_status, "TITLE", "状态: 保存失败");
    }
}