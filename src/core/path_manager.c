#include "core/path_manager.h"
#include "utils/os_env.h"
#include <stdlib.h>
#include <string.h>

// 删除指定索引的路径项
void path_manager_remove_at(StringList *list, int index)
{
    if (!list || index < 0 || index >= list->count)
        return;
    
    free(list->items[index]);
    for (int i = index; i < list->count - 1; i++)
    {
        list->items[i] = list->items[i + 1];
    }
    list->count--;
}

// 向上移动路径项
void path_manager_move_up(StringList *list, int index)
{
    if (!list || index <= 0 || index >= list->count)
        return;
    
    char *temp = list->items[index];
    list->items[index] = list->items[index - 1];
    list->items[index - 1] = temp;
}

// 向下移动路径项
void path_manager_move_down(StringList *list, int index)
{
    if (!list || index < 0 || index >= list->count - 1)
        return;
    
    char *temp = list->items[index];
    list->items[index] = list->items[index + 1];
    list->items[index + 1] = temp;
}

// 清理无效路径项
int path_manager_clean(StringList *list)
{
    if (!list) return 0;
    int removed_count = 0;

    // 从后往前遍历，方便删除
    for (int i = list->count - 1; i >= 0; i--)
    {
        char *item = list->items[i];
        if (!item) continue;

        int should_remove = 0;

        // 1. 检查有效性
        if (!is_path_valid(item))
        {
            should_remove = 1;
        }
        else
        {
            // 2. 检查重复 (检查当前项之前是否出现过)
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
    return removed_count;
}