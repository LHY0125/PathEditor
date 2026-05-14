#ifndef ERROR_CODE_H
#define ERROR_CODE_H

typedef enum {
    ERR_OK = 0,                 // 成功
    ERR_FAILED = -1,            // 失败
    ERR_NULL_PTR = -2,          // 空指针
    ERR_OUT_OF_MEMORY = -3,     // 内存不足
    ERR_FILE_NOT_FOUND = -4,    // 文件不存在
    ERR_PERMISSION_DENIED = -5, // 权限拒绝
    ERR_INVALID_FORMAT = -6,    // 无效格式
    ERR_REGISTRY_FAILED = -7,   // 注册表操作失败
    ERR_NOT_FOUND = -8,         // 未找到
    ERR_EXISTS = -9             // 已存在
} ErrorCode;

const char* error_code_to_string(ErrorCode code);

#endif // ERROR_CODE_H
