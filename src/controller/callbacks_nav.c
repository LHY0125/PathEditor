#include "controller/callbacks.h"
#include "controller/callbacks_internal.h"
#include "core/path_manager.h"
#include "core/lua_config.h"
#include "utils/error_code.h"
#include "utils/logger.h"
#include "utils/ui_constants.h"
#include "ui/ui_utils.h"
#include <stdio.h>

// 按钮回调：上移
int btn_up_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    Ihandle *current_list = get_current_list(dlg);
    int selected = IupGetInt(current_list, "VALUE");
    if (selected <= 1)
        return IUP_DEFAULT;

    StringList *raw_data = get_current_raw_data(dlg);
    ErrorCode result = path_manager_move_up(raw_data, selected - 1);
    if (result != ERR_OK)
    {
        log_error("Failed to move path up at index %d", selected - 1);
    }

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

    ErrorCode result = path_manager_move_down(raw_data, selected - 1);
    if (result != ERR_OK)
    {
        log_error("Failed to move path down at index %d", selected - 1);
    }

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

    if (IupAlarm(_("Confirm Cleanup"), _("This operation will remove all 【invalid paths】 and 【duplicate paths】 from the current list.\nAre you sure you want to continue?"), _("Confirm"), _("Cancel"), NULL) != 1)
    {
        return IUP_DEFAULT;
    }

    int before_count = raw_data->count;
    path_manager_clean(raw_data);
    int removed = before_count - raw_data->count;

    Ihandle *current_list = get_current_list(dlg);
    sync_string_list_to_ui(current_list, raw_data);

    char msg[128];
    snprintf(msg, sizeof(msg), _("Cleanup completed! Removed %d invalid or duplicate paths."), removed);
    IupMessage(_("Info"), msg);
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
