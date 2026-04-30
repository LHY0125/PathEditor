#include "controller/callbacks.h"
#include "controller/callbacks_internal.h"
#include "core/path_manager.h"
#include "core/lua_config.h"
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
    ErrorCode result = path_manager_remove_at(raw_data, selected - 1);
    if (result != ERR_OK)
    {
        log_error("Failed to remove path at index %d", selected - 1);
    }

    sync_string_list_to_ui(current_list, raw_data);

    Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
    if (lbl_status)
        IupSetAttribute(lbl_status, "TITLE", lua_config_get_string("status", "deleted"));

    return IUP_DEFAULT;
}
