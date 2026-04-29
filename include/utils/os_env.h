#ifndef OS_ENV_H
#define OS_ENV_H

#include "utils/error_code.h"

// 检查是否以管理员权限运行
int check_admin(void);

// 检查路径是否有效
int is_path_valid(const char *path);

// 备份注册表
// 参数 backup_path: 自定义备份目录路径，传 NULL 使用 Lua 配置中的默认路径
ErrorCode backup_registry(const char *backup_path);

#endif // OS_ENV_H
