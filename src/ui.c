#include "ui.h"
#include "globals.h"
#include "ui_utils.h"
#include "cb_edit.h"
#include "cb_file.h"
#include "cb_main.h"
#include <stdlib.h>

// 创建列表控件
Ihandle *create_path_list()
{
    Ihandle *list = IupFlatList();
    IupSetAttribute(list, "EXPAND", "YES");
    IupSetAttribute(list, "MULTIPLE", "YES");
    IupSetAttribute(list, "ITEMPADDING", "5x5");
    IupSetAttribute(list, "BACKCOLOR", "255 255 255");
    IupSetAttribute(list, "BORDER", "YES");
    IupSetAttribute(list, "CANFOCUS", "YES");
    IupSetAttribute(list, "HLINE", "NO");
    IupSetCallback(list, "DBLCLICK_CB", (Icallback)list_dblclick_cb);
    IupSetCallback(list, "DROPFILES_CB", (Icallback)list_dropfiles_cb);
    IupSetCallback(list, "K_ANY", (Icallback)list_k_any_cb);
    IupSetCallback(list, "MOTION_CB", (Icallback)list_motion_cb);
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
    btn_undo = IupButton("撤销(Z)", NULL);
    btn_redo = IupButton("重做(Y)", NULL);
    btn_up = IupButton("上移(U)", NULL);
    btn_down = IupButton("下移(O)", NULL);
    btn_clean = IupButton("一键清理", NULL);

    // 设置按钮回调
    IupSetCallback(btn_new, "ACTION", (Icallback)btn_new_cb);
    IupSetCallback(btn_edit, "ACTION", (Icallback)btn_edit_cb);
    IupSetCallback(btn_browse, "ACTION", (Icallback)btn_browse_cb);
    IupSetCallback(btn_del, "ACTION", (Icallback)btn_del_cb);
    IupSetCallback(btn_undo, "ACTION", (Icallback)btn_undo_cb);
    IupSetCallback(btn_redo, "ACTION", (Icallback)btn_redo_cb);
    IupSetCallback(btn_up, "ACTION", (Icallback)btn_up_cb);
    IupSetCallback(btn_down, "ACTION", (Icallback)btn_down_cb);
    IupSetCallback(btn_clean, "ACTION", (Icallback)btn_clean_cb);

    // 设置按钮大小 (宽度和高度都增加约1/4)
    IupSetAttribute(btn_new, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_edit, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_browse, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_del, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_undo, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_redo, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_up, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_down, "RASTERSIZE", "100x32");
    IupSetAttribute(btn_clean, "RASTERSIZE", "100x32");

    Ihandle *vbox_btns = IupVbox(
        btn_new, btn_edit, btn_browse, btn_del,
        IupFill(), // 间隔
        btn_undo, btn_redo,
        IupFill(),
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
    // 创建底部按钮
    btn_help = IupButton("帮助(H)", NULL);
    IupSetCallback(btn_help, "ACTION", (Icallback)btn_help_cb);
    IupSetAttribute(btn_help, "RASTERSIZE", "80x32");

    btn_theme = IupButton("深色模式", NULL);
    IupSetCallback(btn_theme, "ACTION", (Icallback)btn_theme_cb);
    IupSetAttribute(btn_theme, "RASTERSIZE", "80x32");

    lbl_status = IupLabel("就绪");
    IupSetAttribute(lbl_status, "EXPAND", "HORIZONTAL");

    btn_import = IupButton("导入配置", NULL);
    IupSetCallback(btn_import, "ACTION", (Icallback)btn_import_cb);
    IupSetAttribute(btn_import, "RASTERSIZE", "100x32");

    btn_export = IupButton("导出配置", NULL);
    IupSetCallback(btn_export, "ACTION", (Icallback)btn_export_cb);
    IupSetAttribute(btn_export, "RASTERSIZE", "100x32");

    btn_ok = IupButton("确定(O)", NULL);
    IupSetCallback(btn_ok, "ACTION", (Icallback)btn_ok_cb);
    IupSetAttribute(btn_ok, "RASTERSIZE", "100x32");

    btn_cancel = IupButton("取消(C)", NULL);
    IupSetCallback(btn_cancel, "ACTION", (Icallback)btn_cancel_cb);
    IupSetAttribute(btn_cancel, "RASTERSIZE", "100x32");

    Ihandle *hbox_bottom = IupHbox(
        btn_help,
        btn_theme,
        lbl_status,
        IupFill(),
        btn_import,
        btn_export,
        btn_ok,
        btn_cancel,
        NULL);
    IupSetAttribute(hbox_bottom, "GAP", "10");
    IupSetAttribute(hbox_bottom, "ALIGNMENT", "ACENTER");
    IupSetAttribute(hbox_bottom, "MARGIN", "0x0");

    return hbox_bottom;
}

// 创建主对话框
Ihandle *create_main_dialog()
{
    // 创建两个列表
    list_sys = create_path_list();
    list_user = create_path_list();
    list_merged = create_path_list();

    IupSetAttribute(list_merged, "READONLY", "YES");
    IupSetAttribute(list_merged, "MULTIPLE", "NO");
    IupSetAttribute(list_merged, "BGCOLOR", "240 240 240"); // 灰色背景

    // 创建标签页
    tabs_main = IupTabs(list_sys, list_user, list_merged, NULL);
    IupSetAttribute(tabs_main, "TABTITLE0", "系统变量 (System PATH)");
    IupSetAttribute(tabs_main, "TABTITLE1", "用户变量 (User PATH)");
    IupSetAttribute(tabs_main, "TABTITLE2", "合并预览 (Merged PATH)");

    // 设置标签页切换回调
    IupSetCallback(tabs_main, "TABCHANGEPOS_CB", (Icallback)tabs_tabchange_cb);

    // 搜索框
    txt_search = IupText(NULL);
    IupSetAttribute(txt_search, "NAME", "TXT_SEARCH");
    IupSetAttribute(txt_search, "CUEBANNER", "搜索...");
    IupSetCallback(txt_search, "VALUECHANGED_CB", (Icallback)txt_search_cb);
    IupSetAttribute(txt_search, "EXPAND", "HORIZONTAL");

    // 布局
    Ihandle *btns = create_main_buttons();
    Ihandle *hbox_mid = IupHbox(tabs_main, btns, NULL);
    IupSetAttribute(hbox_mid, "GAP", "10");
    IupSetAttribute(hbox_mid, "MARGIN", "0x0");

    Ihandle *bottom = create_bottom_buttons();

    Ihandle *vbox_main = IupVbox(txt_search, hbox_mid, bottom, NULL);
    IupSetAttribute(vbox_main, "GAP", "10");
    IupSetAttribute(vbox_main, "MARGIN", "10x10");

    Ihandle *dlg = IupDialog(vbox_main);
    IupSetAttribute(dlg, "TITLE", "Path Editor");
    IupSetAttribute(dlg, "SIZE", "480x400");

    return dlg;
}