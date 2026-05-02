/*
 * undo_redo.c 单元测试
 * 测试撤销/重做管理器
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include <stdlib.h>
#include "core/undo_redo.h"
#include "core/path_manager.h"
#include "utils/string_ext.h"

/* ==================== Mock 函数 ==================== */

#ifdef TESTING

int is_path_valid(const char *path)
{
    (void)path;
    return 1;
}

void log_info(const char *fmt, ...) { (void)fmt; }
void log_debug(const char *fmt, ...) { (void)fmt; }
void log_warn(const char *fmt, ...) { (void)fmt; }
void log_error(const char *fmt, ...) { (void)fmt; }

#endif

/* ==================== 辅助函数 ==================== */

static OpRecord make_add_record(TargetType target, const char *path)
{
    OpRecord rec;
    memset(&rec, 0, sizeof(rec));
    rec.type = OP_ADD;
    rec.target = target;
    rec.index = -1;
    rec.count = 1;
    rec.old_paths = NULL;
    char **np = (char **)malloc(sizeof(char *));
    np[0] = _strdup(path);
    rec.new_paths = np;
    return rec;
}

static OpRecord make_delete_record(TargetType target, int index, const char *path)
{
    OpRecord rec;
    memset(&rec, 0, sizeof(rec));
    rec.type = OP_DELETE;
    rec.target = target;
    rec.index = index;
    rec.count = 1;
    char **op = (char **)malloc(sizeof(char *));
    op[0] = _strdup(path);
    rec.old_paths = op;
    rec.new_paths = NULL;
    return rec;
}

static OpRecord make_edit_record(TargetType target, int index, const char *old_path, const char *new_path)
{
    OpRecord rec;
    memset(&rec, 0, sizeof(rec));
    rec.type = OP_EDIT;
    rec.target = target;
    rec.index = index;
    rec.count = 1;
    char **op = (char **)malloc(sizeof(char *));
    op[0] = _strdup(old_path);
    rec.old_paths = op;
    char **np = (char **)malloc(sizeof(char *));
    np[0] = _strdup(new_path);
    rec.new_paths = np;
    return rec;
}

static OpRecord make_move_record(OperationType type, TargetType target, int index)
{
    OpRecord rec;
    memset(&rec, 0, sizeof(rec));
    rec.type = type;
    rec.target = target;
    rec.index = index;
    rec.count = 1;
    rec.old_paths = NULL;
    rec.new_paths = NULL;
    return rec;
}

static OpRecord make_clean_record(TargetType target, StringList *old_list)
{
    OpRecord rec;
    memset(&rec, 0, sizeof(rec));
    rec.type = OP_CLEAN;
    rec.target = target;
    rec.index = -1;
    rec.count = old_list->count;
    if (old_list->count > 0)
    {
        char **op = (char **)malloc(old_list->count * sizeof(char *));
        for (int i = 0; i < old_list->count; i++)
            op[i] = _strdup(old_list->items[i]);
        rec.old_paths = op;
    }
    else
    {
        rec.old_paths = NULL;
    }
    rec.new_paths = NULL;
    return rec;
}

/* ==================== 创建/销毁测试 ==================== */

static void test_create_manager(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    assert_non_null(mgr);
    assert_int_equal(mgr->max_size, 10);
    assert_int_equal(mgr->current, -1);
    assert_int_equal(mgr->count, 0);
    destroy_undo_redo_manager(mgr);
}

static void test_create_manager_default_size(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(0);
    assert_non_null(mgr);
    assert_int_equal(mgr->max_size, 50);  /* DEFAULT_MAX_UNDO_RECORDS */
    destroy_undo_redo_manager(mgr);
}

static void test_destroy_null(void **state)
{
    (void)state;
    destroy_undo_redo_manager(NULL);  /* 不应崩溃 */
}

/* ==================== can_undo/can_redo 测试 ==================== */

static void test_can_undo_redo_empty(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    assert_int_equal(can_undo(mgr), 0);
    assert_int_equal(can_redo(mgr), 0);
    destroy_undo_redo_manager(mgr);
}

