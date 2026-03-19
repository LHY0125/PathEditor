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
            int count = IupGetInt(current_list, "COUNT");
            count++;
            IupSetAttributeId(current_list, "", count, buffer);
            IupSetInt(current_list, "COUNT", count);
            
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

    char *current_val = IupGetAttributeId(current_list, "", selected);
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
            IupSetAttributeId(current_list, "", selected, buffer);
            
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

// 双击回调
int list_dblclick_cb(Ihandle *self, int item, char *text)
{
    // 这里的 self 就是触发双击的列表控件
    if (item > 0)
    {
        // 选中该行 (单选)
        set_single_selection(self, item);
        // 调用编辑逻辑
        btn_edit_cb(NULL);
    }
    return IUP_DEFAULT;
}

// 按钮回调：删除 (支持多选)
int btn_del_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    char *value = IupGetAttribute(current_list, "VALUE");
    if (!value)
        return IUP_DEFAULT;

    int len = strlen(value);
    int has_selection = 0;
    for (int i = 0; i < len; i++) {
        if (value[i] == '+') {
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
        if (value[i] == '+')
        {
            int item_index = i + 1; // IUP 索引从 1 开始
            char *val = IupGetAttributeId(current_list, "", item_index);
            
            // 从缓存删除
            if (val && raw_data)
            {
                char *val_copy = _strdup(val);
                remove_from_raw_data(raw_data, val_copy);
                free(val_copy);
            }

            // 从界面删除
            IupSetInt(current_list, "REMOVEITEM", item_index);
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
    char *value = IupGetAttribute(current_list, "VALUE");
    if (!value)
        return IUP_DEFAULT;

    int len = strlen(value);
    char *new_value = _strdup(value);
    int moved = 0;

    // 预检查是否有移动
    for (int i = 1; i < len; i++) {
        if (new_value[i] == '+' && new_value[i - 1] == '-') {
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
            if (new_value[i] == '+' && new_value[i - 1] == '-')
            {
                // 交换列表项内容
                char *curr_text = IupGetAttributeId(current_list, "", i + 1);
                char *prev_text = IupGetAttributeId(current_list, "", i);
                
                // 需要复制，防止指针失效
                char *curr_copy = curr_text ? _strdup(curr_text) : NULL;
                char *prev_copy = prev_text ? _strdup(prev_text) : NULL;
                
                IupSetAttributeId(current_list, "", i, curr_copy);
                IupSetAttributeId(current_list, "", i + 1, prev_copy);
                
                if (curr_copy) free(curr_copy);
                if (prev_copy) free(prev_copy);

                // 交换 raw_data
                if (raw_data && i < raw_data->count) {
                    char *temp = raw_data->items[i];
                    raw_data->items[i] = raw_data->items[i-1];
                    raw_data->items[i-1] = temp;
                }

                // 交换选中状态
                new_value[i] = '-';
                new_value[i - 1] = '+';
            }
        }

        // 更新选中状态
        IupSetAttribute(current_list, "VALUE", new_value);
        refresh_single_list_style(current_list);
    }
    free(new_value);
    return IUP_DEFAULT;
}

// 按钮回调：下移 (支持多选)
int btn_down_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    char *value = IupGetAttribute(current_list, "VALUE");
    if (!value)
        return IUP_DEFAULT;

    int len = strlen(value);
    char *new_value = _strdup(value);
    int moved = 0;

    // 预检查
    for (int i = len - 2; i >= 0; i--) {
        if (new_value[i] == '+' && new_value[i + 1] == '-') {
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
            if (new_value[i] == '+' && new_value[i + 1] == '-')
            {
                // 交换列表项内容
                char *curr_text = IupGetAttributeId(current_list, "", i + 1);
                char *next_text = IupGetAttributeId(current_list, "", i + 2);

                // 需要复制
                char *curr_copy = curr_text ? _strdup(curr_text) : NULL;
                char *next_copy = next_text ? _strdup(next_text) : NULL;

                IupSetAttributeId(current_list, "", i + 2, curr_copy);
                IupSetAttributeId(current_list, "", i + 1, next_copy);

                if (curr_copy) free(curr_copy);
                if (next_copy) free(next_copy);

                // 交换 raw_data
                if (raw_data && i + 1 < raw_data->count) {
                    char *temp = raw_data->items[i];
                    raw_data->items[i] = raw_data->items[i+1];
                    raw_data->items[i+1] = temp;
                }

                // 交换选中状态
                new_value[i] = '-';
                new_value[i + 1] = '+';
            }
        }

        // 更新选中状态
        IupSetAttribute(current_list, "VALUE", new_value);
        refresh_single_list_style(current_list);
    }
    free(new_value);
    return IUP_DEFAULT;
}

// 按钮回调：一键清理
int btn_clean_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    int count = IupGetInt(current_list, "COUNT");
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
        char *item = IupGetAttributeId(current_list, "", i);
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
                char *prev_item = IupGetAttributeId(current_list, "", j);
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
            
            IupSetAttributeId(current_list, "REMOVEITEM", i, NULL);
        }
    }

    refresh_single_list_style(current_list);
    IupMessage("提示", "清理完成！");
    return IUP_DEFAULT;
}