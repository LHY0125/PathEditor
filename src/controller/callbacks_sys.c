#include "controller/callbacks.h"
#include "controller/callbacks_internal.h"
#include "core/app_context.h"
#include "core/registry_service.h"
#include "core/lua_config.h"
#include "utils/error_code.h"
#include "utils/os_env.h"
#include "utils/logger.h"
#include "utils/i18n.h"
#include "utils/ui_constants.h"
#include "ui/ui_utils.h"
#include "ui/dialogs.h"
#include "ui/main_window.h"
#include <stdio.h>
#include <windows.h>

// 按钮回调：确定 (保存所有)
int btn_ok_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx)
        return IUP_DEFAULT;

    if (!check_admin())
    {
        IupMessage("错误", "需要管理员权限才能保存更改！");
        return IUP_DEFAULT;
    }

    backup_registry();

    ErrorCode sys_ok = save_system_paths(&ctx->sys_paths);
    ErrorCode user_ok = save_user_paths(&ctx->user_paths);

    Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);

    if (sys_ok == ERR_OK && user_ok == ERR_OK)
    {
        log_info("Saved system paths: %d, user paths: %d", ctx->sys_paths.count, ctx->user_paths.count);
        SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, (LPARAM)L"Environment", SMTO_ABORTIFHUNG, 5000, NULL);
        IupMessage("成功", "系统和用户 PATH 环境变量均已更新！");
        if (lbl_status)
            IupSetAttribute(lbl_status, "TITLE", lua_config_get_string("status", "saved"));
    }
    else if (sys_ok == ERR_OK)
    {
        IupMessage("提示", "系统变量保存成功，但用户变量保存失败。");
    }
    else if (user_ok == ERR_OK)
    {
        IupMessage("提示", "用户变量保存成功，但系统变量保存失败。");
    }
    else
    {
        log_error("Failed to save paths: sys=%d, user=%d", sys_ok, user_ok);
        IupMessage("错误", "保存失败！");
        if (lbl_status)
            IupSetAttribute(lbl_status, "TITLE", lua_config_get_string("status", "error"));
    }
    return IUP_DEFAULT;
}

// 按钮回调：取消
int btn_cancel_cb(Ihandle *self)
{
    IupExitLoop();
    return IUP_DEFAULT;
}

// 载入所有路径
void load_all_paths(void)
{
    Ihandle *dlg = get_main_dlg();
    if (!dlg)
        return;
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx)
        return;

    if (load_system_paths(&ctx->sys_paths) != ERR_OK)
    {
        log_error("Failed to load system paths");
        IupMessage("错误", "无法打开系统环境变量注册表键，请尝试以管理员身份运行。");
    }
    else
    {
        log_info("Loaded system paths: %d", ctx->sys_paths.count);
    }

    ErrorCode user_result = load_user_paths(&ctx->user_paths);
    if (user_result == ERR_OK)
    {
        log_info("Loaded user paths: %d", ctx->user_paths.count);
    }

    Ihandle *list_sys = IupGetDialogChild(dlg, CTRL_LIST_SYS);
    Ihandle *list_user = IupGetDialogChild(dlg, CTRL_LIST_USER);

    sync_string_list_to_ui(list_sys, &ctx->sys_paths);
    sync_string_list_to_ui(list_user, &ctx->user_paths);

    Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
    if (lbl_status)
        IupSetAttribute(lbl_status, "TITLE", lua_config_get_string("status", "loaded"));
}

// 按钮回调：语言切换
int btn_lang_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    language_select_dialog();
    refresh_main_window_ui(dlg);
    return IUP_DEFAULT;
}

// 按钮回调：帮助
int btn_help_cb(Ihandle *self)
{
    IupMessage("使用说明",
               "1. 本程序用于编辑系统环境变量 PATH。\n"
               "2. 必须以【管理员身份】运行才能保存更改。\n"
               "3. 操作说明：\n"
               "   - 新建：添加新路径到列表末尾。\n"
               "   - 编辑：修改选中的路径。\n"
               "   - 浏览：从文件系统选择目录添加。\n"
               "   - 删除：移除选中的路径。\n"
               "   - 上移/下移：调整路径优先级。\n"
               "   - 导入/导出：备份和恢复 PATH 配置。\n"
               "4. 点击【确定】保存更改并生效。\n"
               "5. 注意：某些正在运行的程序可能需要重启才能识别新的环境变量。\n\n"
               "--------------------------------------------------\n"
               "作者：LHY\n"
               "邮箱：3364451258@qq.com\n"
               "GitHub：https://github.com/LHY0125/PathEditor\n"
               "记得给我的项目点个star！");

    return IUP_DEFAULT;
}
