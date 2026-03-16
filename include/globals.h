#ifndef GLOBALS_H
#define GLOBALS_H

#include <iup.h>

// 注册表路径常量
#define REG_PATH L"SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment"
#define REG_VALUE L"Path"

// 全局控件句柄声明
extern Ihandle *dlg;        // 主对话框句柄
extern Ihandle *list_path;  // 路径列表控件句柄
extern Ihandle *lbl_status; // 状态标签句柄
extern Ihandle *btn_new;    // 新增按钮句柄
extern Ihandle *btn_edit;   // 编辑按钮句柄
extern Ihandle *btn_browse; // 浏览按钮句柄
extern Ihandle *btn_del;    // 删除按钮句柄
extern Ihandle *btn_up;     // 上移按钮句柄
extern Ihandle *btn_down;   // 下移按钮句柄
extern Ihandle *btn_ok;     // 确认按钮句柄
extern Ihandle *btn_cancel; // 取消按钮句柄
extern Ihandle *btn_help;   // 帮助按钮句柄

#endif // GLOBALS_H