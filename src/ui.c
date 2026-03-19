#include "ui.h"
#include <iupcontrols.h>
#include "config.h"
#include "globals.h"
#include "ui_utils.h"
#include "cb_edit.h"
#include "cb_file.h"
#include "cb_main.h"
#include <stdlib.h>

// 创建列表控件
Ihandle *create_path_list()
{
    Ihandle *list = IupMatrix(NULL);
    IupSetAttribute(list, "NUMCOL", "2");
    IupSetAttribute(list, "NUMLIN", "0");
    IupSetAttribute(list, "MARKMODE", "LIN");
    IupSetAttribute(list, "MARKMULTIPLE", "YES");
    IupSetAttribute(list, "READONLY", "NO");
    IupSetAttribute(list, "0:1", "路径");
    IupSetAttribute(list, "0:2", "状态");
    IupSetAttribute(list, "ALIGNMENT1", "ALEFT");
    IupSetAttribute(list, "ALIGNMENT2", "ACENTER");
    IupSetAttribute(list, "EXPAND", "YES");
    IupSetAttribute(list, "ITEMPADDING", UI_LIST_ITEM_PADDING);
    IupSetAttribute(list, "BACKCOLOR", UI_LIST_BGCOLOR);
    IupSetAttribute(list, "BORDER", "YES");
    IupSetAttribute(list, "CANFOCUS", "YES");
    IupSetCallback(list, "CLICK_CB", (Icallback)list_dblclick_cb);
    IupSetCallback(list, "EDITION_CB", (Icallback)matrix_edition_cb);
    IupSetCallback(list, "DROPFILES_CB", (Icallback)list_dropfiles_cb);
    IupSetCallback(list, "K_ANY", (Icallback)list_k_any_cb);
    IupSetCallback(list, "MOUSEMOVE_CB", (Icallback)list_motion_cb);
    return list;
}

// 创建右侧功能按钮区域
Ihandle *create_main_buttons()
{
    // 创建右侧按钮
    btn_new = IupButton("新建(N)", NULL);
    IupSetAttribute(btn_new, "IMAGE", "IUP_FileNew");
    IupSetAttribute(btn_new, "TITLE", "新建(N)");
    btn_edit = IupButton("编辑(E)", NULL);
    IupSetAttribute(btn_edit, "IMAGE", "IUP_EditCopy");
    IupSetAttribute(btn_edit, "TITLE", "编辑(E)");
    btn_browse = IupButton("浏览(B)...", NULL);
    IupSetAttribute(btn_browse, "IMAGE", "IUP_FileOpen");
    IupSetAttribute(btn_browse, "TITLE", "浏览(B)...");
    btn_del = IupButton("删除(D)", NULL);
    IupSetAttribute(btn_del, "IMAGE", "IUP_EditErase");
    IupSetAttribute(btn_del, "TITLE", "删除(D)");
    btn_undo = IupButton("撤销(Z)", NULL);
    IupSetAttribute(btn_undo, "IMAGE", "IUP_EditUndo");
    IupSetAttribute(btn_undo, "TITLE", "撤销(Z)");
    btn_redo = IupButton("重做(Y)", NULL);
    IupSetAttribute(btn_redo, "IMAGE", "IUP_EditRedo");
    IupSetAttribute(btn_redo, "TITLE", "重做(Y)");
    btn_up = IupButton("上移(U)", NULL);
    IupSetAttribute(btn_up, "IMAGE", "IUP_ArrowUp");
    IupSetAttribute(btn_up, "TITLE", "上移(U)");
    btn_down = IupButton("下移(O)", NULL);
    IupSetAttribute(btn_down, "IMAGE", "IUP_ArrowDown");
    IupSetAttribute(btn_down, "TITLE", "下移(O)");
    btn_clean = IupButton("一键清理", NULL);
    IupSetAttribute(btn_clean, "IMAGE", "IUP_ToolsSettings");
    IupSetAttribute(btn_clean, "TITLE", "一键清理");

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

    // 设置按钮大小
    IupSetAttribute(btn_new, "RASTERSIZE", UI_BUTTON_SIZE);
    IupSetAttribute(btn_edit, "RASTERSIZE", UI_BUTTON_SIZE);
    IupSetAttribute(btn_browse, "RASTERSIZE", UI_BUTTON_SIZE);
    IupSetAttribute(btn_del, "RASTERSIZE", UI_BUTTON_SIZE);
    IupSetAttribute(btn_undo, "RASTERSIZE", UI_BUTTON_SIZE);
    IupSetAttribute(btn_redo, "RASTERSIZE", UI_BUTTON_SIZE);
    IupSetAttribute(btn_up, "RASTERSIZE", UI_BUTTON_SIZE);
    IupSetAttribute(btn_down, "RASTERSIZE", UI_BUTTON_SIZE);
    IupSetAttribute(btn_clean, "RASTERSIZE", UI_BUTTON_SIZE);

    Ihandle *vbox_btns = IupVbox(
        btn_new, btn_edit, btn_browse, btn_del,
        IupFill(), // 间隔
        btn_undo, btn_redo,
        IupFill(),
        btn_clean, // 放在上移下移之前，或者最下面，这里放在中间偏下
        IupFill(),
        btn_up, btn_down,
        NULL);
    IupSetAttribute(vbox_btns, "GAP", UI_GAP_BUTTONS);
    IupSetAttribute(vbox_btns, "MARGIN", "0x0");

    return vbox_btns;
}

