#include "cb_file.h"
#include "ui_utils.h"
#include "globals.h"
#include "utils.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <windows.h> // for GetFileAttributesA

// 按钮回调：浏览
int btn_browse_cb(Ihandle *self)
{
    Ihandle *filedlg = IupFileDlg();
    IupSetAttribute(filedlg, "DIALOGTYPE", "DIR");
    IupSetAttribute(filedlg, "TITLE", "选择目录");

    IupPopup(filedlg, IUP_CENTER, IUP_CENTER);

    if (IupGetInt(filedlg, "STATUS") != -1)
    {
        char *value = IupGetAttribute(filedlg, "VALUE");
        if (value)
        {
            // 记录历史
            record_history();

            Ihandle *current_list = get_current_list();
            int count = IupGetInt(current_list, "COUNT");
            count++;
            IupSetAttributeId(current_list, "", count, value);
            IupSetInt(current_list, "COUNT", count);
            
            // 更新选中状态
            set_single_selection(current_list, count);

            // 同步 raw_data
            int pos = IupGetInt(tabs_main, "VALUEPOS");
            StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;
            if (raw_data) {
                add_string_list(raw_data, value);
            }

            refresh_single_list_style(current_list);
        }
    }
    IupDestroy(filedlg);
    return IUP_DEFAULT;
}

// 撤销回调
int btn_undo_cb(Ihandle *self)
{
    StringList sys = {0}, user = {0};
    if (pop_history(&undo_stack, &sys, &user)) {
        // Push current state to redo
        push_history(&redo_stack, &raw_sys_paths, &raw_user_paths);
        
        // Restore
        clear_string_list(&raw_sys_paths);
        clear_string_list(&raw_user_paths);
        raw_sys_paths = sys;
        raw_user_paths = user;
        
        // Refresh UI
        refresh_ui_from_raw(list_sys, &raw_sys_paths);
        refresh_ui_from_raw(list_user, &raw_user_paths);
        
        IupSetAttribute(lbl_status, "TITLE", "状态: 已撤销");
    } else {
        IupSetAttribute(lbl_status, "TITLE", "状态: 没有可撤销的操作");
    }
    return IUP_DEFAULT;
}

// 重做回调
int btn_redo_cb(Ihandle *self)
{
    StringList sys = {0}, user = {0};
    if (pop_history(&redo_stack, &sys, &user)) {
        // Push current state to undo
        push_history(&undo_stack, &raw_sys_paths, &raw_user_paths);
        
        // Restore
        clear_string_list(&raw_sys_paths);
        clear_string_list(&raw_user_paths);
        raw_sys_paths = sys;
        raw_user_paths = user;
        
        // Refresh UI
        refresh_ui_from_raw(list_sys, &raw_sys_paths);
        refresh_ui_from_raw(list_user, &raw_user_paths);
        
        IupSetAttribute(lbl_status, "TITLE", "状态: 已重做");
    } else {
        IupSetAttribute(lbl_status, "TITLE", "状态: 没有可重做的操作");
    }
    return IUP_DEFAULT;
}

// 导出配置
int btn_export_cb(Ihandle *self)
{
    Ihandle *current_list = get_current_list();
    int count = IupGetInt(current_list, "COUNT");
    if (count == 0) {
        IupMessage("提示", "当前列表为空，无法导出");
        return IUP_DEFAULT;
    }
    
    Ihandle *filedlg = IupFileDlg();
    IupSetAttribute(filedlg, "DIALOGTYPE", "SAVE");
    IupSetAttribute(filedlg, "TITLE", "导出配置");
    IupSetAttribute(filedlg, "FILTER", "*.txt");
    IupSetAttribute(filedlg, "FILTERINFO", "Text Files (*.txt)");
    
    IupPopup(filedlg, IUP_CENTER, IUP_CENTER);
    
    if (IupGetInt(filedlg, "STATUS") != -1) {
        char *filename = IupGetAttribute(filedlg, "VALUE");
        if (filename) {
            char final_path[1024];
            strncpy(final_path, filename, sizeof(final_path));
            final_path[sizeof(final_path)-1] = '\0';
            
            // 检查是否以 .txt 结尾 (不区分大小写)
            size_t len = strlen(final_path);
            if (len < 4 || _stricmp(final_path + len - 4, ".txt") != 0) {
                if (len + 4 < sizeof(final_path)) {
                    strcat(final_path, ".txt");
                }
            }
            
            FILE *fp = fopen(final_path, "w");
            if (fp) {
                for (int i = 1; i <= count; i++) {
                    char *item = IupGetAttributeId(current_list, "", i);
                    if (item) fprintf(fp, "%s\n", item);
                }
                fclose(fp);
                IupMessage("提示", "导出成功！");
            } else {
                IupMessage("错误", "无法打开文件进行写入");
            }
        }
    }
    IupDestroy(filedlg);
    return IUP_DEFAULT;
}

