#include "controller/callbacks.h"
#include "core/app_context.h"
#include "core/registry_service.h"
#include "core/path_manager.h"
#include "utils/string_ext.h"
#include "utils/os_env.h"
#include "ui/ui_utils.h"
#include "ui/dialogs.h"
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

// 辅助函数：获取主对话框
static Ihandle *get_main_dlg()
{
    // 在实际情况中，可以通过 IupGetHandle 注册名字，或者通过某个全局/静态缓存
    // 但如果想彻底不用全局，我们可以在 IupSetHandle 里面把主窗口存下来
    return IupGetHandle("MAIN_DIALOG");
}

// 获取当前的缓存数据列表
static StringList *get_current_raw_data(Ihandle *dlg)
{
    AppContext *ctx = get_app_context(dlg);
    if (!ctx)
        return NULL;

    Ihandle *tabs_main = IupGetDialogChild(dlg, "TABS_MAIN");
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    if (pos == 0)
        return &ctx->sys_paths;
    if (pos == 1)
        return &ctx->user_paths;
    return &ctx->sys_paths;
}

// 辅助函数：获取当前选中的列表UI控件
static Ihandle *get_current_list(Ihandle *dlg)
{
    Ihandle *tabs_main = IupGetDialogChild(dlg, "TABS_MAIN");
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    if (pos == 0)
        return IupGetDialogChild(dlg, "LIST_SYS");
    if (pos == 1)
        return IupGetDialogChild(dlg, "LIST_USER");
    return IupGetDialogChild(dlg, "LIST_SYS");
}

// 按钮回调：新建
int btn_new_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    char buffer[1024] = "";
    if (custom_input_dialog("新建环境变量", "请输入路径:", buffer, sizeof(buffer)))
    {
        if (strlen(buffer) > 0)
        {
            StringList *raw_data = get_current_raw_data(dlg);
            add_string_list(raw_data, buffer);

            Ihandle *current_list = get_current_list(dlg);
            sync_string_list_to_ui(current_list, raw_data);

            int count = IupGetInt(current_list, "COUNT");
            IupSetInt(current_list, "VALUE", count);
        }
    }
    return IUP_DEFAULT;
}

// 按钮回调：编辑
int btn_edit_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    Ihandle *current_list = get_current_list(dlg);
    int selected = IupGetInt(current_list, "VALUE");
    if (selected == 0)
        return IUP_DEFAULT;

    StringList *raw_data = get_current_raw_data(dlg);
    if (selected - 1 >= raw_data->count)
        return IUP_DEFAULT;

    char buffer[4096];
    strncpy(buffer, raw_data->items[selected - 1], 4096);
    buffer[4095] = '\0';

    if (custom_input_dialog("编辑环境变量", "编辑路径:", buffer, sizeof(buffer)))
    {
        if (strlen(buffer) > 0)
        {
            free(raw_data->items[selected - 1]);
            raw_data->items[selected - 1] = _strdup(buffer);

            sync_string_list_to_ui(current_list, raw_data);
            IupSetInt(current_list, "VALUE", selected);
        }
    }
    return IUP_DEFAULT;
}

// 双击回调
int list_dblclick_cb(Ihandle *self, int item, char *text)
{
    if (item > 0)
    {
        IupSetInt(self, "VALUE", item);
        btn_edit_cb(self);
    }
    return IUP_DEFAULT;
}

// 按钮回调：浏览
int btn_browse_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    Ihandle *filedlg = IupFileDlg();
    IupSetAttribute(filedlg, "DIALOGTYPE", "DIR");
    IupSetAttribute(filedlg, "TITLE", "选择目录");

    IupPopup(filedlg, IUP_CENTER, IUP_CENTER);

    if (IupGetInt(filedlg, "STATUS") != -1)
    {
        char *value = IupGetAttribute(filedlg, "VALUE");
        if (value)
        {
            StringList *raw_data = get_current_raw_data(dlg);
            add_string_list(raw_data, value);

            Ihandle *current_list = get_current_list(dlg);
            sync_string_list_to_ui(current_list, raw_data);

            int count = IupGetInt(current_list, "COUNT");
            IupSetInt(current_list, "VALUE", count);
        }
    }
    IupDestroy(filedlg);
    return IUP_DEFAULT;
}

// 按钮回调：删除
int btn_del_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    Ihandle *current_list = get_current_list(dlg);
    int selected = IupGetInt(current_list, "VALUE");

    if (selected == 0)
    {
        IupMessage("提示", "请先选择要删除的项");
        return IUP_DEFAULT;
    }

    StringList *raw_data = get_current_raw_data(dlg);
    path_manager_remove_at(raw_data, selected - 1);

    sync_string_list_to_ui(current_list, raw_data);

    Ihandle *lbl_status = IupGetDialogChild(dlg, "LBL_STATUS");
    if (lbl_status)
        IupSetAttribute(lbl_status, "TITLE", "状态: 已删除选中项");

    return IUP_DEFAULT;
}

// 按钮回调：上移
int btn_up_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    Ihandle *current_list = get_current_list(dlg);
    int selected = IupGetInt(current_list, "VALUE");
    if (selected <= 1)
        return IUP_DEFAULT;

    StringList *raw_data = get_current_raw_data(dlg);
    path_manager_move_up(raw_data, selected - 1);

    sync_string_list_to_ui(current_list, raw_data);
    IupSetInt(current_list, "VALUE", selected - 1);

    return IUP_DEFAULT;
}

