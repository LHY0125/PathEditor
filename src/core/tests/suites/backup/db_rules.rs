//! 规则引擎：CRUD、排序/剔除与命中历史。
use path_editor_core::db::rules::{
    apply_and_record, apply_rules, create_rule, delete_rule, list_rules, RuleAction, RuleCondition,
};
use path_editor_core::PathEntry;

fn entry(path: &str) -> PathEntry {
    PathEntry {
        path: path.into(),
        enabled: true,
    }
}

#[test]
fn rules_apply_and_record_hits() {
    let (_dir, mut db) = crate::common::new_db("rules.db");
    create_rule(
        &mut db,
        "剔除 Python",
        RuleCondition::PathContains {
            value: "python".into(),
        },
        RuleAction::Remove,
    )
    .unwrap();
    create_rule(
        &mut db,
        "Java 置顶",
        RuleCondition::PathContains {
            value: "java".into(),
        },
        RuleAction::MoveTop,
    )
    .unwrap();

    let rules = list_rules(&db).unwrap();
    assert_eq!(rules.len(), 2);
    let system = vec![
        entry("C:\\Windows"),
        entry("C:\\Python311"),
        entry("C:\\Java\\bin"),
    ];
    let result = apply_rules(&rules, &system, &[]);
    assert_eq!(
        result
            .system
            .iter()
            .map(|e| e.path.as_str())
            .collect::<Vec<_>>(),
        vec!["C:\\Java\\bin", "C:\\Windows"]
    );
    assert_eq!(result.hits.len(), 2);

    let recorded = apply_and_record(&mut db, &system, &[], None).unwrap();
    assert_eq!(recorded.hits.len(), 2);
    let hits: i64 = db
        .query_row_with("SELECT COUNT(*) FROM rule_hits", [], |r| r.get(0))
        .unwrap();
    assert_eq!(hits, 2);

    let uid = rules[0].uid.clone();
    delete_rule(&mut db, &uid).unwrap();
    assert_eq!(list_rules(&db).unwrap().len(), 1);
    // 级联删除命中历史
    let hits: i64 = db
        .query_row_with("SELECT COUNT(*) FROM rule_hits", [], |r| r.get(0))
        .unwrap();
    assert_eq!(hits, 1);
}
