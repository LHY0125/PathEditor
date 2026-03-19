#include <windows.h>
#include <iup.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include "globals.h"
#include "utils.h"
#include "registry.h"
#include "callbacks.h"
#include "ui.h"
#include "config.h"

/*
!编译命令：
cmake -B build -G "MinGW Makefiles"
cmake --build build
!打包命令：
build_installer.bat
*/

// 主函数
int main(int argc, char **argv)
{
    // 强制设置 UTF8MODE 环境变量，必须在 IupOpen 之前
    putenv("IUP_UTF8MODE=YES");

    IupOpen(&argc, &argv);
    IupSetGlobal("UTF8MODE", "YES");

    // 在管理员模式下，解决无法拖拽文件到列表框的问题 (UIPI)
    // 需要加载 User32.dll 获取 ChangeWindowMessageFilter 函数
    HMODULE hUser32 = LoadLibraryW(L"user32.dll");
    if (hUser32)
    {
        typedef BOOL(WINAPI * ChangeWindowMessageFilterProc)(UINT, DWORD);
        ChangeWindowMessageFilterProc pChangeWindowMessageFilter =
            (ChangeWindowMessageFilterProc)GetProcAddress(hUser32, "ChangeWindowMessageFilter");

        if (pChangeWindowMessageFilter)
        {
            pChangeWindowMessageFilter(WM_DROPFILES, MSGFLT_ADD);
            pChangeWindowMessageFilter(WM_COPYDATA, MSGFLT_ADD);
            pChangeWindowMessageFilter(WM_COPYGLOBALDATA, MSGFLT_ADD);
        }
        FreeLibrary(hUser32);
    }

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

    // 创建右侧按钮区域
    Ihandle *vbox_btns = create_main_buttons();

    // 上部布局：Tabs + 按钮
    Ihandle *hbox_main = IupHbox(tabs_main, vbox_btns, NULL);
    IupSetAttribute(hbox_main, "GAP", UI_HBOX_GAP);
    IupSetAttribute(hbox_main, "MARGIN", UI_HBOX_MARGIN);

    // 状态标签
    lbl_status = IupLabel("状态: 就绪");
    IupSetAttribute(lbl_status, "EXPAND", "HORIZONTAL");

    // 创建底部按钮区域
    Ihandle *hbox_bottom = create_bottom_buttons();

    // 总体布局
    Ihandle *vbox_all = IupVbox(
        IupLabel("环境变量编辑器:"),
        txt_search,
        hbox_main,
        hbox_bottom,
        NULL);
    IupSetAttribute(vbox_all, "MARGIN", UI_VBOX_ALL_MARGIN);
    IupSetAttribute(vbox_all, "GAP", UI_VBOX_ALL_GAP);

    // 创建对话框
    dlg = IupDialog(vbox_all);
    IupSetAttribute(dlg, "TITLE", APP_NAME);                // 对话框标题
    IupSetAttribute(dlg, "RASTERSIZE", UI_DLG_SIZE);        // 对话框初始大小 (像素)
    IupSetAttribute(dlg, "MINSIZE", UI_DLG_MINSIZE);        // 对话框最小大小 (像素)
    IupSetAttribute(dlg, "MINBOX", "NO");
    IupSetAttribute(dlg, "MAXBOX", "NO");

    // 加载数据
    if (!check_admin())
    {
        IupMessage("警告", "程序未以管理员身份运行，您只能查看，无法保存更改！");
        IupSetAttribute(dlg, "TITLE", APP_NAME_READONLY);    // 对话框标题 (只读模式)
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