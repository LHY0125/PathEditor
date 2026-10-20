#![cfg(windows)]
// 一文件一测试：见 M0 §5.3。`.pebak` 走 DPAPI，因此整体限定 Windows。
//
// V-3 整改的守卫测试：**生产备份路径**必须真的写 `.pebak`，而不是明文 `.json`。
// 旧实现里 `write_env_backup_pebak` 只被测试调用，README/CHANGELOG 的
// 「本版起写 .pebak」在合并后即为不实陈述。
//
// **为什么用 `write_env_backup_to_with_key` 而不是 `backup_env_vars()`**：
// 后者内部调 `load_or_create_key(&config_dir())`，而 `config_dir()` 基于
// `dirs::home_dir()`——在 Windows 上读 `USERPROFILE`，设 `HOME` 环境变量
// **不会**重定向它。用它做测试会真实创建 `~/.patheditor/keys/db.key.dpapi`。
// 显式传密钥的入口既验证了同一条写入路径，又不碰用户目录。
use path_editor_core::backup::{
    list_env_backups_in, write_env_backup_to_with_key, EnvBackupHives, EnvBackupPayload,
    EnvBackupVar,
};
use path_editor_core::db::crypto::load_or_create_key;
use path_editor_core::EnvValueKind;
use tempfile::TempDir;

fn sample_payload() -> EnvBackupPayload {
    EnvBackupPayload {
        captured_at: 1_700_000_000_000,
        hives: EnvBackupHives {
            system: vec![],
            user: vec![EnvBackupVar {
                name: "JAVA_HOME".into(),
                kind: EnvValueKind::String,
                value: "C:\\jdk17".into(),
                revision: "r1".into(),
            }],
        },
    }
}

#[test]
fn production_backup_path_writes_pebak_not_plaintext_json() {
    let tmp = TempDir::new().unwrap();
    // 密钥放在临时目录，绝不碰真实的 ~/.patheditor/keys/
    let key = load_or_create_key(tmp.path()).unwrap();

    let backup_dir = tmp.path().join("backups");
    let path = write_env_backup_to_with_key(&backup_dir, &sample_payload(), &key).unwrap();

    // ① 产物必须是 .pebak
    assert!(
        path.extension()
            .map(|x| x.eq_ignore_ascii_case("pebak"))
            .unwrap_or(false),
        "生产备份路径必须写 .pebak，实际写的是 {path:?}"
    );

    // ② 内容不得是明文 JSON，且必须带 PEBK1 魔数
    let raw = std::fs::read(&path).unwrap();
    let as_text = String::from_utf8_lossy(&raw);
    assert!(
        !as_text.contains("\"hives\""),
        "备份正文含明文 JSON 结构，加密未生效"
    );
    assert!(
        !as_text.contains("C:\\\\jdk17"),
        "变量明文值不得出现在 .pebak 里"
    );
    assert_eq!(&raw[..5], b"PEBK1", "缺少 .pebak 魔数");

    // ③ 目录里不得出现明文 .json
    let json_count = std::fs::read_dir(&backup_dir)
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".json")
        })
        .count();
    assert_eq!(json_count, 0, "生产路径不得再生成明文 .json 备份");

    // ④ 列表必须识别加密标志与真实变量数（读文件头，不解密）
    let list = list_env_backups_in(&backup_dir).unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].encrypted, "列表必须标记 encrypted = true");
    assert_eq!(list[0].format, "pebak");
    assert_eq!(list[0].variable_count, 1, "变量数应来自文件头");
}
