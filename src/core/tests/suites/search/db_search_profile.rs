//! 配置写入路径必须**增量维护 FTS 索引**（审查报告 P1-3）。
//!
//! 旧实现只在首次启动全量重建时索引 `profiles`，之后新建/改名/删除的配置
//! 永远搜不到或留下死引用。
//!
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::profiles::{delete_profile, rename_profile, save_profile};
use path_editor_core::db::search::search;
use path_editor_core::domain::history::EventSource;

#[test]
fn profile_writes_update_the_search_index() {
    let (_dir, mut db) = crate::common::new_db("s.db");

    let snap = NewSnapshot {
        kind: SnapshotKind::Profile,
        source: EventSource::Gui,
        system: vec![],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };
    let uid = save_profile(
        &mut db,
        "search-marker-profile",
        Some("desc-marker"),
        &["tag-marker".to_string()],
        snap,
    )
    .unwrap();

    let hits = search(&db, "search-marker-profile", 10).unwrap();
    assert!(
        hits.iter().any(|h| h.kind == "profile" && h.ref_uid == uid),
        "新建配置必须立刻进检索索引"
    );

    // 改名：旧名不得再命中，新名必须命中
    rename_profile(&mut db, "search-marker-profile", "renamed-marker-profile").unwrap();
    let old = search(&db, "search-marker-profile", 10).unwrap();
    assert!(
        old.iter().all(|h| h.ref_uid != uid),
        "改名后旧名不得继续命中（索引必须重写）"
    );
    let new = search(&db, "renamed-marker-profile", 10).unwrap();
    assert!(
        new.iter().any(|h| h.kind == "profile" && h.ref_uid == uid),
        "改名后新名必须命中"
    );

    // 删除：索引行必须移除，不能留下死引用
    delete_profile(&mut db, &uid).unwrap();
    let gone = search(&db, "renamed-marker-profile", 10).unwrap();
    assert!(
        gone.iter().all(|h| h.ref_uid != uid),
        "删除后不得再命中原配置"
    );
}
