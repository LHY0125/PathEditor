#include <windows.h>
#include <iup.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include "core/app_context.h"
#include "core/lua_config.h"
#include "utils/string_ext.h"
#include "utils/os_env.h"
#include "utils/logger.h"
#include "controller/callbacks.h"
#include "ui/main_window.h"

/*
!编译命令：
cmake -B build -G "MinGW Makefiles"
cmake --build build
!打包命令：
build_installer.bat
!运行命令：
powershell -Command "Start-Process 'build\\PathEditor.exe' -Verb RunAs"
*/

// 定义 Windows 消息常量
#ifndef WM_COPYGLOBALDATA
#define WM_COPYGLOBALDATA 0x0049
#endif

// 消息过滤器常量
#ifndef MSGFLT_ADD
#define MSGFLT_ADD 1
#endif

// 主函数
int main(int argc, char **argv)
{
    // 初始化日志系统
    log_init(NULL, LOG_LEVEL_INFO);
    log_info("PathEditor starting...");

    // 强制设置 UTF8MODE 环境变量，必须在 IupOpen 之前
    putenv("IUP_UTF8MODE=YES");

    IupOpen(&argc, &argv);
    IupSetGlobal("UTF8MODE", "YES");

    if (lua_config_init() != 0)
    {
        IupMessage("警告", "Lua 配置系统初始化失败，将使用默认值");
    }

    log_info("Lua config initialized");

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

    // 禁用默认的全局按键处理
    IupSetGlobal("INPUTCALLBACKS", "NO");

    // 创建应用上下文
    AppContext *ctx = create_app_context();
    if (!ctx)
    {
        IupMessage("错误", "无法分配内存创建应用上下文");
        IupClose();
        return 1;
    }

    Ihandle *dlg = create_main_window();

    // 绑定上下文到对话框
    IupSetAttribute(dlg, "APP_CONTEXT", (char *)ctx);
    // 注册主窗口句柄，方便其他地方获取
    IupSetHandle("MAIN_DIALOG", dlg);

    // 检查管理员权限
    if (!check_admin())
    {
        IupMessage("警告", lua_config_get_string("status", "admin_warning"));

        Ihandle *lbl_status = IupGetDialogChild(dlg, "LBL_STATUS");
        if (lbl_status)
            IupSetAttribute(lbl_status, "TITLE", lua_config_get_string("status", "readonly"));

        Ihandle *btn_new = IupGetDialogChild(dlg, "BTN_NEW");
        Ihandle *btn_edit = IupGetDialogChild(dlg, "BTN_EDIT");
        Ihandle *btn_browse = IupGetDialogChild(dlg, "BTN_BROWSE");
        Ihandle *btn_del = IupGetDialogChild(dlg, "BTN_DEL");
        Ihandle *btn_up = IupGetDialogChild(dlg, "BTN_UP");
        Ihandle *btn_down = IupGetDialogChild(dlg, "BTN_DOWN");
        Ihandle *btn_clean = IupGetDialogChild(dlg, "BTN_CLEAN");
        Ihandle *btn_ok = IupGetDialogChild(dlg, "BTN_OK");

        if (btn_new)
            IupSetAttribute(btn_new, "ACTIVE", "NO");
        if (btn_edit)
            IupSetAttribute(btn_edit, "ACTIVE", "NO");
        if (btn_browse)
            IupSetAttribute(btn_browse, "ACTIVE", "NO");
        if (btn_del)
            IupSetAttribute(btn_del, "ACTIVE", "NO");
        if (btn_up)
            IupSetAttribute(btn_up, "ACTIVE", "NO");
        if (btn_down)
            IupSetAttribute(btn_down, "ACTIVE", "NO");
        if (btn_clean)
            IupSetAttribute(btn_clean, "ACTIVE", "NO");
        if (btn_ok)
            IupSetAttribute(btn_ok, "ACTIVE", "NO");

        Ihandle *btn_import = IupGetDialogChild(dlg, "BTN_IMPORT");
        Ihandle *btn_export = IupGetDialogChild(dlg, "BTN_EXPORT");
        if (btn_import)
            IupSetAttribute(btn_import, "ACTIVE", "NO");
        if (btn_export)
            IupSetAttribute(btn_export, "ACTIVE", "NO");
    }

    IupShowXY(dlg, IUP_CENTER, IUP_CENTER);

    // IUP List APPEND 属性需要在控件 Map 之后才能生效
    // IupShowXY 会触发 Map
    load_all_paths();

    IupMainLoop();

    log_info("PathEditor exiting...");
    destroy_app_context(ctx);
    lua_config_destroy();
    log_destroy();
    IupClose();

    return 0;
}