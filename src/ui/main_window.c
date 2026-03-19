#include "ui/main_window.h"
#include "controller/callbacks.h"
#include "config.h"
#include <stddef.h>

// 创建路径列表控件
static Ihandle *create_path_list(const char *name)
{
    Ihandle *list = IupFlatList();
    IupSetAttribute(list, "NAME", name);
    IupSetAttribute(list, "EXPAND", "YES");
    IupSetAttribute(list, "ITEMPADDING", UI_LIST_ITEM_PADDING);
    IupSetAttribute(list, "BACKCOLOR", UI_LIST_BACKCOLOR);
    IupSetAttribute(list, "BORDER", "YES");
    IupSetAttribute(list, "CANFOCUS", "YES");
    IupSetAttribute(list, "HLINE", "NO");
    IupSetCallback(list, "DBLCLICK_CB", (Icallback)list_dblclick_cb);
    IupSetCallback(list, "DROPFILES_CB", (Icallback)list_dropfiles_cb);
    IupSetCallback(list, "K_ANY", (Icallback)list_k_any_cb);
    return list;
}

// 创建主窗口
Ihandle* create_main_window(void)
{
    // 创建系统路径列表
    Ihandle *list_sys = create_path_list("LIST_SYS");
    // 创建用户路径列表
    Ihandle *list_user = create_path_list("LIST_USER");

    // 创建搜索框
    Ihandle *txt_search = IupText(NULL);
    IupSetAttribute(txt_search, "NAME", "TXT_SEARCH");
    IupSetAttribute(txt_search, "EXPAND", "HORIZONTAL");
    IupSetAttribute(txt_search, "CUEBANNER", "输入关键词搜索...");
    IupSetCallback(txt_search, "VALUECHANGED_CB", (Icallback)txt_search_cb);

    // 创建选项卡
    Ihandle *tabs_main = IupTabs(
        IupVbox(list_sys, NULL),
        IupVbox(list_user, NULL),
        NULL);
    IupSetAttribute(tabs_main, "NAME", "TABS_MAIN");
    IupSetAttribute(tabs_main, "TABTITLE0", "系统变量 (System)");
    IupSetAttribute(tabs_main, "TABTITLE1", "用户变量 (User)");
    IupSetAttribute(tabs_main, "TABTYPE", "TOP");

    // 创建操作按钮
    Ihandle *btn_new = IupButton("新建(N)", NULL);
    IupSetAttribute(btn_new, "NAME", "BTN_NEW");
    Ihandle *btn_edit = IupButton("编辑(E)", NULL);
    IupSetAttribute(btn_edit, "NAME", "BTN_EDIT");
    Ihandle *btn_browse = IupButton("浏览(B)...", NULL);
    IupSetAttribute(btn_browse, "NAME", "BTN_BROWSE");
    Ihandle *btn_del = IupButton("删除(D)", NULL);
    IupSetAttribute(btn_del, "NAME", "BTN_DEL");
    Ihandle *btn_up = IupButton("上移(U)", NULL);
    IupSetAttribute(btn_up, "NAME", "BTN_UP");
    Ihandle *btn_down = IupButton("下移(O)", NULL);
    IupSetAttribute(btn_down, "NAME", "BTN_DOWN");
    Ihandle *btn_clean = IupButton("一键清理", NULL);
    IupSetAttribute(btn_clean, "NAME", "BTN_CLEAN");

    // 设置按钮回调
    IupSetCallback(btn_new, "ACTION", (Icallback)btn_new_cb);
    IupSetCallback(btn_edit, "ACTION", (Icallback)btn_edit_cb);
    IupSetCallback(btn_browse, "ACTION", (Icallback)btn_browse_cb);
    IupSetCallback(btn_del, "ACTION", (Icallback)btn_del_cb);
    IupSetCallback(btn_up, "ACTION", (Icallback)btn_up_cb);
    IupSetCallback(btn_down, "ACTION", (Icallback)btn_down_cb);
    IupSetCallback(btn_clean, "ACTION", (Icallback)btn_clean_cb);

    // 设置按钮大小
    IupSetAttribute(btn_new, "RASTERSIZE", UI_BTN_RASTERSIZE);
    IupSetAttribute(btn_edit, "RASTERSIZE", UI_BTN_RASTERSIZE);
    IupSetAttribute(btn_browse, "RASTERSIZE", UI_BTN_RASTERSIZE);
    IupSetAttribute(btn_del, "RASTERSIZE", UI_BTN_RASTERSIZE);
    IupSetAttribute(btn_up, "RASTERSIZE", UI_BTN_RASTERSIZE);
    IupSetAttribute(btn_down, "RASTERSIZE", UI_BTN_RASTERSIZE);
    IupSetAttribute(btn_clean, "RASTERSIZE", UI_BTN_RASTERSIZE);

    // 创建操作按钮垂直布局
    Ihandle *vbox_btns = IupVbox(
        btn_new, btn_edit, btn_browse, btn_del,
        IupFill(),
        btn_clean,
        IupFill(),
        btn_up, btn_down,
        NULL);
    IupSetAttribute(vbox_btns, "GAP", UI_VBOX_GAP);
    IupSetAttribute(vbox_btns, "MARGIN", UI_VBOX_MARGIN);

    // 创建主窗口水平布局
    Ihandle *hbox_main = IupHbox(tabs_main, vbox_btns, NULL);
    IupSetAttribute(hbox_main, "GAP", UI_HBOX_GAP);
    IupSetAttribute(hbox_main, "MARGIN", UI_HBOX_MARGIN);

    // 创建状态标签
    Ihandle *lbl_status = IupLabel("状态: 就绪");
    IupSetAttribute(lbl_status, "NAME", "LBL_STATUS");
    IupSetAttribute(lbl_status, "EXPAND", "HORIZONTAL");

    // 创建底部按钮
    Ihandle *btn_ok = IupButton("确定", NULL);
    IupSetAttribute(btn_ok, "NAME", "BTN_OK");
    Ihandle *btn_cancel = IupButton("取消", NULL);
    IupSetAttribute(btn_cancel, "NAME", "BTN_CANCEL");
    Ihandle *btn_help = IupButton("帮助(?)", NULL);
    IupSetAttribute(btn_help, "NAME", "BTN_HELP");

    // 设置底部按钮回调
    IupSetCallback(btn_ok, "ACTION", (Icallback)btn_ok_cb);
    IupSetCallback(btn_cancel, "ACTION", (Icallback)btn_cancel_cb);
    IupSetCallback(btn_help, "ACTION", (Icallback)btn_help_cb);

    // 设置底部按钮大小
    IupSetAttribute(btn_ok, "RASTERSIZE", UI_BTN_RASTERSIZE);
    IupSetAttribute(btn_cancel, "RASTERSIZE", UI_BTN_RASTERSIZE);
    IupSetAttribute(btn_help, "RASTERSIZE", UI_BTN_RASTERSIZE);

    // 创建底部按钮水平布局
    Ihandle *hbox_bottom = IupHbox(lbl_status, IupFill(), btn_help, btn_ok, btn_cancel, NULL);
    IupSetAttribute(hbox_bottom, "GAP", UI_HBOX_GAP);
    IupSetAttribute(hbox_bottom, "MARGIN", UI_HBOX_MARGIN);
    IupSetAttribute(hbox_bottom, "ALIGNMENT", UI_HBOX_ALIGNMENT);

    // 创建主窗口垂直布局
    Ihandle *vbox_all = IupVbox(
        IupLabel("环境变量编辑器:"),
        txt_search,
        hbox_main,
        hbox_bottom,
        NULL);
    IupSetAttribute(vbox_all, "MARGIN", UI_VBOX_ALL_MARGIN);
    IupSetAttribute(vbox_all, "GAP", UI_VBOX_ALL_GAP);

    // 创建主窗口对话框
    Ihandle *dlg = IupDialog(vbox_all);
    IupSetAttribute(dlg, "TITLE", APP_NAME);
    IupSetAttribute(dlg, "RASTERSIZE", UI_DLG_SIZE);
    IupSetAttribute(dlg, "MINSIZE", UI_DLG_MINSIZE);
    IupSetAttribute(dlg, "MINBOX", "NO");
    IupSetAttribute(dlg, "MAXBOX", "NO");

    return dlg;
}