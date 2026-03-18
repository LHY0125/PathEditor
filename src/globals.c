#include <stdlib.h>
#include "globals.h"

// 全局控件定义
Ihandle *dlg = NULL;                                                                                              // 主对话框
Ihandle *tabs_main = NULL;                                                                                        // 主选项卡
Ihandle *list_sys = NULL, *list_user = NULL, *list_merged = NULL;                                                 // 列表控件
Ihandle *lbl_status = NULL;                                                                                       // 状态栏
Ihandle *btn_new = NULL, *btn_edit = NULL, *btn_browse = NULL, *btn_del = NULL, *btn_up = NULL, *btn_down = NULL; // 右侧按钮
Ihandle *btn_undo = NULL, *btn_redo = NULL;                                                                       // 撤销重做按钮
Ihandle *btn_import = NULL, *btn_export = NULL;                                                                   // 导入导出按钮
Ihandle *btn_ok = NULL, *btn_cancel = NULL, *btn_help = NULL;                                                     // 确认取消帮助按钮
Ihandle *btn_clean = NULL;                                                                                        // 一键清理按钮
Ihandle *btn_theme = NULL;                                                                                        // 主题切换按钮
Ihandle *txt_search = NULL;                                                                                       // 搜索框

// 历史记录栈
HistoryStack undo_stack = {0};
HistoryStack redo_stack = {0};

// 全局变量定义
StringList raw_sys_paths = {0};
StringList raw_user_paths = {0};
int is_dark_mode = 0; // 默认浅色模式
