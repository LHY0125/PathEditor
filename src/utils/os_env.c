#include "utils/os_env.h"
#include "utils/string_ext.h"
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <direct.h>

// 检查管理员权限
int check_admin(void)
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

// 内部：检查路径是否存在
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

// 备份注册表
void backup_registry(void)
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