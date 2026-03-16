#ifndef GLOBALS_H
#define GLOBALS_H

#include <iup.h>

// 注册表路径常量
#define REG_PATH_SYS L"SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment"
#define REG_PATH_USER L"Environment"
#define REG_VALUE L"Path"

// 全局控件句柄声明
extern Ihandle *dlg;        // 主对话框句柄
extern Ihandle *tabs_main;  // 标签页容器
extern Ihandle *list_sys;   // 系统变量列表
extern Ihandle *list_user;  // 用户变量列表
extern Ihandle *lbl_status; // 状态标签句柄
extern Ihandle *btn_new;    // 新增按钮句柄
extern Ihandle *btn_edit;   // 编辑按钮句柄
extern Ihandle *btn_browse; // 浏览按钮句柄
extern Ihandle *btn_del;    // 删除按钮句柄
extern Ihandle *btn_up;     // 上移按钮句柄
extern Ihandle *btn_down;   // 下移按钮句柄
extern Ihandle *btn_clean;  // 一键清理按钮句柄
extern Ihandle *btn_ok;     // 确认按钮句柄
extern Ihandle *btn_cancel; // 取消按钮句柄
extern Ihandle *btn_help;   // 帮助按钮句柄
extern Ihandle *txt_search; // 搜索框

// 简单字符串列表结构，用于搜索缓存
typedef struct {
    char **items;
    int count;
    int capacity;
} StringList;

extern StringList raw_sys_paths;
extern StringList raw_user_paths;

// 缓存操作函数声明
void init_string_list(StringList *list);
void add_string_list(StringList *list, const char *str);
void clear_string_list(StringList *list);

#endif // GLOBALS_H