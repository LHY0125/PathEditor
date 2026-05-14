#ifndef APP_CONTEXT_H
#define APP_CONTEXT_H

#include "utils/string_ext.h"
#include <iup.h>

// 应用上下文结构体,用于存储应用运行时的状态
typedef struct {
    StringList sys_paths;
    StringList user_paths;
} AppContext;

// 创建应用上下文
AppContext* create_app_context(void);

// 销毁应用上下文
void destroy_app_context(AppContext* ctx);

// 获取应用上下文
AppContext* get_app_context(Ihandle *ih);

#endif // APP_CONTEXT_H
