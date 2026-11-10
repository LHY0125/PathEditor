//! 跨设备同步：bundle、合并、冲突与墓碑。

mod common;

#[path = "suites/sync/db_bundle_crypto.rs"]
mod db_bundle_crypto;

#[path = "suites/sync/db_sync_conflict.rs"]
mod db_sync_conflict;

#[path = "suites/sync/db_sync_conflict_remote_applies.rs"]
mod db_sync_conflict_remote_applies;

#[path = "suites/sync/db_sync_export.rs"]
mod db_sync_export;

#[path = "suites/sync/db_sync_import.rs"]
mod db_sync_import;

#[path = "suites/sync/db_sync_import_limits.rs"]
mod db_sync_import_limits;

#[path = "suites/sync/db_sync_merge.rs"]
mod db_sync_merge;

#[path = "suites/sync/db_sync_profile_idempotent.rs"]
mod db_sync_profile_idempotent;

#[path = "suites/sync/db_sync_profile_fields.rs"]
mod db_sync_profile_fields;

#[path = "suites/sync/db_sync_profile_invalid_name.rs"]
mod db_sync_profile_invalid_name;

#[path = "suites/sync/db_sync_profile_name_conflict.rs"]
mod db_sync_profile_name_conflict;

#[path = "suites/sync/db_sync_profile_order.rs"]
mod db_sync_profile_order;

#[path = "suites/sync/db_sync_profile_resurrect.rs"]
mod db_sync_profile_resurrect;

#[path = "suites/sync/db_sync_profile_tombstone.rs"]
mod db_sync_profile_tombstone;
