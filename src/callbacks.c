#include "callbacks.h"
#include "globals.h"
#include "registry.h"
#include "utils.h"
#include <string.h>
#include <stdio.h>

// 简单的自定义输入对话框，支持更宽的输入框
// 返回 1 表示确定，0 表示取消
int show_custom_input_dialog(const char *title, const char *label_text, char *buffer, int buffer_size)
{
    Ihandle *text = IupText(NULL);
    IupSetAttribute(text, "VALUE", buffer);
    IupSetAttribute(text, "EXPAND", "HORIZONTAL");
    IupSetAttribute(text, "RASTERSIZE", "600x");   // 设置宽度为600像素
    IupSetAttribute(text, "VISIBLECOLUMNS", "80"); // 设置可见列数

    // 如果是编辑模式，全选文本
    if (strlen(buffer) > 0)
    {
        IupSetAttribute(text, "SELECTION", "ALL");
    }

    Ihandle *dlg = IupDialog(
        IupVbox(
            IupLabel(label_text),
            text,
            IupHbox(
                IupFill(),
                IupButton("确定", "1"), // "1" 会被返回
                IupButton("取消", "0"), // "0" 会被返回
                NULL),
            NULL));

    // 布局设置
    IupSetAttribute(dlg, "TITLE", title);
    IupSetAttribute(dlg, "MINBOX", "NO");
    IupSetAttribute(dlg, "MAXBOX", "NO");
    IupSetAttribute(dlg, "RESIZE", "NO");
    IupSetAttribute(dlg, "MARGIN", "10x10");
    IupSetAttribute(dlg, "GAP", "10");

    // 按钮响应
    // 这是一个简单的 hack：IupPopup 的参数 x, y 如果是 IUP_CENTER 等，它是一个模态循环。
    // 但是标准的 IupPopup 不返回按钮值。
    // 我们使用 IupAlarm 类似的逻辑，或者使用 IUP 提供的 IupGetParam。
    // 为了最简单实现，我们使用 IUP 的 IupGetParam，但是它很难调整宽度。
    // 所以还是手动构建对话框。

    // 为了获取返回值，我们需要设置按钮回调。
    // 但为了避免定义额外的全局函数，我们可以使用 IupPopup 阻塞特性。
    // 我们需要定义两个简单的回调函数。
    // 为了简化，这里定义两个静态辅助函数。

    // 由于 C 语言闭包限制，我们需要用全局或静态变量传递状态，或者使用 Dialog 的 Attribute。
    IupSetAttribute(dlg, "MY_STATUS", "0");

    // 注册回调 (使用 IupSetCallback 注册 lambda 类似的逻辑比较难，这里用名字)
    // 必须定义全局函数。为了避免污染，我们在文件顶部定义静态函数。
    // 见下文的 static int on_dialog_ok...

    // 由于不能在函数内部定义函数，我们需要调整代码结构。
    // 见下文重构。

    return 0; // 占位
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

// 真正的实现函数
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

// 辅助函数：获取当前选中的列表
Ihandle *get_current_list()
{
    // 获取当前选中的 Tab 索引
    // 注意：IupTabs 的 VALUE 属性在某些版本可能返回 handle，某些版本返回 pos
    // 这里使用 IupGetInt(tabs_main, "VALUEPOS") 更稳妥
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    if (pos == 0)
        return list_sys;
    if (pos == 1)
        return list_user;
    return list_sys; // 默认
}

// 按钮回调：新建
int btn_new_cb(Ihandle *self)
{
    char buffer[1024] = "";
    if (custom_input_dialog("新建环境变量", "请输入路径:", buffer, sizeof(buffer)))
    {
        if (strlen(buffer) > 0)
        {
            Ihandle *current_list = get_current_list();
            int count = IupGetInt(current_list, "COUNT");
            count++;
            IupSetAttributeId(current_list, "", count, buffer);
            IupSetInt(current_list, "COUNT", count);
            IupSetInt(current_list, "VALUE", count);

            refresh_single_list_style(current_list);
        }
    }
    return IUP_DEFAULT;
}

// 按钮回调：编辑
int btn_edit_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    int selected = IupGetInt(current_list, "VALUE");
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
            IupSetAttributeId(current_list, "", selected, buffer);
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
        // 选中该行
        IupSetInt(self, "VALUE", item);
        // 调用编辑逻辑
        btn_edit_cb(NULL);
    }
    return IUP_DEFAULT;
}

