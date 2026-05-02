#include <windows.h>
#include <iup.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include <libintl.h>
#include "core/app_context.h"
#include "utils/ui_constants.h"
#include "core/lua_config.h"
#include "utils/string_ext.h"
#include "utils/os_env.h"
#include "utils/logger.h"
#include "utils/i18n.h"
#include "controller/callbacks.h"
#include "ui/main_window.h"
#include "ui/ui_utils.h"

// 需要在非管理员模式禁用的按钮列表
#define ADMIN_DISABLE_COUNT 10
static const char* ADMIN_DISABLE_BUTTONS[] = {
    CTRL_BTN_NEW, CTRL_BTN_EDIT, CTRL_BTN_BROWSE, CTRL_BTN_DEL,
    CTRL_BTN_UP, CTRL_BTN_DOWN, CTRL_BTN_CLEAN, CTRL_BTN_OK,
    CTRL_BTN_IMPORT, CTRL_BTN_EXPORT
};

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
    _wputenv_s(L"IUP_UTF8MODE", L"YES");

    IupOpen(&argc, &argv);
    IupSetGlobal("UTF8MODE", "YES");

    if (lua_config_init() != 0)
    {
        IupMessage(_("Warning"), "Lua 配置系统初始化失败，将使用默认值");
    }

    log_info("Lua config initialized");

    // 初始化国际化系统
    i18n_init(NULL);

    // 初始化深色模式（从配置文件加载）
    init_dark_mode();

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
        IupMessage(_("Error"), "无法分配内存创建应用上下文");
        IupClose();
        return 1;
    }

    Ihandle *dlg = create_main_window();

    // 如果深色模式已启用，应用深色主题
    if (get_dark_mode())
    {
        const char *dark_bg = lua_config_get_string("theme", "dark_bg");
        if (dark_bg && *dark_bg)
            IupSetAttribute(dlg, "BGCOLOR", dark_bg);

        // 更新深色模式按钮标题
        Ihandle *btn_darkmode = IupGetDialogChild(dlg, CTRL_BTN_DARKMODE);
        if (btn_darkmode)
            IupSetAttribute(btn_darkmode, "TITLE",
                _(lua_config_get_string("button", "lightmode")));
    }

    // 绑定上下文到对话框
    IupSetAttribute(dlg, "APP_CONTEXT", (char *)ctx);
    // 注册主窗口句柄，方便其他地方获取
    IupSetHandle("MAIN_DIALOG", dlg);

    // 检查管理员权限
    if (!check_admin())
    {
        const char *admin_msg = lua_config_get_string("status", "admin_warning");
        IupMessage(_("Warning"), admin_msg ? _(admin_msg) : "需要管理员权限才能编辑环境变量");

        // 设置只读状态提示
        Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
        if (lbl_status)
        {
            const char *readonly_msg = lua_config_get_string("status", "readonly");
            IupSetAttribute(lbl_status, "TITLE", readonly_msg ? _(readonly_msg) : "只读模式");
        }

        // 禁用所有需要管理员权限的按钮
        for (int i = 0; i < ADMIN_DISABLE_COUNT; i++)
        {
            Ihandle *btn = IupGetDialogChild(dlg, ADMIN_DISABLE_BUTTONS[i]);
            if (btn)
                IupSetAttribute(btn, "ACTIVE", "NO");
        }
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