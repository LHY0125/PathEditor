#ifndef OS_ENV_H
#define OS_ENV_H

// 检查是否以管理员权限运行
int check_admin(void);

// 检查路径是否有效
int is_path_valid(const char *path);

// 备份注册表
void backup_registry(void);

#endif // OS_ENV_H
