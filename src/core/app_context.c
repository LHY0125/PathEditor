#include "core/app_context.h"
#include <stdlib.h>

// 创建应用上下文
AppContext *create_app_context(void)
{
    AppContext *ctx = (AppContext *)malloc(sizeof(AppContext));
    if (ctx)
    {
        init_string_list(&ctx->sys_paths);
        init_string_list(&ctx->user_paths);
    }
    return ctx;
}

// 销毁应用上下文
void destroy_app_context(AppContext *ctx)
{
    if (ctx)
    {
        clear_string_list(&ctx->sys_paths);
        clear_string_list(&ctx->user_paths);
        free(ctx);
    }
}

// 获取应用上下文
AppContext *get_app_context(Ihandle *ih)
{
    if (!ih)
        return NULL;
    Ihandle *dlg = IupGetDialog(ih);
    if (!dlg)
        return NULL;
    return (AppContext *)IupGetAttribute(dlg, "APP_CONTEXT");
}