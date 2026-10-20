#![cfg(windows)]
// 一文件一测试：见 M0 §5.3。`.pebak` 走 DPAPI，因此整体限定 Windows。
//
// **P1-2 回归守卫**：备份索引必须有生产调用者。
//
// 缺口清单 P1-2：首版 `index_backups` 只被测试调用，`backup_list` 读 `backups`
// 表但没人往里写，命令恒返回空。修复在 `setup_database`（启动时）与
// `backup_list`（每次调用前增量）两处。
//
// 本测试验证「索引 → 列表」这条链路本身能闭环，覆盖「启动后新增的备份也能被看到」。
use path_editor_core::backup::{
    list_env_backups_in, write_env_backup_to_with_key, EnvBackupPayload,
};
use path_editor_core::backup::{EnvBackupHives, EnvBackupVar};
use path_editor_core::db::backups::{index_backups, list_backups};
use path_editor_core::db::crypto::key_hex;
use path_editor_core::db::crypto::load_or_create_key;
use path_editor_core::db::Db;
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

    // ① 空目录：索引 0 条，列表空
    assert_eq!(index_backups(&mut db, &backup_dir).unwrap(), 0);
    assert!(list_backups(&db).unwrap().is_empty());

    // ② 启动后新增一份备份 → 再次索引必须看到它（模拟 backup_list 的增量索引）
    write_env_backup_to_with_key(&backup_dir, &payload(2), &key).unwrap();
    let n = index_backups(&mut db, &backup_dir).unwrap();
    assert_eq!(n, 1, "新增的备份必须被索引");

    let list = list_backups(&db).unwrap();
    assert_eq!(list.len(), 1, "backup_list 必须能看到启动后新增的备份");
    assert_eq!(list[0].variable_count, 2, "变量数来自 .pebak 文件头");

    // ③ 再加一份 → 索引累加，不覆盖
    write_env_backup_to_with_key(&backup_dir, &payload(3), &key).unwrap();
    index_backups(&mut db, &backup_dir).unwrap();
    let list = list_backups(&db).unwrap();
    assert_eq!(list.len(), 2, "两份备份都应被索引");

    // ④ 重复索引同一批文件必须幂等（按 file_path 去重，不产生重复行）
    for _ in 0..3 {
        index_backups(&mut db, &backup_dir).unwrap();
    }
    assert_eq!(
        list_backups(&db).unwrap().len(),
        2,
        "重复索引不得产生重复行"
    );

    // ⑤ 列表与目录枚举的数量一致（交叉校验，防止索引漏文件）
    let on_disk = list_env_backups_in(&backup_dir).unwrap().len();
    assert_eq!(
        list_backups(&db).unwrap().len(),
        on_disk,
        "索引条数必须与磁盘上的备份文件数一致"
    );
}
