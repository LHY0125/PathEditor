//! 同步相关退出码：`Sync` → 4（需人工解决）、`SyncFormat` → 1（重试无用）。
//!
//! **为什么必须有这个测试**：这两个码在代码里只差三行 `match` 分支，
//! 但语义完全不同。若它们被合并成一个码，脚本无法区分「需要用户介入」
//! 与「文件坏了」——前者该提示、后者该报错退出。
//!
//! 一文件一测试：见 M0 §5.3。
use path_editor_core::{CoreError, ErrorCode};

#[test]
fn sync_exit_codes_are_distinguished() {
    // 新增的两个码
    assert_eq!(CoreError::new(ErrorCode::Sync, "op", "冲突").exit_code(), 4);
    assert_eq!(
        CoreError::new(ErrorCode::SyncFormat, "op", "包格式错误").exit_code(),
        1,
        "包格式错误重试无用，归入一般错误"
    );

    // 既有码不受影响
    assert_eq!(
        CoreError::new(ErrorCode::Conflict, "op", "冲突").exit_code(),
        3
    );
    assert_eq!(
        CoreError::new(ErrorCode::Crypto, "op", "密钥").exit_code(),
        5
    );
    assert_eq!(
        CoreError::new(ErrorCode::Database, "op", "库").exit_code(),
        1
    );

    // `Sync` 不可重试、`SyncFormat` 也不可重试——两者都需要人做点什么，
    // 但都不是「同一条命令重跑就能好」。
    assert!(!CoreError::new(ErrorCode::Sync, "op", "冲突").retryable);
    assert!(!CoreError::new(ErrorCode::SyncFormat, "op", "格式").retryable);

    // serde 名必须是 camelCase（前端 `ERROR_CODES` 白名单按它匹配）
    let json = serde_json::to_string(&ErrorCode::SyncFormat).unwrap();
    assert_eq!(json, "\"syncFormat\"", "serde 名必须是 syncFormat");
    let json2 = serde_json::to_string(&ErrorCode::Sync).unwrap();
    assert_eq!(json2, "\"sync\"");
}
