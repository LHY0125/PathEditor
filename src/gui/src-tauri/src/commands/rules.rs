//! PATH 整理规则命令。
use crate::commands::db_state::{with_db, with_db_mut, DbState};
use path_editor_core::db::rules::{self, Rule, RuleAction, RuleApplyResult, RuleCondition};
use path_editor_core::{CoreError, PathEntry};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCreateRequest {
    pub name: String,
    pub contains: String,
    pub action: String,
}

#[tauri::command(async)]
pub fn rules_list(state: State<'_, DbState>) -> Result<Vec<Rule>, CoreError> {
    with_db(&state, "rules_list", rules::list_rules)
}

#[tauri::command(async)]
pub fn rules_create(
    request: RuleCreateRequest,
    state: State<'_, DbState>,
) -> Result<String, CoreError> {
    let action = if request.action == "remove" {
        RuleAction::Remove
    } else {
        RuleAction::MoveTop
    };
    with_db_mut(&state, "rules_create", |db| {
        rules::create_rule(
            db,
            &request.name,
            RuleCondition::PathContains {
                value: request.contains,
            },
            action,
        )
    })
}

#[tauri::command(async)]
pub fn rules_delete(uid: String, state: State<'_, DbState>) -> Result<(), CoreError> {
    with_db_mut(&state, "rules_delete", |db| rules::delete_rule(db, &uid))
}

#[tauri::command(async)]
pub fn rules_set_enabled(
    uid: String,
    enabled: bool,
    state: State<'_, DbState>,
) -> Result<(), CoreError> {
    with_db_mut(&state, "rules_set_enabled", |db| {
        rules::set_rule_enabled(db, &uid, enabled)
    })
}

/// 预览/应用规则；`record` 为 true 时写命中历史。
#[tauri::command(async)]
pub fn rules_apply(
    system: Vec<PathEntry>,
    user: Vec<PathEntry>,
    record: bool,
    state: State<'_, DbState>,
) -> Result<RuleApplyResult, CoreError> {
    with_db_mut(&state, "rules_apply", |db| {
        if record {
            rules::apply_and_record(db, &system, &user, None)
        } else {
            let rules = rules::list_rules(db)?;
            Ok(rules::apply_rules(&rules, &system, &user))
        }
    })
}
