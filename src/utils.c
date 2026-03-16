#include "utils.h"
#include "globals.h"
#include <stdio.h>
#include <stdlib.h>
#include <iup.h>
#include <time.h>
#include <direct.h>
#include <string.h>

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

// 检查路径是否存在
static int path_exists(const char *path)
{
    // 如果包含 %，说明是变量，无法直接检查存在性，默认视为有效
    if (strchr(path, '%'))
        return 1;

    wchar_t *wpath = utf8_to_wide(path);
    if (!wpath)
        return 0;

    DWORD attr = GetFileAttributesW(wpath);
    free(wpath);

    if (attr == INVALID_FILE_ATTRIBUTES)
        return 0;
    return (attr & FILE_ATTRIBUTE_DIRECTORY); // 必须是目录
}

// 检查路径是否存在（公开给外部使用）
int is_path_valid(const char *path)
{
    return path_exists(path);
}

// 刷新列表样式（斑马纹 + 有效性检查）
void refresh_single_list_style(Ihandle *list)
{
    if (!list)
        return;
    int count = IupGetInt(list, "COUNT");

    // 用于查重的简单数组（实际项目可以用哈希表）
    // 为了简单，我们只用双重循环检查重复，性能在几百个条目下没问题

    for (int i = 1; i <= count; i++)
    {
        char *item = IupGetAttributeId(list, "", i);
        if (!item)
            continue;

        // 默认颜色：黑字
        char fg_color[32] = "0 0 0";

        // 1. 检查有效性
        if (!path_exists(item))
        {
            // 无效路径：红色
            sprintf(fg_color, "255 0 0");
        }

        // 2. 检查重复 (只检查当前项之前的项，如果之前出现过，当前项标橙)
        for (int j = 1; j < i; j++)
        {
            char *prev_item = IupGetAttributeId(list, "", j);
            if (prev_item && _stricmp(item, prev_item) == 0) // Windows 路径不区分大小写
            {
                // 重复路径：橙色
                sprintf(fg_color, "255 128 0");
                break;
            }
        }

        IupSetAttributeId(list, "ITEMFGCOLOR", i, fg_color);

        // 斑马纹背景
        if (i % 2 == 0)
        {
            IupSetAttributeId(list, "ITEMBGCOLOR", i, "245 245 245");
        }
        else
        {
            IupSetAttributeId(list, "ITEMBGCOLOR", i, "255 255 255");
        }
    }
}

// 刷新所有列表样式
void refresh_list_style()
{
    refresh_single_list_style(list_sys);
    refresh_single_list_style(list_user);
}

// 备份注册表
void backup_registry()
{
    // 创建 records 目录
    if (_mkdir("records") == -1)
    {
        // 目录可能已存在，忽略错误
    }

    // 获取当前时间
    time_t t = time(NULL);
    struct tm *tm_info = localtime(&t);
    char time_str[64];
    strftime(time_str, sizeof(time_str), "%Y%m%d_%H%M%S", tm_info);

    // 构造备份文件名
    char filename[256];
    snprintf(filename, sizeof(filename), "records\\backup_%s.reg", time_str);

    // 构造 reg export 命令
    char full_cmd[1024];
    snprintf(full_cmd, sizeof(full_cmd), "reg.exe export \"HKLM\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment\" \"%s\" /y", filename);

    // 使用 CreateProcess 隐藏窗口执行
    STARTUPINFOA si;
    PROCESS_INFORMATION pi;
    memset(&si, 0, sizeof(si));
    si.cb = sizeof(si);
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = SW_HIDE; // 隐藏窗口
    memset(&pi, 0, sizeof(pi));

    if (CreateProcessA(NULL, full_cmd, NULL, NULL, FALSE, CREATE_NO_WINDOW, NULL, NULL, &si, &pi))
    {
        // 等待进程结束
        WaitForSingleObject(pi.hProcess, INFINITE);

        DWORD exit_code = 0;
        GetExitCodeProcess(pi.hProcess, &exit_code);

        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
    }
}

// 初始化字符串列表
void init_string_list(StringList *list)
{
    list->items = NULL;
    list->count = 0;
    list->capacity = 0;
}

// 不区分大小写的字符串查找 (简单实现)
char *stristr(const char *haystack, const char *needle)
{
    if (!haystack || !needle)
        return NULL;
    if (*needle == '\0')
        return (char *)haystack;

    const char *h = haystack;
    const char *n = needle;
    const char *h_start = haystack;

    while (*h)
    {
        h_start = h;
        n = needle;
        while (*h && *n && (tolower((unsigned char)*h) == tolower((unsigned char)*n)))
        {
            h++;
            n++;
        }
        if (*n == '\0')
            return (char *)h_start;
        h = h_start + 1;
    }
    return NULL;
}

// 添加字符串到列表
void add_string_list(StringList *list, const char *str)
{
    if (list->count >= list->capacity)
    {
        list->capacity = (list->capacity == 0) ? 16 : list->capacity * 2;
        list->items = (char **)realloc(list->items, list->capacity * sizeof(char *));
    }
    list->items[list->count] = _strdup(str); // 复制字符串
    list->count++;
}

// 清空字符串列表
void clear_string_list(StringList *list)
{
    for (int i = 0; i < list->count; i++)
    {
        free(list->items[i]);
    }
    free(list->items);
    list->items = NULL;
    list->count = 0;
    list->capacity = 0;
}