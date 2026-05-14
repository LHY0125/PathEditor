#ifndef UI_UTILS_H
#define UI_UTILS_H

#include <iup.h>
#include "utils/string_ext.h"

// 刷新单个列表框样式
void refresh_single_list_style(Ihandle *list);

// 同步字符串列表到 UI 列表框
void sync_string_list_to_ui(Ihandle *list_ui, const StringList *str_list);

#endif // UI_UTILS_H
