#ifndef UI_UTILS_H
#define UI_UTILS_H

#include <iup.h>
#include "globals.h"

// 辅助函数声明
int get_first_selected_index(Ihandle *list);
void set_single_selection(Ihandle *list, int index);
void refresh_ui_from_raw(Ihandle *list, StringList *raw);
void record_history();
int custom_input_dialog(const char *title, const char *label_text, char *buffer, int buffer_size);
Ihandle *get_current_list();
void remove_from_raw_data(StringList *list, const char *str);
void toggle_edit_buttons(int enable);
void apply_theme();

#endif // UI_UTILS_H
