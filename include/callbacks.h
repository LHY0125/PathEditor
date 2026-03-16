#ifndef CALLBACKS_H
#define CALLBACKS_H

#include <iup.h>

int btn_new_cb(Ihandle* self);
int btn_edit_cb(Ihandle* self);
int btn_browse_cb(Ihandle* self);
int btn_del_cb(Ihandle* self);
int btn_up_cb(Ihandle* self);
int btn_down_cb(Ihandle* self);
int btn_ok_cb(Ihandle* self);
int btn_cancel_cb(Ihandle* self);
int btn_help_cb(Ihandle* self);

#endif // CALLBACKS_H