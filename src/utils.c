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

// 展开环境变量
char *expand_env_vars(const char *path)
{
    if (!path)
        return NULL;

    // 先转换为宽字符，因为ExpandEnvironmentStringsW不支持UTF-8
    wchar_t *wpath = utf8_to_wide(path);
    if (!wpath)
        return NULL;

    DWORD size = ExpandEnvironmentStringsW(wpath, NULL, 0);
    if (size == 0)
    {
        free(wpath);
        return NULL;
    }

    wchar_t *wexpanded = (wchar_t *)malloc(size * sizeof(wchar_t));
    ExpandEnvironmentStringsW(wpath, wexpanded, size);
    free(wpath);

    char *expanded = wide_to_utf8(wexpanded);
    free(wexpanded);

    return expanded;
}

// 检查路径是否存在
static int path_exists(const char *path)
{
    char *expanded_path = expand_env_vars(path);
    if (!expanded_path)
        return 0;

    wchar_t *wpath = utf8_to_wide(expanded_path);
    free(expanded_path);

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
    int count = IupGetInt(list, "NUMLIN");

    for (int i = 1; i <= count; i++)
    {
        char *item = IupGetAttributeId2(list, "", i, 1);
        if (!item)
            continue;

        const char *status_str = "正常";
        char bg_color[32] = "";

        // 1. 检查有效性
        if (!path_exists(item))
        {
            // 无效路径：红色背景
            strcpy(bg_color, "255 100 100"); // 柔和的红色
            status_str = "无效";
        }
        else
        {
            // 2. 检查重复 (只检查当前项之前的项，如果之前出现过，当前项标橙)
            for (int j = 1; j < i; j++)
            {
                char *prev_item = IupGetAttributeId2(list, "", j, 1);
                if (prev_item && _stricmp(item, prev_item) == 0) // Windows 路径不区分大小写
                {
                    // 重复路径：橙色背景
                    strcpy(bg_color, "255 165 0");
                    status_str = "重复";
                    break;
                }
            }
        }

        IupSetAttributeId2(list, "", i, 2, status_str);

        // 如果没有特殊状态，使用斑马纹背景
        if (strlen(bg_color) == 0)
        {
            if (is_dark_mode)
            {
                if (i % 2 == 0)
                    strcpy(bg_color, "60 60 60");
                else
                    strcpy(bg_color, "50 50 50");
            }
            else
            {
                if (i % 2 == 0)
                    strcpy(bg_color, "245 245 245");
                else
                    strcpy(bg_color, "255 255 255");
            }
        }

        char attr_bg[32];
        sprintf(attr_bg, "BGCOLOR%d:*", i);
        IupSetAttribute(list, attr_bg, bg_color);

        // 恢复前景色
        char attr_fg[32];
        sprintf(attr_fg, "FGCOLOR%d:*", i);
        if (is_dark_mode)
            IupSetAttribute(list, attr_fg, "255 255 255");
        else
            IupSetAttribute(list, attr_fg, "0 0 0");
    }

    IupSetAttribute(list, "REDRAW", "ALL"); // Ensure Matrix repaints colors
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

// 复制字符串列表
void copy_string_list(StringList *dest, StringList *src)
{
    init_string_list(dest);
    if (!src || src->count == 0)
        return;
    for (int i = 0; i < src->count; i++)
    {
        add_string_list(dest, src->items[i]);
    }
}

// 初始化历史栈
void init_history_stack(HistoryStack *stack)
{
    stack->top = NULL;
    stack->count = 0;
}

// 压入历史
void push_history(HistoryStack *stack, StringList *sys, StringList *user)
{
    HistoryNode *node = (HistoryNode *)malloc(sizeof(HistoryNode));
    if (!node)
        return;

    copy_string_list(&node->sys_paths, sys);
    copy_string_list(&node->user_paths, user);

    node->next = stack->top;
    stack->top = node;
    stack->count++;

    // 简单限制：如果超过 50 个，就不处理底部了（太麻烦），反正内存够用
}

// 弹出历史
int pop_history(HistoryStack *stack, StringList *out_sys, StringList *out_user)
{
    if (!stack->top)
        return 0;

    HistoryNode *node = stack->top;
    stack->top = node->next;
    stack->count--;

    // 转移所有权，避免复制
    *out_sys = node->sys_paths;
    *out_user = node->user_paths;

    free(node);
    return 1;
}

// 清空历史栈
void clear_history_stack(HistoryStack *stack)
{
    while (stack->top)
    {
        HistoryNode *node = stack->top;
        stack->top = node->next;

        clear_string_list(&node->sys_paths);
        clear_string_list(&node->user_paths);
        free(node);
    }
    stack->count = 0;
}