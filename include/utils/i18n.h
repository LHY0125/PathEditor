#ifndef I18N_H
#define I18N_H

#include <libintl.h>
#include <locale.h>

#ifndef _
#define _(s) gettext(s)
#endif

// 初始化国际化系统
void i18n_init(const char* default_lang);

// 检测系统语言并返回语言代码
const char* i18n_detect_system_language(void);

// 切换语言
void i18n_change_language(const char* lang);

// 获取当前语言
const char* i18n_get_current_language(void);

#endif // I18N_H
