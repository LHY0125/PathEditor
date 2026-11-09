//! PATH 整理规则：条件匹配 + 排序/剔除动作 + 命中历史。
//!
//! 规则只作用于「草稿/预览」，不会自行写注册表；调用方拿到结果后仍走
//! 原有保存编排。

use super::{db_err, Db};
use crate::{CoreError, ErrorCode, PathEntry};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 规则条件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuleCondition {
    /// 路径包含指定子串（大小写不敏感）
    PathContains { value: String },
}

/// 规则动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleAction {
    /// 命中项移到最前（保持相对顺序）
    MoveTop,
    /// 命中项从 PATH 中剔除
    Remove,
}

/// 一条规则。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    /// 稳定 uid
    pub uid: String,
    /// 名称
    pub name: String,
    /// 是否启用
    pub enabled: bool,
    /// 优先级（小者先执行）
    pub priority: i64,
    /// 条件
    pub condition: RuleCondition,
    /// 动作
    pub action: RuleAction,
    /// 创建时间
    pub created_at: i64,
    /// 更新时间
    pub updated_at: i64,
}

/// 一次规则命中的记录（尚未落库）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleHit {
    /// 规则 uid
    pub rule_uid: String,
    /// 命中条数
    pub matched_count: usize,
}

/// 规则执行结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleApplyResult {
    /// 处理后的系统 PATH
    pub system: Vec<PathEntry>,
    /// 处理后的用户 PATH
    pub user: Vec<PathEntry>,
    /// 每条规则的命中条数
    pub hits: Vec<RuleHitView>,
}

/// 命中记录的展示形状。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleHitView {
    /// 规则 uid
    pub rule_uid: String,
    /// 命中条数
    pub matched_count: usize,
}

fn validate(name: &str, condition: &RuleCondition) -> Result<(), CoreError> {
    if name.trim().is_empty() {
        return Err(CoreError::new(
            ErrorCode::InvalidName,
            "rules",
            "规则名称不能为空",
        ));
    }
    if name.chars().count() > 100 {
        return Err(CoreError::new(
            ErrorCode::InvalidName,
            "rules",
            "规则名称过长",
        ));
    }
    match condition {
        RuleCondition::PathContains { value } if value.trim().is_empty() => Err(CoreError::new(
            ErrorCode::InvalidValue,
            "rules",
            "匹配子串不能为空",
        )),
        _ => Ok(()),
    }
}

