#ifndef REGISTRY_SERVICE_H
#define REGISTRY_SERVICE_H

#include "utils/string_ext.h"

// 加载系统变量和用户变量到字符串列表
int load_system_paths(StringList *list);
int load_user_paths(StringList *list);

// 从字符串列表保存系统变量和用户变量
int save_system_paths(const StringList *list);
int save_user_paths(const StringList *list);

#endif // REGISTRY_SERVICE_H
