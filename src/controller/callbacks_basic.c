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

            refresh_undo_redo_buttons(dlg);
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

            refresh_undo_redo_buttons(dlg);
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

            refresh_undo_redo_buttons(dlg);
        }
    }
    IupDestroy(filedlg);
    return IUP_DEFAULT;
}

// 按钮回调：删除（支持多选批量删除）
int btn_del_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    Ihandle *current_list = get_current_list(dlg);

    // 解析多选索引（VALUE 为 "1;3;5" 格式）
    char *value_str = IupGetAttribute(current_list, "VALUE");
    if (!value_str || value_str[0] == '\0')
    {
        IupMessage(_("Info"), _("Please select an item to delete first"));
        return IUP_DEFAULT;
    }

    StringList *raw_data = get_current_raw_data(dlg);
    if (!raw_data || raw_data->count == 0)
        return IUP_DEFAULT;

    // 解析选中索引并排序（从大到小，方便从尾部删除避免索引偏移）
    int indices[256];
    int sel_count = 0;
    char *token = strtok(value_str, ";");
    while (token && sel_count < 256)
    {
        int idx = atoi(token) - 1;  // 转为 0-based
        if (idx >= 0 && idx < raw_data->count)
            indices[sel_count++] = idx;
        token = strtok(NULL, ";");
    }

    if (sel_count == 0)
        return IUP_DEFAULT;

    // 从大到小排序
    for (int i = 0; i < sel_count - 1; i++)
    {
        for (int j = i + 1; j < sel_count; j++)
        {
            if (indices[i] < indices[j])
            {
                int tmp = indices[i];
                indices[i] = indices[j];
                indices[j] = tmp;
            }
        }
    }

    // 记录撤销信息（所有被删除的路径）
    char **old_paths = (char **)malloc(sel_count * sizeof(char *));
    for (int i = 0; i < sel_count; i++)
        old_paths[i] = _strdup(string_list_get(raw_data, indices[i]));

    push_record(dlg, OP_DELETE, indices[sel_count - 1], sel_count, old_paths, NULL);

    for (int i = 0; i < sel_count; i++)
        free(old_paths[i]);
    free(old_paths);

    // 从大到小删除（避免索引偏移）
    for (int i = 0; i < sel_count; i++)
        path_manager_remove_at(raw_data, indices[i]);

    sync_string_list_to_ui(current_list, raw_data);

    Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
    if (lbl_status)
    {
        char msg[64];
        snprintf(msg, sizeof(msg), _("Deleted %d items"), sel_count);
        IupSetAttribute(lbl_status, "TITLE", msg);
    }

    refresh_undo_redo_buttons(dlg);
    return IUP_DEFAULT;
}
