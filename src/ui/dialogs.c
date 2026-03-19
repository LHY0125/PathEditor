#include "ui/dialogs.h"
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
    IupSetAttribute(text, "RASTERSIZE", "500x");
    IupSetAttribute(text, "NAME", "INPUT_TEXT");

    Ihandle *btn_ok = IupButton("确定", NULL);
    IupSetCallback(btn_ok, "ACTION", on_dialog_ok);
    IupSetAttribute(btn_ok, "RASTERSIZE", "100x32");

    Ihandle *btn_cancel = IupButton("取消", NULL);
    IupSetCallback(btn_cancel, "ACTION", on_dialog_cancel);
    IupSetAttribute(btn_cancel, "RASTERSIZE", "100x32");

    Ihandle *vbox = IupVbox(
        IupLabel(label_text),
        text,
        IupHbox(IupFill(), btn_ok, btn_cancel, NULL),
        NULL);
    IupSetAttribute(vbox, "MARGIN", "15x15");
    IupSetAttribute(vbox, "GAP", "10");

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
            strncpy(buffer, val, buffer_size);
            buffer[buffer_size - 1] = '\0';
        }
    }

    IupDestroy(dlg);
    return result;
}