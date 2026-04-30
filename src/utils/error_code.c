#include "utils/error_code.h"
#include <string.h>

const char* error_code_to_string(ErrorCode code)
{
    switch (code)
    {
        case ERR_OK:
            return "Success";
        case ERR_FAILED:
            return "Operation failed";
        case ERR_NULL_PTR:
            return "Null pointer error";
        case ERR_OUT_OF_MEMORY:
            return "Out of memory";
        case ERR_FILE_NOT_FOUND:
            return "File not found";
        case ERR_PERMISSION_DENIED:
            return "Permission denied";
        case ERR_INVALID_FORMAT:
            return "Invalid format";
        case ERR_REGISTRY_FAILED:
            return "Registry operation failed";
        case ERR_NOT_FOUND:
            return "Item not found";
        case ERR_EXISTS:
            return "Item already exists";
        case ERR_INVALID_INDEX:
            return "Invalid index";
        default:
            return "Unknown error";
    }
}

// 注意：error_code_to_message 需要链接 intl 库，
// 如果不需要在错误码模块使用国际化，可以直接使用 error_code_to_string
const char* error_code_to_message(ErrorCode code)
{
    return error_code_to_string(code);
}