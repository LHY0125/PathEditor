#ifndef CONFIG_H
#define CONFIG_H

// ============================================================================
// UI的配置常量
// ============================================================================

// 应用程序名称
#define APP_NAME "PathEditor"    // 编辑环境变量应用程序名称
#define APP_NAME_READONLY "PathEditor (只读模式)" // 编辑环境变量只读模式标题

// 对话框设置
#define UI_DLG_SIZE "800x800"    // 对话框默认大小 (像素)
#define UI_DLG_MINSIZE "800x800" // 对话框最小大小 (像素)

// 列表控件设置
#define UI_LIST_ITEM_PADDING "5x5"      // 列表项内边距
#define UI_LIST_BACKCOLOR "255 255 255" // 列表背景颜色

// 按钮设置
#define UI_BTN_RASTERSIZE "100x32" // 按钮默认大小

// 布局间隙和边距
#define UI_VBOX_GAP "5"            // 垂直布局项间隙
#define UI_VBOX_MARGIN "0x0"       // 垂直布局外边距
#define UI_VBOX_ALL_MARGIN "10x10" // 垂直布局总外边距
#define UI_VBOX_ALL_GAP "5"        // 垂直布局总间隙

#define UI_HBOX_GAP "10"            // 水平布局项间隙
#define UI_HBOX_MARGIN "10x10"      // 水平布局外边距
#define UI_HBOX_ALIGNMENT "ACENTER" // 水平布局对齐方式

#endif // CONFIG_H
