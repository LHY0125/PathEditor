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
        IupMessage(_("Error"), _("Administrator privileges are required to save changes!"));
        return IUP_DEFAULT;
    }

    // 询问用户是否自定义备份目录
    char custom_backup_dir[MAX_PATH] = "";
    int do_backup = 1;  // 是否执行备份

    int backup_choice = IupAlarm(_("Backup Settings"),
                                  _("Would you like to customize the backup directory?\n\n"
                                    "Select 'Use Default' to backup to %%APPDATA%%/PathEditor/backups/\n"
                                    "Select 'Custom Directory' to choose another location"),
                                  _("Use Default"), _("Custom Directory"), _("Skip Backup"));

    if (backup_choice == 2)  // 自定义目录
    {
        Ihandle *filedlg = IupFileDlg();
        IupSetAttribute(filedlg, "DIALOGTYPE", "DIR");
        IupSetAttribute(filedlg, "TITLE", "选择备份目录");

        IupPopup(filedlg, IUP_CENTER, IUP_CENTER);

        if (IupGetInt(filedlg, "STATUS") != -1)
        {
            char *value = IupGetAttribute(filedlg, "VALUE");
            if (value)
                strncpy(custom_backup_dir, value, MAX_PATH - 1);
        }
        IupDestroy(filedlg);

        if (strlen(custom_backup_dir) == 0)
        {
            IupMessage(_("Hint"), _("No directory selected, will use default backup path."));
        }
    }
    else if (backup_choice == 3)  // 跳过备份
    {
        int skip_confirm = IupAlarm(_("Confirm"), _("Are you sure you want to skip backup?\nSkipping backup may cause inability to recover!"),
                                    _("Skip Anyway"), _("Go Back"), NULL);
        if (skip_confirm != 1)
        {
            // 用户反悔，重新询问
            return btn_ok_cb(self);
        }
        do_backup = 0;
    }

    // 执行备份（如果用户没有跳过）
    if (do_backup)
    {
        const char *backup_path = strlen(custom_backup_dir) > 0 ? custom_backup_dir : NULL;
        ErrorCode backup_result = backup_registry(backup_path);
        if (backup_result != ERR_OK)
        {
            log_error("Backup failed: error code %d", backup_result);
            const char *reason = "未知错误";
            if (backup_result == ERR_FAILED)
                reason = "无法获取 AppData 路径";
            else if (backup_result == ERR_FILE_NOT_FOUND)
                reason = "无法创建备份目录或文件";
            else if (backup_result == ERR_REGISTRY_FAILED)
                reason = "无法读取注册表中的 PATH 值";

            char msg[512];
            snprintf(msg, sizeof(msg), "备份失败！原因：%s\n\n是否继续保存？\n（继续保存可能导致无法恢复）", reason);
            int choice = IupAlarm(_("Warning"), msg, _("Continue Saving"), _("Cancel"), NULL);
            if (choice != 1)
                return IUP_DEFAULT;
        }
    }

    ErrorCode sys_ok = save_system_paths(&ctx->sys_paths);
    ErrorCode user_ok = save_user_paths(&ctx->user_paths);

    Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);

    if (sys_ok == ERR_OK && user_ok == ERR_OK)
    {
        log_info("Saved system paths: %d, user paths: %d", ctx->sys_paths.count, ctx->user_paths.count);
        SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, (LPARAM)L"Environment", SMTO_ABORTIFHUNG, 5000, NULL);
        IupMessage(_("Success"), _("Both system and user PATH environment variables have been updated!"));
        if (lbl_status)
            IupSetAttribute(lbl_status, "TITLE", lua_config_get_string("status", "saved"));
    }
    else if (sys_ok == ERR_OK)
    {
        IupMessage(_("Info"), _("System variables saved successfully, but user variables failed to save."));
    }
    else if (user_ok == ERR_OK)
    {
        IupMessage(_("Info"), _("User variables saved successfully, but system variables failed to save."));
    }
    else
    {
        log_error("Failed to save paths: sys=%d, user=%d", sys_ok, user_ok);
        IupMessage(_("Error"), _("Failed to save!"));
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
        IupMessage(_("Error"), _("Unable to open system environment variable registry key, please try running as administrator."));
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
    IupMessage(_("Usage Instructions"),
               _("1. This program is used to edit system environment variable PATH.\n"
                 "2. Must run as 【Administrator】 to save changes.\n"
                 "3. Operations:\n"
                 "   - New: Add new path to end of list.\n"
                 "   - Edit: Modify selected path.\n"
                 "   - Browse: Select directory from file system to add.\n"
                 "   - Delete: Remove selected path.\n"
                 "   - Up/Down: Adjust path priority.\n"
                 "   - Import/Export: Backup and restore PATH configuration.\n"
                 "4. Click 【OK】 to save changes and apply.\n"
                 "5. Note: Some running programs may need to restart to recognize new environment variables.\n\n"
                 "--------------------------------------------------\n"
                 "Author: LHY\n"
                 "Email: 3364451258@qq.com\n"
                 "GitHub: https://github.com/LHY0125/PathEditor\n"
                 "Don't forget to star my project!"));

    return IUP_DEFAULT;
}

// 对话框全局快捷键回调
int dlg_k_any_cb(Ihandle *self, int c)
{
    if (c == K_cN)  // Ctrl+N 新建
    {
        btn_new_cb(self);
        return IUP_IGNORE;
    }
    if (c == K_cS)  // Ctrl+S 保存
    {
        btn_ok_cb(self);
        return IUP_IGNORE;
    }
    if (c == K_cF)  // Ctrl+F 聚焦搜索框
    {
        Ihandle *txt_search = IupGetDialogChild(self, CTRL_TXT_SEARCH);
        if (txt_search)
            IupSetFocus(txt_search);
        return IUP_IGNORE;
    }
    return IUP_DEFAULT;
}