// 按钮回调：浏览
int btn_browse_cb(Ihandle *self)
{
    Ihandle *filedlg = IupFileDlg();
    IupSetAttribute(filedlg, "DIALOGTYPE", "DIR");
    IupSetAttribute(filedlg, "TITLE", "选择目录");

    IupPopup(filedlg, IUP_CENTER, IUP_CENTER);

    if (IupGetInt(filedlg, "STATUS") != -1)
    {
        char *value = IupGetAttribute(filedlg, "VALUE");
        if (value)
        {
            Ihandle *current_list = get_current_list();
            int count = IupGetInt(current_list, "COUNT");
            count++;
            IupSetAttributeId(current_list, "", count, value);
            IupSetInt(current_list, "COUNT", count);
            IupSetInt(current_list, "VALUE", count);

            refresh_single_list_style(current_list);
        }
    }
    IupDestroy(filedlg);
    return IUP_DEFAULT;
}

// 辅助函数：从 raw_data 中删除指定字符串
static void remove_from_raw_data(StringList *list, const char *str)
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

// 按钮回调：删除
int btn_del_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    int selected = IupGetInt(current_list, "VALUE");

    if (selected == 0)
    {
        IupMessage("提示", "请先选择要删除的项");
        return IUP_DEFAULT;
    }

    // 获取当前要删除的内容
    char *val = IupGetAttributeId(current_list, "", selected);

    // 确认删除
    // char msg[1024];
    // snprintf(msg, sizeof(msg), "确定要删除以下路径吗？\n\n%s", val ? val : "(空)");
    // if (IupAlarm("确认删除", msg, "是", "否", NULL) != 1)
    //     return IUP_DEFAULT;

    // 从 raw_data 缓存中同步删除
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;

    // 注意：必须先保存 val 的副本，因为 REMOVEITEM 可能会导致 val 指针失效（如果它是指向列表内部缓冲区的）
    char *val_copy = val ? _strdup(val) : NULL;

    // 先从界面删除
    // IupSetAttribute(current_list, "REMOVEITEM", "SELECTED");
    // 改为按索引删除，防止失去焦点导致 SELECTED 失效
    IupSetInt(current_list, "REMOVEITEM", selected);

    // 再从缓存删除
    if (val_copy && raw_data)
    {
        remove_from_raw_data(raw_data, val_copy);
        free(val_copy);
    }

    // 重新刷新
    refresh_single_list_style(current_list);

    // 更新状态栏，告知用户删除了什么
    IupSetAttribute(lbl_status, "TITLE", "状态: 已删除选中项");

    return IUP_DEFAULT;
}

// 按钮回调：上移
int btn_up_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    int selected = IupGetInt(current_list, "VALUE");
    if (selected <= 1)
        return IUP_DEFAULT; // 已经是第一个或未选中

    char *current = IupGetAttributeId(current_list, "", selected);
    char *prev = IupGetAttributeId(current_list, "", selected - 1);

    // 交换内容
    char buf_curr[4096], buf_prev[4096];
    strncpy(buf_curr, current, 4096);
    buf_curr[4095] = '\0';
    strncpy(buf_prev, prev, 4096);
    buf_prev[4095] = '\0';

    IupSetAttributeId(current_list, "", selected, buf_prev);
    IupSetAttributeId(current_list, "", selected - 1, buf_curr);

    IupSetInt(current_list, "VALUE", selected - 1);

    // 刷新样式（虽然颜色不需要变，但为了保险）
    refresh_single_list_style(current_list);
    return IUP_DEFAULT;
}

// 按钮回调：下移
int btn_down_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    int selected = IupGetInt(current_list, "VALUE");
    int count = IupGetInt(current_list, "COUNT");
    if (selected == 0 || selected >= count)
        return IUP_DEFAULT;

    char *current = IupGetAttributeId(current_list, "", selected);
    char *next = IupGetAttributeId(current_list, "", selected + 1);

    char buf_curr[4096], buf_next[4096];
    strncpy(buf_curr, current, 4096);
    buf_curr[4095] = '\0';
    strncpy(buf_next, next, 4096);
    buf_next[4095] = '\0';

    IupSetAttributeId(current_list, "", selected, buf_next);
    IupSetAttributeId(current_list, "", selected + 1, buf_curr);

    IupSetInt(current_list, "VALUE", selected + 1);

    refresh_single_list_style(current_list);
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
            IupSetAttributeId(current_list, "REMOVEITEM", i, NULL);
        }
    }

    refresh_single_list_style(current_list);
    IupMessage("提示", "清理完成！");
    return IUP_DEFAULT;
}

