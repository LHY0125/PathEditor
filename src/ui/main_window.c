#include "ui/main_window.h"
#include "controller/callbacks.h"
#include "core/lua_config.h"
#include "utils/i18n.h"
#include "utils/ui_constants.h"
#include <stddef.h>
#include <string.h>

// 创建路径列表控件
static Ihandle *create_path_list(const char *name)
{
    Ihandle *list = IupFlatList();
    IupSetAttribute(list, "NAME", name);
    IupSetAttribute(list, "EXPAND", "YES");
    IupSetAttribute(list, "ITEMPADDING", lua_config_get_string("list", "item_padding"));
    IupSetAttribute(list, "BACKCOLOR", lua_config_get_string("list", "backcolor"));
    IupSetAttribute(list, "BORDER", "YES");
    IupSetAttribute(list, "CANFOCUS", "YES");
    IupSetAttribute(list, "HLINE", "NO");
    IupSetCallback(list, "DBLCLICK_CB", (Icallback)list_dblclick_cb);
    IupSetCallback(list, "DROPFILES_CB", (Icallback)list_dropfiles_cb);
    IupSetCallback(list, "K_ANY", (Icallback)list_k_any_cb);
    return list;
}

// 创建主窗口
Ihandle *create_main_window(void)
{
    // 创建系统路径列表
    Ihandle *list_sys = create_path_list(CTRL_LIST_SYS);
    // 创建用户路径列表
    Ihandle *list_user = create_path_list(CTRL_LIST_USER);

    // 创建搜索框
    Ihandle *txt_search = IupText(NULL);
    IupSetAttribute(txt_search, "NAME", CTRL_TXT_SEARCH);
    IupSetAttribute(txt_search, "EXPAND", "HORIZONTAL");
    IupSetAttribute(txt_search, "CUEBANNER", lua_config_get_string("label", "search_placeholder"));
    IupSetCallback(txt_search, "VALUECHANGED_CB", (Icallback)txt_search_cb);

    // 创建选项卡
    Ihandle *tabs_main = IupTabs(
        IupVbox(list_sys, NULL),
        IupVbox(list_user, NULL),
        NULL);
    IupSetAttribute(tabs_main, "NAME", CTRL_TABS_MAIN);
    IupSetAttribute(tabs_main, "TABTITLE0", _(lua_config_get_string("label", "tab_sys")));
    IupSetAttribute(tabs_main, "TABTITLE1", _(lua_config_get_string("label", "tab_user")));
    IupSetAttribute(tabs_main, "TABTYPE", "TOP");

    // 创建操作按钮
    Ihandle *btn_new = IupButton(_(lua_config_get_string("button", "new")), NULL);
    IupSetAttribute(btn_new, "NAME", CTRL_BTN_NEW);

    Ihandle *btn_edit = IupButton(_(lua_config_get_string("button", "edit")), NULL);
    IupSetAttribute(btn_edit, "NAME", CTRL_BTN_EDIT);

    Ihandle *btn_browse = IupButton(_(lua_config_get_string("button", "browse")), NULL);
    IupSetAttribute(btn_browse, "NAME", CTRL_BTN_BROWSE);

    Ihandle *btn_del = IupButton(_(lua_config_get_string("button", "del")), NULL);
    IupSetAttribute(btn_del, "NAME", CTRL_BTN_DEL);

    Ihandle *btn_up = IupButton(_(lua_config_get_string("button", "up")), NULL);
    IupSetAttribute(btn_up, "NAME", CTRL_BTN_UP);

    Ihandle *btn_down = IupButton(_(lua_config_get_string("button", "down")), NULL);
    IupSetAttribute(btn_down, "NAME", CTRL_BTN_DOWN);

    Ihandle *btn_clean = IupButton(_(lua_config_get_string("button", "clean")), NULL);
    IupSetAttribute(btn_clean, "NAME", CTRL_BTN_CLEAN);

    Ihandle *btn_import = IupButton(_(lua_config_get_string("button", "import")), NULL);
    IupSetAttribute(btn_import, "NAME", CTRL_BTN_IMPORT);

    Ihandle *btn_export = IupButton(_(lua_config_get_string("button", "export")), NULL);
    IupSetAttribute(btn_export, "NAME", CTRL_BTN_EXPORT);

    // 创建语言切换按钮
    Ihandle *btn_lang = IupButton(_("Language"), NULL);
    IupSetAttribute(btn_lang, "NAME", CTRL_BTN_LANG);
    IupSetCallback(btn_lang, "ACTION", (Icallback)btn_lang_cb);

    // 设置按钮回调
    IupSetCallback(btn_new, "ACTION", (Icallback)btn_new_cb);
    IupSetCallback(btn_edit, "ACTION", (Icallback)btn_edit_cb);
    IupSetCallback(btn_browse, "ACTION", (Icallback)btn_browse_cb);
    IupSetCallback(btn_del, "ACTION", (Icallback)btn_del_cb);
    IupSetCallback(btn_up, "ACTION", (Icallback)btn_up_cb);
    IupSetCallback(btn_down, "ACTION", (Icallback)btn_down_cb);
    IupSetCallback(btn_clean, "ACTION", (Icallback)btn_clean_cb);
    IupSetCallback(btn_import, "ACTION", (Icallback)btn_import_cb);
    IupSetCallback(btn_export, "ACTION", (Icallback)btn_export_cb);

    // 设置按钮大小
    const char *btn_size = lua_config_get_string("button", "rastersize");
    IupSetAttribute(btn_new, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_edit, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_browse, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_del, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_up, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_down, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_clean, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_import, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_export, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_lang, "RASTERSIZE", btn_size);

    // 创建操作按钮垂直布局
    Ihandle *vbox_btns = IupVbox(
        btn_new, btn_edit, btn_browse, btn_del,
        IupFill(),
        btn_clean,
        IupFill(),
        btn_import, btn_export,
        btn_up, btn_down,
        NULL);
    IupSetAttribute(vbox_btns, "GAP", lua_config_get_string("layout", "vbox_gap"));
    IupSetAttribute(vbox_btns, "MARGIN", lua_config_get_string("layout", "vbox_margin"));

    // 创建主窗口水平布局
    Ihandle *hbox_main = IupHbox(tabs_main, vbox_btns, NULL);
    IupSetAttribute(hbox_main, "GAP", lua_config_get_string("layout", "hbox_gap"));
    IupSetAttribute(hbox_main, "MARGIN", lua_config_get_string("layout", "hbox_margin"));

    // 创建状态标签
    Ihandle *lbl_status = IupLabel(lua_config_get_string("status", "normal"));
    IupSetAttribute(lbl_status, "NAME", CTRL_LBL_STATUS);
    IupSetAttribute(lbl_status, "EXPAND", "HORIZONTAL");

    // 创建底部按钮
    Ihandle *btn_ok = IupButton(_(lua_config_get_string("button", "ok")), NULL);
    IupSetAttribute(btn_ok, "NAME", CTRL_BTN_OK);
    Ihandle *btn_cancel = IupButton(_(lua_config_get_string("button", "cancel")), NULL);
    IupSetAttribute(btn_cancel, "NAME", CTRL_BTN_CANCEL);
    Ihandle *btn_help = IupButton(_(lua_config_get_string("button", "help")), NULL);
    IupSetAttribute(btn_help, "NAME", CTRL_BTN_HELP);

    // 设置底部按钮回调
    IupSetCallback(btn_ok, "ACTION", (Icallback)btn_ok_cb);
    IupSetCallback(btn_cancel, "ACTION", (Icallback)btn_cancel_cb);
    IupSetCallback(btn_help, "ACTION", (Icallback)btn_help_cb);

    // 设置底部按钮大小
    IupSetAttribute(btn_ok, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_cancel, "RASTERSIZE", btn_size);
    IupSetAttribute(btn_help, "RASTERSIZE", btn_size);

    // 创建底部按钮水平布局
    Ihandle *hbox_bottom = IupHbox(lbl_status, IupFill(), btn_help, btn_lang, btn_ok, btn_cancel, NULL);
    IupSetAttribute(hbox_bottom, "GAP", lua_config_get_string("layout", "hbox_gap"));
    IupSetAttribute(hbox_bottom, "MARGIN", lua_config_get_string("layout", "hbox_margin"));
    IupSetAttribute(hbox_bottom, "ALIGNMENT", lua_config_get_string("layout", "hbox_alignment"));

    // 创建主窗口垂直布局
    Ihandle *vbox_all = IupVbox(
        IupLabel(lua_config_get_string("label", "title")),
        txt_search,
        hbox_main,
        hbox_bottom,
        NULL);
    IupSetAttribute(vbox_all, "MARGIN", lua_config_get_string("layout", "vbox_all_margin"));
    IupSetAttribute(vbox_all, "GAP", lua_config_get_string("layout", "vbox_all_gap"));

    // 创建主窗口对话框
    Ihandle *dlg = IupDialog(vbox_all);
    IupSetAttribute(dlg, "TITLE", _(lua_config_get_string("app", "name")));
    IupSetAttribute(dlg, "RASTERSIZE", lua_config_get_string("dialog", "size"));
    IupSetAttribute(dlg, "MINSIZE", lua_config_get_string("dialog", "minsize"));
    IupSetAttribute(dlg, "MINBOX", "NO");
    IupSetAttribute(dlg, "MAXBOX", "NO");

    return dlg;
}

