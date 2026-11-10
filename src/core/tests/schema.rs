//! 建库、schema 迁移与启动引导。

mod common;

#[path = "suites/schema/bootstrap_initialize.rs"]
mod bootstrap_initialize;

#[path = "suites/schema/db_schema_current.rs"]
mod db_schema_current;

#[path = "suites/schema/db_schema_open.rs"]
mod db_schema_open;

#[path = "suites/schema/db_schema_derived.rs"]
mod db_schema_derived;

#[path = "suites/schema/db_health.rs"]
mod db_health;
