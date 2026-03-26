#ifndef MAIN_WINDOW_H
#define MAIN_WINDOW_H

#include <iup.h>

// 创建主窗口
Ihandle* create_main_window(void);

// 刷新 UI 文本（语言切换时调用）
void refresh_main_window_ui(Ihandle *main_dlg);

#endif // MAIN_WINDOW_H
