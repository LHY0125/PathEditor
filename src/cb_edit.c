#include "cb_edit.h"
#include "ui_utils.h"
#include "globals.h"
#include "utils.h"
#include <string.h>
#include <stdlib.h>

// 按钮回调：新建
int btn_new_cb(Ihandle *self)
{
    char buffer[1024] = "";
    if (custom_input_dialog("新建环境变量", "请输入路径:", buffer, sizeof(buffer)))
    {
        if (strlen(buffer) > 0)
        {
            // 记录历史
            record_history();

            Ihandle *current_list = get_current_list();
            int count = IupGetInt(current_list, "NUMLIN");
            count++;
            IupSetInt(current_list, "NUMLIN", count);
            IupSetAttributeId2(current_list, "", count, 1, buffer);
            
            // 更新选中状态
            set_single_selection(current_list, count);

            // 同时添加到 raw_data
            int pos = IupGetInt(tabs_main, "VALUEPOS");
            StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;
            if (raw_data) {
                add_string_list(raw_data, buffer);
            }

            refresh_single_list_style(current_list);
        }
    }
    return IUP_DEFAULT;
}

// 按钮回调：编辑
int btn_edit_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    // 获取第一个选中的项
    int selected = get_first_selected_index(current_list);
    if (selected == 0)
        return IUP_DEFAULT;

    char *current_val = IupGetAttributeId2(current_list, "", selected, 1);
    char buffer[4096]; // 假设单个路径不超过4096
    if (current_val)
    {
        strncpy(buffer, current_val, 4096);
        buffer[4095] = '\0';
    }
    else
    {
        buffer[0] = '\0';
    }

    if (custom_input_dialog("编辑环境变量", "编辑路径:", buffer, sizeof(buffer)))
    {
        if (strlen(buffer) > 0)
        {
            // 记录历史
            record_history();

            // 更新 UI
            IupSetAttributeId2(current_list, "", selected, 1, buffer);
            
            // 更新 raw_data
            int pos = IupGetInt(tabs_main, "VALUEPOS");
            StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;
            
            char *filter = IupGetAttribute(txt_search, "VALUE");
            if (!filter || strlen(filter) == 0) {
                 if (raw_data && selected <= raw_data->count) {
                     free(raw_data->items[selected-1]);
                     raw_data->items[selected-1] = _strdup(buffer);
                 }
            } else {
                // 搜索状态下，忽略同步问题，或者编辑后清除搜索。
            }

            refresh_single_list_style(current_list);
        }
    }
    return IUP_DEFAULT;
}

// 单击/双击回调 (用于 IupMatrix 的 CLICK_CB)
int list_dblclick_cb(Ihandle *self, int lin, int col, char *status)
{
    if (lin > 0)
    {
        // 如果是双击
        if (strchr(status, 'D') || strchr(status, 'd'))
        {
            // 选中该行 (单选)
            set_single_selection(self, lin);
            
            // 如果双击的不是第一列(路径)，则调用弹窗编辑
            if (col != 1) {
                btn_edit_cb(NULL);
            }
            // 如果是第一列，默认进入内联编辑，无需干预
        }
        // 如果是右键点击 (status包含 '3')
        else if (strchr(status, '3'))
        {
            // 选中该行
            set_single_selection(self, lin);
            
            // 创建右键菜单
            Ihandle *item_new = IupItem("新建", NULL);
            IupSetAttribute(item_new, "IMAGE", "IUP_FileNew");
            IupSetCallback(item_new, "ACTION", (Icallback)btn_new_cb);

            Ihandle *item_edit = IupItem("编辑", NULL);
            IupSetAttribute(item_edit, "IMAGE", "IUP_EditCopy");
            IupSetCallback(item_edit, "ACTION", (Icallback)btn_edit_cb);

            Ihandle *item_del = IupItem("删除", NULL);
            IupSetAttribute(item_del, "IMAGE", "IUP_EditErase");
            IupSetCallback(item_del, "ACTION", (Icallback)btn_del_cb);

            Ihandle *item_up = IupItem("上移", NULL);
            IupSetAttribute(item_up, "IMAGE", "IUP_ArrowUp");
            IupSetCallback(item_up, "ACTION", (Icallback)btn_up_cb);

            Ihandle *item_down = IupItem("下移", NULL);
            IupSetAttribute(item_down, "IMAGE", "IUP_ArrowDown");
            IupSetCallback(item_down, "ACTION", (Icallback)btn_down_cb);

            Ihandle *menu = IupMenu(
                item_new,
                item_edit,
                item_del,
                IupSeparator(),
                item_up,
                item_down,
                NULL
            );

            IupPopup(menu, IUP_MOUSEPOS, IUP_MOUSEPOS);
            IupDestroy(menu);
        }
    }
    return IUP_DEFAULT;
}

