// 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::scan_history::{diff_runs, list_runs, record_run};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn scan_runs_persist_and_diff_between_two_scans() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("sr.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // ── 三次扫描，构造「首次出现」「最近出现」语义相反的场景 ──
    //
    // run1: C:\dup, C:\old
    // run2: C:\dup, C:\new          （dup 连续出现）
    // run3: C:\dup, C:\old          （old 消失后又回来）
    //
    // 对 old：first_seen = run1，last_seen = run3
    // 对 dup：first_seen = run1，last_seen = run3
    let r1 = record_run(&mut db, r#"["C:\\dup","C:\\old"]"#, "[]").unwrap();
    let r2 = record_run(&mut db, r#"["C:\\dup","C:\\new"]"#, "[]").unwrap();
    let r3 = record_run(&mut db, r#"["C:\\dup","C:\\old"]"#, "[]").unwrap();

    // ── list_runs：按时间倒序 ──
    let list = list_runs(&db, 10).unwrap();
    assert_eq!(list.len(), 3, "应记录三次扫描");
    assert_eq!(list[0].uid, r3, "最新在前");
    assert_eq!(list[2].uid, r1, "最旧在后");

    // ── 两次扫描的 diff ──
    let d12 = diff_runs(&db, &r1, &r2).unwrap();
    assert_eq!(d12.appeared, vec!["C:\\new".to_string()]);
    assert_eq!(d12.disappeared, vec!["C:\\old".to_string()]);
    assert_eq!(d12.persistent, vec!["C:\\dup".to_string()]);

    // ──：首次出现与最近出现必须都能算，且语义不同 ──
    //
    // 比 r2 与 r3（r1 与 r3 的集合完全相同，是另一个有价值的用例，见下）
    let d23 = diff_runs(&db, &r2, &r3).unwrap();
    // r2=[dup,new] → r3=[dup,old]
    assert_eq!(d23.appeared, vec!["C:\\old".to_string()], "old 重新出现");
    assert_eq!(d23.disappeared, vec!["C:\\new".to_string()], "new 消失");
    assert_eq!(d23.persistent, vec!["C:\\dup".to_string()]);

    // 集合完全相同的情形：没有任何新增/消失，但历史首末仍要能算
    let d13 = diff_runs(&db, &r1, &r3).unwrap();
    assert!(d13.appeared.is_empty(), "r1 与 r3 的冲突集合相同");
    assert!(d13.disappeared.is_empty());
    assert_eq!(d13.persistent, vec!["C:\\dup", "C:\\old"]);

    // 对 old：首次出现是 r1（虽然中间 r2 时它不在）
    let first_old = d13
        .first_seen
        .iter()
        .find(|f| f.conflict == "C:\\old")
        .expect("old 必须出现在 first_seen 里");
    assert_eq!(
        first_old.run_uid, r1,
        "old 首次出现应是 r1，实际 {}",
        first_old.run_uid
    );

    // 对 dup：首次出现是 r1，最近出现是 r3 —— 两个字段指向**不同**的 run
    let first_dup = d13
        .first_seen
        .iter()
        .find(|f| f.conflict == "C:\\dup")
        .expect("dup 必须出现在 first_seen 里");
    let last_dup = d13
        .last_seen
        .iter()
        .find(|l| l.conflict == "C:\\dup")
        .expect("dup 必须出现在 last_seen 里");
    assert_eq!(first_dup.run_uid, r1, "dup 首次出现是 r1");
    assert_eq!(last_dup.run_uid, r3, "dup 最近出现是 r3");
    assert_ne!(
        first_dup.run_uid, last_dup.run_uid,
        "首次与最近必须能指向不同的 run——否则两个字段是同一件事"
    );

    // new 只在 r2 出现过：首次与最近都是 r2
    let first_new = d13
        .first_seen
        .iter()
        .find(|f| f.conflict == "C:\\new")
        .expect("new 必须出现在 first_seen 里");
    let last_new = d13
        .last_seen
        .iter()
        .find(|l| l.conflict == "C:\\new")
        .expect("new 必须出现在 last_seen 里");
    assert_eq!(first_new.run_uid, r2);
    assert_eq!(last_new.run_uid, r2);
    assert_eq!(
        first_new.run_uid, last_new.run_uid,
        "只出现一次的冲突首末相同"
    );

    // ── ：record_run 必须触发 daily_stats 重建 ──
    //
    // 本断言验证的是「副作用发生了」（SUM 从 0 变非 0），不是「值正确」。
    // 原因：`daily_stats.conflicts` 的口径是「当日扫描次数」，三次 record_run
    // 落在同一天时是 3；跨日时分摊到多行。断言具体值会依赖运行时刻，不稳定。
    let sum: i64 = db
        .query_row_with(
            "SELECT COALESCE(SUM(conflicts),0) FROM daily_stats",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        sum >= 3,
        "record_run 必须触发 daily_stats 重建（M-6），实际 conflicts 合计 {sum}"
    );

    // ── 幂等性：重复 diff 不改变结果 ──
    let again = diff_runs(&db, &r1, &r3).unwrap();
    assert_eq!(again.appeared, d13.appeared);
    assert_eq!(again.persistent, d13.persistent);
}
