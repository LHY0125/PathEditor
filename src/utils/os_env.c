#include "utils/os_env.h"
#include "utils/string_ext.h"
#include "utils/logger.h"
#include "core/lua_config.h"
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <direct.h>
#include <shlobj.h>

// 获取可执行文件所在目录（带缓存）
static char s_exe_dir[256] = {0};

void get_exe_dir(char *buf, size_t size)
{
    if (s_exe_dir[0] != '\0')
    {
        strncpy(buf, s_exe_dir, size - 1);
        buf[size - 1] = '\0';
        return;
    }
    DWORD len = GetModuleFileNameA(NULL, buf, (DWORD)size);
    if (len > 0 && len < size)
    {
        char *last_sep = strrchr(buf, '\\');
        if (!last_sep) last_sep = strrchr(buf, '/');
        if (last_sep) *last_sep = '\0';
    }
    strncpy(s_exe_dir, buf, sizeof(s_exe_dir) - 1);
    s_exe_dir[sizeof(s_exe_dir) - 1] = '\0';
}

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
// 参数 backup_path: 自定义备份目录路径，传 NULL 使用 Lua 配置中的默认路径
// 返回值：
//   ERR_OK - 备份成功（系统和用户 PATH 都已备份）
//   ERR_FAILED - AppData 路径获取失败
//   ERR_FILE_NOT_FOUND - 备份文件创建失败
//   ERR_REGISTRY_FAILED - 系统和用户 PATH 都读取失败
ErrorCode backup_registry(const char *backup_path)
{
    wchar_t backup_dir[MAX_PATH];

    if (backup_path && strlen(backup_path) > 0)
    {
        // 使用用户指定的路径
        wchar_t *wpath = utf8_to_wide(backup_path);
        if (wpath)
        {
            wcsncpy(backup_dir, wpath, MAX_PATH - 1);
            backup_dir[MAX_PATH - 1] = L'\0';
            free(wpath);
        }
        else
        {
            log_error("Failed to convert backup path to wide string");
            return ERR_FAILED;
        }
    }
    else
    {
        // 使用默认路径：Lua 配置 > %APPDATA%/PathEditor/backups/
        const char *lua_dir = lua_config_get_string("backup", "dir");
        if (lua_dir && strlen(lua_dir) > 0)
        {
            wchar_t *wpath = utf8_to_wide(lua_dir);
            if (wpath)
            {
                wcsncpy(backup_dir, wpath, MAX_PATH - 1);
                backup_dir[MAX_PATH - 1] = L'\0';
                free(wpath);
            }
            else
            {
                log_error("Failed to convert Lua backup dir to wide string");
                return ERR_FAILED;
            }
        }
        else
        {
            // 获取 AppData 路径作为默认值
            wchar_t appdata_path[MAX_PATH];
            if (SHGetFolderPathW(NULL, CSIDL_APPDATA, NULL, 0, appdata_path) != S_OK)
            {
                log_error("Failed to get AppData path");
                return ERR_FAILED;
            }
            swprintf(backup_dir, MAX_PATH, L"%s\\PathEditor\\backups", appdata_path);
        }
    }

    // 创建备份目录（递归创建中间目录）
    if (SHCreateDirectoryExW(NULL, backup_dir, NULL) != ERROR_SUCCESS)
    {
        log_error("Failed to create backup directory: %ls", backup_dir);
        return ERR_FILE_NOT_FOUND;
    }

    // 生成时间戳
    time_t t = time(NULL);
    struct tm tm_info;
    localtime_s(&tm_info, &t);
    wchar_t timestamp[64];
    wcsftime(timestamp, sizeof(timestamp) / sizeof(timestamp[0]), L"%Y%m%d_%H%M%S", &tm_info);

    // 构造备份文件名
    wchar_t backup_file[MAX_PATH];
    swprintf(backup_file, MAX_PATH, L"%s\\path_backup_%s.txt", backup_dir, timestamp);

    // 打开文件
    FILE *fp = _wfopen(backup_file, L"w, ccs=UTF-8");
    if (!fp)
    {
        log_error("Failed to create backup file: %ls", backup_file);
        return ERR_FILE_NOT_FOUND;
    }

    // 备份系统 PATH
    HKEY hKey;
    int sys_ok = 0;
    int user_ok = 0;

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
                    sys_ok = 1;
                }
                free(buffer);
            }
        }
        RegCloseKey(hKey);
    }
    else
    {
        log_warn("Failed to open system PATH registry key");
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
                    user_ok = 1;
                }
                free(buffer);
            }
        }
        RegCloseKey(hKey);
    }
    else
    {
        log_warn("Failed to open user PATH registry key");
    }

    fclose(fp);

    // 判断备份结果
    if (!sys_ok && !user_ok)
    {
        log_error("Backup failed: both system and user PATH read failed");
        return ERR_REGISTRY_FAILED;
    }

    if (!sys_ok)
        log_warn("Backup partial: system PATH read failed, only user PATH backed up");
    if (!user_ok)
        log_warn("Backup partial: user PATH read failed, only system PATH backed up");

    log_info("Backup completed: sys=%d, user=%d, file=%ls", sys_ok, user_ok, backup_file);
    return ERR_OK;
}