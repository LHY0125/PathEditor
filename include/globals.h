#ifndef GLOBALS_H
#define GLOBALS_H

#include <iup.h>

// 注册表路径常量
#define REG_PATH_SYS L"SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment"
#define REG_PATH_USER L"Environment"
#define REG_VALUE L"Path"

// 全局控件句柄声明
extern Ihandle *dlg;            // 主对话框句柄
extern Ihandle *tabs_main;      // 标签页容器
extern Ihandle *list_sys;       // 系统变量列表
extern Ihandle *list_user;      // 用户变量列表
extern Ihandle *list_merged;    // 合并变量列表
extern Ihandle *lbl_status;     // 状态标签句柄
extern Ihandle *btn_new;        // 新增按钮句柄
extern Ihandle *btn_edit;       // 编辑按钮句柄
extern Ihandle *btn_browse;     // 浏览按钮句柄
extern Ihandle *btn_del;        // 删除按钮句柄
extern Ihandle *btn_up;         // 上移按钮句柄
extern Ihandle *btn_down;       // 下移按钮句柄
extern Ihandle *btn_clean;      // 一键清理按钮句柄
extern Ihandle *btn_undo;       // 撤销按钮句柄
extern Ihandle *btn_redo;       // 重做按钮句柄
extern Ihandle *btn_import;     // 导入按钮句柄
extern Ihandle *btn_export;     // 导出按钮句柄
extern Ihandle *btn_theme;      // 主题切换按钮句柄
extern Ihandle *btn_ok;         // 确认按钮句柄
extern int is_dark_mode;        // 深色模式状态
extern Ihandle *btn_cancel;     // 取消按钮句柄
extern Ihandle *btn_help;       // 帮助按钮句柄
extern Ihandle *txt_search;     // 搜索框

// 简单字符串列表结构，用于搜索缓存
typedef struct {
    char **items;
    int count;
    int capacity;
} StringList;

extern StringList raw_sys_paths;
extern StringList raw_user_paths;

// 历史记录节点
typedef struct HistoryNode {
    StringList sys_paths;
    StringList user_paths;
    struct HistoryNode *next;
} HistoryNode;

// 历史记录栈
typedef struct {
    HistoryNode *top;
    int count;
} HistoryStack;

extern HistoryStack undo_stack;
extern HistoryStack redo_stack;
extern Ihandle *btn_undo;
extern Ihandle *btn_redo;

// 缓存操作函数声明
void init_string_list(StringList *list);
void add_string_list(StringList *list, const char *str);
void clear_string_list(StringList *list);
void copy_string_list(StringList *dest, StringList *src);

// 历史记录操作
void init_history_stack(HistoryStack *stack);
void push_history(HistoryStack *stack, StringList *sys, StringList *user);
int pop_history(HistoryStack *stack, StringList *out_sys, StringList *out_user);
void clear_history_stack(HistoryStack *stack);

#endif // GLOBALS_H