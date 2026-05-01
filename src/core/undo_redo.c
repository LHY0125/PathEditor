#include "core/undo_redo.h"
#include "core/path_manager.h"
#include <stdlib.h>
#include <string.h>
#include "utils/safe_string.h"
#include "utils/logger.h"

#define DEFAULT_MAX_UNDO_RECORDS 50

static char *copy_string(const char *str)
{
    if (!str)
        return NULL;
    return _strdup(str);
}

static void free_string_array(char **arr, int count)
{
    if (!arr)
        return;
    for (int i = 0; i < count; i++)
    {
        if (arr[i])
            free(arr[i]);
    }
    free(arr);
}

static char **copy_string_array(const char **arr, int count)
{
    if (!arr || count <= 0)
        return NULL;

    char **copy = (char **)malloc(count * sizeof(char *));
    if (!copy)
        return NULL;

    for (int i = 0; i < count; i++)
    {
        copy[i] = copy_string(arr[i]);
    }
    return copy;
}

static void init_op_record(OpRecord *record)
{
    memset(record, 0, sizeof(OpRecord));
}

static void free_op_record(OpRecord *record)
{
    if (record->old_paths)
        free_string_array(record->old_paths, record->count);
    if (record->new_paths)
        free_string_array(record->new_paths, record->count);
    init_op_record(record);
}

UndoRedoManager *create_undo_redo_manager(int max_size)
{
    if (max_size <= 0)
        max_size = DEFAULT_MAX_UNDO_RECORDS;

    UndoRedoManager *mgr = (UndoRedoManager *)malloc(sizeof(UndoRedoManager));
    if (!mgr)
        return NULL;

    mgr->records = (OpRecord *)malloc(max_size * sizeof(OpRecord));
    if (!mgr->records)
    {
        free(mgr);
        return NULL;
    }

    mgr->max_size = max_size;
    mgr->current = -1;
    mgr->count = 0;

    for (int i = 0; i < max_size; i++)
        init_op_record(&mgr->records[i]);

    return mgr;
}

void destroy_undo_redo_manager(UndoRedoManager *mgr)
{
    if (!mgr)
        return;

    for (int i = 0; i < mgr->count; i++)
        free_op_record(&mgr->records[i]);

    free(mgr->records);
    free(mgr);
}

int push_undo_record(UndoRedoManager *mgr, const OpRecord *record)
{
    if (!mgr || !record)
        return -1;

    // 如果 current 不是在最新位置（已经撤销过），清除重做历史
    while (mgr->count > mgr->current + 1)
    {
        mgr->count--;
        free_op_record(&mgr->records[mgr->count]);
    }

    // 如果已满，移除最旧的记录
    if (mgr->count >= mgr->max_size)
    {
        // 移除第一条记录
        free_op_record(&mgr->records[0]);
        for (int i = 0; i < mgr->max_size - 1; i++)
            mgr->records[i] = mgr->records[i + 1];
        init_op_record(&mgr->records[mgr->max_size - 1]);
        mgr->current--;
    }

    int pos = mgr->count;
    mgr->records[pos] = *record;
    mgr->records[pos].old_paths = copy_string_array((const char **)record->old_paths, record->count);
    mgr->records[pos].new_paths = copy_string_array((const char **)record->new_paths, record->count);

    mgr->current = pos;
    mgr->count = pos + 1;

    return 0;
}

static void apply_record(UndoRedoManager *mgr, int record_index, int is_undo)
{
    (void)mgr;
    (void)record_index;
    (void)is_undo;
    // 此函数已废弃，撤销/重做逻辑在 undo() 和 redo() 中直接实现
}

