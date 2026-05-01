#include "ui/ui_utils.h"
#include "utils/os_env.h"
#include "utils/string_ext.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

// 刷新列表样式（斑马纹 + 有效性检查）
void refresh_single_list_style(Ihandle *list)
{
    if (!list)
        return;
    int count = IupGetInt(list, "COUNT");

    for (int i = 1; i <= count; i++)
    {
        char *item = IupGetAttributeId(list, "", i);
        if (!item)
            continue;

        // 默认颜色：黑字
        char fg_color[32] = "0 0 0";

        // 1. 检查有效性
        if (!is_path_valid(item))
        {
            // 无效路径：红色
            sprintf(fg_color, "255 0 0");
        }

        // 2. 检查重复 (只检查当前项之前的项，如果之前出现过，当前项标橙)
        for (int j = 1; j < i; j++)
        {
            char *prev_item = IupGetAttributeId(list, "", j);
            if (prev_item && _stricmp(item, prev_item) == 0) // Windows 路径不区分大小写
            {
                // 重复路径：橙色
                sprintf(fg_color, "255 128 0");
                break;
            }
        }

        IupSetAttributeId(list, "ITEMFGCOLOR", i, fg_color);

        // 斑马纹背景
        if (i % 2 == 0)
        {
            IupSetAttributeId(list, "ITEMBGCOLOR", i, "245 245 245");
        }
        else
        {
            IupSetAttributeId(list, "ITEMBGCOLOR", i, "255 255 255");
        }
    }
}

// 同步字符串列表到 UI 列表
void sync_string_list_to_ui(Ihandle *list_ui, const StringList *str_list)
{
    if (!list_ui || !str_list) return;

    IupSetAttribute(list_ui, "REMOVEITEM", "ALL");
    
    for (int i = 0; i < str_list->count; i++)
    {
        const char *item = string_list_get(str_list, i);
        IupSetAttributeId(list_ui, "", i + 1, item);

        // 对含环境变量的路径设置悬停提示
        if (item && strchr(item, '%'))
        {
            char *expanded = expand_env_vars(item);
            if (expanded)
            {
                IupSetAttributeId(list_ui, "ITEMTIP", i + 1, expanded);
                free(expanded);
            }
        }
    }
    IupSetInt(list_ui, "COUNT", str_list->count);
    
    refresh_single_list_style(list_ui);
}