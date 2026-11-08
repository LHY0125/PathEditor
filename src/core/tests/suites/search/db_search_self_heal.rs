//
// FTS 自愈：启动时按**行数一致性**判断派生表是否脱节，而不是只看「空/非空」。
//
// 旧行为（仅当 `search_fts` 为空才重建）无法修复「索引非空但缺配置行」的旧库 ——
// 升级后增量维护只覆盖新写入，历史缺口永远补不上，搜索永远搜不到那些配置。
use path_editor_core::db::history::NewSnapshot;
use path_editor_core::db::profiles::save_profile;
use path_editor_core::db::search;
use path_editor_core::domain::history::EventSource;
use path_editor_core::{app::bootstrap, PathEntry, PathSnapshot};

fn entry(path: &str, enabled: bool) -> PathEntry {
    PathEntry {
        path: path.to_string(),
        enabled,
    }
}

#[test]
fn fts_index_needs_rebuild_when_profile_rows_are_missing() {
    let (_dir, mut db) = crate::common::new_db("fts_self_heal.db");

    // 先建立 PATH 基线（这一步会增量索引快照），保证索引里「有内容」。
    bootstrap::ensure_baseline_from(
        &mut db,
        PathSnapshot {
            system: vec![entry("C:\\A", true)],
            user: vec![entry("D:\\U", true)],
        },
    )
    .unwrap();

    save_profile(
        &mut db,
        "Legacy",
        Some("旧库里的配置"),
        &["work".to_string()],
        NewSnapshot {
            kind: path_editor_core::db::history::SnapshotKind::Profile,
            source: EventSource::Cli,
            system: vec![entry("C:\\A", true)],
            user: vec![entry("D:\\U", true)],
            env: Vec::new(),
            parent_uid: None,
        },
    )
    .unwrap();

    // 构造「旧库」形态：索引非空，但配置行缺失。
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM search_fts WHERE kind = 'profile'", [])
            .map_err(|e| {
                path_editor_core::CoreError::new(
                    path_editor_core::ErrorCode::Internal,
                    "test_setup",
                    format!("删除索引行失败: {e}"),
                )
            })?;
        Ok(())
    })
    .unwrap();

    assert!(
        search::index_count(&db).unwrap() > 0,
        "前置：索引必须非空，否则本用例退化成旧的「空了才重建」"
    );
    assert!(
        search::index_needs_rebuild(&db).unwrap(),
        "索引缺配置行时必须判为需要重建"
    );

    search::rebuild_index(&mut db).unwrap();
    assert!(
        !search::index_needs_rebuild(&db).unwrap(),
        "重建后行数应与实体表一致"
    );
}
