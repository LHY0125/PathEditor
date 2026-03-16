#include "callbacks.h"
#include "globals.h"
#include "registry.h"
#include <string.h>
#include <stdio.h>

// 按钮回调：新建
int btn_new_cb(Ihandle *self)
{
    char buffer[1024] = "";
    if (IupGetParam("新建环境变量", NULL, NULL, "路径: %s\n", buffer, NULL))
    {
        if (strlen(buffer) > 0)
        {
            int count = IupGetInt(list_path, "COUNT");
            count++;
            IupSetAttributeId(list_path, "", count, buffer);
            IupSetInt(list_path, "COUNT", count);
            IupSetInt(list_path, "VALUE", count);
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
    strncpy(buffer, current_val, 4096);
    buffer[4095] = '\0';

    if (IupGetParam("编辑环境变量", NULL, NULL, "路径: %s\n", buffer, NULL))
    {
        if (strlen(buffer) > 0)
        {
            IupSetAttributeId(list_path, "", selected, buffer);
        }
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
               "5. 注意：某些正在运行的程序可能需要重启才能识别新的环境变量。");
    return IUP_DEFAULT;
}