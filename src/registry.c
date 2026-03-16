#include "registry.h"
#include "globals.h"
#include "utils.h"
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>

// 从注册表加载PATH
void load_path()
{
    HKEY hKey;
    LONG res = RegOpenKeyExW(HKEY_LOCAL_MACHINE, REG_PATH, 0, KEY_READ, &hKey);
    if (res != ERROR_SUCCESS)
    {
        char msg[512];
        snprintf(msg, sizeof(msg), "无法打开注册表键 (HKLM)。\n路径: %ls\n错误码: %ld\n\n请尝试右键点击程序 -> '以管理员身份运行'。", REG_PATH, res);
        IupMessage("错误", msg);
        IupSetAttribute(lbl_status, "TITLE", "状态: 注册表读取失败");
        return;
    }

    DWORD type, size;
    res = RegQueryValueExW(hKey, REG_VALUE, NULL, &type, NULL, &size);
    if (res == ERROR_SUCCESS)
    {
        // 安全分配内存：size 是字节数，多分配 2 个字节给 null 终止符
        wchar_t *buffer = (wchar_t *)malloc(size + 2);
        if (!buffer)
        {
            IupMessage("错误", "内存分配失败！");
            RegCloseKey(hKey);
            return;
        }

        // 初始化内存
        memset(buffer, 0, size + 2);

        if (RegQueryValueExW(hKey, REG_VALUE, NULL, &type, (LPBYTE)buffer, &size) == ERROR_SUCCESS)
        {
            // 重新实现分割逻辑，避免 wcstok 的兼容性问题
            wchar_t *current = buffer;
            wchar_t *next_semicolon = NULL;
            int count = 0;

            // 清空列表
            IupSetAttribute(list_path, "REMOVEITEM", "ALL");

            // 检查内容是否为空
            if (wcslen(buffer) == 0)
            {
                IupMessage("提示", "读取到的 PATH 变量为空！");
            }

            while (*current)
            {
                // 查找下一个分号
                next_semicolon = wcschr(current, L';');
                if (next_semicolon)
                {
                    *next_semicolon = L'\0'; // 临时截断
                }

                // 跳过空项
                if (wcslen(current) > 0)
                {
                    char *utf8_str = wide_to_utf8(current);

                    count++;
                    IupSetAttributeId(list_path, "", count, utf8_str);

                    free(utf8_str);
                }

                if (next_semicolon)
                {
                    current = next_semicolon + 1;
                }
                else
                {
                    break;
                }
            }

            IupSetInt(list_path, "COUNT", count); // 显式设置列表项数量
            IupSetInt(list_path, "VALUE", 1);     // 选中第一项

            char status_msg[100];
            sprintf(status_msg, "状态: 已加载 %d 个条目", count);
            IupSetAttribute(lbl_status, "TITLE", status_msg);
        }
        else
        {
            IupMessage("错误", "读取 PATH 值内容失败！");
            IupSetAttribute(lbl_status, "TITLE", "状态: 读取值内容失败");
        }
        free(buffer);
    }
    else
    {
        char msg[256];
        sprintf(msg, "查询 PATH 值大小失败。错误码: %ld", res);
        IupMessage("错误", msg);
        IupSetAttribute(lbl_status, "TITLE", "状态: 查询值失败");
    }
    RegCloseKey(hKey);
}

// 保存PATH到注册表
void save_path()
{
    if (!check_admin())
    {
        IupMessage("错误", "需要管理员权限才能保存更改！\n请重新以管理员身份运行程序。");
        return;
    }

    int count = IupGetInt(list_path, "COUNT");
    if (count == 0)
    {
        // 警告：清空PATH是很危险的
        if (IupAlarm("警告", "PATH 为空！这可能导致系统命令无法使用。\n确定要保存吗？", "确定", "取消", NULL) != 1)
        {
            return;
        }
    }

    // 计算所需缓冲区大小
    size_t total_len = 0;
    for (int i = 1; i <= count; i++)
    {
        char *item = IupGetAttributeId(list_path, "", i);
        if (item)
        {
            wchar_t *witem = utf8_to_wide(item);
            total_len += wcslen(witem) + 1; // +1 for ';'
            free(witem);
        }
    }
    total_len += 1; // null terminator

    wchar_t *buffer = (wchar_t *)malloc(total_len * sizeof(wchar_t));
    if (!buffer)
    {
        IupMessage("错误", "内存分配失败 (保存时)！");
        return;
    }
    buffer[0] = L'\0';

    for (int i = 1; i <= count; i++)
    {
        char *item = IupGetAttributeId(list_path, "", i);
        if (item)
        {
            wchar_t *witem = utf8_to_wide(item);
            wcscat(buffer, witem);
            if (i < count)
            {
                wcscat(buffer, L";");
            }
            free(witem);
        }
    }

    // 写入注册表
    HKEY hKey;
    if (RegOpenKeyExW(HKEY_LOCAL_MACHINE, REG_PATH, 0, KEY_WRITE, &hKey) == ERROR_SUCCESS)
    {
        // 使用 REG_EXPAND_SZ 类型，因为 PATH 可能包含 %SystemRoot%
        DWORD size = (wcslen(buffer) + 1) * sizeof(wchar_t);
        if (RegSetValueExW(hKey, REG_VALUE, 0, REG_EXPAND_SZ, (LPBYTE)buffer, size) == ERROR_SUCCESS)
        {
            // 发送系统广播通知环境变量已更改
            SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, (LPARAM)L"Environment", SMTO_ABORTIFHUNG, 5000, NULL);
            IupMessage("成功", "PATH 环境变量已更新！");
            IupSetAttribute(lbl_status, "TITLE", "状态: 保存成功");
        }
        else
        {
            IupMessage("错误", "写入注册表失败！");
            IupSetAttribute(lbl_status, "TITLE", "状态: 保存失败");
        }
        RegCloseKey(hKey);
    }
    else
    {
        IupMessage("错误", "无法打开注册表进行写入。请检查权限！");
        IupSetAttribute(lbl_status, "TITLE", "状态: 打开注册表失败");
    }

    free(buffer);
}