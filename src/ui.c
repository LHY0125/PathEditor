#include "ui.h"
#include "globals.h"
#include "callbacks.h"
#include <stdlib.h>

// 创建列表控件
Ihandle *create_path_list()
{
    Ihandle *list = IupFlatList();
    IupSetAttribute(list, "EXPAND", "YES");
    IupSetAttribute(list, "ITEMPADDING", "5x5");
    IupSetAttribute(list, "BACKCOLOR", "255 255 255");
    IupSetAttribute(list, "BORDER", "YES");
    IupSetAttribute(list, "CANFOCUS", "YES");
    IupSetAttribute(list, "HLINE", "NO");
    IupSetCallback(list, "DBLCLICK_CB", (Icallback)list_dblclick_cb);
    IupSetCallback(list, "DROPFILES_CB", (Icallback)list_dropfiles_cb);
    IupSetCallback(list, "K_ANY", (Icallback)list_k_any_cb);
    return list;
}

// 创建右侧功能按钮区域
Ihandle *create_main_buttons()
{
    // 创建右侧按钮
    btn_new = IupButton("新建(N)", NULL);
    btn_edit = IupButton("编辑(E)", NULL);
    btn_browse = IupButton("浏览(B)...", NULL);
    btn_del = IupButton("删除(D)", NULL);
    btn_up = IupButton("上移(U)", NULL);
    btn_down = IupButton("下移(O)", NULL);
    btn_clean = IupButton("一键清理", NULL);

    // 设置按钮回调
    IupSetCallback(btn_new, "ACTION", (Icallback)btn_new_cb);
    IupSetCallback(btn_edit, "ACTION", (Icallback)btn_edit_cb);
    IupSetCallback(btn_browse, "ACTION", (Icallback)btn_browse_cb);
    IupSetCallback(btn_del, "ACTION", (Icallback)btn_del_cb);
    IupSetCallback(btn_up, "ACTION", (Icallback)btn_up_cb);
    IupSetCallback(btn_down, "ACTION", (Icallback)btn_down_cb);
    IupSetCallback(btn_clean, "ACTION", (Icallback)btn_clean_cb);

    // 设置按钮大小 (宽度和高度都增加约1/4)
    IupSetAttribute(btn_new, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_edit, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_browse, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_del, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_up, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_down, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_clean, "RASTERSIZE", "100x32");

    Ihandle *vbox_btns = IupVbox(
        btn_new, btn_edit, btn_browse, btn_del,
        IupFill(), // 间隔
        btn_clean, // 放在上移下移之前，或者最下面，这里放在中间偏下
        IupFill(),
        btn_up, btn_down,
        NULL);
    IupSetAttribute(vbox_btns, "GAP", "5");
    IupSetAttribute(vbox_btns, "MARGIN", "0x0");
    
    return vbox_btns;
}

// 创建底部按钮区域
Ihandle *create_bottom_buttons()
{
    // 底部按钮
    btn_ok = IupButton("确定", NULL);
    btn_cancel = IupButton("取消", NULL);
    btn_help = IupButton("帮助(?)", NULL);

    IupSetCallback(btn_ok, "ACTION", (Icallback)btn_ok_cb);
    IupSetCallback(btn_cancel, "ACTION", (Icallback)btn_cancel_cb);
    IupSetCallback(btn_help, "ACTION", (Icallback)btn_help_cb);

    IupSetAttribute(btn_ok, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_cancel, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_help, "RASTERSIZE", "100x32");

    Ihandle *hbox_bottom = IupHbox(lbl_status, IupFill(), btn_help, btn_ok, btn_cancel, NULL);
    IupSetAttribute(hbox_bottom, "GAP", "10");
    IupSetAttribute(hbox_bottom, "MARGIN", "10x10");
    IupSetAttribute(hbox_bottom, "ALIGNMENT", "ACENTER");

    return hbox_bottom;
}