// 创建底部按钮区域
Ihandle *create_bottom_buttons()
{
    // 创建底部按钮
    btn_help = IupButton("帮助(H)", NULL);
    IupSetAttribute(btn_help, "IMAGE", "IUP_MessageInfo");
    IupSetAttribute(btn_help, "TITLE", "帮助(H)"); // Force title
    IupSetCallback(btn_help, "ACTION", (Icallback)btn_help_cb);
    IupSetAttribute(btn_help, "RASTERSIZE", UI_BUTTON_SMALL_SIZE);

    btn_theme = IupButton("深色模式", NULL);
    IupSetAttribute(btn_theme, "IMAGE", "IUP_WebFormat"); // Fixed icon
    IupSetAttribute(btn_theme, "TITLE", "深色模式");      // Force title
    IupSetCallback(btn_theme, "ACTION", (Icallback)btn_theme_cb);
    IupSetAttribute(btn_theme, "RASTERSIZE", UI_BUTTON_SMALL_SIZE);

    lbl_status = IupLabel("就绪");
    IupSetAttribute(lbl_status, "EXPAND", "HORIZONTAL");

    btn_import = IupButton("导入配置", NULL);
    IupSetAttribute(btn_import, "IMAGE", "IUP_FileOpen");
    IupSetAttribute(btn_import, "TITLE", "导入配置"); // Force title
    IupSetCallback(btn_import, "ACTION", (Icallback)btn_import_cb);
    IupSetAttribute(btn_import, "RASTERSIZE", UI_BUTTON_SIZE);

    btn_export = IupButton("导出配置", NULL);
    IupSetAttribute(btn_export, "IMAGE", "IUP_FileSave"); // Fixed icon
    IupSetAttribute(btn_export, "TITLE", "导出配置");     // Force title
    IupSetCallback(btn_export, "ACTION", (Icallback)btn_export_cb);
    IupSetAttribute(btn_export, "RASTERSIZE", UI_BUTTON_SIZE);

    btn_ok = IupButton("确定(O)", NULL);
    IupSetAttribute(btn_ok, "IMAGE", "IUP_FileSave");
    IupSetAttribute(btn_ok, "TITLE", "确定(O)"); // Force title
    IupSetCallback(btn_ok, "ACTION", (Icallback)btn_ok_cb);
    IupSetAttribute(btn_ok, "RASTERSIZE", UI_BUTTON_SIZE);

    btn_cancel = IupButton("取消(C)", NULL);
    IupSetAttribute(btn_cancel, "IMAGE", "IUP_MessageError");
    IupSetAttribute(btn_cancel, "TITLE", "取消(C)"); // Force title
    IupSetCallback(btn_cancel, "ACTION", (Icallback)btn_cancel_cb);
    IupSetAttribute(btn_cancel, "RASTERSIZE", UI_BUTTON_SIZE);

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
    IupSetAttribute(hbox_bottom, "GAP", UI_GAP_BOTTOM);
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
    IupSetAttribute(list_merged, "MARKMULTIPLE", "NO");
    IupSetAttribute(list_merged, "BGCOLOR", UI_LIST_MERGED_BGCOLOR); // 灰色背景

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
    IupSetAttribute(hbox_mid, "GAP", UI_GAP_MAIN);
    IupSetAttribute(hbox_mid, "MARGIN", "0x0");

    Ihandle *bottom = create_bottom_buttons();

    Ihandle *vbox_main = IupVbox(txt_search, hbox_mid, bottom, NULL);
    IupSetAttribute(vbox_main, "GAP", UI_GAP_MAIN);
    IupSetAttribute(vbox_main, "MARGIN", UI_MARGIN_MAIN);

    Ihandle *dlg = IupDialog(vbox_main);
    IupSetAttribute(dlg, "TITLE", UI_WINDOW_TITLE);
    IupSetAttribute(dlg, "RASTERSIZE", UI_WINDOW_SIZE);
    IupSetAttribute(dlg, "MINSIZE", UI_WINDOW_SIZE);

    return dlg;
}