// 导入配置
int btn_import_cb(Ihandle *self)
{
    Ihandle *filedlg = IupFileDlg();
    IupSetAttribute(filedlg, "DIALOGTYPE", "OPEN");
    IupSetAttribute(filedlg, "TITLE", "导入配置");
    IupSetAttribute(filedlg, "FILTER", "*.txt");
    IupSetAttribute(filedlg, "FILTERINFO", "Text Files (*.txt)");
    
    IupPopup(filedlg, IUP_CENTER, IUP_CENTER);
    
    if (IupGetInt(filedlg, "STATUS") != -1) {
        char *filename = IupGetAttribute(filedlg, "VALUE");
        if (filename) {
            FILE *fp = fopen(filename, "r");
            if (fp) {
                // Record history
                record_history();
                
                Ihandle *current_list = get_current_list();
                int pos = IupGetInt(tabs_main, "VALUEPOS");
                StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;
                
                char line[4096];
                int imported_count = 0;
                while (fgets(line, sizeof(line), fp)) {
                    // Trim newline
                    size_t len = strlen(line);
                    while (len > 0 && (line[len-1] == '\n' || line[len-1] == '\r')) {
                        line[len-1] = '\0';
                        len--;
                    }
                    if (len > 0) {
                        // Add to UI
                        int count = IupGetInt(current_list, "COUNT");
                        count++;
                        IupSetAttributeId(current_list, "", count, line);
                        IupSetInt(current_list, "COUNT", count);
                        
                        // Add to raw_data
                        if (raw_data) add_string_list(raw_data, line);
                        
                        imported_count++;
                    }
                }
                fclose(fp);
                
                refresh_single_list_style(current_list);
                
                char msg[64];
                snprintf(msg, sizeof(msg), "导入成功！共导入 %d 条路径。", imported_count);
                IupMessage("提示", msg);
            } else {
                IupMessage("错误", "无法打开文件进行读取");
            }
        }
    }
    IupDestroy(filedlg);
    return IUP_DEFAULT;
}

// 拖拽回调
int list_dropfiles_cb(Ihandle *self, const char *filename, int num, int x, int y)
{
    // 获取当前列表和原始数据
    // 注意：拖拽的目标列表可能是 list_sys 或 list_user，由 self 参数决定
    // 但为了确保数据一致性，我们还是重新获取一下
    Ihandle *current_list = self;
    StringList *raw_data = NULL;
    if (self == list_sys)
        raw_data = &raw_sys_paths;
    else if (self == list_user)
        raw_data = &raw_user_paths;
    else
        return IUP_DEFAULT; // 异常情况

    // 检查拖入的是否为目录
    DWORD attr = GetFileAttributesA(filename);
    if (attr != INVALID_FILE_ATTRIBUTES && (attr & FILE_ATTRIBUTE_DIRECTORY))
    {
        // 记录历史
        record_history();

        // 如果正在搜索，先清空搜索框
        IupSetAttribute(txt_search, "VALUE", "");

        // 添加到列表末尾
        int count = IupGetInt(current_list, "COUNT");
        count++;
        IupSetAttributeId(current_list, "", count, filename);
        IupSetInt(current_list, "COUNT", count);
        
        // 更新选中状态
        set_single_selection(current_list, count);

        // 同时添加到原始数据缓存，确保搜索时能搜到
        if (raw_data)
        {
            add_string_list(raw_data, filename);
        }

        refresh_single_list_style(current_list);
    }
    else
    {
        // 如果拖入的不是文件夹，可以在状态栏提示
        IupSetAttribute(lbl_status, "TITLE", "提示: 只能拖拽文件夹添加到 PATH");
    }

    return IUP_DEFAULT;
}
