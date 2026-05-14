#ifndef PATH_MANAGER_H
#define PATH_MANAGER_H

#include "utils/string_ext.h"
#include "utils/error_code.h"

// 移除列表中指定索引的项
ErrorCode path_manager_remove_at(StringList *list, int index);

// 上移指定索引的项
ErrorCode path_manager_move_up(StringList *list, int index);

// 下移指定索引的项
ErrorCode path_manager_move_down(StringList *list, int index);

// 清理无效和重复的路径
ErrorCode path_manager_clean(StringList *list);

#endif // PATH_MANAGER_H
