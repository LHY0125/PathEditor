#include "core/path_manager.h"
#include "utils/os_env.h"
#include "utils/error_code.h"
#include "utils/logger.h"
#include <stdlib.h>
#include <string.h>

// 删除指定索引的路径项
ErrorCode path_manager_remove_at(StringList *list, int index)
{
    if (!list || index < 0 || index >= list->count)
        return ERR_NULL_PTR;
    
    free(list->items[index]);
    for (int i = index; i < list->count - 1; i++)
    {
        list->items[i] = list->items[i + 1];
    }
    list->items[list->count - 1] = NULL;
    list->count--;
    return ERR_OK;
}

// 向上移动路径项
ErrorCode path_manager_move_up(StringList *list, int index)
{
    if (!list || index <= 0 || index >= list->count)
        return ERR_NULL_PTR;
    
    char *temp = list->items[index];
    list->items[index] = list->items[index - 1];
    list->items[index - 1] = temp;
    return ERR_OK;
}

// 向下移动路径项
ErrorCode path_manager_move_down(StringList *list, int index)
{
    if (!list || index < 0 || index >= list->count - 1)
        return ERR_NULL_PTR;
    
    char *temp = list->items[index];
    list->items[index] = list->items[index + 1];
    list->items[index + 1] = temp;
    return ERR_OK;
}

// 清理无效路径项
ErrorCode path_manager_clean(StringList *list)
{
    if (!list) return ERR_NULL_PTR;
    
    int removed_count = 0;

    for (int i = list->count - 1; i >= 0; i--)
    {
        char *item = list->items[i];
        if (!item) continue;

        int should_remove = 0;

        if (!is_path_valid(item))
        {
            should_remove = 1;
        }
        else
        {
            for (int j = 0; j < i; j++)
            {
                char *prev_item = list->items[j];
                if (prev_item && _stricmp(item, prev_item) == 0)
                {
                    should_remove = 1;
                    break;
                }
            }
        }

        if (should_remove)
        {
            path_manager_remove_at(list, i);
            removed_count++;
        }
    }
    
    log_info("Cleaned paths: removed %d invalid/duplicate paths, remaining %d", 
            removed_count, list->count);
    return ERR_OK;
}