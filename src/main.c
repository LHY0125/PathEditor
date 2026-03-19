#include <iup.h>
#include <stdio.h>
#include <stdlib.h>
#include "globals.h"
#include "utils.h"
#include "registry.h"
#include "ui.h"
#include "cb_main.h"

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
    // 初始化 IUP
    if (IupOpen(&argc, &argv) == IUP_ERROR)
    {
        return 1;
    }

    // 开启 UTF-8 支持
    IupSetGlobal("UTF8MODE", "YES");

    // 启用 UIPI 绕过，允许拖拽
    HMODULE hUser32 = LoadLibraryA("user32.dll");
    if (hUser32)
    {
        typedef BOOL(WINAPI * ChangeWindowMessageFilterProc)(UINT, DWORD);
        ChangeWindowMessageFilterProc pChangeWindowMessageFilter = (ChangeWindowMessageFilterProc)GetProcAddress(hUser32, "ChangeWindowMessageFilter");
        if (pChangeWindowMessageFilter)
        {
            // WM_DROPFILES = 0x0233, WM_COPYDATA = 0x004A, MSGFLT_ADD = 1
            pChangeWindowMessageFilter(0x0233, 1);
            pChangeWindowMessageFilter(0x004A, 1);
        }
        FreeLibrary(hUser32);
    }

    // 初始化历史栈
    init_history_stack(&undo_stack);
    init_history_stack(&redo_stack);

    // 创建主界面
    dlg = create_main_dialog();

    // 设置全局按键回调 (如果在 ui.c 中未设置)
    IupSetCallback(dlg, "K_ANY", (Icallback)dialog_k_any_cb);

    // 加载数据
    if (!check_admin())
    {
        IupMessage("警告", "未检测到管理员权限！\n您可能无法保存更改。\n请右键以【管理员身份运行】。");
    }

    load_all_paths();

    // 显示对话框
    IupShowXY(dlg, IUP_CENTER, IUP_CENTER);

    // 进入主循环
    IupMainLoop();

    // 清理资源
    IupClose();
    return 0;
}