#ifndef CB_EDIT_H
#define CB_EDIT_H

#include <iup.h>

// 编辑相关回调
int btn_new_cb(Ihandle *self);
int btn_edit_cb(Ihandle *self);
int list_dblclick_cb(Ihandle *self, int item, char *text);
int btn_del_cb(Ihandle *self);
int btn_up_cb(Ihandle *self);
int btn_down_cb(Ihandle *self);
int btn_clean_cb(Ihandle *self);

#endif // CB_EDIT_H
