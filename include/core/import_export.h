#ifndef IMPORT_EXPORT_H
#define IMPORT_EXPORT_H

#include "utils/string_ext.h"

#define EXPORT_VERSION "1.0"

typedef struct {
    StringList system;
    StringList user;
} ExportData;

// 导出 PATH 到文件
int export_paths_to_file(const ExportData *data, const char *filepath);

// 从文件导入 PATH (返回是否包含全部格式)
int import_paths_from_file(const char *filepath, ExportData *data);

#endif // IMPORT_EXPORT_H