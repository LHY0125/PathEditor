#include <windows.h>
#include <iup.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include "globals.h"
#include "utils.h"
#include "registry.h"
#include "callbacks.h"

// 全局变量定义
StringList raw_sys_paths = {0};
StringList raw_user_paths = {0};

// 全局控件定义
Ihandle *dlg, *tabs_main, *list_sys, *list_user, *lbl_status;   // 主窗口、标签页、系统路径列表、用户路径列表、状态标签
Ihandle *btn_new, *btn_edit, *btn_browse, *btn_del, *btn_up, *btn_down; // 右侧按钮
Ihandle *btn_ok, *btn_cancel, *btn_help; // 确认取消帮助按钮
Ihandle *btn_clean;     // 一键清理按钮
Ihandle *txt_search;    // 搜索框

// 辅助函数：创建列表控件
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

// 主函数
int main(int argc, char **argv)
{
    // 强制设置 UTF8MODE 环境变量，必须在 IupOpen 之前
    putenv("IUP_UTF8MODE=YES");

    IupOpen(&argc, &argv);
    IupSetGlobal("UTF8MODE", "YES");

    // 创建两个列表控件
    list_sys = create_path_list();
    list_user = create_path_list();

    // 创建搜索框
    txt_search = IupText(NULL);
    IupSetAttribute(txt_search, "EXPAND", "HORIZONTAL");
    IupSetAttribute(txt_search, "CUEBANNER", "输入关键词搜索...");
    IupSetCallback(txt_search, "VALUECHANGED_CB", (Icallback)txt_search_cb);

    // 创建 Tabs
    tabs_main = IupTabs(
        IupVbox(list_sys, NULL),
        IupVbox(list_user, NULL),
        NULL);
    IupSetAttribute(tabs_main, "TABTITLE0", "系统变量 (System)");
    IupSetAttribute(tabs_main, "TABTITLE1", "用户变量 (User)");
    IupSetAttribute(tabs_main, "TABTYPE", "TOP");

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

    // 上部布局：Tabs + 按钮
    Ihandle *hbox_main = IupHbox(tabs_main, vbox_btns, NULL);
    IupSetAttribute(hbox_main, "GAP", "10");
    IupSetAttribute(hbox_main, "MARGIN", "10x10");

    // 状态标签
    lbl_status = IupLabel("状态: 就绪");
    IupSetAttribute(lbl_status, "EXPAND", "HORIZONTAL");

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

    // 总体布局
    Ihandle *vbox_all = IupVbox(
        IupLabel("环境变量编辑器:"),
        txt_search,
        hbox_main,
        hbox_bottom,
        NULL);
    IupSetAttribute(vbox_all, "MARGIN", "10x10");
    IupSetAttribute(vbox_all, "GAP", "5");

    // 创建对话框
    dlg = IupDialog(vbox_all);
    IupSetAttribute(dlg, "TITLE", "编辑环境变量 (IUP版)");
    IupSetAttribute(dlg, "SIZE", "500x400"); // 稍微调大一点
    IupSetAttribute(dlg, "MINBOX", "NO");
    IupSetAttribute(dlg, "MAXBOX", "NO");

    // 加载数据
    if (!check_admin())
    {
        IupMessage("警告", "程序未以管理员身份运行，您只能查看，无法保存更改！");
        IupSetAttribute(dlg, "TITLE", "编辑环境变量 (只读模式)");
        IupSetAttribute(lbl_status, "TITLE", "状态: 只读模式 (权限不足)");

        // 禁用修改类按钮
        IupSetAttribute(btn_new, "ACTIVE", "NO");
        IupSetAttribute(btn_edit, "ACTIVE", "NO");
        IupSetAttribute(btn_browse, "ACTIVE", "NO");
        IupSetAttribute(btn_del, "ACTIVE", "NO");
        IupSetAttribute(btn_up, "ACTIVE", "NO");
        IupSetAttribute(btn_down, "ACTIVE", "NO");
        IupSetAttribute(btn_clean, "ACTIVE", "NO");
        IupSetAttribute(btn_ok, "ACTIVE", "NO");
    }

    IupShowXY(dlg, IUP_CENTER, IUP_CENTER);

    // IUP List APPEND 属性需要在控件 Map 之后才能生效
    // IupShowXY 会触发 Map
    load_all_paths();

    IupMainLoop();
    IupClose();
    return 0;
}