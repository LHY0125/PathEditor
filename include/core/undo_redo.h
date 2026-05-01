#ifndef UNDO_REDO_H
#define UNDO_REDO_H

#include "utils/string_ext.h"

// 操作类型
typedef enum {
    OP_ADD,       // 添加路径
    OP_DELETE,    // 删除路径
    OP_EDIT,      // 编辑路径
    OP_MOVE_UP,   // 上移
    OP_MOVE_DOWN, // 下移
    OP_CLEAN,     // 清理（批量删除）
    OP_CLEAR,     // 清空列表
    OP_IMPORT     // 导入
} OperationType;

// 目标类型（哪个列表）
typedef enum {
    TARGET_SYSTEM,  // 系统变量
    TARGET_USER     // 用户变量
} TargetType;

// 单个操作记录
typedef struct {
    OperationType type;
    TargetType target;
    int index;
    int count;          // 用于批量操作（如清理、导入）
    char **old_paths;   // 操作前的路径列表（用于撤销）
    char **new_paths;   // 操作后的路径列表（用于重做）
} OpRecord;

// 撤销/重做管理器
typedef struct {
    OpRecord *records;
    int max_size;       // 最大历史记录数
    int current;        // 当前指针位置（-1表示最新操作之后）
    int count;          // 实际记录数
} UndoRedoManager;

// 创建撤销/重做管理器
UndoRedoManager *create_undo_redo_manager(int max_size);

// 销毁撤销/重做管理器
void destroy_undo_redo_manager(UndoRedoManager *mgr);

// 添加操作记录
int push_undo_record(UndoRedoManager *mgr, const OpRecord *record);

// 执行撤销
int undo(UndoRedoManager *mgr, StringList *sys_paths, StringList *user_paths);

// 执行重做
int redo(UndoRedoManager *mgr, StringList *sys_paths, StringList *user_paths);

// 检查是否可以撤销
int can_undo(const UndoRedoManager *mgr);

// 检查是否可以重做
int can_redo(const UndoRedoManager *mgr);

// 清空历史记录
void clear_undo_redo_history(UndoRedoManager *mgr);

#endif // UNDO_REDO_H