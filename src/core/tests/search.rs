//! FTS5 全文检索与自愈。

mod common;

#[path = "suites/search/db_search_escape.rs"]
mod db_search_escape;

#[path = "suites/search/db_search_fts.rs"]
mod db_search_fts;

#[path = "suites/search/db_search_incremental.rs"]
mod db_search_incremental;

#[path = "suites/search/db_search_profile.rs"]
mod db_search_profile;

#[path = "suites/search/db_search_self_heal.rs"]
mod db_search_self_heal;

#[path = "suites/search/db_search_write_path.rs"]
mod db_search_write_path;
