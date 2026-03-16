#ifndef CALLBACKS_H
#define CALLBACKS_H

#include <iup.h>

// 按钮回调
int btn_new_cb(Ihandle *self);
int btn_edit_cb(Ihandle *self);
int btn_browse_cb(Ihandle *self);
int btn_del_cb(Ihandle *self);
int btn_up_cb(Ihandle *self);
int btn_down_cb(Ihandle *self);
int btn_clean_cb(Ihandle *self);
int btn_ok_cb(Ihandle *self);
int btn_cancel_cb(Ihandle *self);
int btn_help_cb(Ihandle *self);

// 搜索回调
int txt_search_cb(Ihandle *self);

// 双击回调
int list_dblclick_cb(Ihandle *self, int item, char *text);

// 拖拽回调
int list_dropfiles_cb(Ihandle *self, const char *filename, int num, int x, int y);

// 键盘按键回调
int list_k_any_cb(Ihandle *self, int c);

#endif // CALLBACKS_H