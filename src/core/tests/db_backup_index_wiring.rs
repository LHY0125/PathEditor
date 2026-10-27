#![cfg(windows)]
// 一文件一测试（SQLCipher）。`.pebak` 走 DPAPI，因此整体限定 Windows。
//
// 回归守卫：备份索引必须有生产调用者。
//
// 缺口清单：首版 `index_backups` 只被测试调用，`backup_list` 读 `backups`
// 表但没人往里写，命令恒返回空。现在生产入口是
// `refresh_and_list`（每次调用前增量索引），启动引导也会索引一次。
//
// 本测试验证「刷新 + 列表」这条生产路径本身能闭环，覆盖「启动后新增的备份也能被看到」。
use path_editor_core::db::backup_index::refresh_and_list_at;
use path_editor_core::db::crypto::key_hex;
use path_editor_core::db::crypto::load_or_create_key;
use path_editor_core::db::Db;
use path_editor_core::platform::backup::{
    list_env_backups_in, write_env_backup_to_with_key, EnvBackupPayload,
};
use path_editor_core::platform::backup::{EnvBackupHives, EnvBackupVar};
use path_editor_core::EnvValueKind;
use tempfile::TempDir;

fn payload(n: usize) -> EnvBackupPayload {
    EnvBackupPayload {
        captured_at: 1_700_000_000_000,
        hives: EnvBackupHives {
            system: vec![],
            user: (0..n)
                .map(|i| EnvBackupVar {
                    name: format!("VAR_{i}"),
                    kind: EnvValueKind::String,
                    value: format!("v{i}"),
                    revision: format!("r{i}"),
                })
                .collect(),
        },
    }
}

#[test]
fn backup_index_sees_backups_created_after_first_index() {
    let tmp = TempDir::new().unwrap();
    let key = load_or_create_key(tmp.path()).unwrap();
    let mut db = Db::open_encrypted(&tmp.path().join("b.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let backup_dir = tmp.path().join("backups");

    // ① 空目录：生产入口返回空列表
    assert!(refresh_and_list_at(&mut db, &backup_dir)
        .unwrap()
        .is_empty());

    // ② 启动后新增一份备份 → 生产入口一次调用必须看到它
    write_env_backup_to_with_key(&backup_dir, &payload(2), &key).unwrap();
    let list = refresh_and_list_at(&mut db, &backup_dir).unwrap();
    assert_eq!(list.len(), 1, "backup_list 必须能看到启动后新增的备份");
    assert_eq!(list[0].variable_count, 2, "变量数来自 .pebak 文件头");

    // ③ 再加一份 → 索引累加，不覆盖
    write_env_backup_to_with_key(&backup_dir, &payload(3), &key).unwrap();
    assert_eq!(
        refresh_and_list_at(&mut db, &backup_dir).unwrap().len(),
        2,
        "两份备份都应被索引"
    );

    // ④ 重复刷新必须幂等（按 file_path 去重，不产生重复行）
    for _ in 0..3 {
        refresh_and_list_at(&mut db, &backup_dir).unwrap();
    }
    assert_eq!(
        refresh_and_list_at(&mut db, &backup_dir).unwrap().len(),
        2,
        "重复索引不得产生重复行"
    );

    // ⑤ 列表与目录枚举的数量一致（交叉校验，防止索引漏文件）
    let on_disk = list_env_backups_in(&backup_dir).unwrap().len();
    assert_eq!(
        refresh_and_list_at(&mut db, &backup_dir).unwrap().len(),
        on_disk,
        "索引条数必须与磁盘上的备份文件数一致"
    );
}
