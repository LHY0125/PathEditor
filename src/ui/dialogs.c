#include "ui/dialogs.h"
#include "core/lua_config.h"
#include "utils/safe_string.h"
#include "utils/i18n.h"
#include <iup.h>
#include <string.h>

// 静态辅助函数：对话框确定
static int on_dialog_ok(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    IupSetAttribute(dlg, "MY_STATUS", "1");
    return IUP_CLOSE;
}

// 静态辅助函数：对话框取消
static int on_dialog_cancel(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    IupSetAttribute(dlg, "MY_STATUS", "0");
    return IUP_CLOSE;
}

// 真正的实现函数
int custom_input_dialog(const char *title, const char *label_text, char *buffer, int buffer_size)
{
    Ihandle *text = IupText(NULL);
    IupSetAttribute(text, "VALUE", buffer);
    IupSetAttribute(text, "EXPAND", "HORIZONTAL");
    IupSetAttribute(text, "RASTERSIZE", lua_config_get_string("input_dialog", "text_size"));
    IupSetAttribute(text, "NAME", "INPUT_TEXT");

    Ihandle *btn_ok = IupButton(_(lua_config_get_string("button", "ok")), NULL);
    IupSetCallback(btn_ok, "ACTION", on_dialog_ok);
    IupSetAttribute(btn_ok, "RASTERSIZE", lua_config_get_string("button", "rastersize"));

    Ihandle *btn_cancel = IupButton(_(lua_config_get_string("button", "cancel")), NULL);
    IupSetCallback(btn_cancel, "ACTION", on_dialog_cancel);
    IupSetAttribute(btn_cancel, "RASTERSIZE", lua_config_get_string("button", "rastersize"));

    Ihandle *vbox = IupVbox(
        IupLabel(label_text),
        text,
        IupHbox(IupFill(), btn_ok, btn_cancel, NULL),
        NULL);
    IupSetAttribute(vbox, "MARGIN", lua_config_get_string("input_dialog", "margin"));
    IupSetAttribute(vbox, "GAP", lua_config_get_string("input_dialog", "gap"));

    Ihandle *dlg = IupDialog(vbox);
    IupSetAttribute(dlg, "TITLE", title);
    IupSetAttribute(dlg, "MINBOX", "NO");
    IupSetAttribute(dlg, "MAXBOX", "NO");
    IupSetAttribute(dlg, "RESIZE", "NO");

    IupSetAttributeHandle(dlg, "DEFAULTENTER", btn_ok);
    IupSetAttributeHandle(dlg, "DEFAULTESC", btn_cancel);

    IupPopup(dlg, IUP_CENTER, IUP_CENTER);

    int result = IupGetInt(dlg, "MY_STATUS");
    if (result == 1)
    {
        char *val = IupGetAttribute(text, "VALUE");
        if (val)
        {
            safe_strcpy(buffer, buffer_size, val);
        }
    }

    IupDestroy(dlg);
    return result;
}

// 语言选择对话框
int language_select_dialog(void)
{
    const char *current_lang = i18n_get_current_language();

    Ihandle *list = IupList(NULL);
    IupSetAttribute(list, "NAME", "LANG_LIST");
    IupSetAttribute(list, "DROPDOWN", "YES");
    IupSetAttribute(list, "VALUE", (strcmp(current_lang, "zh_CN") == 0) ? "1" : "2");
    IupSetAttribute(list, "1", _("Chinese (Simplified)"));
    IupSetAttribute(list, "2", _("English"));
    IupSetAttribute(list, "RASTERSIZE", lua_config_get_string("language", "list_size"));

    Ihandle *btn_ok = IupButton(_("OK"), NULL);
    IupSetCallback(btn_ok, "ACTION", on_dialog_ok);
    IupSetAttribute(btn_ok, "RASTERSIZE", lua_config_get_string("button", "rastersize"));

    Ihandle *btn_cancel = IupButton(_("Cancel"), NULL);
    IupSetCallback(btn_cancel, "ACTION", on_dialog_cancel);
    IupSetAttribute(btn_cancel, "RASTERSIZE", lua_config_get_string("button", "rastersize"));

    Ihandle *vbox = IupVbox(
        IupLabel(_("Language")),
        list,
        IupHbox(IupFill(), btn_ok, btn_cancel, NULL),
        NULL);
    IupSetAttribute(vbox, "MARGIN", lua_config_get_string("language", "margin"));
    IupSetAttribute(vbox, "GAP", lua_config_get_string("language", "gap"));

    Ihandle *dlg = IupDialog(vbox);
    IupSetAttribute(dlg, "TITLE", _("Language"));
    IupSetAttribute(dlg, "MINBOX", "NO");
    IupSetAttribute(dlg, "MAXBOX", "NO");
    IupSetAttribute(dlg, "RESIZE", "NO");
    IupSetAttribute(dlg, "RASTERSIZE", lua_config_get_string("language", "dialog_size"));

    IupSetAttributeHandle(dlg, "DEFAULTENTER", btn_ok);
    IupSetAttributeHandle(dlg, "DEFAULTESC", btn_cancel);

    IupPopup(dlg, IUP_CENTER, IUP_CENTER);

    int result = IupGetInt(dlg, "MY_STATUS");
    if (result == 1)
    {
        int selected = IupGetInt(list, "VALUE");
        if (selected == 1)
            i18n_change_language("zh_CN");
        else
            i18n_change_language("en_US");
    }

    IupDestroy(dlg);
    return result;
}