static void test_can_undo_null(void **state)
{
    (void)state;
    assert_int_equal(can_undo(NULL), 0);
    assert_int_equal(can_redo(NULL), 0);
}

/* ==================== push_undo_record 测试 ==================== */

static void test_push_record(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    OpRecord rec = make_add_record(TARGET_USER, "C:\\Test");

    int result = push_undo_record(mgr, &rec);

    assert_int_equal(result, 0);
    assert_int_equal(mgr->count, 1);
    assert_int_equal(mgr->current, 0);
    assert_int_equal(can_undo(mgr), 1);
    assert_int_equal(can_redo(mgr), 0);

    /* 清理 */
    free(rec.new_paths[0]);
    free(rec.new_paths);
    destroy_undo_redo_manager(mgr);
}

static void test_push_null_mgr(void **state)
{
    (void)state;
    OpRecord rec = make_add_record(TARGET_USER, "C:\\Test");
    int result = push_undo_record(NULL, &rec);
    assert_int_equal(result, -1);
    free(rec.new_paths[0]);
    free(rec.new_paths);
}

static void test_push_null_record(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    int result = push_undo_record(mgr, NULL);
    assert_int_equal(result, -1);
    destroy_undo_redo_manager(mgr);
}

/* ==================== OP_ADD undo/redo 测试 ==================== */

