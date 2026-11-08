//! 跨层契约与本机密钥生命周期。

mod common;

#[path = "suites/contracts/dacl_principal_check.rs"]
mod dacl_principal_check;

#[path = "suites/contracts/db_key_lifecycle.rs"]
mod db_key_lifecycle;

#[path = "suites/contracts/error_exit_codes.rs"]
mod error_exit_codes;
