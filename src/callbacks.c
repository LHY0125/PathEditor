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
                IupButton("确定", "1"), // "1" will be returned by IupPopup
                IupButton("取消", "0"), // "0" will be returned by IupPopup
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

// 按钮回调：新建
int btn_new_cb(Ihandle *self)
{
    char buffer[1024] = "";
    if (custom_input_dialog("新建环境变量", "请输入路径:", buffer, sizeof(buffer)))
    {
        if (strlen(buffer) > 0)
        {
            int count = IupGetInt(list_path, "COUNT");
            count++;
            IupSetAttributeId(list_path, "", count, buffer);
            IupSetInt(list_path, "COUNT", count);
            IupSetInt(list_path, "VALUE", count);

            refresh_list_style();
        }
    }
    return IUP_DEFAULT;
}

// 按钮回调：编辑
int btn_edit_cb(Ihandle *self)
{
    int selected = IupGetInt(list_path, "VALUE");
    if (selected == 0)
        return IUP_DEFAULT;

    char *current_val = IupGetAttributeId(list_path, "", selected);
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
            IupSetAttributeId(list_path, "", selected, buffer);
            refresh_list_style();
        }
    }
    return IUP_DEFAULT;
}

// 双击回调
int list_dblclick_cb(Ihandle *self, int item, char *text)
{
    // 这里的 item 是点击的行号
    if (item > 0)
    {
        // 选中该行
        IupSetInt(list_path, "VALUE", item);
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
            int count = IupGetInt(list_path, "COUNT");
            count++;
            IupSetAttributeId(list_path, "", count, value);
            IupSetInt(list_path, "COUNT", count);
            IupSetInt(list_path, "VALUE", count);

            refresh_list_style();
        }
    }
    IupDestroy(filedlg);
    return IUP_DEFAULT;
}

// 按钮回调：删除
int btn_del_cb(Ihandle *self)
{
    int selected = IupGetInt(list_path, "VALUE");
    if (selected == 0)
        return IUP_DEFAULT;

    IupSetAttribute(list_path, "REMOVEITEM", "SELECTED");

    // 重新刷新，因为删除了中间项，后面的奇偶性变了
    refresh_list_style();
    return IUP_DEFAULT;
}

// 按钮回调：上移
int btn_up_cb(Ihandle *self)
{
    int selected = IupGetInt(list_path, "VALUE");
    if (selected <= 1)
        return IUP_DEFAULT; // 已经是第一个或未选中

    char *current = IupGetAttributeId(list_path, "", selected);
    char *prev = IupGetAttributeId(list_path, "", selected - 1);

    // 交换内容
    char buf_curr[4096], buf_prev[4096];
    strncpy(buf_curr, current, 4096);
    buf_curr[4095] = '\0';
    strncpy(buf_prev, prev, 4096);
    buf_prev[4095] = '\0';

    IupSetAttributeId(list_path, "", selected, buf_prev);
    IupSetAttributeId(list_path, "", selected - 1, buf_curr);

    IupSetInt(list_path, "VALUE", selected - 1);

    // 刷新样式（虽然颜色不需要变，但为了保险）
    refresh_list_style();
    return IUP_DEFAULT;
}

// 按钮回调：下移
int btn_down_cb(Ihandle *self)
{
    int selected = IupGetInt(list_path, "VALUE");
    int count = IupGetInt(list_path, "COUNT");
    if (selected == 0 || selected >= count)
        return IUP_DEFAULT;

    char *current = IupGetAttributeId(list_path, "", selected);
    char *next = IupGetAttributeId(list_path, "", selected + 1);

    char buf_curr[4096], buf_next[4096];
    strncpy(buf_curr, current, 4096);
    buf_curr[4095] = '\0';
    strncpy(buf_next, next, 4096);
    buf_next[4095] = '\0';

    IupSetAttributeId(list_path, "", selected, buf_next);
    IupSetAttributeId(list_path, "", selected + 1, buf_curr);

    IupSetInt(list_path, "VALUE", selected + 1);

    refresh_list_style();
    return IUP_DEFAULT;
}

// 按钮回调：确定
int btn_ok_cb(Ihandle *self)
{
    save_path();
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