void refresh_main_window_ui(Ihandle *main_dlg)
{
    if (!main_dlg)
        return;

    // 设置窗口标题
    IupSetAttribute(main_dlg, "TITLE", _(lua_config_get_string("app", "name")));

    // 设置选项卡标题（需要设置在 Tabs 控件上，而非 Dialog）
    Ihandle *tabs = IupGetDialogChild(main_dlg, CTRL_TABS_MAIN);
    if (tabs)
    {
        IupSetAttribute(tabs, "TABTITLE0", _(lua_config_get_string("label", "tab_sys")));
        IupSetAttribute(tabs, "TABTITLE1", _(lua_config_get_string("label", "tab_user")));
    }

    // 辅助函数：设置子控件标题
    #define SET_CHILD_TITLE(btn_name, text_key) \
        do { \
            Ihandle *btn = IupGetDialogChild(main_dlg, btn_name); \
            if (btn) IupSetAttribute(btn, "TITLE", _(lua_config_get_string("button", text_key))); \
        } while(0)

    SET_CHILD_TITLE(CTRL_BTN_NEW, "new");
    SET_CHILD_TITLE(CTRL_BTN_EDIT, "edit");
    SET_CHILD_TITLE(CTRL_BTN_BROWSE, "browse");
    SET_CHILD_TITLE(CTRL_BTN_DEL, "del");
    SET_CHILD_TITLE(CTRL_BTN_UP, "up");
    SET_CHILD_TITLE(CTRL_BTN_DOWN, "down");
    SET_CHILD_TITLE(CTRL_BTN_CLEAN, "clean");
    SET_CHILD_TITLE(CTRL_BTN_IMPORT, "import");
    SET_CHILD_TITLE(CTRL_BTN_EXPORT, "export");
    SET_CHILD_TITLE(CTRL_BTN_OK, "ok");
    SET_CHILD_TITLE(CTRL_BTN_CANCEL, "cancel");
    SET_CHILD_TITLE(CTRL_BTN_HELP, "help");

    #undef SET_CHILD_TITLE

    // 语言切换按钮标题单独处理（不是从配置读取）
    Ihandle *btn_lang = IupGetDialogChild(main_dlg, CTRL_BTN_LANG);
    if (btn_lang)
        IupSetAttribute(btn_lang, "TITLE", _("Language"));
}