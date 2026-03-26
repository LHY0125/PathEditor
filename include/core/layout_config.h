#ifndef LAYOUT_CONFIG_H
#define LAYOUT_CONFIG_H

// 布局配置结构体
typedef struct {
    int vbox_gap;           // 垂直布局间距
    int vbox_margin_width;  // 垂直布局外边距宽度
    int vbox_margin_height; // 垂直布局外边距高度
    int hbox_gap;           // 水平布局间距
    int hbox_margin_width;  // 水平布局外边距宽度
    int hbox_margin_height; // 水平布局外边距高度
    int button_width;       // 按钮宽度
    int button_height;      // 按钮高度
} LayoutConfig;

// 默认布局配置
extern const LayoutConfig DEFAULT_LAYOUT;

#endif // LAYOUT_CONFIG_H