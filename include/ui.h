#ifndef UI_H
#define UI_H

#include <iup.h>

// 创建列表控件
Ihandle *create_path_list();

// 创建右侧功能按钮区域
Ihandle *create_main_buttons();

// 创建底部按钮区域
Ihandle *create_bottom_buttons();

Ihandle *create_main_dialog();

#endif // UI_H