#include "ui_utils.h"
#include "globals.h"
#include "utils.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

// 获取第一个选中的索引（1-based），如果没有选中则返回 0
int get_first_selected_index(Ihandle *list)
{
    char *value = IupGetAttribute(list, "VALUE");
    if (!value)
        return 0;
    int len = strlen(value);
    for (int i = 0; i < len; i++)
    {
        if (value[i] == '+')
            return i + 1;
    }
    return 0;
}

// 设置单选（1-based）
void set_single_selection(Ihandle *list, int index)
{
    int count = IupGetInt(list, "COUNT");
    if (count <= 0)
        return;

    char *new_val = (char *)malloc(count + 1);
    if (!new_val)
        return;

    for (int i = 0; i < count; i++)
    {
        new_val[i] = '-';
    }
    new_val[count] = '\0';

    if (index >= 1 && index <= count)
    {
        new_val[index - 1] = '+';
    }

    IupSetAttribute(list, "VALUE", new_val);
    free(new_val);
}

// 从原始数据刷新UI
void refresh_ui_from_raw(Ihandle *list, StringList *raw)
{
    IupSetAttribute(list, "REMOVEITEM", "ALL");
    for (int i = 0; i < raw->count; i++)
    {
        IupSetAttributeId(list, "", i + 1, raw->items[i]);
    }
    IupSetInt(list, "COUNT", raw->count);
    refresh_single_list_style(list);
}

// 记录历史
void record_history()
{
    push_history(&undo_stack, &raw_sys_paths, &raw_user_paths);
    clear_history_stack(&redo_stack);
    // 更新按钮状态（可选）
    // IupSetAttribute(btn_undo, "ACTIVE", "YES");
    // IupSetAttribute(btn_redo, "ACTIVE", "NO");
}

// 静态辅助函数：对话框确定
static int on_dialog_ok(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    IupSetAttribute(dlg, "MY_STATUS", "1");
    return IUP_CLOSE;
}

// 静态辅助函数：对话框取消
static int on_dialog_cancel(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    IupSetAttribute(dlg, "MY_STATUS", "0");
    return IUP_CLOSE;
}

// 自定义输入对话框
int custom_input_dialog(const char *title, const char *label_text, char *buffer, int buffer_size)
{
    Ihandle *text = IupText(NULL);
    IupSetAttribute(text, "VALUE", buffer);
    IupSetAttribute(text, "EXPAND", "HORIZONTAL");
    IupSetAttribute(text, "RASTERSIZE", "500x");
    IupSetAttribute(text, "NAME", "INPUT_TEXT");

    Ihandle *btn_ok = IupButton("确定", NULL);
    IupSetCallback(btn_ok, "ACTION", on_dialog_ok);
    IupSetAttribute(btn_ok, "RASTERSIZE", "100x32");

    Ihandle *btn_cancel = IupButton("取消", NULL);
    IupSetCallback(btn_cancel, "ACTION", on_dialog_cancel);
    IupSetAttribute(btn_cancel, "RASTERSIZE", "100x32");

    Ihandle *vbox = IupVbox(
        IupLabel(label_text),
        text,
        IupHbox(IupFill(), btn_ok, btn_cancel, NULL),
        NULL);
    IupSetAttribute(vbox, "MARGIN", "15x15");
    IupSetAttribute(vbox, "GAP", "10");

    Ihandle *dlg = IupDialog(vbox);
    IupSetAttribute(dlg, "TITLE", title);
    IupSetAttribute(dlg, "MINBOX", "NO");
    IupSetAttribute(dlg, "MAXBOX", "NO");
    IupSetAttribute(dlg, "RESIZE", "NO");

    IupSetAttributeHandle(dlg, "DEFAULTENTER", btn_ok);
    IupSetAttributeHandle(dlg, "DEFAULTESC", btn_cancel);
    
    // 应用主题到对话框
    if (is_dark_mode) {
        IupSetAttribute(dlg, "BGCOLOR", "50 50 50");
        IupSetAttribute(text, "BGCOLOR", "30 30 30");
        IupSetAttribute(text, "FGCOLOR", "255 255 255");
        IupSetAttribute(IupGetChild(vbox, 0), "FGCOLOR", "255 255 255"); // Label
    }

    IupPopup(dlg, IUP_CENTER, IUP_CENTER);

    int result = IupGetInt(dlg, "MY_STATUS");
    if (result == 1)
    {
        char *val = IupGetAttribute(text, "VALUE");
        if (val)
        {
            strncpy(buffer, val, buffer_size);
            buffer[buffer_size - 1] = '\0';
        }
    }

    IupDestroy(dlg);
    return result;
}

// 获取当前选中的列表
Ihandle *get_current_list()
{
    // 获取当前选中的 Tab 索引
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    if (pos == 0)
        return list_sys;
    if (pos == 1)
        return list_user;
    if (pos == 2)
        return list_merged;
    return list_sys; // 默认
}

// 从 raw_data 中删除指定字符串
void remove_from_raw_data(StringList *list, const char *str)
{
    if (!list || !str)
        return;
    for (int i = 0; i < list->count; i++)
    {
        if (strcmp(list->items[i], str) == 0)
        {
            free(list->items[i]);
            // 移动后续元素
            for (int j = i; j < list->count - 1; j++)
            {
                list->items[j] = list->items[j + 1];
            }
            list->count--;
            break; // 假设没有重复，只删除第一个匹配
        }
    }
}

// 切换编辑按钮状态
void toggle_edit_buttons(int enable)
{
    char *val = enable ? "YES" : "NO";
    IupSetAttribute(btn_new, "ACTIVE", val);        // 新建按钮
    IupSetAttribute(btn_edit, "ACTIVE", val);       // 编辑按钮
    IupSetAttribute(btn_browse, "ACTIVE", val);     // 浏览按钮
    IupSetAttribute(btn_del, "ACTIVE", val);        // 删除按钮
    IupSetAttribute(btn_clean, "ACTIVE", val);      // 清除按钮
    IupSetAttribute(btn_up, "ACTIVE", val);         // 上移按钮
    IupSetAttribute(btn_down, "ACTIVE", val);       // 下移按钮
    IupSetAttribute(btn_import, "ACTIVE", val);     // 导入按钮
    IupSetAttribute(btn_export, "ACTIVE", "YES");   // 导出按钮始终可用
}

// 应用主题
void apply_theme()
{
    char *bg_color = is_dark_mode ? "50 50 50" : "240 240 240";
    char *fg_color = is_dark_mode ? "255 255 255" : "0 0 0";
    char *list_bg = is_dark_mode ? "60 60 60" : "255 255 255";
    char *text_bg = is_dark_mode ? "30 30 30" : "255 255 255";
    
    // 主对话框
    IupSetAttribute(dlg, "BGCOLOR", bg_color);
    
    // 列表
    IupSetAttribute(list_sys, "BACKCOLOR", list_bg);
    IupSetAttribute(list_sys, "FGCOLOR", fg_color);
    IupSetAttribute(list_user, "BACKCOLOR", list_bg);
    IupSetAttribute(list_user, "FGCOLOR", fg_color);
    IupSetAttribute(list_merged, "BACKCOLOR", list_bg);
    IupSetAttribute(list_merged, "FGCOLOR", fg_color);

    // 文本框
    IupSetAttribute(txt_search, "BGCOLOR", text_bg);
    IupSetAttribute(txt_search, "FGCOLOR", fg_color);
    
    // 标签
    IupSetAttribute(lbl_status, "FGCOLOR", fg_color);
    
    // 刷新列表样式
    refresh_list_style();
    refresh_single_list_style(list_merged);
}
