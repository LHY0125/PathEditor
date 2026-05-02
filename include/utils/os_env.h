#ifndef OS_ENV_H
#define OS_ENV_H

#include "utils/error_code.h"
#include <stddef.h>

// 获取可执行文件所在目录（带缓存）
// buf: 输出缓冲区，size: 缓冲区大小
void get_exe_dir(char *buf, size_t size);

// 检查是否以管理员权限运行
int check_admin(void);

// 检查路径是否有效
int is_path_valid(const char *path);

// 备份注册表
// 参数 backup_path: 自定义备份目录路径，传 NULL 使用 Lua 配置中的默认路径
ErrorCode backup_registry(const char *backup_path);

#endif // OS_ENV_H
