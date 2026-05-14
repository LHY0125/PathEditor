#ifndef REGISTRY_SERVICE_H
#define REGISTRY_SERVICE_H

#include "utils/string_ext.h"
#include "utils/error_code.h"

// 加载系统变量和用户变量到字符串列表
ErrorCode load_system_paths(StringList *list);
ErrorCode load_user_paths(StringList *list);

// 从字符串列表保存系统变量和用户变量
ErrorCode save_system_paths(const StringList *list);
ErrorCode save_user_paths(const StringList *list);

#endif // REGISTRY_SERVICE_H
