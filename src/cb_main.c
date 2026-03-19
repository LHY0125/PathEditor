#include "cb_main.h"
#include "ui_utils.h"
#include "globals.h"
#include "registry.h"
#include "utils.h"
#include "cb_edit.h"
#include "cb_file.h"
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

// 搜索回调
int txt_search_cb(Ihandle *self)
{
    char *filter = IupGetAttribute(self, "VALUE");
    if (!filter)
        return IUP_DEFAULT;

    // 获取当前选中的 Tab 索引
    int pos = IupGetInt(tabs_main, "VALUEPOS");
    Ihandle *current_list = (pos == 0) ? list_sys : list_user;
    StringList *raw_data = (pos == 0) ? &raw_sys_paths : &raw_user_paths;

    // 清空列表
    IupSetInt(current_list, "NUMLIN", 0);

    // 重新填充
    int count = 0;
    for (int i = 0; i < raw_data->count; i++)
    {
        // 如果 filter 为空，或包含 filter (不区分大小写)
        if (strlen(filter) == 0 || stristr(raw_data->items[i], filter) != NULL)
        {
            count++;
            IupSetInt(current_list, "NUMLIN", count);
            IupSetAttributeId2(current_list, "", count, 1, raw_data->items[i]);
        }
    }

    refresh_single_list_style(current_list);

    return IUP_DEFAULT;
}

// 键盘按键回调
int list_k_any_cb(Ihandle *self, int c)
{
    // 处理 Delete 键
    if (c == K_DEL)
    {
        btn_del_cb(NULL);
        return IUP_IGNORE; // 阻止默认处理
    }
    return IUP_DEFAULT;
}

// 鼠标移动回调 (用于 IupMatrix 的 MOUSEMOVE_CB)
int list_motion_cb(Ihandle *self, int lin, int col)
{
    if (lin > 0)
    {
        char *item = IupGetAttributeId2(self, "", lin, 1);
        if (item)
        {
            char *expanded = expand_env_vars(item);
            if (expanded)
            {
                IupSetAttribute(self, "TIP", expanded);
                free(expanded);
            }
            else
            {
                IupSetAttribute(self, "TIP", item);
            }
        }
        else
        {
            IupSetAttribute(self, "TIP", NULL);
        }
    }
    else
    {
        IupSetAttribute(self, "TIP", NULL);
    }
    return IUP_DEFAULT;
}

// 对话框全局按键回调
int dialog_k_any_cb(Ihandle *self, int c)
{
    switch (c)
    {
    case K_cN: // Ctrl+N 新建
        btn_new_cb(NULL);
        return IUP_IGNORE;
    case K_cS: // Ctrl+S 保存
        btn_ok_cb(NULL);
        return IUP_IGNORE;
    case K_cF: // Ctrl+F 搜索
        if (txt_search)
        {
            IupSetFocus(txt_search);
        }
        return IUP_IGNORE;
    case K_cZ: // Ctrl+Z 撤销
        btn_undo_cb(NULL);
        return IUP_IGNORE;
    case K_cY: // Ctrl+Y 重做
        btn_redo_cb(NULL);
        return IUP_IGNORE;
    }
    return IUP_DEFAULT;
}

// 按钮回调：确定
int btn_ok_cb(Ihandle *self)
{
    save_all_paths();
    return IUP_DEFAULT;
}

// 按钮回调：取消
int btn_cancel_cb(Ihandle *self)
{
    IupExitLoop();
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
               "   - 导入/导出：备份和恢复配置。\n"
               "   - 快捷键：\n"
               "     Ctrl+N: 新建路径\n"
               "     Ctrl+S: 保存更改\n"
               "     Ctrl+F: 聚焦搜索框\n"
               "     Ctrl+Z: 撤销\n"
               "     Ctrl+Y: 重做\n"
               "4. 点击【确定】保存更改并生效。\n"
               "5. 注意：某些正在运行的程序可能需要重启才能识别新的环境变量。\n\n"
               "--------------------------------------------------\n"
               "作者：LHY\n"
               "邮箱：3364451258@qq.com\n"
               "GitHub：https://github.com/LHY0125/PathEditor\n"
               "记得给我的项目点个star！");
    return IUP_DEFAULT;
}

// 标签页切换回调
int tabs_tabchange_cb(Ihandle *self, int new_pos, int old_pos)
{
    if (new_pos == 2)
    {
        // 合并预览模式
        IupSetInt(list_merged, "NUMLIN", 0);
        int count = 0;

        // 添加系统变量
        for (int i = 0; i < raw_sys_paths.count; i++)
        {
            count++;
            IupSetInt(list_merged, "NUMLIN", count);
            IupSetAttributeId2(list_merged, "", count, 1, raw_sys_paths.items[i]);
        }

        // 添加用户变量
        for (int i = 0; i < raw_user_paths.count; i++)
        {
            count++;
            IupSetInt(list_merged, "NUMLIN", count);
            IupSetAttributeId2(list_merged, "", count, 1, raw_user_paths.items[i]);
        }

        refresh_single_list_style(list_merged);

        // 禁用编辑按钮
        toggle_edit_buttons(0);
    }
    else
    {
        // 编辑模式 (检查管理员权限)
        if (check_admin())
        {
            toggle_edit_buttons(1);
        }
        else
        {
            toggle_edit_buttons(0);
        }
    }
    return IUP_DEFAULT;
}

// 主题切换回调
int btn_theme_cb(Ihandle *self)
{
    is_dark_mode = !is_dark_mode;
    if (is_dark_mode)
        IupSetAttribute(btn_theme, "TITLE", "浅色模式");
    else
        IupSetAttribute(btn_theme, "TITLE", "深色模式");
        
    apply_theme();
    return IUP_DEFAULT;
}