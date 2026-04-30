#ifndef IMPORT_EXPORT_H
#define IMPORT_EXPORT_H

#include "utils/string_ext.h"
#include "utils/error_code.h"

#define EXPORT_VERSION "1.0"

// 导出数据类型
typedef enum {
    EXPORT_JSON,   // JSON 格式
    EXPORT_CSV     // CSV 格式
} ExportFormat;

// 导出数据结构
// 注意：此结构体用于导出时是只读的，items 指针指向外部 StringList 的数据
// 不要对 ExportData 调用 clear_string_list，会破坏原始数据
typedef struct {
    StringList system;
    StringList user;
} ExportData;

// 导出 PATH 到文件（自动根据扩展名选择格式）
ErrorCode export_paths_to_file(const ExportData *data, const char *filepath);

// 导出 PATH 到指定格式的文件
ErrorCode export_paths_to_format(const ExportData *data, const char *filepath, ExportFormat format);

// 从文件导入 PATH
ErrorCode import_paths_from_file(const char *filepath, ExportData *data);

// 验证路径格式是否有效
int is_valid_path_format(const char *path);

#endif // IMPORT_EXPORT_H