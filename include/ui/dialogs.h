#ifndef DIALOGS_H
#define DIALOGS_H

// 自定义输入对话框
// 返回值：0-取消，1-确认
int custom_input_dialog(const char *title, const char *label_text, char *buffer, int buffer_size);

#endif // DIALOGS_H
