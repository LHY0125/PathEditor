#ifndef IMPORT_EXPORT_H
#define IMPORT_EXPORT_H

#include "utils/string_ext.h"

#define EXPORT_VERSION "1.0"

// 导出 PATH 到文件
int export_paths_to_file(const StringList *list, const char *filepath, int is_system);

// 从文件导入 PATH
int import_paths_from_file(const char *filepath, StringList *list);

#endif // IMPORT_EXPORT_H