int undo(UndoRedoManager *mgr, StringList *sys_paths, StringList *user_paths)
{
    if (!mgr || !can_undo(mgr))
        return -1;

    OpRecord *rec = &mgr->records[mgr->current];
    StringList *target = (rec->target == TARGET_SYSTEM) ? sys_paths : user_paths;

    switch (rec->type)
    {
    case OP_ADD:
        // 撤销添加：删除刚添加的路径
        if (rec->count > 0 && target->count > 0)
        {
            // 删除最后添加的那条
            free(target->items[target->count - 1]);
            target->count--;
        }
        break;

    case OP_DELETE:
        // 撤销删除：恢复被删除的路径
        for (int i = 0; i < rec->count; i++)
        {
            if (rec->old_paths[i])
                add_string_list(target, rec->old_paths[i]);
        }
        break;

    case OP_EDIT:
        // 撤销编辑：恢复到原值
        if (rec->old_paths[0])
            string_list_set(target, rec->index, rec->old_paths[0]);
        break;

    case OP_MOVE_UP:
    case OP_MOVE_DOWN:
        // 撤销移动：反向移动一次
        if (rec->type == OP_MOVE_UP)
            path_manager_move_down(target, rec->index - 1);
        else
            path_manager_move_up(target, rec->index + 1);
        break;

    case OP_CLEAN:
    case OP_IMPORT:
        // 撤销清理/导入：恢复到原列表
        clear_string_list(target);
        for (int i = 0; i < rec->count; i++)
        {
            if (rec->old_paths[i])
                add_string_list(target, rec->old_paths[i]);
        }
        break;

    case OP_CLEAR:
        // 撤销清空：恢复所有路径
        for (int i = 0; i < rec->count; i++)
        {
            if (rec->old_paths[i])
                add_string_list(target, rec->old_paths[i]);
        }
        break;

    default:
        break;
    }

    mgr->current--;
    return 0;
}

int redo(UndoRedoManager *mgr, StringList *sys_paths, StringList *user_paths)
{
    if (!mgr || !can_redo(mgr))
        return -1;

    mgr->current++;
    OpRecord *rec = &mgr->records[mgr->current];
    StringList *target = (rec->target == TARGET_SYSTEM) ? sys_paths : user_paths;

    switch (rec->type)
    {
    case OP_ADD:
        // 重做添加：重新添加路径
        for (int i = 0; i < rec->count; i++)
        {
            if (rec->new_paths[i])
                add_string_list(target, rec->new_paths[i]);
        }
        break;

    case OP_DELETE:
        // 重做删除：重新删除路径
        for (int i = 0; i < rec->count; i++)
        {
            // 找到并删除对应路径
            for (int j = 0; j < target->count; j++)
            {
                if (target->items[j] && rec->old_paths[i] &&
                    strcmp(target->items[j], rec->old_paths[i]) == 0)
                {
                    free(target->items[j]);
                    // 移动后面的元素
                    for (int k = j; k < target->count - 1; k++)
                        target->items[k] = target->items[k + 1];
                    target->count--;
                    break;
                }
            }
        }
        break;

    case OP_EDIT:
        // 重做编辑：应用新值
        if (rec->new_paths[0])
            string_list_set(target, rec->index, rec->new_paths[0]);
        break;

    case OP_MOVE_UP:
    case OP_MOVE_DOWN:
        // 重做移动：再次移动
        if (rec->type == OP_MOVE_UP)
            path_manager_move_up(target, rec->index);
        else
            path_manager_move_down(target, rec->index);
        break;

    case OP_CLEAN:
    case OP_IMPORT:
        // 重做清理/导入：应用新列表
        clear_string_list(target);
        for (int i = 0; i < rec->count; i++)
        {
            if (rec->new_paths[i])
                add_string_list(target, rec->new_paths[i]);
        }
        break;

    case OP_CLEAR:
        // 重做清空：清空列表
        clear_string_list(target);
        break;

    default:
        break;
    }

    return 0;
}

int can_undo(const UndoRedoManager *mgr)
{
    if (!mgr)
        return 0;
    return mgr->current >= 0;
}

int can_redo(const UndoRedoManager *mgr)
{
    if (!mgr)
        return 0;
    return mgr->current < mgr->count - 1;
}

void clear_undo_redo_history(UndoRedoManager *mgr)
{
    if (!mgr)
        return;

    for (int i = 0; i < mgr->count; i++)
        free_op_record(&mgr->records[i]);

    mgr->current = -1;
    mgr->count = 0;
}