#ifndef UI_UTILS_H
#define UI_UTILS_H

#include <iup.h>
#include "utils/string_ext.h"

// 刷新单个列表框样式
// 功能说明:
//  1. 路径有效性检查：无效路径显示红色前景色 (255 0 0)
//  2. 重复检查：重复路径显示橙色前景色 (255 128 0)，只检查当前项之前的项
//  3. 斑马纹背景：奇偶行交替显示不同背景色 (白/灰)
// 注意: 该函数需要IUP控件已设置NAME属性
void refresh_single_list_style(Ihandle *list);

// 同步字符串列表到 UI 列表框
// 将StringList中的所有项同步到IUP FlatList控件中
// 会先清空列表然后重新添加所有项，最后刷新样式
void sync_string_list_to_ui(Ihandle *list_ui, const StringList *str_list);

// 深色模式状态管理
void set_dark_mode(int enabled);
int get_dark_mode(void);
void init_dark_mode(void);

#endif // UI_UTILS_H
