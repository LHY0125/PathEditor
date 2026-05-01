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
        ctx->undo_redo_mgr = create_undo_redo_manager(50);
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
        destroy_undo_redo_manager(ctx->undo_redo_mgr);
        free(ctx);
    }
}