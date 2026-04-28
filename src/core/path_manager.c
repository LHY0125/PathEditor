#include "core/path_manager.h"
#include "utils/os_env.h"
#include "utils/error_code.h"
#include "utils/logger.h"
#include <stdlib.h>
#include <string.h>

// 删除指定索引的路径项
ErrorCode path_manager_remove_at(StringList *list, int index)
{
    if (!list)
        return ERR_NULL_PTR;
    if (index < 0 || index >= list->count)
        return ERR_INVALID_INDEX;

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
    if (!list)
        return ERR_NULL_PTR;
    if (index <= 0 || index >= list->count)
        return ERR_INVALID_INDEX;

    char *temp = list->items[index];
    list->items[index] = list->items[index - 1];
    list->items[index - 1] = temp;
    return ERR_OK;
}

// 向下移动路径项
ErrorCode path_manager_move_down(StringList *list, int index)
{
    if (!list)
        return ERR_NULL_PTR;
    if (index < 0 || index >= list->count - 1)
        return ERR_INVALID_INDEX;

    char *temp = list->items[index];
    list->items[index] = list->items[index + 1];
    list->items[index + 1] = temp;
    return ERR_OK;
}

// 清理无效路径项
// 算法：先标记需要删除的项，然后从后向前批量删除，减少内存移动
ErrorCode path_manager_clean(StringList *list)
{
    if (!list) return ERR_NULL_PTR;
    if (list->count == 0) return ERR_OK;

    // 分配标记数组
    char *marks = (char *)calloc(list->count, sizeof(char));
    if (!marks) return ERR_OUT_OF_MEMORY;

    int removed_count = 0;

    // 第一遍：标记无效路径和重复路径
    for (int i = list->count - 1; i >= 0; i--)
    {
        char *item = list->items[i];
        if (!item)
        {
            marks[i] = 1;
            removed_count++;
            continue;
        }

        // 检查路径有效性
        if (!is_path_valid(item))
        {
            marks[i] = 1;
            removed_count++;
            continue;
        }

        // 检查是否与前面的项重复（只检查未被标记的项）
        for (int j = 0; j < i; j++)
        {
            if (!marks[j] && list->items[j] && _stricmp(item, list->items[j]) == 0)
            {
                marks[i] = 1;
                removed_count++;
                break;
            }
        }
    }

    // 第二遍：从后向前删除标记的项，避免多次内存移动
    for (int i = list->count - 1; i >= 0; i--)
    {
        if (marks[i])
        {
            free(list->items[i]);
            // 移动后续元素
            for (int j = i; j < list->count - 1; j++)
            {
                list->items[j] = list->items[j + 1];
            }
            list->items[list->count - 1] = NULL;
            list->count--;
        }
    }

    free(marks);

    log_info("Cleaned paths: removed %d invalid/duplicate paths, remaining %d",
            removed_count, list->count);
    return ERR_OK;
}