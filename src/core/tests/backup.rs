//! 备份索引、配置、变更日志与保留策略。

mod common;

#[path = "suites/backup/db_backup_index_wiring.rs"]
mod db_backup_index_wiring;

#[path = "suites/backup/db_backup_production.rs"]
mod db_backup_production;

#[path = "suites/backup/db_backups_index.rs"]
mod db_backups_index;

#[path = "suites/backup/db_maintenance_prune.rs"]
mod db_maintenance_prune;

#[path = "suites/backup/db_change_log.rs"]
mod db_change_log;

#[path = "suites/backup/db_change_log_wiring.rs"]
mod db_change_log_wiring;

#[path = "suites/backup/db_profiles.rs"]
mod db_profiles;

#[path = "suites/backup/db_profiles_by_name.rs"]
mod db_profiles_by_name;

#[path = "suites/backup/db_profile_apply.rs"]
mod db_profile_apply;

#[path = "suites/backup/db_profile_apply_hive_failure.rs"]
mod db_profile_apply_hive_failure;