// 内联编辑回调
int matrix_edition_cb(Ihandle *self, int lin, int col, int mode, int update)
{
    if (mode == 1)
    {
        // 进入编辑模式前，记录历史
        record_history();
    }
    else if (mode == 0 && update == 1)
    {
        // 结束编辑并更新了值
        // 同步到 raw_data
        int pos = IupGetInt(tabs_main, "VALUEPOS");
        StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;
        
        char *filter = IupGetAttribute(txt_search, "VALUE");
        if (!filter || strlen(filter) == 0) {
            if (raw_data && lin > 0 && lin <= raw_data->count) {
                char *new_val = IupGetAttributeId2(self, "", lin, 1);
                if (new_val) {
                    free(raw_data->items[lin-1]);
                    raw_data->items[lin-1] = _strdup(new_val);
                }
            }
        }

        refresh_single_list_style(self);
    }
    return IUP_DEFAULT;
}


// 按钮回调：删除 (支持多选)
int btn_del_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    char *value = IupGetAttribute(current_list, "MARKED");
    if (!value)
        return IUP_DEFAULT;

    int len = strlen(value);
    int has_selection = 0;
    for (int i = 0; i < len; i++) {
        if (value[i] == '1') {
            has_selection = 1;
            break;
        }
    }

    if (!has_selection)
    {
        IupMessage("提示", "请先选择要删除的项");
        return IUP_DEFAULT;
    }

    // 记录历史
    record_history();

    // 获取 raw_data 缓存
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;

    // 从后往前遍历删除，避免索引错位
    for (int i = len - 1; i >= 0; i--)
    {
        if (value[i] == '1')
        {
            int item_index = i + 1; // IUP 索引从 1 开始
            char *val = IupGetAttributeId2(current_list, "", item_index, 1);
            
            // 从缓存删除
            if (val && raw_data)
            {
                char *val_copy = _strdup(val);
                remove_from_raw_data(raw_data, val_copy);
                free(val_copy);
            }

            // 从界面删除
            IupSetInt(current_list, "DELLIN", item_index);
        }
    }

    // 重新刷新
    refresh_single_list_style(current_list);

    // 更新状态栏
    IupSetAttribute(lbl_status, "TITLE", "状态: 已删除选中项");

    return IUP_DEFAULT;
}

// 按钮回调：上移 (支持多选)
int btn_up_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    char *value = IupGetAttribute(current_list, "MARKED");
    if (!value)
        return IUP_DEFAULT;

    int len = strlen(value);
    char *new_value = _strdup(value);
    int moved = 0;

    // 预检查是否有移动
    for (int i = 1; i < len; i++) {
        if (new_value[i] == '1' && new_value[i - 1] == '0') {
            moved = 1;
            break;
        }
    }

    if (moved) {
        // 记录历史
        record_history();
        
        // 同步 raw_data (假设非搜索状态，raw_data 与 UI 一致)
        int pos = IupGetInt(tabs_main, "VALUEPOS");
        StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;

        // 从前往后遍历，如果当前项被选中且前一项未选中，则交换
        for (int i = 1; i < len; i++)
        {
            if (new_value[i] == '1' && new_value[i - 1] == '0')
            {
                // 交换列表项内容
                char *curr_text = IupGetAttributeId2(current_list, "", i + 1, 1);
                char *prev_text = IupGetAttributeId2(current_list, "", i, 1);
                
                // 需要复制，防止指针失效
                char *curr_copy = curr_text ? _strdup(curr_text) : NULL;
                char *prev_copy = prev_text ? _strdup(prev_text) : NULL;
                
                IupSetAttributeId2(current_list, "", i, 1, curr_copy);
                IupSetAttributeId2(current_list, "", i + 1, 1, prev_copy);
                
                if (curr_copy) free(curr_copy);
                if (prev_copy) free(prev_copy);

                // 交换 raw_data
                if (raw_data && i < raw_data->count) {
                    char *temp = raw_data->items[i];
                    raw_data->items[i] = raw_data->items[i-1];
                    raw_data->items[i-1] = temp;
                }

                // 交换选中状态
                new_value[i] = '0';
                new_value[i - 1] = '1';
            }
        }

        // 更新选中状态
        IupSetAttribute(current_list, "MARKED", new_value);
        refresh_single_list_style(current_list);
    }
    free(new_value);
    return IUP_DEFAULT;
}

