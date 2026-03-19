#ifndef CB_MAIN_H
#define CB_MAIN_H

#include <iup.h>

// 主界面交互回调
int txt_search_cb(Ihandle *self);
int list_k_any_cb(Ihandle *self, int c);
int list_motion_cb(Ihandle *self, int lin, int col);
int dialog_k_any_cb(Ihandle *self, int c);
int btn_ok_cb(Ihandle *self);
int btn_cancel_cb(Ihandle *self);
int btn_help_cb(Ihandle *self);
int tabs_tabchange_cb(Ihandle *self, int new_pos, int old_pos);
int btn_theme_cb(Ihandle *self);

#endif // CB_MAIN_H
