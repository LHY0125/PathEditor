#ifndef CALLBACKS_INTERNAL_H
#define CALLBACKS_INTERNAL_H

#include <iup.h>
#include "core/app_context.h"
#include "core/undo_redo.h"
#include "utils/i18n.h"

// 内部辅助函数声明（供各 callbacks_*.c 文件共享）
// 这些函数不对外暴露，仅在 controller 层内部使用

// 获取主对话框句柄
Ihandle *get_main_dlg(void);

// 从对话框获取应用上下文
AppContext *get_app_context_from_dlg(Ihandle *dlg);

// 获取当前活动的数据列表（根据 Tab 页切换）
StringList *get_current_raw_data(Ihandle *dlg);

// 获取当前活动的列表 UI 控件
Ihandle *get_current_list(Ihandle *dlg);

// 刷新撤销/重做按钮的启用状态
void refresh_undo_redo_buttons(Ihandle *dlg);

// 同步合并预览列表
void sync_merged_list(Ihandle *dlg);

// 获取当前 Tab 对应的 TargetType
TargetType get_current_target(Ihandle *dlg);

// 创建并推送撤销记录
void push_record(Ihandle *dlg, OperationType op_type, int index, int count,
                 char **old_paths, char **new_paths);

#endif // CALLBACKS_INTERNAL_H
