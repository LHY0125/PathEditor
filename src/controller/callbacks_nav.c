#include "controller/callbacks.h"
#include "controller/callbacks_internal.h"
#include "core/path_manager.h"
#include "core/lua_config.h"
#include "core/undo_redo.h"
#include "utils/error_code.h"
#include "utils/logger.h"
#include "utils/ui_constants.h"
#include "utils/safe_string.h"
#include "ui/ui_utils.h"
#include <stdio.h>
#include <stdlib.h>

// 辅助函数：检查当前目标是系统还是用户
static TargetType get_current_target(Ihandle *dlg)
{
    Ihandle *tabs = IupGetDialogChild(dlg, CTRL_TABS_MAIN);
    if (tabs)
    {
        int tab = IupGetInt(tabs, "VALUE");
        return (tab == 1) ? TARGET_SYSTEM : TARGET_USER;
    }
    return TARGET_USER;
}

// 辅助函数：创建并推送撤销记录
static void push_record(Ihandle *dlg, OperationType op_type, int index, int count,
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

// 按钮回调：上移
int btn_up_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    Ihandle *current_list = get_current_list(dlg);
    int selected = IupGetInt(current_list, "VALUE");
    if (selected <= 1)
        return IUP_DEFAULT;

    StringList *raw_data = get_current_raw_data(dlg);
    int move_index = selected - 1;

    // 记录撤销信息
    char *path = safe_strdup(string_list_get(raw_data, move_index));
    char *old_paths[1] = {path};
    char *new_paths[1] = {safe_strdup(path)};
    push_record(dlg, OP_MOVE_UP, move_index, 1, old_paths, new_paths);
    free(path);
    free(new_paths[0]);

    ErrorCode result = path_manager_move_up(raw_data, move_index);
    if (result != ERR_OK)
    {
        log_error("Failed to move path up at index %d", move_index);
    }

    sync_string_list_to_ui(current_list, raw_data);
    IupSetInt(current_list, "VALUE", selected - 1);

    refresh_undo_redo_buttons(dlg);
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

    int move_index = selected - 1;

    // 记录撤销信息
    char *path = safe_strdup(string_list_get(raw_data, move_index));
    char *old_paths[1] = {path};
    char *new_paths[1] = {safe_strdup(path)};
    push_record(dlg, OP_MOVE_DOWN, move_index, 1, old_paths, new_paths);
    free(path);
    free(new_paths[0]);

    ErrorCode result = path_manager_move_down(raw_data, move_index);
    if (result != ERR_OK)
    {
        log_error("Failed to move path down at index %d", move_index);
    }

    sync_string_list_to_ui(current_list, raw_data);
    IupSetInt(current_list, "VALUE", selected + 1);

    refresh_undo_redo_buttons(dlg);
    return IUP_DEFAULT;
}

// 按钮回调：一键清理
int btn_clean_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    StringList *raw_data = get_current_raw_data(dlg);
    if (!raw_data || raw_data->count == 0)
        return IUP_DEFAULT;

    if (IupAlarm(_("Confirm Cleanup"), _("This operation will remove all 【invalid paths】 and 【duplicate paths】 from the current list.\nAre you sure you want to continue?"), _("Confirm"), _("Cancel"), NULL) != 1)
    {
        return IUP_DEFAULT;
    }

    int before_count = raw_data->count;

    // 记录撤销信息（清理前的所有路径）
    char **old_paths = (char **)malloc(before_count * sizeof(char *));
    for (int i = 0; i < before_count; i++)
        old_paths[i] = safe_strdup(raw_data->items[i]);

    push_record(dlg, OP_CLEAN, 0, before_count, old_paths, NULL);

    for (int i = 0; i < before_count; i++)
        free(old_paths[i]);
    free(old_paths);

    path_manager_clean(raw_data);
    int removed = before_count - raw_data->count;

    Ihandle *current_list = get_current_list(dlg);
    sync_string_list_to_ui(current_list, raw_data);

    char msg[128];
    snprintf(msg, sizeof(msg), _("Cleanup completed! Removed %d invalid or duplicate paths."), removed);
    IupMessage(_("Info"), msg);

    refresh_undo_redo_buttons(dlg);
    return IUP_DEFAULT;
}

int btn_undo_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx || !ctx->undo_redo_mgr)
        return IUP_DEFAULT;

    if (!can_undo(ctx->undo_redo_mgr))
        return IUP_DEFAULT;

    undo(ctx->undo_redo_mgr, &ctx->sys_paths, &ctx->user_paths);

    Ihandle *list_sys = IupGetDialogChild(dlg, CTRL_LIST_SYS);
    Ihandle *list_user = IupGetDialogChild(dlg, CTRL_LIST_USER);
    sync_string_list_to_ui(list_sys, &ctx->sys_paths);
    sync_string_list_to_ui(list_user, &ctx->user_paths);

    Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
    if (lbl_status)
        IupSetAttribute(lbl_status, "TITLE", _("Undo completed"));

    refresh_undo_redo_buttons(dlg);
    return IUP_DEFAULT;
}

int btn_redo_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx || !ctx->undo_redo_mgr)
        return IUP_DEFAULT;

    if (!can_redo(ctx->undo_redo_mgr))
        return IUP_DEFAULT;

    redo(ctx->undo_redo_mgr, &ctx->sys_paths, &ctx->user_paths);

    Ihandle *list_sys = IupGetDialogChild(dlg, CTRL_LIST_SYS);
    Ihandle *list_user = IupGetDialogChild(dlg, CTRL_LIST_USER);
    sync_string_list_to_ui(list_sys, &ctx->sys_paths);
    sync_string_list_to_ui(list_user, &ctx->user_paths);

    Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
    if (lbl_status)
        IupSetAttribute(lbl_status, "TITLE", _("Redo completed"));

    refresh_undo_redo_buttons(dlg);
    return IUP_DEFAULT;
}

// 键盘按键回调
int list_k_any_cb(Ihandle *self, int c)
{
    if (IupGetInt(self, "ACTIVE") == 0)
        return IUP_DEFAULT;

    if (c == K_cZ)  // Ctrl+Z 撤销
    {
        btn_undo_cb(self);
        return IUP_IGNORE;
    }

    if (c == K_cY)  // Ctrl+Y 重做
    {
        btn_redo_cb(self);
        return IUP_IGNORE;
    }

    if (c == K_DEL)  // DEL 键
    {
        btn_del_cb(self);
        return IUP_IGNORE;
    }
    return IUP_DEFAULT;
}
