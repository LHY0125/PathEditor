// 一文件一测试：见 M0 §5.3。
//
// **偏离记录（计划 Task 4 的测试）**：计划版测试插完快照就直接调 `rebuild_refs`，
// 但 `rebuild_refs` 只处理 `app_state.current_snapshot_id` 指向的那一份——
// 不设当前快照时 `sid` 为 None，函数返回 0，测试必然失败。
// 本测试补上 `set_current_snapshot`（与 Task 3 同一模式）。
//
// 计划版还把 `name: "PATH"` 放进 `env`，而 `insert_snapshot` 会拒绝它（D13 的
// CHECK）；计划自己的注释也说「换成合法名字更清晰」。这里按注释建议改。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{
    app_state_version, insert_snapshot, set_current_snapshot, EnvValueRow, NewSnapshot,
    SnapshotKind,
};
use path_editor_core::db::refs::{dependencies, dependents, find_cycles, parse_refs, rebuild_refs};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

fn env(hive: &str, name: &str, value: &str) -> EnvValueRow {
    EnvValueRow {
        hive: hive.into(),
        name: name.into(),
        kind: "ExpandString".into(),
        value: Some(value.into()),
        revision: "r".into(),
        sensitive: false,
        restorable: true,
    }
}

#[test]
fn var_references_parses_edges_and_detects_cycles() {
    // ── 纯函数：只认 %NAME%，忽略普通目录 ──
    assert_eq!(
        parse_refs("%JAVA_HOME%\\bin;%M2_HOME%\\bin;C:\\plain"),
        vec!["JAVA_HOME", "M2_HOME"]
    );
    // 边界：单个 % / 空 / 非法字符 / 相邻
    assert!(parse_refs("%").is_empty());
    assert!(parse_refs("%%").is_empty());
    assert!(parse_refs("%A-B%").is_empty(), "连字符不是合法变量名字符");
    assert_eq!(parse_refs("%A%%B%"), vec!["A", "B"], "相邻引用都要抽出来");
    assert_eq!(parse_refs("%a%"), vec!["A"], "统一大写");

    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("r.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // 依赖链：PATH → JAVA_HOME → JDK_HOME（用于验证传递依赖）
    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![PathEntry {
                path: "%JAVA_HOME%\\bin".into(),
                enabled: true,
            }],
            user: vec![PathEntry {
                path: "%M2_HOME%\\bin".into(),
                enabled: true,
            }],
            env: vec![
                env("system", "JAVA_HOME", "%JDK_HOME%"),
                env("system", "JDK_HOME", "C:\\jdk"),
                // 自引用不算边
                env("system", "SELFY", "%SELFY%"),
            ],
            parent_uid: None,
        },
    )
    .unwrap();

    // **必须设当前快照**，否则 rebuild_refs 找不到要处理的快照
    let v = app_state_version(&db).unwrap();
    assert!(
        set_current_snapshot(&mut db, &uid, v).unwrap(),
        "设置当前快照失败"
    );

    let n = rebuild_refs(&mut db).unwrap();
    assert!(n >= 3, "至少 3 条边，实际 {n}");

    // ── 反向：谁引用了 JAVA_HOME ──
    let deps = dependents(&db, "JAVA_HOME").unwrap();
    assert!(!deps.is_empty(), "JAVA_HOME 应被系统 PATH 引用");
    assert_eq!(deps[0].from_name, "PATH");
    assert_eq!(deps[0].from_hive, "system");

    // 小写查询也要能命中（内部统一大写）
    assert!(
        !dependents(&db, "java_home").unwrap().is_empty(),
        "查询必须大小写不敏感"
    );

    // ── 前向：PATH 传递依赖了谁（递归 CTE）──
    let forward = dependencies(&db, "PATH").unwrap();
    assert!(
        forward.iter().any(|e| e.to_name == "JAVA_HOME"),
        "PATH 直接依赖 JAVA_HOME"
    );
    assert!(
        forward
            .iter()
            .any(|e| e.to_name == "JDK_HOME" && e.depth == 2),
        "JDK_HOME 必须是深度 2（PATH → JAVA_HOME → JDK_HOME），实际: {forward:?}"
    );
    // 自引用不产生边
    assert!(
        !forward
            .iter()
            .any(|e| e.to_name == "SELFY" && e.from_name == "SELFY"),
        "自引用不得成为边"
    );

    // ── 无环 ──
    assert!(find_cycles(&db).unwrap().is_empty(), "当前图无环");

    // ── 造一个环：JDK_HOME → JAVA_HOME（与 JAVA_HOME → JDK_HOME 成环）──
    let uid2 = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![],
            user: vec![],
            env: vec![
                env("system", "JAVA_HOME", "%JDK_HOME%"),
                env("system", "JDK_HOME", "%JAVA_HOME%"),
            ],
            parent_uid: None,
        },
    )
    .unwrap();
    let v2 = app_state_version(&db).unwrap();
    set_current_snapshot(&mut db, &uid2, v2).unwrap();
    rebuild_refs(&mut db).unwrap();

    let cycles = find_cycles(&db).unwrap();
    assert!(!cycles.is_empty(), "JAVA_HOME ⇄ JDK_HOME 应被检出为环");

    // ── 幂等：重复重建不增加边数 ──
    let before: i64 = db
        .query_row_with("SELECT count(*) FROM var_references", [], |r| r.get(0))
        .unwrap();
    rebuild_refs(&mut db).unwrap();
    let after: i64 = db
        .query_row_with("SELECT count(*) FROM var_references", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after, "rebuild_refs 必须幂等");
}
