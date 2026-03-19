#include "globals.h"
#include <windows.h>
#include <iup.h>

// 全局变量定义
StringList raw_sys_paths = {0};
StringList raw_user_paths = {0};

// 全局控件定义
Ihandle *dlg, *tabs_main, *list_sys, *list_user, *lbl_status;           // 主窗口、标签页、系统路径列表、用户路径列表、状态标签
Ihandle *btn_new, *btn_edit, *btn_browse, *btn_del, *btn_up, *btn_down; // 右侧按钮
Ihandle *btn_ok, *btn_cancel, *btn_help;                                // 确认取消帮助按钮
Ihandle *btn_clean;                                                     // 一键清理按钮
Ihandle *txt_search;                                                    // 搜索框