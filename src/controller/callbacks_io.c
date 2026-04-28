#include "controller/callbacks.h"
#include "controller/callbacks_internal.h"
#include "core/app_context.h"
#include "core/import_export.h"
#include "core/lua_config.h"
#include "utils/string_ext.h"
#include "utils/safe_string.h"
#include "utils/error_code.h"
#include "utils/os_env.h"
#include "utils/logger.h"
#include "utils/ui_constants.h"
#include "ui/ui_utils.h"
#include <string.h>
#include <stdio.h>
#include <windows.h>

// 按钮回调：导入
int btn_import_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx)
        return IUP_DEFAULT;

    if (!check_admin())
    {
        IupMessage("错误", "需要管理员权限才能导入 PATH！");
        return IUP_DEFAULT;
    }

    Ihandle *filedlg = IupFileDlg();
    IupSetAttribute(filedlg, "DIALOGTYPE", "OPEN");
    IupSetAttribute(filedlg, "TITLE", lua_config_get_string("label", "import_title"));
    IupSetAttribute(filedlg, "FILTER", "json");
    IupSetAttribute(filedlg, "EXTFILTER", "JSON 文件 (*.json)|*.json|文本文件 (*.txt)|*.txt|所有文件 (*.*)|*.*");

    IupPopup(filedlg, IUP_CENTER, IUP_CENTER);

    if (IupGetInt(filedlg, "STATUS") != -1)
    {
        char *filepath = IupGetAttribute(filedlg, "VALUE");
        if (filepath)
        {
            ExportData imported;
            ErrorCode import_result = import_paths_from_file(filepath, &imported);
            if (import_result == ERR_OK)
            {
                int has_system = imported.system.count > 0;
                int has_user = imported.user.count > 0;

                if (!has_system && !has_user)
                {
                    IupMessage("错误", "文件中没有找到有效的路径！");
                    IupDestroy(filedlg);
                    return IUP_DEFAULT;
                }

                int choice = 0;
                if (has_system && has_user)
                {
                    choice = IupAlarm("导入选项", "请选择导入目标：",
                                      "仅系统变量", "仅用户变量", "全部导入");
                }
                else if (has_system)
                {
                    choice = 3;
                }
                else
                {
                    choice = 2;
                }

                int total_imported = 0;

                if (choice == 1 || choice == 3)
                {
                    clear_string_list(&ctx->sys_paths);
                    for (int i = 0; i < imported.system.count; i++)
                    {
                        add_string_list(&ctx->sys_paths, imported.system.items[i]);
                    }
                    Ihandle *list_sys = IupGetDialogChild(dlg, CTRL_LIST_SYS);
                    sync_string_list_to_ui(list_sys, &ctx->sys_paths);
                    total_imported += imported.system.count;
                }

                if (choice == 2 || choice == 3)
                {
                    clear_string_list(&ctx->user_paths);
                    for (int i = 0; i < imported.user.count; i++)
                    {
                        add_string_list(&ctx->user_paths, imported.user.items[i]);
                    }
                    Ihandle *list_user = IupGetDialogChild(dlg, CTRL_LIST_USER);
                    sync_string_list_to_ui(list_user, &ctx->user_paths);
                    total_imported += imported.user.count;
                }

                char msg[256];
                snprintf(msg, sizeof(msg), "成功导入 %d 个路径！", total_imported);
                IupMessage("导入成功", msg);

                Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
                if (lbl_status)
                    IupSetAttribute(lbl_status, "TITLE", lua_config_get_string("status", "loaded"));
            }
            else
            {
                log_error("Import failed: error code %d", import_result);
                IupMessage("错误", "导入失败，请检查文件格式是否正确！");
            }
        }
    }
    IupDestroy(filedlg);
    return IUP_DEFAULT;
}

// 按钮回调：导出
int btn_export_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx)
        return IUP_DEFAULT;

    ExportData data;
    data.system = ctx->sys_paths;
    data.user = ctx->user_paths;

    Ihandle *filedlg = IupFileDlg();
    IupSetAttribute(filedlg, "DIALOGTYPE", "SAVE");
    IupSetAttribute(filedlg, "TITLE", lua_config_get_string("label", "export_title"));
    IupSetAttribute(filedlg, "FILTER", "json");
    IupSetAttribute(filedlg, "EXTFILTER", "JSON 文件 (*.json)|*.json");
    IupSetAttribute(filedlg, "DEFAULTEXT", "json");

    char default_name[64];
    snprintf(default_name, sizeof(default_name), "path_all.json");
    IupSetAttribute(filedlg, "VALUE", default_name);

    IupPopup(filedlg, IUP_CENTER, IUP_CENTER);

    if (IupGetInt(filedlg, "STATUS") != -1)
    {
        char *filepath = IupGetAttribute(filedlg, "VALUE");
        if (filepath)
        {
            char final_path[MAX_PATH];
            if (strchr(filepath, '.') == NULL)
            {
                snprintf(final_path, sizeof(final_path), "%s.json", filepath);
            }
            else
            {
                safe_strcpy(final_path, sizeof(final_path), filepath);
            }
            filepath = final_path;

            ErrorCode export_result = export_paths_to_file(&data, filepath);
            if (export_result == ERR_OK)
            {
                char msg[512];
                snprintf(msg, sizeof(msg), "成功导出！\n系统变量: %d 个\n用户变量: %d 个\n\n保存位置: %s",
                         data.system.count, data.user.count, filepath);
                IupMessage("导出成功", msg);
            }
            else
            {
                log_error("Export failed: error code %d", export_result);
                IupMessage("错误", "导出失败！");
            }
        }
    }
    IupDestroy(filedlg);
    return IUP_DEFAULT;
}
