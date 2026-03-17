#ifndef CB_FILE_H
#define CB_FILE_H

#include <iup.h>

// 文件和历史记录回调
int btn_browse_cb(Ihandle *self);
int btn_undo_cb(Ihandle *self);
int btn_redo_cb(Ihandle *self);
int btn_export_cb(Ihandle *self);
int btn_import_cb(Ihandle *self);
int list_dropfiles_cb(Ihandle *self, const char *filename, int num, int x, int y);

#endif // CB_FILE_H