fn row_to_rule(r: &rusqlite::Row<'_>) -> rusqlite::Result<Rule> {
    let condition: String = r.get(4)?;
    let action: String = r.get(5)?;
    Ok(Rule {
        uid: r.get(0)?,
        name: r.get(1)?,
        enabled: r.get::<_, i64>(2)? != 0,
        priority: r.get(3)?,
        condition: serde_json::from_str(&condition).unwrap_or(RuleCondition::PathContains {
            value: String::new(),
        }),
        action: serde_json::from_str(&action).unwrap_or(RuleAction::MoveTop),
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

/// 列出全部规则（优先级升序）。
pub fn list_rules(db: &Db) -> Result<Vec<Rule>, CoreError> {
    let mut stmt = db.prepare(
        "SELECT uid, name, enabled, priority, condition_json, action_json,
                created_at, updated_at
           FROM rules ORDER BY priority ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([], row_to_rule)
        .map_err(|e| db_err("list_rules", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("list_rules", e))?;
    Ok(rows)
}

/// 新建规则（uid 自动生成）。
pub fn create_rule(
    db: &mut Db,
    name: &str,
    condition: RuleCondition,
    action: RuleAction,
) -> Result<String, CoreError> {
    validate(name, &condition)?;
    let now = chrono::Utc::now().timestamp_millis();
    let uid = Uuid::now_v7().to_string();
    let priority = list_rules(db)?.last().map(|r| r.priority + 1).unwrap_or(0);
    db.with_transaction(|tx| {
        tx.execute(
            "INSERT INTO rules (uid, name, enabled, priority, condition_json, action_json,
                                created_at, updated_at)
             VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?6)",
            params![
                uid,
                name.trim(),
                priority,
                serde_json::to_string(&condition).unwrap_or_else(|_| "{}".into()),
                serde_json::to_string(&action).unwrap_or_else(|_| "\"moveTop\"".into()),
                now
            ],
        )
        .map_err(|e| db_err("create_rule", e))?;
        Ok(())
    })?;
    Ok(uid)
}

/// 删除规则（命中历史随外键级联删除）。
pub fn delete_rule(db: &mut Db, uid: &str) -> Result<(), CoreError> {
    let changed = db.with_transaction(|tx| {
        tx.execute("DELETE FROM rules WHERE uid = ?1", params![uid])
            .map_err(|e| db_err("delete_rule", e))
    })?;
    if changed == 0 {
        return Err(CoreError::new(
            ErrorCode::NotFound,
            "delete_rule",
            "规则不存在",
        ));
    }
    Ok(())
}

/// 启用/禁用规则。
pub fn set_rule_enabled(db: &mut Db, uid: &str, enabled: bool) -> Result<(), CoreError> {
    let now = chrono::Utc::now().timestamp_millis();
    let changed = db.with_transaction(|tx| {
        tx.execute(
            "UPDATE rules SET enabled = ?1, updated_at = ?2 WHERE uid = ?3",
            params![i64::from(enabled), now, uid],
        )
        .map_err(|e| db_err("set_rule_enabled", e))
    })?;
    if changed == 0 {
        return Err(CoreError::new(
            ErrorCode::NotFound,
            "set_rule_enabled",
            "规则不存在",
        ));
    }
    Ok(())
}

fn matches(rule: &Rule, entry: &PathEntry) -> bool {
    match &rule.condition {
        RuleCondition::PathContains { value } => entry
            .path
            .to_lowercase()
            .contains(&value.trim().to_lowercase()),
    }
}

/// 对给定 PATH 应用规则（纯函数）。
pub fn apply_rules(rules: &[Rule], system: &[PathEntry], user: &[PathEntry]) -> RuleApplyResult {
    let mut system = system.to_vec();
    let mut user = user.to_vec();
    let mut hits = Vec::new();
    for rule in rules.iter().filter(|rule| rule.enabled) {
        let matched_count: usize;
        match rule.action {
            RuleAction::Remove => {
                // retain 只负责计数；Remove 直接丢弃命中项。
                let mut keep = Vec::with_capacity(system.len());
                let mut count = 0usize;
                for entry in system.drain(..) {
                    if matches(rule, &entry) {
                        count += 1;
                    } else {
                        keep.push(entry);
                    }
                }
                system = keep;
                let mut keep = Vec::with_capacity(user.len());
                for entry in user.drain(..) {
                    if matches(rule, &entry) {
                        count += 1;
                    } else {
                        keep.push(entry);
                    }
                }
                user = keep;
                matched_count = count;
            }
            RuleAction::MoveTop => {
                let mut sys_hit = Vec::new();
                let mut sys_rest = Vec::new();
                for entry in system {
                    if matches(rule, &entry) {
                        sys_hit.push(entry);
                    } else {
                        sys_rest.push(entry);
                    }
                }
                let mut usr_hit = Vec::new();
                let mut usr_rest = Vec::new();
                for entry in user {
                    if matches(rule, &entry) {
                        usr_hit.push(entry);
                    } else {
                        usr_rest.push(entry);
                    }
                }
                matched_count = sys_hit.len() + usr_hit.len();
                system = sys_hit.into_iter().chain(sys_rest).collect();
                user = usr_hit.into_iter().chain(usr_rest).collect();
            }
        }
        if matched_count > 0 {
            hits.push(RuleHit {
                rule_uid: rule.uid.clone(),
                matched_count,
            });
        }
    }
    RuleApplyResult {
        system,
        user,
        hits: hits
            .into_iter()
            .map(|hit| RuleHitView {
                rule_uid: hit.rule_uid,
                matched_count: hit.matched_count,
            })
            .collect(),
    }
}

/// 应用规则并记录命中历史（`event_uid` 可空）。
pub fn apply_and_record(
    db: &mut Db,
    system: &[PathEntry],
    user: &[PathEntry],
    event_uid: Option<&str>,
) -> Result<RuleApplyResult, CoreError> {
    let rules = list_rules(db)?;
    let result = apply_rules(&rules, system, user);
    let now = chrono::Utc::now().timestamp_millis();
    db.with_transaction(|tx| {
        for hit in &result.hits {
            let rule_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM rules WHERE uid = ?1",
                    params![hit.rule_uid],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| db_err("rule_hit_lookup", e))?;
            if let Some(rule_id) = rule_id {
                tx.execute(
                    "INSERT INTO rule_hits (rule_id, event_uid, hit_at, matched_count)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![rule_id, event_uid, now, hit.matched_count as i64],
                )
                .map_err(|e| db_err("rule_hit", e))?;
            }
        }
        Ok(())
    })?;
    Ok(result)
}