// 按钮回调：下移 (支持多选)
int btn_down_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    char *value = IupGetAttribute(current_list, "MARKED");
    if (!value)
        return IUP_DEFAULT;

    int len = strlen(value);
    char *new_value = _strdup(value);
    int moved = 0;

    // 预检查
    for (int i = len - 2; i >= 0; i--) {
        if (new_value[i] == '1' && new_value[i + 1] == '0') {
            moved = 1;
            break;
        }
    }

    if (moved) {
        // 记录历史
        record_history();

        // 同步 raw_data
        int pos = IupGetInt(tabs_main, "VALUEPOS");
        StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;

        // 从后往前遍历，如果当前项被选中且后一项未选中，则交换
        for (int i = len - 2; i >= 0; i--)
        {
            if (new_value[i] == '1' && new_value[i + 1] == '0')
            {
                // 交换列表项内容
                char *curr_text = IupGetAttributeId2(current_list, "", i + 1, 1);
                char *next_text = IupGetAttributeId2(current_list, "", i + 2, 1);

                // 需要复制
                char *curr_copy = curr_text ? _strdup(curr_text) : NULL;
                char *next_copy = next_text ? _strdup(next_text) : NULL;

                IupSetAttributeId2(current_list, "", i + 2, 1, curr_copy);
                IupSetAttributeId2(current_list, "", i + 1, 1, next_copy);

                if (curr_copy) free(curr_copy);
                if (next_copy) free(next_copy);

                // 交换 raw_data
                if (raw_data && i + 1 < raw_data->count) {
                    char *temp = raw_data->items[i];
                    raw_data->items[i] = raw_data->items[i+1];
                    raw_data->items[i+1] = temp;
                }

                // 交换选中状态
                new_value[i] = '0';
                new_value[i + 1] = '1';
            }
        }

        // 更新选中状态
        IupSetAttribute(current_list, "MARKED", new_value);
        refresh_single_list_style(current_list);
    }
    free(new_value);
    return IUP_DEFAULT;
}

// 按钮回调：一键清理
int btn_clean_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    int count = IupGetInt(current_list, "NUMLIN");
    if (count == 0)
        return IUP_DEFAULT;

    // 弹出确认对话框
    if (IupAlarm("确认清理", "此操作将移除当前列表中所有【无效路径】和【重复路径】。\n确定要继续吗？", "确定", "取消", NULL) != 1)
    {
        return IUP_DEFAULT;
    }

    // 记录历史 (放在循环外，一次操作)
    record_history();
    
    // 获取 raw_data 用于同步删除
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;

    // 从后往前遍历删除，避免索引错位
    for (int i = count; i >= 1; i--)
    {
        char *item = IupGetAttributeId2(current_list, "", i, 1);
        if (!item)
            continue;

        int should_remove = 0;

        // 1. 检查有效性
        if (!is_path_valid(item))
        {
            should_remove = 1;
        }
        else
        {
            // 2. 检查重复 (检查当前项之前是否出现过)
            // 注意：这里需要再次遍历，性能稍低但最稳妥
            for (int j = 1; j < i; j++)
            {
                char *prev_item = IupGetAttributeId2(current_list, "", j, 1);
                if (prev_item && _stricmp(item, prev_item) == 0)
                {
                    should_remove = 1;
                    break;
                }
            }
        }

        if (should_remove)
        {
            // 从 raw_data 删除
            if (raw_data) {
                char *val_copy = _strdup(item);
                remove_from_raw_data(raw_data, val_copy);
                free(val_copy);
            }
            
            IupSetInt(current_list, "DELLIN", i);
        }
    }

    refresh_single_list_style(current_list);
    IupMessage("提示", "清理完成！");
    return IUP_DEFAULT;
}