// 按钮回调：下移
int btn_down_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    Ihandle *current_list = get_current_list(dlg);
    int selected = IupGetInt(current_list, "VALUE");
    StringList *raw_data = get_current_raw_data(dlg);

    if (selected == 0 || selected >= raw_data->count)
        return IUP_DEFAULT;

    path_manager_move_down(raw_data, selected - 1);

    sync_string_list_to_ui(current_list, raw_data);
    IupSetInt(current_list, "VALUE", selected + 1);

    return IUP_DEFAULT;
}

// 按钮回调：一键清理
int btn_clean_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    StringList *raw_data = get_current_raw_data(dlg);
    if (!raw_data || raw_data->count == 0)
        return IUP_DEFAULT;

    if (IupAlarm("确认清理", "此操作将移除当前列表中所有【无效路径】和【重复路径】。\n确定要继续吗？", "确定", "取消", NULL) != 1)
    {
        return IUP_DEFAULT;
    }

    int removed = path_manager_clean(raw_data);

    Ihandle *current_list = get_current_list(dlg);
    sync_string_list_to_ui(current_list, raw_data);

    char msg[128];
    snprintf(msg, sizeof(msg), "清理完成！共移除了 %d 个无效或重复路径。", removed);
    IupMessage("提示", msg);
    return IUP_DEFAULT;
}

// 搜索回调
int txt_search_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    char *filter = IupGetAttribute(self, "VALUE");
    if (!filter)
        return IUP_DEFAULT;

    Ihandle *current_list = get_current_list(dlg);
    StringList *raw_data = get_current_raw_data(dlg);

    IupSetAttribute(current_list, "REMOVEITEM", "ALL");

    int count = 0;
    for (int i = 0; i < raw_data->count; i++)
    {
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
    Ihandle *dlg = IupGetDialog(self);
    Ihandle *current_list = self;
    AppContext *ctx = get_app_context(dlg);
    if (!ctx)
        return IUP_DEFAULT;

    StringList *raw_data = NULL;
    if (self == IupGetDialogChild(dlg, "LIST_SYS"))
        raw_data = &ctx->sys_paths;
    else if (self == IupGetDialogChild(dlg, "LIST_USER"))
        raw_data = &ctx->user_paths;
    else
        return IUP_DEFAULT;

    DWORD attr = GetFileAttributesA(filename);
    if (attr != INVALID_FILE_ATTRIBUTES && (attr & FILE_ATTRIBUTE_DIRECTORY))
    {
        Ihandle *txt_search = IupGetDialogChild(dlg, "TXT_SEARCH");
        if (txt_search)
            IupSetAttribute(txt_search, "VALUE", "");

        add_string_list(raw_data, filename);
        sync_string_list_to_ui(current_list, raw_data);

        IupSetInt(current_list, "VALUE", raw_data->count);
    }
    else
    {
        Ihandle *lbl_status = IupGetDialogChild(dlg, "LBL_STATUS");
        if (lbl_status)
            IupSetAttribute(lbl_status, "TITLE", "提示: 只能拖拽文件夹添加到 PATH");
    }

    return IUP_DEFAULT;
}

// 键盘按键回调
int list_k_any_cb(Ihandle *self, int c)
{
    if (c == K_DEL)
    {
        btn_del_cb(self);
        return IUP_IGNORE;
    }
    return IUP_DEFAULT;
}

// 按钮回调：确定 (保存所有)
int btn_ok_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    AppContext *ctx = get_app_context(dlg);
    if (!ctx)
        return IUP_DEFAULT;

    if (!check_admin())
    {
        IupMessage("错误", "需要管理员权限才能保存更改！");
        return IUP_DEFAULT;
    }

    backup_registry();

    int sys_ok = save_system_paths(&ctx->sys_paths);
    int user_ok = save_user_paths(&ctx->user_paths);

    Ihandle *lbl_status = IupGetDialogChild(dlg, "LBL_STATUS");

    if (sys_ok && user_ok)
    {
        SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, (LPARAM)L"Environment", SMTO_ABORTIFHUNG, 5000, NULL);
        IupMessage("成功", "系统和用户 PATH 环境变量均已更新！");
        if (lbl_status)
            IupSetAttribute(lbl_status, "TITLE", "状态: 全部保存成功");
    }
    else if (sys_ok)
    {
        IupMessage("提示", "系统变量保存成功，但用户变量保存失败。");
    }
    else if (user_ok)
    {
        IupMessage("提示", "用户变量保存成功，但系统变量保存失败。");
    }
    else
    {
        IupMessage("错误", "保存失败！");
        if (lbl_status)
            IupSetAttribute(lbl_status, "TITLE", "状态: 保存失败");
    }
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

// 载入所有路径
void load_all_paths(void)
{
    Ihandle *dlg = get_main_dlg();
    if (!dlg)
        return;
    AppContext *ctx = get_app_context(dlg);
    if (!ctx)
        return;

    if (!load_system_paths(&ctx->sys_paths))
    {
        IupMessage("错误", "无法打开系统环境变量注册表键，请尝试以管理员身份运行。");
    }
    load_user_paths(&ctx->user_paths);

    Ihandle *list_sys = IupGetDialogChild(dlg, "LIST_SYS");
    Ihandle *list_user = IupGetDialogChild(dlg, "LIST_USER");

    sync_string_list_to_ui(list_sys, &ctx->sys_paths);
    sync_string_list_to_ui(list_user, &ctx->user_paths);

    Ihandle *lbl_status = IupGetDialogChild(dlg, "LBL_STATUS");
    if (lbl_status)
        IupSetAttribute(lbl_status, "TITLE", "状态: 已加载系统和用户变量");
}