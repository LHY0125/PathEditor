#ifndef APP_CONTEXT_H
#define APP_CONTEXT_H

#include "utils/string_ext.h"
#include "core/undo_redo.h"

// 应用上下文结构体,用于存储应用运行时的状态
typedef struct {
    StringList sys_paths;
    StringList user_paths;
    UndoRedoManager *undo_redo_mgr;  // 撤销/重做管理器
} AppContext;

// 创建应用上下文
AppContext* create_app_context(void);

// 销毁应用上下文
void destroy_app_context(AppContext* ctx);

#endif // APP_CONTEXT_H
