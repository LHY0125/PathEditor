#include "controller/callbacks.h"
#include "controller/callbacks_internal.h"
#include "core/app_context.h"
#include "core/lua_config.h"
#include "utils/string_ext.h"
#include "utils/safe_string.h"
#include "utils/ui_constants.h"
#include "ui/ui_utils.h"
#include <string.h>
#include <windows.h>

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
        const char *item = string_list_get(raw_data, i);
        if (strlen(filter) == 0 || stristr(item, filter) != NULL)
        {
            count++;
            IupSetAttributeId(current_list, "", count, item);
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
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx)
        return IUP_DEFAULT;

    StringList *raw_data = NULL;
    if (self == IupGetDialogChild(dlg, CTRL_LIST_SYS))
        raw_data = &ctx->sys_paths;
    else if (self == IupGetDialogChild(dlg, CTRL_LIST_USER))
        raw_data = &ctx->user_paths;
    else
        return IUP_DEFAULT;

    wchar_t *wfilename = utf8_to_wide(filename);
    DWORD attr = wfilename ? GetFileAttributesW(wfilename) : INVALID_FILE_ATTRIBUTES;
    free(wfilename);
    if (attr != INVALID_FILE_ATTRIBUTES && (attr & FILE_ATTRIBUTE_DIRECTORY))
    {
        Ihandle *txt_search = IupGetDialogChild(dlg, CTRL_TXT_SEARCH);
        if (txt_search)
            IupSetAttribute(txt_search, "VALUE", "");

        add_string_list(raw_data, filename);
        sync_string_list_to_ui(current_list, raw_data);

        IupSetInt(current_list, "VALUE", raw_data->count);
    }
    else
    {
        Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
        if (lbl_status)
            IupSetAttribute(lbl_status, "TITLE", lua_config_get_string("status", "drag_folder_only"));
    }

    return IUP_DEFAULT;
}