// 搜索回调
int txt_search_cb(Ihandle *self)
{
    char *filter = IupGetAttribute(self, "VALUE");
    if (!filter)
        return IUP_DEFAULT;

    // 获取当前选中的 Tab 索引
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    Ihandle *current_list = (pos == 0) ? list_sys : list_user;
    StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;

    // 清空列表
    IupSetAttribute(current_list, "REMOVEITEM", "ALL");

    // 重新填充
    int count = 0;
    for (int i = 0; i < raw_data->count; i++)
    {
        // 如果 filter 为空，或包含 filter (不区分大小写)
        if (strlen(filter) == 0 || stristr(raw_data->items[i], filter) != NULL)
        {
            count++;
            IupSetAttributeId(current_list, "", count, raw_data->items[i]);
        }
    }

    IupSetInt(current_list, "COUNT", count);
    refresh_single_list_style(current_list);

    return IUP_DEFAULT;
}

// 拖拽回调
int list_dropfiles_cb(Ihandle *self, const char *filename, int num, int x, int y)
{
    // 获取当前列表和原始数据
    // 注意：拖拽的目标列表可能是 list_sys 或 list_user，由 self 参数决定
    // 但为了确保数据一致性，我们还是重新获取一下
    Ihandle *current_list = self;
    StringList *raw_data = NULL;
    if (self == list_sys)
        raw_data = &raw_sys_paths;
    else if (self == list_user)
        raw_data = &raw_user_paths;
    else
        return IUP_DEFAULT; // 异常情况

    // 检查拖入的是否为目录
    DWORD attr = GetFileAttributesA(filename);
    if (attr != INVALID_FILE_ATTRIBUTES && (attr & FILE_ATTRIBUTE_DIRECTORY))
    {
        // 如果正在搜索，先清空搜索框
        IupSetAttribute(txt_search, "VALUE", "");

        // 添加到列表末尾
        int count = IupGetInt(current_list, "COUNT");
        count++;
        IupSetAttributeId(current_list, "", count, filename);
        IupSetInt(current_list, "COUNT", count);
        IupSetInt(current_list, "VALUE", count); // 选中新添加的项

        // 同时添加到原始数据缓存，确保搜索时能搜到
        if (raw_data)
        {
            add_string_list(raw_data, filename);
        }

        refresh_single_list_style(current_list);
    }
    else
    {
        // 如果拖入的不是文件夹，可以在状态栏提示
        IupSetAttribute(lbl_status, "TITLE", "提示: 只能拖拽文件夹添加到 PATH");
    }

    return IUP_DEFAULT;
}

// 键盘按键回调
int list_k_any_cb(Ihandle *self, int c)
{
    // 处理 Delete 键
    if (c == K_DEL)
    {
        btn_del_cb(NULL);
        return IUP_IGNORE; // 阻止默认处理
    }
    return IUP_DEFAULT;
}

// 按钮回调：确定
int btn_ok_cb(Ihandle *self)
{
    save_all_paths();
    return IUP_DEFAULT;
}

// 按钮回调：取消
int btn_cancel_cb(Ihandle *self)
{
    IupExitLoop();
    return IUP_DEFAULT;
}

// 按钮回调：帮助
int btn_help_cb(Ihandle *self)
{
    IupMessage("使用说明",
               "1. 本程序用于编辑系统环境变量 PATH。\n"
               "2. 必须以【管理员身份】运行才能保存更改。\n"
               "3. 操作说明：\n"
               "   - 新建：添加新路径到列表末尾。\n"
               "   - 编辑：修改选中的路径。\n"
               "   - 浏览：从文件系统选择目录添加。\n"
               "   - 删除：移除选中的路径。\n"
               "   - 上移/下移：调整路径优先级。\n"
               "4. 点击【确定】保存更改并生效。\n"
               "5. 注意：某些正在运行的程序可能需要重启才能识别新的环境变量。\n\n"
               "--------------------------------------------------\n"
               "作者：LHY\n"
               "邮箱：3364451258@qq.com\n"
               "GitHub：https://github.com/LHY0125/PathEditor\n"
               "记得给我的项目点个star！");
    return IUP_DEFAULT;
}