static void test_undo_add(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    /* 添加路径 */
    add_string_list(&user, "C:\\Test");
    OpRecord rec = make_add_record(TARGET_USER, "C:\\Test");
    push_undo_record(mgr, &rec);

    /* 撤销添加 */
    int result = undo(mgr, &sys, &user);
    assert_int_equal(result, 0);
    assert_int_equal(user.count, 0);
    assert_int_equal(can_redo(mgr), 1);

    free(rec.new_paths[0]);
    free(rec.new_paths);
    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

static void test_redo_add(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    add_string_list(&user, "C:\\Test");
    OpRecord rec = make_add_record(TARGET_USER, "C:\\Test");
    push_undo_record(mgr, &rec);

    undo(mgr, &sys, &user);
    assert_int_equal(user.count, 0);

    /* 重做添加 */
    int result = redo(mgr, &sys, &user);
    assert_int_equal(result, 0);
    assert_int_equal(user.count, 1);
    assert_string_equal(string_list_get(&user, 0), "C:\\Test");

    free(rec.new_paths[0]);
    free(rec.new_paths);
    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

/* ==================== OP_DELETE undo/redo 测试 ==================== */

static void test_undo_delete(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    add_string_list(&user, "C:\\Path1");
    add_string_list(&user, "C:\\Path2");

    /* 记录删除操作 */
    OpRecord rec = make_delete_record(TARGET_USER, 0, "C:\\Path1");
    push_undo_record(mgr, &rec);

    /* 模拟删除 */
    path_manager_remove_at(&user, 0);
    assert_int_equal(user.count, 1);

    /* 撤销删除 */
    int result = undo(mgr, &sys, &user);
    assert_int_equal(result, 0);
    assert_int_equal(user.count, 2);

    free(rec.old_paths[0]);
    free(rec.old_paths);
    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

static void test_redo_delete(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    add_string_list(&user, "C:\\Path1");
    add_string_list(&user, "C:\\Path2");

    OpRecord rec = make_delete_record(TARGET_USER, 0, "C:\\Path1");
    push_undo_record(mgr, &rec);

    path_manager_remove_at(&user, 0);
    undo(mgr, &sys, &user);
    assert_int_equal(user.count, 2);

    /* 重做删除 */
    int result = redo(mgr, &sys, &user);
    assert_int_equal(result, 0);
    assert_int_equal(user.count, 1);
    assert_string_equal(string_list_get(&user, 0), "C:\\Path2");

    free(rec.old_paths[0]);
    free(rec.old_paths);
    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

/* ==================== OP_EDIT undo/redo 测试 ==================== */

static void test_undo_edit(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    add_string_list(&user, "C:\\Old");

    OpRecord rec = make_edit_record(TARGET_USER, 0, "C:\\Old", "C:\\New");
    push_undo_record(mgr, &rec);

    /* 模拟编辑 */
    string_list_set(&user, 0, "C:\\New");
    assert_string_equal(string_list_get(&user, 0), "C:\\New");

    /* 撤销编辑 */
    undo(mgr, &sys, &user);
    assert_string_equal(string_list_get(&user, 0), "C:\\Old");

    free(rec.old_paths[0]);
    free(rec.old_paths);
    free(rec.new_paths[0]);
    free(rec.new_paths);
    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

static void test_redo_edit(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    add_string_list(&user, "C:\\Old");

    OpRecord rec = make_edit_record(TARGET_USER, 0, "C:\\Old", "C:\\New");
    push_undo_record(mgr, &rec);

    string_list_set(&user, 0, "C:\\New");
    undo(mgr, &sys, &user);

    /* 重做编辑 */
    redo(mgr, &sys, &user);
    assert_string_equal(string_list_get(&user, 0), "C:\\New");

    free(rec.old_paths[0]);
    free(rec.old_paths);
    free(rec.new_paths[0]);
    free(rec.new_paths);
    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

/* ==================== OP_MOVE undo/redo 测试 ==================== */

static void test_undo_move_up(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    add_string_list(&user, "A");
    add_string_list(&user, "B");
    add_string_list(&user, "C");

    /* 记录上移操作 (index=2, C 上移到 B 前面) */
    OpRecord rec = make_move_record(OP_MOVE_UP, TARGET_USER, 2);
    push_undo_record(mgr, &rec);

    /* 模拟上移 */
    path_manager_move_up(&user, 2);
    assert_string_equal(string_list_get(&user, 0), "A");
    assert_string_equal(string_list_get(&user, 1), "C");
    assert_string_equal(string_list_get(&user, 2), "B");

    /* 撤销上移 */
    undo(mgr, &sys, &user);
    assert_string_equal(string_list_get(&user, 0), "A");
    assert_string_equal(string_list_get(&user, 1), "B");
    assert_string_equal(string_list_get(&user, 2), "C");

    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

static void test_redo_move_up(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    add_string_list(&user, "A");
    add_string_list(&user, "B");
    add_string_list(&user, "C");

    OpRecord rec = make_move_record(OP_MOVE_UP, TARGET_USER, 2);
    push_undo_record(mgr, &rec);

    path_manager_move_up(&user, 2);
    undo(mgr, &sys, &user);

    /* 重做上移 */
    redo(mgr, &sys, &user);
    assert_string_equal(string_list_get(&user, 1), "C");
    assert_string_equal(string_list_get(&user, 2), "B");

    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

/* ==================== OP_CLEAN undo/redo 测试 ==================== */

static void test_undo_clean(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    add_string_list(&user, "C:\\Valid");
    add_string_list(&user, "C:\\Invalid");
    add_string_list(&user, "C:\\AlsoValid");

    /* 记录清理前的列表 */
    OpRecord rec = make_clean_record(TARGET_USER, &user);
    push_undo_record(mgr, &rec);

    /* 模拟清理（清空） */
    clear_string_list(&user);
    assert_int_equal(user.count, 0);

    /* 撤销清理 */
    undo(mgr, &sys, &user);
    assert_int_equal(user.count, 3);
    assert_string_equal(string_list_get(&user, 0), "C:\\Valid");
    assert_string_equal(string_list_get(&user, 1), "C:\\Invalid");
    assert_string_equal(string_list_get(&user, 2), "C:\\AlsoValid");

    /* 清理 OpRecord 中的 old_paths */
    for (int i = 0; i < rec.count; i++)
        free(rec.old_paths[i]);
    free(rec.old_paths);

    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

/* ==================== 连续 undo/redo 测试 ==================== */

static void test_multiple_undo_redo(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    /* 3 次添加 */
    OpRecord r1 = make_add_record(TARGET_USER, "A");
    OpRecord r2 = make_add_record(TARGET_USER, "B");
    OpRecord r3 = make_add_record(TARGET_USER, "C");

    add_string_list(&user, "A");
    push_undo_record(mgr, &r1);
    add_string_list(&user, "B");
    push_undo_record(mgr, &r2);
    add_string_list(&user, "C");
    push_undo_record(mgr, &r3);

    assert_int_equal(user.count, 3);

    /* 连续撤销 3 次 */
    undo(mgr, &sys, &user);
    undo(mgr, &sys, &user);
    undo(mgr, &sys, &user);
    assert_int_equal(user.count, 0);
    assert_int_equal(can_undo(mgr), 0);
    assert_int_equal(can_redo(mgr), 1);

    /* 连续重做 3 次 */
    redo(mgr, &sys, &user);
    redo(mgr, &sys, &user);
    redo(mgr, &sys, &user);
    assert_int_equal(user.count, 3);
    assert_int_equal(can_redo(mgr), 0);

    free(r1.new_paths[0]); free(r1.new_paths);
    free(r2.new_paths[0]); free(r2.new_paths);
    free(r3.new_paths[0]); free(r3.new_paths);
    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

/* ==================== 空栈 undo/redo 测试 ==================== */

static void test_undo_empty_stack(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    int result = undo(mgr, &sys, &user);
    assert_int_equal(result, -1);

    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

static void test_redo_empty_stack(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);
    StringList sys, user;
    init_string_list(&sys);
    init_string_list(&user);

    int result = redo(mgr, &sys, &user);
    assert_int_equal(result, -1);

    clear_string_list(&sys);
    clear_string_list(&user);
    destroy_undo_redo_manager(mgr);
}

/* ==================== clear_undo_redo_history 测试 ==================== */

static void test_clear_history(void **state)
{
    (void)state;
    UndoRedoManager *mgr = create_undo_redo_manager(10);

    OpRecord rec = make_add_record(TARGET_USER, "C:\\Test");
    push_undo_record(mgr, &rec);
    assert_int_equal(mgr->count, 1);

    clear_undo_redo_history(mgr);
    assert_int_equal(mgr->count, 0);
    assert_int_equal(mgr->current, -1);
    assert_int_equal(can_undo(mgr), 0);

    free(rec.new_paths[0]);
    free(rec.new_paths);
    destroy_undo_redo_manager(mgr);
}

/* ==================== 主函数 ==================== */

int main(void)
{
    const struct CMUnitTest tests[] = {
        /* 创建/销毁 */
        cmocka_unit_test(test_create_manager),
        cmocka_unit_test(test_create_manager_default_size),
        cmocka_unit_test(test_destroy_null),

        /* can_undo/can_redo */
        cmocka_unit_test(test_can_undo_redo_empty),
        cmocka_unit_test(test_can_undo_null),

        /* push_undo_record */
        cmocka_unit_test(test_push_record),
        cmocka_unit_test(test_push_null_mgr),
        cmocka_unit_test(test_push_null_record),

        /* OP_ADD */
        cmocka_unit_test(test_undo_add),
        cmocka_unit_test(test_redo_add),

        /* OP_DELETE */
        cmocka_unit_test(test_undo_delete),
        cmocka_unit_test(test_redo_delete),

        /* OP_EDIT */
        cmocka_unit_test(test_undo_edit),
        cmocka_unit_test(test_redo_edit),

        /* OP_MOVE */
        cmocka_unit_test(test_undo_move_up),
        cmocka_unit_test(test_redo_move_up),

        /* OP_CLEAN */
        cmocka_unit_test(test_undo_clean),

        /* 连续操作 */
        cmocka_unit_test(test_multiple_undo_redo),

        /* 边界情况 */
        cmocka_unit_test(test_undo_empty_stack),
        cmocka_unit_test(test_redo_empty_stack),
        cmocka_unit_test(test_clear_history),
    };

    return cmocka_run_group_tests(tests, NULL, NULL);
}
