#include "utils/i18n.h"
#include "utils/logger.h"
#include "utils/os_env.h"
#include "core/lua_config.h"
#include <windows.h>
#include <stdio.h>
#include <string.h>

static char current_language[16] = {0};
static char locale_path[256] = {0};

static void load_saved_language(void)
{
    char path[512];
    get_exe_dir(path, sizeof(path));
    strncat(path, "\\language.txt", sizeof(path) - strlen(path) - 1);

    FILE *fp = fopen(path, "r");
    if (fp != NULL)
    {
        char lang[16] = {0};
        if (fgets(lang, sizeof(lang), fp) != NULL)
        {
            int len = strlen(lang);
            if (len > 0 && lang[len - 1] == '\n')
                lang[len - 1] = '\0';
            if (strlen(lang) > 0)
            {
                size_t copy_len = sizeof(current_language) - 1;
                if (strlen(lang) < copy_len)
                    copy_len = strlen(lang);
                memcpy(current_language, lang, copy_len);
                current_language[copy_len] = '\0';
                log_info("Loaded saved language: %s", current_language);
                fclose(fp);
                return;
            }
        }
        fclose(fp);
    }
}

void i18n_init(const char *default_lang)
{
    setlocale(LC_ALL, "");

    char dir[256];
    get_exe_dir(dir, sizeof(dir));
    snprintf(locale_path, sizeof(locale_path), "%s/locale", dir);
    bindtextdomain("messages", locale_path);
    textdomain("messages");

    load_saved_language();

    if (strlen(current_language) == 0)
    {
        const char *detected = i18n_detect_system_language();
        size_t copy_len = sizeof(current_language) - 1;
        size_t src_len = strlen(detected);
        if (src_len < copy_len)
            copy_len = src_len;
        memcpy(current_language, detected, copy_len);
        current_language[copy_len] = '\0';
    }

    char mo_path[512];
    snprintf(mo_path, sizeof(mo_path), "%s/%s/LC_MESSAGES/%s.mo", locale_path, current_language, current_language);
    bindtextdomain("messages", mo_path);

    log_info("i18n initialized with language: %s", current_language);
}

const char *i18n_detect_system_language(void)
{
    LANGID langId = GetUserDefaultUILanguage();

    log_info("Detected system language ID: 0x%04X", langId);

    if (langId == 0x0804)
    {
        return "zh_CN";
    }

    return "en_US";
}

void i18n_change_language(const char *lang)
{
    if (!lang || strlen(lang) == 0)
        return;

    if (strcmp(current_language, lang) == 0)
        return;

    size_t copy_len = sizeof(current_language) - 1;
    size_t src_len = strlen(lang);
    if (src_len < copy_len)
        copy_len = src_len;
    memcpy(current_language, lang, copy_len);
    current_language[copy_len] = '\0';

    char new_lang_path[512];
    snprintf(new_lang_path, sizeof(new_lang_path), "%s/%s/LC_MESSAGES/%s.mo", locale_path, lang, lang);
    bindtextdomain("messages", new_lang_path);

    char path[512];
    get_exe_dir(path, sizeof(path));
    strncat(path, "\\language.txt", sizeof(path) - strlen(path) - 1);

    FILE *fp = fopen(path, "w");
    if (fp != NULL)
    {
        fprintf(fp, "%s\n", lang);
        fclose(fp);
    }

    log_info("Language changed to: %s", lang);
}

const char *i18n_get_current_language(void)
{
    return current_language;
}