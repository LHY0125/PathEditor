#include "controller/callbacks.h"
#include "controller/callbacks_internal.h"
#include "core/path_manager.h"
#include "core/lua_config.h"
#include "core/undo_redo.h"
#include "utils/string_ext.h"
#include "utils/safe_string.h"
#include "utils/error_code.h"
#include "utils/logger.h"
#include "utils/ui_constants.h"
#include "ui/ui_utils.h"
#include "ui/dialogs.h"
#include <string.h>
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

// 按钮回调：新建
int btn_new_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    char buffer[PATH_BUFFER_SIZE] = "";
    if (custom_input_dialog(_("New Environment Variable"), _("Please enter a path:"), buffer, sizeof(buffer)))
    {
        if (strlen(buffer) > 0)
        {
            StringList *raw_data = get_current_raw_data(dlg);

            // 检查是否已存在重复路径
            if (string_list_contains(raw_data, buffer))
            {
                IupMessage(_("Warning"), _("This path already exists and will not be added again."));
                return IUP_DEFAULT;
            }

            // 记录撤销信息（添加前的状态）
            char *path_copy = _strdup(buffer);
            char *paths[1] = {path_copy};
            push_record(dlg, OP_ADD, raw_data->count, 1, paths, NULL);
            free(path_copy);

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

    char buffer[PATH_BUFFER_SIZE];
    safe_strcpy(buffer, sizeof(buffer), string_list_get(raw_data, selected - 1));

    if (custom_input_dialog(_("Edit Environment Variable"), _("Edit path:"), buffer, sizeof(buffer)))
    {
        if (strlen(buffer) > 0)
        {
            // 记录撤销信息（编辑前的值）
            char *old_path = _strdup(string_list_get(raw_data, selected - 1));
            char *new_path = _strdup(buffer);
            char *old_paths[1] = {old_path};
            char *new_paths[1] = {new_path};
            push_record(dlg, OP_EDIT, selected - 1, 1, old_paths, new_paths);
            free(old_path);
            free(new_path);

            string_list_set(raw_data, selected - 1, buffer);

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
    IupSetAttribute(filedlg, "TITLE", lua_config_get_string("dialog", "select_dir"));

    IupPopup(filedlg, IUP_CENTER, IUP_CENTER);

    if (IupGetInt(filedlg, "STATUS") != -1)
    {
        char *value = IupGetAttribute(filedlg, "VALUE");
        if (value)
        {
            StringList *raw_data = get_current_raw_data(dlg);

            // 检查是否已存在重复路径
            if (string_list_contains(raw_data, value))
            {
                IupMessage(_("Warning"), _("This path already exists and will not be added again."));
                IupDestroy(filedlg);
                return IUP_DEFAULT;
            }

            // 记录撤销信息（添加前的状态）
            char *path_copy = _strdup(value);
            char *paths[1] = {path_copy};
            push_record(dlg, OP_ADD, raw_data->count, 1, paths, NULL);
            free(path_copy);

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
        IupMessage(_("Info"), _("Please select an item to delete first"));
        return IUP_DEFAULT;
    }

    StringList *raw_data = get_current_raw_data(dlg);
    int del_index = selected - 1;

    // 记录撤销信息（被删除的路径）
    char *del_path = _strdup(string_list_get(raw_data, del_index));
    char *paths[1] = {del_path};
    push_record(dlg, OP_DELETE, del_index, 1, paths, NULL);
    free(del_path);

    ErrorCode result = path_manager_remove_at(raw_data, del_index);
    if (result != ERR_OK)
    {
        log_error("Failed to remove path at index %d", del_index);
    }

    sync_string_list_to_ui(current_list, raw_data);

    Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
    if (lbl_status)
        IupSetAttribute(lbl_status, "TITLE", lua_config_get_string("status", "deleted"));

    return IUP_DEFAULT;
}
