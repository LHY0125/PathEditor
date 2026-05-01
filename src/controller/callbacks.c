#include "controller/callbacks.h"
#include "controller/callbacks_internal.h"
#include "core/app_context.h"
#include "core/undo_redo.h"
#include "utils/ui_constants.h"
#include <iup.h>

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
