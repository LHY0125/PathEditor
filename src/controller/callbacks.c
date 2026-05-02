#include "controller/callbacks.h"
#include "controller/callbacks_internal.h"
#include "core/app_context.h"
#include "core/undo_redo.h"
#include "utils/ui_constants.h"
#include "ui/ui_utils.h"
#include <iup.h>
#include <string.h>

// 辅助函数：获取主对话框
Ihandle *get_main_dlg(void)
{
    return IupGetHandle("MAIN_DIALOG");
}

// 辅助函数：从对话框获取应用上下文（core层不依赖IUP，因此在此实现）
AppContext *get_app_context_from_dlg(Ihandle *dlg)
{
    if (!dlg)
        return NULL;
    return (AppContext *)IupGetAttribute(dlg, "APP_CONTEXT");
}

// 获取当前的缓存数据列表
StringList *get_current_raw_data(Ihandle *dlg)
{
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx)
        return NULL;

    Ihandle *tabs_main = IupGetDialogChild(dlg, CTRL_TABS_MAIN);
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    if (pos == 0)
        return &ctx->sys_paths;
    if (pos == 1)
        return &ctx->user_paths;
    return &ctx->sys_paths;
}

// 辅助函数：获取当前选中的列表UI控件
Ihandle *get_current_list(Ihandle *dlg)
{
    Ihandle *tabs_main = IupGetDialogChild(dlg, CTRL_TABS_MAIN);
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    if (pos == 0)
        return IupGetDialogChild(dlg, CTRL_LIST_SYS);
    if (pos == 1)
        return IupGetDialogChild(dlg, CTRL_LIST_USER);
    return IupGetDialogChild(dlg, CTRL_LIST_SYS);
}

// 刷新撤销/重做按钮的启用状态
void refresh_undo_redo_buttons(Ihandle *dlg)
{
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx || !ctx->undo_redo_mgr)
        return;

    Ihandle *btn_undo = IupGetDialogChild(dlg, CTRL_BTN_UNDO);
    Ihandle *btn_redo = IupGetDialogChild(dlg, CTRL_BTN_REDO);

    if (btn_undo)
        IupSetAttribute(btn_undo, "ACTIVE", can_undo(ctx->undo_redo_mgr) ? "YES" : "NO");
    if (btn_redo)
        IupSetAttribute(btn_redo, "ACTIVE", can_redo(ctx->undo_redo_mgr) ? "YES" : "NO");
}

// 同步合并预览列表（sys + user）
void sync_merged_list(Ihandle *dlg)
{
    AppContext *ctx = get_app_context_from_dlg(dlg);
    Ihandle *list_merged = IupGetDialogChild(dlg, CTRL_LIST_MERGED);
    if (!ctx || !list_merged)
        return;

    IupSetAttribute(list_merged, "REMOVEITEM", "ALL");

    int pos = 1;
    for (int i = 0; i < ctx->sys_paths.count; i++)
        IupSetAttributeId(list_merged, "", pos++, string_list_get(&ctx->sys_paths, i));
    for (int i = 0; i < ctx->user_paths.count; i++)
        IupSetAttributeId(list_merged, "", pos++, string_list_get(&ctx->user_paths, i));

    int total = ctx->sys_paths.count + ctx->user_paths.count;
    IupSetInt(list_merged, "COUNT", total);
    refresh_single_list_style(list_merged);
}

// 获取当前 Tab 对应的 TargetType
TargetType get_current_target(Ihandle *dlg)
{
    Ihandle *tabs = IupGetDialogChild(dlg, CTRL_TABS_MAIN);
    if (tabs)
    {
        int pos = IupGetInt(tabs, "VALUEPOS");
        return (pos == 0) ? TARGET_SYSTEM : TARGET_USER;
    }
    return TARGET_USER;
}

// 辅助函数：创建并推送撤销记录
void push_record(Ihandle *dlg, OperationType op_type, int index, int count,
                 char **old_paths, char **new_paths)
{
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx || !ctx->undo_redo_mgr)
        return;

    OpRecord record;
    record.type = op_type;
    record.target = get_current_target(dlg);
    record.index = index;
    record.count = count;
    record.old_paths = old_paths;
    record.new_paths = new_paths;

    push_undo_record(ctx->undo_redo_mgr, &record);
}
