#include "utils/os_env.h"
#include "utils/string_ext.h"
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <direct.h>
#include <shlobj.h>

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
// 备份到 %APPDATA%/PathEditor/backups/ 目录下
ErrorCode backup_registry(void)
{
    // 获取 AppData 路径
    wchar_t appdata_path[MAX_PATH];
    if (SHGetFolderPathW(NULL, CSIDL_APPDATA, NULL, 0, appdata_path) != S_OK)
    {
        return ERR_FAILED;
    }

    // 创建备份目录
    wchar_t backup_dir[MAX_PATH];
    swprintf(backup_dir, MAX_PATH, L"%s\\PathEditor\\backups", appdata_path);
    CreateDirectoryW(backup_dir, NULL);

    // 生成时间戳
    time_t t = time(NULL);
    struct tm *tm_info = localtime(&t);
    wchar_t timestamp[64];
    wcsftime(timestamp, sizeof(timestamp), L"%Y%m%d_%H%M%S", tm_info);

    // 构造备份文件名
    wchar_t backup_file[MAX_PATH];
    swprintf(backup_file, MAX_PATH, L"%s\\path_backup_%s.txt", backup_dir, timestamp);

    // 打开文件
    FILE *fp = _wfopen(backup_file, L"w, ccs=UTF-8");
    if (!fp)
        return ERR_FAILED;

    // 备份系统 PATH
    HKEY hKey;
    int success = 0;

    if (RegOpenKeyExW(HKEY_LOCAL_MACHINE,
                      L"SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
                      0, KEY_READ, &hKey) == ERROR_SUCCESS)
    {
        DWORD type, size;
        if (RegQueryValueExW(hKey, L"Path", NULL, &type, NULL, &size) == ERROR_SUCCESS)
        {
            wchar_t *buffer = (wchar_t *)malloc(size + 2);
            if (buffer)
            {
                memset(buffer, 0, size + 2);
                if (RegQueryValueExW(hKey, L"Path", NULL, &type, (LPBYTE)buffer, &size) == ERROR_SUCCESS)
                {
                    fwprintf(fp, L"# System PATH Backup\n");
                    fwprintf(fp, L"%s\n\n", buffer);
                    success = 1;
                }
                free(buffer);
            }
        }
        RegCloseKey(hKey);
    }

    // 备份用户 PATH
    if (RegOpenKeyExW(HKEY_CURRENT_USER,
                      L"Environment",
                      0, KEY_READ, &hKey) == ERROR_SUCCESS)
    {
        DWORD type, size;
        if (RegQueryValueExW(hKey, L"Path", NULL, &type, NULL, &size) == ERROR_SUCCESS)
        {
            wchar_t *buffer = (wchar_t *)malloc(size + 2);
            if (buffer)
            {
                memset(buffer, 0, size + 2);
                if (RegQueryValueExW(hKey, L"Path", NULL, &type, (LPBYTE)buffer, &size) == ERROR_SUCCESS)
                {
                    fwprintf(fp, L"# User PATH Backup\n");
                    fwprintf(fp, L"%s\n", buffer);
                    success = 1;
                }
                free(buffer);
            }
        }
        RegCloseKey(hKey);
    }

    fclose(fp);
    return success ? ERR_OK : ERR_FAILED;
}