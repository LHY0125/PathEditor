//! 历史、快照、派生表与对比。

mod common;

#[path = "suites/history/db_app_state.rs"]
mod db_app_state;

#[path = "suites/history/db_current_snapshot_read.rs"]
mod db_current_snapshot_read;

#[path = "suites/history/db_drift.rs"]
mod db_drift;

#[path = "suites/history/db_event_filter.rs"]
mod db_event_filter;

#[path = "suites/history/db_event_snapshot_link.rs"]
mod db_event_snapshot_link;

#[path = "suites/history/db_history_roundtrip.rs"]
mod db_history_roundtrip;

#[path = "suites/history/db_history_sink.rs"]
mod db_history_sink;

#[path = "suites/history/db_restore_append_only.rs"]
mod db_restore_append_only;

#[path = "suites/history/db_restore_snapshot.rs"]
mod db_restore_snapshot;

#[path = "suites/history/db_path_save_state.rs"]
mod db_path_save_state;

#[path = "suites/history/db_derived_rebuild.rs"]
mod db_derived_rebuild;

#[path = "suites/history/db_derived_watermark.rs"]
mod db_derived_watermark;

#[path = "suites/history/db_scan_runs.rs"]
mod db_scan_runs;

#[path = "suites/history/db_stats_daily.rs"]
mod db_stats_daily;

#[path = "suites/history/db_var_references.rs"]
mod db_var_references;

#[path = "suites/history/db_compare.rs"]
mod db_compare;

#[path = "suites/history/db_compare_toggle.rs"]
mod db_compare_toggle;

#[path = "suites/history/db_compare_user_only.rs"]
mod db_compare_user_only;

#[path = "suites/history/db_retention.rs"]
mod db_retention;

#[path = "suites/history/db_reveal_value.rs"]
mod db_reveal_value;
