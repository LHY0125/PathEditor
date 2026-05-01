#ifndef UI_CONSTANTS_H
#define UI_CONSTANTS_H

// 缓冲区大小常量
#define PATH_BUFFER_SIZE 4096

// 控件名称常量 - 统一管理所有IUP控件名称字符串
// 使用这些常量替代硬编码字符串，便于维护和减少拼写错误

// 列表控件
#define CTRL_LIST_SYS "LIST_SYS"
#define CTRL_LIST_USER "LIST_USER"
#define CTRL_LIST_MERGED "LIST_MERGED"

// 选项卡
#define CTRL_TABS_MAIN "TABS_MAIN"

// 搜索框
#define CTRL_TXT_SEARCH "TXT_SEARCH"

// 状态标签
#define CTRL_LBL_STATUS "LBL_STATUS"

// 操作按钮
#define CTRL_BTN_NEW "BTN_NEW"
#define CTRL_BTN_EDIT "BTN_EDIT"
#define CTRL_BTN_BROWSE "BTN_BROWSE"
#define CTRL_BTN_DEL "BTN_DEL"
#define CTRL_BTN_UP "BTN_UP"
#define CTRL_BTN_DOWN "BTN_DOWN"
#define CTRL_BTN_CLEAN "BTN_CLEAN"
#define CTRL_BTN_IMPORT "BTN_IMPORT"
#define CTRL_BTN_EXPORT "BTN_EXPORT"
#define CTRL_BTN_OK "BTN_OK"
#define CTRL_BTN_CANCEL "BTN_CANCEL"
#define CTRL_BTN_HELP "BTN_HELP"
#define CTRL_BTN_LANG "BTN_LANG"
#define CTRL_BTN_DARKMODE "BTN_DARKMODE"

// 撤销/重做按钮
#define CTRL_BTN_UNDO "BTN_UNDO"
#define CTRL_BTN_REDO "BTN_REDO"

#endif // UI_CONSTANTS_H
