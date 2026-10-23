use super::config::*;
use super::*;
use crate::env_var::{revision_of, EnvHive, EnvValueKind};
use crate::error::ErrorCode;
use crate::persist::{Versioned, PERSIST_SCHEMA_VERSION};
use crate::registry::store::memory::MemoryHive;
use crate::registry::store::EnvHiveStore;
use crate::registry::{
    create_env_var_in_store, delete_env_var_force_in_store, update_env_var_force_in_store,
};
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::types::FromRegValue;

#[test]
fn get_appdata_dir_returns_non_empty() {
    assert!(!get_appdata_dir().is_empty());
}

#[test]
fn backup_registry_with_custom_dir() {
    let dir = std::env::temp_dir().join("patheditor_test_backup_custom");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let result = backup_registry(Some(dir.to_string_lossy().to_string()));
    // 可能因无权限读取注册表而失败，但不应 panic
    if let Ok(path) = result {
        assert!(path.contains("patheditor_test_backup_custom"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// `backup_registry(None)` 走**默认目录**且不 panic。
///
/// **两条硬要求**：
/// 1. **必须隔离**：默认目录是用户的 `~/.patheditor/backups/`，那里有 **20 份轮换上限**
///    ——无隔离地跑一次就会**挤掉一份真实备份**（不只是「污染」，是**数据被静默淘汰**）。
///    故经 `with_temp_backup_dir` 把 `PATHEDITOR_BACKUP_DIR` 重定向到临时目录。
/// 2. **必须断言**：丢弃返回值等于什么都没验证（注册表不可读时也「通过」）。
///    可读时断言备份文件**落在隔离目录内**；不可读时断言不 panic 并说明原因。
#[test]
fn backup_registry_default_dir_uses_redirected_dir_without_panic() {
    with_temp_backup_dir(|dir| {
        let result = backup_registry(None);

        match result {
            Ok(path) => {
                // 真机可读注册表：备份必须落在被重定向的默认目录内。
                let path = std::path::PathBuf::from(&path);
                assert!(
                    path.starts_with(dir),
                    "默认目录备份必须落在隔离目录内，实为 {path:?}（隔离目录 {dir:?}）"
                );
                assert!(path.exists(), "返回的备份路径必须真实存在: {path:?}");
            }
            Err(e) => {
                // 无权限读取注册表（如 CI 无交互会话）时无法生成备份，属预期；
                // 关键是**不 panic**，且错误信息可读。
                assert!(!e.message.is_empty(), "失败路径也必须给出可读的错误信息");
            }
        }
    });
}

///  接线测试：`backup_registry` 写入后**确实调用了** path 轮换。
///
/// 单测 `rotate_path_backups_*` 只能证明函数本身正确，**证明不了它被接线**
/// （若 Step 5 的调用漏写或条件写反，那些测试照样全绿）。本用例在临时目录预置
/// 超量旧备份，调一次 `backup_registry(None)`，断言目录不再无限增长。
///
/// 无权限读注册表时 `backup_registry` 返回 `Err`（不 panic），此时无法验证接线，
/// 故按 `Ok` 分支断言（与 `backup_registry_with_custom_dir` 同手法）。
/// 注意：本测试用**默认目录**，故必须经 `PATHEDITOR_BACKUP_DIR` 隔离，
/// 避免触碰用户真实备份目录。
#[test]
fn backup_registry_triggers_path_rotation() {
    with_temp_backup_dir(|dir| {
        // 预置 25 份旧 path 备份（文件名时间戳递增，字典序即时间序）。
        for i in 0..25 {
            std::fs::write(
                dir.join(format!("path_backup_202609{:02}_120000_000.txt", i)),
                "PATH",
            )
            .unwrap();
        }
        // backup_registry 会写一份新的；随后应轮换到 keep 份（缺省 20）。
        if backup_registry(None).is_ok() {
            let remaining = std::fs::read_dir(dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.starts_with("path_backup_") && n.ends_with(".txt")
                })
                .count();
            assert_eq!(
                remaining, PATH_BACKUP_KEEP,
                "写入后必须触发轮换，目录内 path 备份收敛到 keep 份"
            );
        }
    });
}

/// 采集必须排除保留名（Path）与 Unsupported 类型，并携带明文与 revision。
///
/// **先断言 Ok** —— 遇到 Unsupported 必须跳过该变量而非让整次采集失败。
#[test]
fn collect_hive_vars_excludes_reserved_and_unsupported() {
    let hive = MemoryHive::new(true);
    hive.seed("JAVA_HOME", "C:\\jdk17", REG_SZ);
    hive.seed("API_TOKEN", "secret-value", REG_SZ);
    hive.seed("Path", "C:\\Windows", REG_EXPAND_SZ); // 保留名，须排除
    hive.seed("SomeDword", "1", REG_DWORD); // Unsupported，须跳过（不是报错）

    // 关键：必须 Ok。若实现走 read_env_var，这里会 Err 而失败。
    let vars = collect_hive_vars_in_store(&hive, EnvHive::User)
        .expect("遇到 Unsupported 类型必须跳过该变量，而不是整次采集失败");

    let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
    assert!(names.contains(&"JAVA_HOME"), "普通变量必须被采集");
    assert!(
        names.contains(&"API_TOKEN"),
        "敏感变量也必须采集（备份要能还原）"
    );
    assert!(!names.contains(&"Path"), "保留名必须排除");
    assert!(!names.contains(&"SomeDword"), "Unsupported 类型必须被跳过");

    let java = vars.iter().find(|v| v.name == "JAVA_HOME").unwrap();
    assert_eq!(java.value, "C:\\jdk17", "备份必须携带明文值");
    assert_eq!(java.kind, EnvValueKind::String);
    assert_eq!(java.revision.len(), 16, "revision 是 16 位十六进制摘要");
}

///  回归：Unsupported 只在被跳过时出现，**不得**使同一 hive 的其他变量丢失。
#[test]
fn collect_hive_vars_keeps_others_when_unsupported_present() {
    let hive = MemoryHive::new(true);
    hive.seed("A_DWORD", "1", REG_DWORD);
    hive.seed("B_NORMAL", "b", REG_SZ);
    hive.seed("C_DWORD", "2", REG_DWORD);
    hive.seed("D_NORMAL", "d", REG_EXPAND_SZ);

    let vars = collect_hive_vars_in_store(&hive, EnvHive::User).expect("必须跳过而非失败");
    let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();

    assert_eq!(vars.len(), 2, "两个 Unsupported 被跳过，两个正常变量保留");
    assert!(names.contains(&"B_NORMAL"));
    assert!(names.contains(&"D_NORMAL"));
}

/// 采集的 revision 必须与 core 的 revision_of 同口径（恢复时据此判冲突）。
#[test]
fn collect_hive_vars_revision_matches_revision_of() {
    let hive = MemoryHive::new(true);
    hive.seed("MY_VAR", "value-1", REG_SZ);

    let vars = collect_hive_vars_in_store(&hive, EnvHive::User).expect("采集失败");
    let expected = revision_of("MY_VAR", REG_SZ, "value-1");

    assert_eq!(vars[0].revision, expected);
}

/// 备份目录可用环境变量重定向，供测试隔离；未设置时回落 ~/.patheditor/backups。
///
/// **同时重定向配置文件**（`PATHEDITOR_CONFIG_FILE`）指向本临时目录下一个
/// **不存在**的文件：这 15 个调用 [`write_env_backup_to`] 的用例都没有、也
/// 不应控制 `env_backup_keep`，若不重定向就会读**开发者真实的**
/// `~/.patheditor/config.ini`。开发者一旦为手工验证写入 `env_backup_keep = 5`，
/// 这些用例的轮换就会在运行中途删掉各自的夹具，失败表现与配置毫无关系。
/// 指向不存在的文件使 `read_env_backup_keep` 走「文件不存在 → 静默回落默认」
/// 分支，各用例的有效保留份数与重定向之前（默认 20）完全一致。
fn with_temp_backup_dir<F: FnOnce(&std::path::Path)>(f: F) {
    // 环境变量是进程级的，与其他触碰它的测试互斥
    let _guard = crate::persist::test_persist_lock();
    let dir =
        std::env::temp_dir().join(format!("patheditor_test_env_backup_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("PATHEDITOR_BACKUP_DIR", &dir);
    // 不存在 → read_env_backup_keep 静默回落 ENV_BACKUP_KEEP，隔离开发者真实配置
    std::env::set_var("PATHEDITOR_CONFIG_FILE", dir.join("no-such-config.ini"));
    f(&dir);
    std::env::remove_var("PATHEDITOR_BACKUP_DIR");
    std::env::remove_var("PATHEDITOR_CONFIG_FILE");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `PATHEDITOR_BACKUP_DIR` 必须真正把备份目录整体重定向（否则测试隔离失效，
/// 所有备份测试会写到用户的真实 `~/.patheditor/backups/`）。
#[test]
fn backup_dir_is_redirected_by_env_var() {
    with_temp_backup_dir(|dir| {
        assert_eq!(env_backup_dir(), dir, "备份目录必须被重定向到测试临时目录");
        assert_eq!(
            get_appdata_dir(),
            dir.to_string_lossy(),
            "PATH 备份目录访问器也必须继承同一重定向"
        );
    });
}

/// 重定向变量为空时必须回落到默认目录，而不是把备份写到进程当前工作目录。
#[test]
fn backup_dir_empty_env_var_falls_back_to_default() {
    let _guard = crate::persist::test_persist_lock();
    let default_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("backups");

    std::env::set_var("PATHEDITOR_BACKUP_DIR", "");
    let redirected = env_backup_dir();
    std::env::remove_var("PATHEDITOR_BACKUP_DIR");

    assert_eq!(redirected, default_dir, "空值必须回落默认目录");
    assert!(!redirected.as_os_str().is_empty());
}

fn sample_payload() -> EnvBackupPayload {
    EnvBackupPayload {
        captured_at: 1_758_400_000_000,
        hives: EnvBackupHives {
            system: vec![],
            user: vec![EnvBackupVar {
                name: "JAVA_HOME".into(),
                kind: EnvValueKind::String,
                value: "C:\\jdk17".into(),
                revision: "a1b2c3d4e5f60718".into(),
            }],
        },
    }
}

/// 落盘文件必须带 schemaVersion 信封，且能被读回（往返一致）。
#[test]
fn write_env_backup_round_trips_through_versioned_envelope() {
    // **行为变更**：生产入口现在写 DPAPI 加密的 `.pebak`，
    // 「带 schemaVersion 信封的明文 JSON」这一形态不再是生产产物。
    // 本测试原本断言 `extension == json` 与内容含 schemaVersion；
    // 改成断言「生产入口写的是 .pebak」+「遗留 .json 仍可读」两条，
    // 因为后者才是现在仍成立的兼容承诺。
    with_temp_backup_dir(|dir| {
        // 加密路径：显式传密钥（不碰真实 ~/.patheditor/keys/）
        let key_dir = tempfile::TempDir::new().unwrap();
        let key = crate::db::crypto::load_or_create_key(key_dir.path()).unwrap();
        let path = write_env_backup_to_with_key(dir, &sample_payload(), &key).expect("写备份失败");
        assert!(path.exists());
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("env_backup_"),
            "文件名必须以 env_backup_ 开头"
        );
        assert_eq!(path.extension().unwrap(), "pebak", "生产产物必须是 .pebak");

        // 遗留明文格式仍然可读
        let legacy = dir.join("env_backup_20260101_000000_000.json");
        let versioned = Versioned {
            schema_version: PERSIST_SCHEMA_VERSION,
            inner: sample_payload(),
        };
        std::fs::write(&legacy, serde_json::to_string(&versioned).unwrap()).unwrap();
        let back = read_env_backup(&legacy).expect("遗留 .json 必须仍可读");
        assert_eq!(back, sample_payload());
    });
}

/// 对抗性测试：轮换只删自己生成的 env_backup_*.json，其他文件一律不动。
///
/// **本测试必须真正发生删除**：25 份备份全部唯一，`keep=20` 时轮换必然删 5 份。
/// 若文件名重复导致候选数 ≤ keep，轮换会提前返回、一份不删——测试就退化成
/// 空断言，无法发现「误删外部文件」的 bug。故此处额外断言删除确实发生。
#[test]
fn rotate_env_backups_never_deletes_foreign_files() {
    with_temp_backup_dir(|dir| {
        // 25 份互不相同的正常备份（超过保留数 20），确保轮换真的删除
        for i in 0..25 {
            let name = format!("env_backup_202601{:02}_120000_000.json", i);
            std::fs::write(dir.join(&name), format!("{{\"n\":{i}}}")).unwrap();
        }
        // 干扰文件：PATH 备份、.bak、损坏隔离、用户自放文件
        std::fs::write(dir.join("path_backup_20260920_152446_043.txt"), "PATH").unwrap();
        // 名字刻意排在待删区间最前（字典序最小）：若过滤条件漏掉 `.ends_with(".json")`
        // 这一半，本文件会被当成候选删掉，断言才会真正失败。
        std::fs::write(dir.join("env_backup_20250101_000000_000.json.bak"), "BAK").unwrap();
        // 另一个 .bak 落在保留区间内（新于 20 份中最旧的一份），
        // 验证「保留区间内的外部文件同样不会被删」。
        std::fs::write(dir.join("env_backup_old.json.bak"), "BAK").unwrap();
        std::fs::write(dir.join("random_file.corrupt-20260920-120000000"), "C").unwrap();
        std::fs::write(dir.join("我的笔记.txt"), "note").unwrap();
        std::fs::write(dir.join("env_backup_note.md"), "md").unwrap();

        let removed = rotate_env_backups(dir, ENV_BACKUP_KEEP).expect("轮换失败");

        // 前置条件：轮换确实删了东西，下面的「未删除」断言才有意义
        assert_eq!(
            removed.len(),
            5,
            "25 份候选 - keep 20 份 = 应删 5 份；未删除说明测试空转"
        );
        // 删除目标必须全部是本功能生成的备份，一个外部文件都不许碰
        for path in &removed {
            let name = path.file_name().unwrap().to_string_lossy();
            assert!(
                name.starts_with("env_backup_") && name.ends_with(".json"),
                "删除了非本功能生成的文件: {name}"
            );
        }

        assert!(
            dir.join("path_backup_20260920_152446_043.txt").exists(),
            "PATH 备份不得删除"
        );
        assert!(
            dir.join("env_backup_20250101_000000_000.json.bak").exists(),
            ".bak 不得删除"
        );
        assert!(
            dir.join("env_backup_old.json.bak").exists(),
            ".bak 不得删除（保留区间内）"
        );
        assert!(
            dir.join("random_file.corrupt-20260920-120000000").exists(),
            "损坏隔离文件不得删除"
        );
        assert!(dir.join("我的笔记.txt").exists(), "无关文件不得删除");
        assert!(
            dir.join("env_backup_note.md").exists(),
            "非 .json 的同前缀文件不得删除"
        );
    });
}

/// 轮换后 env_backup_*.json 恰好保留 keep 份。
#[test]
fn rotate_env_backups_keeps_exactly_keep_files() {
    with_temp_backup_dir(|dir| {
        for i in 0..25 {
            std::fs::write(
                dir.join(format!("env_backup_202601{:02}_120000_000.json", i)),
                "{}",
            )
            .unwrap();
        }
        rotate_env_backups(dir, ENV_BACKUP_KEEP).expect("轮换失败");

        let remaining = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with("env_backup_") && n.ends_with(".json")
            })
            .count();
        assert_eq!(remaining, ENV_BACKUP_KEEP);
    });
}

/// 端到端回归：经写入路径之后，目录稳态**恰好**等于 keep 份。
///
/// 这才是验收标准 11（「轮换保留 config.ini 的 env_backup_keep 份」）的
/// 口径——用户看到的是目录里有多少份，不是 `rotate_env_backups` 单次调用的
/// 结果。函数级测试看不到「先轮换、后写入」把稳态推高到 `keep + 1` 的偏差，
/// 必须走完整写入路径整体断言。
#[test]
fn write_env_backup_steady_state_is_exactly_keep_files() {
    with_temp_backup_dir(|dir| {
        let count = |dir: &std::path::Path| {
            std::fs::read_dir(dir)
                .unwrap()
                .flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.starts_with("env_backup_") && n.ends_with(".json")
                })
                .count()
        };
        const KEEP: usize = 5;

        // 预置远超 keep 份的旧备份
        for i in 0..25 {
            std::fs::write(
                dir.join(format!("env_backup_202601{:02}_120000_000.json", i)),
                "{}",
            )
            .unwrap();
        }
        assert_eq!(count(dir), 25, "预置基线");

        let mut written_paths = Vec::new();
        for round in 0..4 {
            let written =
                write_env_backup_to_with_keep(dir, &sample_payload(), KEEP).expect("写备份失败");
            assert!(
                written.exists(),
                "第 {round} 轮：刚写入的备份必须保留，不得被自己轮换掉"
            );
            assert_eq!(
                count(dir),
                KEEP,
                "第 {round} 轮写入后应为恰好 {KEEP} 份；若为 {} 说明轮换与写入顺序颠倒",
                KEEP + 1
            );
            written_paths.push(written);
            // 文件名时间戳精度到毫秒；错开 20ms 确保每轮都产生新文件
            // （Windows 计时器粒度约 15.6ms），否则同名覆盖会让本测试失去意义。
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        // 前置条件：四轮确实各写了不同文件，稳态断言才覆盖「keep+1 → keep」回缩
        let unique: std::collections::HashSet<_> = written_paths.iter().collect();
        assert_eq!(unique.len(), 4, "四轮写入应产生 4 个不同文件，否则测试空转");

        // 最新的一份必须仍在（时间戳最大，永不落入删除集）
        let newest = written_paths.last().unwrap();
        assert!(newest.exists(), "最新备份必须保留");
    });
}

/// **生产入口** `write_env_backup_to` 必须读重定向后的配置文件里的保留份数。
///
/// 与 [`config_file_path_is_redirected_by_env_var`] 的分工：那条证明访问器本身
/// 会重定向，这条证明**只走 `write_env_backup_to` 一个入口**的生产链路确实用了
/// 它 —— 若 `env_backup_keep()` 绕过 `config_file_path()` 直接读真实 `~/.patheditor/
/// config.ini`（本机若恰好无此文件则为默认 20），15 个既有用例的隔离会全部失效，
/// 而访问器级测试仍然全绿。本用例把这条链路钉死。
///
/// 可证伪性：把 `env_backup_keep()` 里传到 `read_env_backup_keep` 的参数从
/// `config_file_path()` 改回真实路径 → 保留份数变成真实配置/默认 20，
/// `count(dir)` 会是 26 而不是 4，断言失败。
#[test]
fn write_env_backup_uses_keep_from_redirected_config() {
    // 环境变量是进程级的；本用例自己管理目录，不套 with_temp_backup_dir
    let _guard = crate::persist::test_persist_lock();
    let dir = std::env::temp_dir().join(format!(
        "patheditor_keep_redirect_{}_{}",
        std::process::id(),
        "write"
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("re-directed-config.ini");
    std::fs::write(&cfg, "env_backup_keep = 4\n").unwrap();

    std::env::set_var("PATHEDITOR_BACKUP_DIR", &dir);
    std::env::set_var("PATHEDITOR_CONFIG_FILE", &cfg);

    // 预置 25 份旧备份（遗留明文格式），走同一条轮换路径写 1 份：
    // 稳态应为 keep=4 份。
    //
    // 用 `write_env_backup_to_with_key` 而非 `write_env_backup_to`：
    // 后者会真的去读写 `~/.patheditor/keys/`（`config_dir()` 基于
    // `dirs::home_dir()`，不受环境变量重定向）。
    for i in 0..25 {
        std::fs::write(
            dir.join(format!("env_backup_202601{:02}_120000_000.json", i)),
            "{}",
        )
        .unwrap();
    }
    let key_dir = tempfile::TempDir::new().unwrap();
    let key = crate::db::crypto::load_or_create_key(key_dir.path()).unwrap();
    let written = write_env_backup_to_with_key(&dir, &sample_payload(), &key).expect("写备份失败");
    assert!(written.exists(), "刚写入的备份必须保留");

    // 轮换同时覆盖 .json 与 .pebak，因此按前缀计数
    let count = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("env_backup_"))
        .count();

    std::env::remove_var("PATHEDITOR_BACKUP_DIR");
    std::env::remove_var("PATHEDITOR_CONFIG_FILE");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        count, 4,
        "生产入口必须按重定向配置的 env_backup_keep=4 轮换；若为 26 说明读的是默认 20"
    );
}

/// 测试用日志捕获器：把 `log::warn!` 的消息收进静态缓冲，供断言。
///
/// 存在的理由：`read_env_backup_keep` 的「不可读要 warn、不存在不 warn」这一
/// 要求**只能通过真实 log 输出观察**。分类函数的单测能证明分类结果正确，却
/// 证明不了 `read_env_backup_keep` 确实调用了它——把 warn 整段删掉，那些测试
/// 依然全绿。故此处按 `log` crate 的全局 logger 接口接一个最小捕获器。
struct CapturingLogger;

static CAPTURED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

impl log::Log for CapturingLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            // 捕获器是全局的，而 cargo 测试并行跑：锁被 poisoned 时取内层数据
            // 继续，避免别的测试 panic 后本测试连锁失败（同 persist::test_persist_lock）。
            let mut buf = match CAPTURED.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            buf.push(record.args().to_string());
        }
    }
    fn flush(&self) {}
}

/// 安装捕获器（全局 logger 只能设置一次，已设置则忽略错误）。
fn install_capture_logger() {
    static LOGGER: CapturingLogger = CapturingLogger;
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Warn);
}

/// 清空捕获缓冲，隔离本用例的日志。
fn clear_captured() {
    let mut buf = match CAPTURED.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    buf.clear();
}

/// 取捕获缓冲的守卫（poisoned 时取内层数据）。
///
/// 返回守卫而非克隆：`Vec<String>` 无共享所有权，调用方借守卫即可 filter/any，
/// 无拷贝；且守卫在语句末即释放，不会跨 `clear_captured()` 持锁。
fn captured() -> std::sync::MutexGuard<'static, Vec<String>> {
    match CAPTURED.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// **「不可读要 warn、不存在不 warn」——直接断言真实 log 输出。**
///
/// 这是对该要求在集成点上的唯一有效验证：把 `read_env_backup_keep` 里的
/// warn 分支删掉、或退回「NotFound 与不可读合并处理」的老写法，本测试都会
/// 失败（后者表现为不可读时一条 warn 都没有）。
///
/// 断言一律**按本用例的临时路径过滤**：捕获器是全局的、cargo 测试并行跑，
/// 别的测试（如 `config_unreadable_path_falls_back_without_panic`）也会发出
/// 同样含「不可读」字样的日志。若只匹配关键字，情形 1 的「必须静默」断言会
/// 被邻居的日志污染成假失败。
#[test]
fn read_error_warns_only_when_unreadable() {
    install_capture_logger();
    // 路径含进程 id 与专用前缀，保证与其它测试的临时路径不重叠
    let dir =
        std::env::temp_dir().join(format!("patheditor_cfg_warn_probe_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir_marker = dir.to_string_lossy().into_owned();

    // 情形 1：文件不存在 → 必须静默（首次使用的正常情形，不得刷警告）
    let missing = dir.join("config.ini");
    clear_captured();
    assert_eq!(read_env_backup_keep(&missing), ENV_BACKUP_KEEP);
    let missing_msgs: Vec<String> = captured()
        .iter()
        .filter(|m| m.contains("不可读") && m.contains(&dir_marker))
        .cloned()
        .collect();
    assert!(
        missing_msgs.is_empty(),
        "文件不存在必须静默回落，不得记 warn；实测: {missing_msgs:?}"
    );

    // 情形 2：路径指向目录（不可读）→ 必须 warn，且消息含该路径
    clear_captured();
    assert_eq!(read_env_backup_keep(&dir), ENV_BACKUP_KEEP);
    let all_msgs: Vec<String> = captured().clone();
    let unreadable_msgs: Vec<&String> = all_msgs
        .iter()
        .filter(|m| m.contains("不可读") && m.contains(&dir_marker))
        .collect();
    assert!(
        !unreadable_msgs.is_empty(),
        "文件不可读必须记一条含该路径的 warn；实测全部日志: {all_msgs:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// 读配置失败的分类（`:43` 那处曾把「不存在」与「不可读」合并处理）：
/// `NotFound` 静默回落，其它错误必须 warn。
///
/// 这里直接调 `should_warn_on_read_error` 断言分类结果——这是 `read_env_backup_keep`
/// 中**真实执行**的那行判定，不是 mock。`read_to_string` 的错误构造依赖 OS，
/// 无法在测试里造出任意 `ErrorKind`，故分类逻辑本身被抽成纯函数以便断言。
#[test]
fn read_error_kind_separates_missing_from_unreadable() {
    let missing = std::io::Error::from(std::io::ErrorKind::NotFound);
    assert!(
        !should_warn_on_read_error(&missing),
        "文件不存在是首次使用的正常情形，必须静默回落（且不得创建文件）"
    );

    for kind in [
        std::io::ErrorKind::PermissionDenied,
        std::io::ErrorKind::IsADirectory,
        std::io::ErrorKind::InvalidData,
        std::io::ErrorKind::Other,
    ] {
        assert!(
            should_warn_on_read_error(&std::io::Error::from(kind)),
            "{kind:?} 属于「不可读」，必须 warn 而非静默"
        );
    }
}

/// 「文件整体不可读」回落默认值，且**不 panic、无副作用**。
///
/// 用**路径指向目录**构造不可读：`read_to_string` 对目录返回非 `NotFound` 的
/// 错误（本机实测 Windows 为 `PermissionDenied`，os error 5），从而真实走进
/// 非 `NotFound` 分支。不用 chmod——Windows 上语义不同且 CI 是 Windows。
///
/// 前置断言保证本测试不空转：若某平台上目录读取竟然返回 `NotFound`，
/// 该断言会失败并提醒改用别的构造手段，而不是让测试悄悄退化成
/// 「又一次覆盖了不存在的情形」。
#[test]
fn config_unreadable_path_falls_back_without_panic() {
    let dir =
        std::env::temp_dir().join(format!("patheditor_cfg_unreadable_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // 前置条件：指向目录必须产生非 NotFound 错误，否则下面断言的不是目标分支
    let probe = std::fs::read_to_string(&dir).expect_err("读取目录必须失败");
    assert_ne!(
        probe.kind(),
        std::io::ErrorKind::NotFound,
        "目录读取返回 NotFound，本测试未覆盖「不可读」分支；需改用其它构造手段"
    );

    // 被测行为：不 panic、回落默认值
    assert_eq!(read_env_backup_keep(&dir), ENV_BACKUP_KEEP);

    let _ = std::fs::remove_dir_all(&dir);
}

/// 配置文件缺失 → 回落默认值，且**不创建文件**。
#[test]
fn config_missing_returns_default_and_creates_nothing() {
    let _guard = crate::persist::test_persist_lock();
    let dir = std::env::temp_dir().join(format!("patheditor_cfg_missing_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("config.ini");

    assert_eq!(read_env_backup_keep(&cfg), ENV_BACKUP_KEEP);
    assert!(!cfg.exists(), "读配置不得有副作用");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 五种回落情形（键缺失 / 非整数 / 负数 / 空值 / 未知键）都返回默认值而不是报错。
#[test]
fn config_invalid_values_fall_back_to_default() {
    let dir = std::env::temp_dir().join(format!("patheditor_cfg_invalid_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("config.ini");

    for body in [
        "; 只有注释\n",            // 键缺失
        "env_backup_keep = abc\n", // 非整数
        "env_backup_keep = -5\n",  // 负数
        "env_backup_keep =\n",     // 空值
        "other_key = 1\n",         // 未知键
    ] {
        std::fs::write(&cfg, body).unwrap();
        assert_eq!(
            read_env_backup_keep(&cfg),
            ENV_BACKUP_KEEP,
            "回落失败: {body:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// 合法值被采纳；`;` 与 `#` 注释、行内两侧空格都能正确解析。
#[test]
fn config_reads_valid_value_with_comments_and_spaces() {
    let dir = std::env::temp_dir().join(format!("patheditor_cfg_valid_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("config.ini");

    std::fs::write(&cfg, "; 注释行\n# 另一种注释\nenv_backup_keep   =   7   \n").unwrap();
    assert_eq!(read_env_backup_keep(&cfg), 7);

    let _ = std::fs::remove_dir_all(&dir);
}

/// `PATHEDITOR_CONFIG_FILE` 必须真正把配置**文件**整体重定向。
///
/// 可证伪性（逐条对应一个会失败的实现改动）：
/// - 删掉 `config_file_path` 里的环境变量分支 → `config_file_path()` 返回真实
///   `~/.patheditor/config.ini`，第一处 `assert_eq` 失败；
/// - 把重定向当成**目录**（再拼 `config.ini`）→ 返回的是 `..\config.ini` 而非
///   给定的完整路径，同样在第一处失败；
/// - 让 `env_backup_keep()` 绕过 `config_file_path()` 直接读真实路径 → 第三处
///   读到的是真实配置（或默认 20）而非 3，失败。
///
/// 第三处断言是本用例的关键：它证明「重定向 → 生产入口 `env_backup_keep()`」
/// 这条链路真的通了，而不是只有 `config_file_path()` 这个访问器改了。
#[test]
fn config_file_path_is_redirected_by_env_var() {
    // 环境变量是进程级的，与其他触碰它的测试互斥
    let _guard = crate::persist::test_persist_lock();
    let dir = std::env::temp_dir().join(format!("patheditor_cfg_redirect_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // 刻意放在子目录里并取一个**不同于** `config.ini` 的文件名：
    // 若实现把重定向值当成目录用，两者都会露馅。
    let cfg = dir.join("sub").join("my-config.ini");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    std::fs::write(&cfg, "env_backup_keep = 3\n").unwrap();

    std::env::set_var("PATHEDITOR_CONFIG_FILE", &cfg);
    let resolved = config_file_path();
    let keep = env_backup_keep();
    std::env::remove_var("PATHEDITOR_CONFIG_FILE");

    assert_eq!(
        resolved, cfg,
        "PATHEDITOR_CONFIG_FILE 必须原样作为完整文件路径返回"
    );
    assert_eq!(
        keep, 3,
        "重定向后的 config.ini 里的 env_backup_keep 必须经生产入口读到"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// 重定向变量**未设置**与**设置为空**两种情况都必须回落真实
/// `~/.patheditor/config.ini`（不得把配置读到当前工作目录）。
///
/// 可证伪性：把空值当成合法重定向（去掉 `!file.is_empty()` 判断）→ 空值分支
/// 返回空路径，第二处断言失败；把回落路径改掉 → 两处断言都失败。
#[test]
fn config_file_path_falls_back_to_home_without_env_var() {
    let _guard = crate::persist::test_persist_lock();
    let default = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("config.ini");

    // 情形 1：未设置
    std::env::remove_var("PATHEDITOR_CONFIG_FILE");
    assert_eq!(
        config_file_path(),
        default,
        "未设置 PATHEDITOR_CONFIG_FILE 时必须回落 ~/.patheditor/config.ini"
    );

    // 情形 2：设置为空（Windows 上 set_var 空串等价于删除，故先 set 再断言）
    std::env::set_var("PATHEDITOR_CONFIG_FILE", "");
    let empty = config_file_path();
    std::env::remove_var("PATHEDITOR_CONFIG_FILE");
    assert_eq!(empty, default, "空值必须回落默认配置文件路径");
}

/// 列表只枚举目录、不解析内容：损坏文件不得让列表整体失败。
#[test]
fn list_env_backups_survives_corrupt_file() {
    with_temp_backup_dir(|dir| {
        std::fs::write(
            dir.join("env_backup_20260901_120000_000.json"),
            "not json at all",
        )
        .unwrap();
        std::fs::write(
            dir.join("env_backup_20260902_120000_000.json"),
            "{\"broken\":",
        )
        .unwrap();
        // 干扰文件不得出现在列表里
        std::fs::write(dir.join("path_backup_20260920_152446_043.txt"), "PATH").unwrap();
        std::fs::write(dir.join("env_backup_x.json.bak"), "BAK").unwrap();

        let list = list_env_backups_in(dir).expect("损坏文件不得使列表失败");

        assert_eq!(list.len(), 2, "只有两个 env_backup_*.json");
        assert!(list.iter().all(|i| i.file.starts_with("env_backup_")));
    });
}

/// 列表按时间倒序（最新在前）。
#[test]
fn list_env_backups_sorted_newest_first() {
    with_temp_backup_dir(|dir| {
        for ts in [
            "20260901_120000_000",
            "20260903_120000_000",
            "20260902_120000_000",
        ] {
            std::fs::write(dir.join(format!("env_backup_{ts}.json")), "{}").unwrap();
        }
        let list = list_env_backups_in(dir).unwrap();
        assert_eq!(list[0].file, "env_backup_20260903_120000_000.json");
        assert_eq!(list[2].file, "env_backup_20260901_120000_000.json");
    });
}

/// 路径校验拒绝非 .json、非备份目录、超大文件。
#[test]
fn validate_backup_path_rejects_invalid() {
    with_temp_backup_dir(|dir| {
        assert!(
            validate_backup_path("C:\\Windows\\System32\\config\\SAM").is_err(),
            "非 .json 必须拒绝"
        );
        assert!(
            validate_backup_path("C:\\some\\other\\file.json").is_err(),
            "既不在备份目录也不带 env_backup_ 前缀必须拒绝"
        );

        // 带前缀但超大 → 拒绝
        let big = dir.join("env_backup_huge.json");
        std::fs::write(&big, vec![b'x'; 1024 * 1024 + 1]).unwrap();
        assert!(
            validate_backup_path(&big.to_string_lossy()).is_err(),
            "超过 1 MiB 必须拒绝"
        );

        // 正常文件 → 通过
        let ok = dir.join("env_backup_20260901_120000_000.json");
        std::fs::write(&ok, "{}").unwrap();
        assert!(validate_backup_path(&ok.to_string_lossy()).is_ok());
    });
}

/// S4 逐规则隔离：每条拒绝规则都必须**单独可证伪**。
///
/// 上面那条测试里，「非 .json 必须拒绝」与「既不在备份目录也不带前缀必须拒绝」
/// 两条断言各自被**另一条规则**兜住了：`SAM` 没有扩展名，但去掉扩展名规则后
/// 它仍会被来源规则拒绝；`C:\some\other\file.json` 不存在，去掉来源规则后仍会
/// 被存在性规则拒绝。删掉任一条规则，那两条断言照样通过——是自证的断言，
/// 测不到东西。
///
/// 本测试为每条规则各造一个**只有该规则能拒**的输入：其余规则的前置条件全满足，
/// 删掉对应规则即变 `Ok`。
#[test]
fn validate_backup_path_rejects_each_rule_in_isolation() {
    with_temp_backup_dir(|dir| {
        // 规则 1（扩展名）单独生效：文件在备份目录内、存在、体积正常，
        // 唯一的问题是没有 .json 扩展名。
        let wrong_ext = dir.join("env_backup_20260901_120000_000.txt");
        std::fs::write(&wrong_ext, "{}").unwrap();
        assert!(
            validate_backup_path(&wrong_ext.to_string_lossy()).is_err(),
            "目录内的非 .json 文件必须被扩展名规则拒绝（其余三条规则均满足）"
        );

        // 规则 2（来源）单独生效：文件存在、体积正常、扩展名为 .json，
        // 唯一的问题是既不在备份目录内、又没有 env_backup_ 前缀。
        // **必须真实存在**——否则会被存在性规则兜住而失去隔离性。
        let foreign = std::env::temp_dir().join(format!(
            "patheditor_foreign_backup_{}.json",
            std::process::id()
        ));
        std::fs::write(&foreign, "{}").unwrap();
        let foreign_result = validate_backup_path(&foreign.to_string_lossy());
        let _ = std::fs::remove_file(&foreign);
        assert!(
            foreign_result.is_err(),
            "备份目录之外、无 env_backup_ 前缀的 .json 必须被来源规则拒绝\
                 （其余三条规则均满足）"
        );

        // 规则 3（存在性）单独生效：路径在备份目录内、带前缀、扩展名为 .json，
        // 唯一的问题是文件不存在。
        let missing = dir.join("env_backup_20260902_120000_000.json");
        assert!(
            !missing.exists(),
            "前置条件：该文件必须不存在，否则本用例测的不是存在性规则"
        );
        assert!(
            validate_backup_path(&missing.to_string_lossy()).is_err(),
            "不存在的备份文件必须被存在性规则拒绝（其余三条规则均满足）"
        );
    });
}

/// 目录不存在不是错误：返回空列表（首次使用时的正常情形）。
#[test]
fn list_env_backups_missing_dir_returns_empty() {
    let missing = std::env::temp_dir().join(format!(
        "patheditor_no_such_backup_dir_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&missing);

    let list = list_env_backups_in(&missing).expect("目录不存在必须返回空列表，而不是报错");
    assert!(list.is_empty());
}

/// 公开入口 `list_env_backups()` 必须枚举被重定向的默认目录。
#[test]
fn list_env_backups_public_entry_uses_default_dir() {
    with_temp_backup_dir(|dir| {
        std::fs::write(dir.join("env_backup_20260901_120000_000.json"), "{}").unwrap();

        let list = list_env_backups().expect("列表失败");
        assert_eq!(
            list.len(),
            1,
            "公开入口必须走默认（被重定向的）备份目录，而不是别处"
        );
        assert_eq!(list[0].file, "env_backup_20260901_120000_000.json");
    });
}

/// 构造只含 user hive 的备份载荷（其余测试都只需要 user 侧）。
fn payload_with(user: Vec<EnvBackupVar>) -> EnvBackupPayload {
    EnvBackupPayload {
        captured_at: 0,
        hives: EnvBackupHives {
            system: vec![],
            user,
        },
    }
}

/// 构造一个备份条目，revision 由本条目的 name/type/value 自洽算出 ——
/// 即「备份时该变量就是这个值」。
fn backup_var(name: &str, value: &str, vtype: RegType) -> EnvBackupVar {
    // 先算 revision 再移动 vtype；`RegType` 非 Copy（同 collect_hive_vars_in_store）。
    let revision = revision_of(name, vtype.clone(), value);
    EnvBackupVar {
        name: name.into(),
        kind: EnvValueKind::from_reg_type(vtype),
        value: value.into(),
        revision,
    }
}

/// 差异三类（新增 / 冲突 / 删除）都要被识别，无变化的变量不进差异。
///
/// **本测试原先断言 `modified == 1`，与契约第 4 条互斥**（见
/// `preview_marks_conflict_when_revision_differs` 与 `RestoreChangeKind::Modified`
/// 的注释）：`revision_of` 对 name+type+value 取值，故「值变了」与「备份已过期」
/// 是同一条件，一律判 `Conflict`。改判后本测试的 `CHANGED` 走的就是 `Conflict`
/// 分支，而 `SAME` 仍覆盖「revision 相同 → 无变化」这条。
#[test]
fn preview_classifies_added_conflict_removed() {
    let hive = MemoryHive::new(true);
    hive.seed("SAME", "v", REG_SZ); // 与备份一致，无变化
    hive.seed("CHANGED", "new-value", REG_SZ); // 备份里是旧值 → revision 不符 → Conflict
    hive.seed("EXTRA", "x", REG_SZ); // 备份里没有 → Removed

    let payload = payload_with(vec![
        backup_var("SAME", "v", REG_SZ),
        backup_var("CHANGED", "old-value", REG_SZ),
        backup_var("BRAND_NEW", "n", REG_SZ),
    ]);

    let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

    assert_eq!(preview.added, 1, "BRAND_NEW 应记为新增");
    assert_eq!(
        preview.conflicts, 1,
        "CHANGED 的 revision 与当前不符，应记为冲突"
    );
    assert_eq!(preview.removed, 1, "EXTRA 应记为删除");
    // Modified 当前不可达（见枚举文档）；钉住这一点，避免将来误以为它是活路径。
    assert_eq!(
        preview.modified, 0,
        "Modified 不可达：值变化与备份过期是同一条件，一律判 Conflict"
    );
    assert_eq!(
        preview.changes.len(),
        3,
        "恰好三条差异（新增/冲突/删除），无变化的不进列表"
    );
    assert!(
        !preview.changes.iter().any(|c| c.name == "SAME"),
        "无变化的变量不进差异"
    );
    // 三条差异的 kind 逐一钉住，避免「计数对但分类错」蒙混过关
    let kind_of = |name: &str| {
        preview
            .changes
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.kind)
            .expect("差异缺失")
    };
    assert_eq!(kind_of("BRAND_NEW"), RestoreChangeKind::Added);
    assert_eq!(kind_of("CHANGED"), RestoreChangeKind::Conflict);
    assert_eq!(kind_of("EXTRA"), RestoreChangeKind::Removed);
}

///  / 契约第 4 条：备份中的 revision 与当前不一致时记为 Conflict。
///
/// **可证伪性**：这里走的是 `Conflict` 分支而非「无变化」分支——当前值是
/// `"changed-by-other-tool"`，备份的 revision 由 `"backed-up-value"` 算出，
/// 两者必然不同（`revision_of` 对 value 取值）。若把契约第 4 条改成
/// 「revision 不同也不进差异」，`conflicts` 会掉到 0 而本测试失败。
#[test]
fn preview_marks_conflict_when_revision_differs() {
    let hive = MemoryHive::new(true);
    hive.seed("MY_VAR", "changed-by-other-tool", REG_SZ);

    let payload = payload_with(vec![backup_var("MY_VAR", "backed-up-value", REG_SZ)]);

    let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

    assert_eq!(preview.conflicts, 1, "外部改动必须被识别为冲突");
    assert_eq!(preview.added, 0, "该变量当前存在，不得同时记为新增");
    assert_eq!(preview.removed, 0, "该变量在备份中存在，不得记为删除");
    assert_eq!(preview.changes[0].kind, RestoreChangeKind::Conflict);
    assert_eq!(preview.changes[0].name, "MY_VAR");
    assert_eq!(preview.changes[0].hive, EnvHive::User);
}

/// 契约第 3 条：两端 revision 一致时该变量**完全不出现在差异里**。
///
/// 与上一条构成配对——同一形状的输入，只有 revision 是否相符这一个变量，
/// 一条要求进差异、一条要求不进，两条合起来才真正钉住契约第 3、4 条。
#[test]
fn preview_omits_unchanged_variable() {
    let hive = MemoryHive::new(true);
    hive.seed("MY_VAR", "same-value", REG_SZ);

    // revision 由同一份 name+type+value 算出，与注册表当前值一致
    let payload = payload_with(vec![backup_var("MY_VAR", "same-value", REG_SZ)]);

    let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

    assert_eq!(preview.changes.len(), 0, "值未变的变量必须零差异");
    assert_eq!(preview.added, 0);
    assert_eq!(preview.conflicts, 0);
    assert_eq!(preview.removed, 0);
}

/// 契约第 1 条（Unsupported 半边）：`Unsupported` 类型的当前值不构成「删除」差异。
///
/// **必须放进 `REG_MULTI_SZ`，不能只用 `REG_DWORD`**：`REG_DWORD` 的解码
/// （`String::from_reg_value` 对非字符串类型返回 `Err`）会先把该变量滤掉，
/// 于是「跳过 Unsupported」这条判定**删掉也照样通过** —— 断言被另一条守卫
/// 兜住，不可证伪（变异验证第 2 项实测如此）。`REG_MULTI_SZ` 能被解码成字符串
/// （winreg 把多字符串按 `\n` 连接），只有 `is_writable()` 这一条能拦下它，
/// 断言才真正钉在契约第 1 条上。
///
/// 两种类型都保留：`REG_MULTI_SZ` 提供可证伪性，`REG_DWORD` 覆盖实际最常见
/// 的非字符串类型。
#[test]
fn preview_ignores_unsupported_current_vars() {
    let hive = MemoryHive::new(true);
    hive.seed("SomeDword", "1", REG_DWORD); // 非字符串类型，解码阶段即被滤掉
    hive.seed("SomeMultiSz", "a", REG_MULTI_SZ); // 可解码，只有 is_writable 能拦住

    let preview =
        preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload_with(vec![])).unwrap();

    assert_eq!(
        preview.removed, 0,
        "Unsupported 变量的存在不构成「删除」差异"
    );
    assert_eq!(preview.changes.len(), 0, "Unsupported 不得产生任何变体");
}

/// 契约第 1 条（保留名半边）：注册表中的 `Path` 不参与差异计算。
///
/// 用 `path`（小写）作输入是有意的：`is_reserved` 忽略大小写，而若实现
/// 误把保留名判定写成精确匹配 `"Path"`，本用例会失败。
///
/// **可证伪性**：去掉实现里的 `is_reserved` 跳过，`path` 就会以原始名落进
/// `Removed`，`removed` 由 0 变 1 而失败。
#[test]
fn preview_ignores_reserved_current_vars() {
    let hive = MemoryHive::new(true);
    hive.seed("path", "C:\\Windows", REG_EXPAND_SZ); // 保留名，须排除
    hive.seed("KEEP_ME", "v", REG_SZ); // 普通变量，用来证明不是「整表被跳过」

    let payload = payload_with(vec![backup_var("KEEP_ME", "v", REG_SZ)]);

    let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

    assert_eq!(preview.removed, 0, "保留名不得构成删除差异");
    assert_eq!(preview.changes.len(), 0, "保留名不得产生任何变体");
    // 前置条件：KEEP_ME 确实在两表中且值一致（否则本测试空转）
    assert!(
        !preview
            .changes
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case("path")),
        "保留名不得出现在差异中"
    );
}

/// 契约第 5 条：`Removed` 必须携带**注册表返回的原始大小写名字**，
/// **不能**是小写化的比较键。
///
/// **可证伪性**：注册表里存的是 `MixedCaseVar`（而非全大写），若实现把
/// 小写键直接当作展示名输出（`mixedcasevar`），断言 `== "MixedCaseVar"` 即失败。
/// 用全大写名（如 `EXTRA`）测这一条是**测不到的**——那种输入下原始名与
/// 小写键只在大小写上不同的话仍可通过，故此处刻意用混合大小写。
#[test]
fn preview_removed_entry_keeps_registry_original_case() {
    let hive = MemoryHive::new(true);
    hive.seed("MixedCaseVar", "x", REG_SZ); // 备份里没有 → 应判 Removed

    let preview =
        preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload_with(vec![])).unwrap();

    assert_eq!(preview.removed, 1, "该变量应被判为删除");
    let removed = &preview.changes[0];
    assert_eq!(removed.kind, RestoreChangeKind::Removed);
    assert_eq!(
        removed.name, "MixedCaseVar",
        "Removed 必须用注册表返回的原始大小写，而不是小写比较键（mixedcasevar）"
    );
    assert_ne!(
        removed.name,
        removed.name.to_ascii_lowercase(),
        "前置条件：该名字本身必须含大写，否则本测试区分不出大小写保留"
    );
}

/// 契约第 6 条：变量名比较**忽略大小写**，两个方向都要成立。
///
/// - 备份写 `MixedCaseVar`、注册表存 `mixedcasevar` → 应认出是**同一个**变量
///   （不进 Added），且因 revision 相同而**零差异**；
/// - 备份写 `SAMEVAR`、注册表存 `samevar` → 同上。
///
/// **可证伪性**：把实现里的 `to_ascii_lowercase()` 换成直接比较名字（区分大小写），
/// `current.get(&key)` 会查不到 → 该变量被判 `Added`，`added` 由 0 变 1 而失败。
#[test]
fn preview_matches_names_case_insensitively() {
    let hive = MemoryHive::new(true);
    hive.seed("mixedcasevar", "v1", REG_SZ);
    hive.seed("samevar", "v2", REG_SZ);

    let payload = payload_with(vec![
        backup_var("MixedCaseVar", "v1", REG_SZ),
        backup_var("SAMEVAR", "v2", REG_SZ),
    ]);

    let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

    assert_eq!(
        preview.added, 0,
        "仅大小写不同的同名变量必须被认出是同一个，不得判为新增"
    );
    assert_eq!(preview.removed, 0, "反向同理：不得因大小写差异判为删除");
    assert_eq!(preview.conflicts, 0, "值一致，不得判为冲突");
    assert_eq!(preview.changes.len(), 0, "两端大小写不同但值相同 → 零差异");
}

/// 契约第 5 与第 6 条的组合：大小写不敏感配对后，**未配对的**那个才判删除，
/// 且仍用注册表原始名。避免「配对成功」的断言掩盖「未配对时用了小写键」。
#[test]
fn preview_pairs_case_insensitively_and_reports_unmatched_original() {
    let hive = MemoryHive::new(true);
    hive.seed("PairedVar", "same", REG_SZ); // 备份里有（不同大小写）→ 无差异
    hive.seed("OrphanVar", "x", REG_SZ); // 备份里没有 → Removed

    let payload = payload_with(vec![backup_var("PAIREDVAR", "same", REG_SZ)]);

    let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

    assert_eq!(preview.removed, 1, "只有未配对的 OrphanVar 应判删除");
    assert_eq!(preview.added, 0, "PAIREDVAR 已配对，不得判新增");
    assert_eq!(
        preview.changes[0].name, "OrphanVar",
        "删除项须用注册表原始名"
    );
}

/// 两个 hive 都要被扫描，且差异各自带上正确的 hive 标记。
///
/// **可证伪性**：把 `preview_restore_in_stores` 里对 system 的调用删掉，
/// 本测试的 `added` 会从 1 掉到 0（system 侧那条没了）而失败。
#[test]
fn preview_covers_both_hives_and_tags_each_change() {
    let sys = MemoryHive::new(true);
    let usr = MemoryHive::new(true);
    usr.seed("USER_ONLY", "u", REG_SZ);

    let payload = EnvBackupPayload {
        captured_at: 0,
        hives: EnvBackupHives {
            system: vec![backup_var("SYS_NEW", "s", REG_SZ)],
            user: vec![backup_var("USER_ONLY", "u", REG_SZ)],
        },
    };

    let preview = preview_restore_in_stores(&sys, &usr, &payload).unwrap();

    assert_eq!(preview.added, 1);
    assert_eq!(preview.removed, 0);
    let change = preview
        .changes
        .iter()
        .find(|c| c.name == "SYS_NEW")
        .expect("缺 SYS_NEW");
    assert_eq!(
        change.hive,
        EnvHive::System,
        "system 侧的差异必须带 System 标记"
    );
}

/// 枚举失败必须上报为错误（code=`Io`），而不是静默产出空差异。
///
/// **判定只认 `code`**（项目硬约束），不得匹配 message 文本。
#[test]
fn preview_reports_enumeration_failure() {
    let mut hive = MemoryHive::new(true);
    hive.fail_enum = true;

    let err = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload_with(vec![]))
        .expect_err("枚举失败必须上报");

    assert_eq!(err.code, ErrorCode::Io, "枚举失败的错误码必须是 Io");
}

/// 读取并校验备份文件：正常文件往返一致，损坏文件判 `Parse` 并隔离。
#[test]
fn read_env_backup_round_trips_and_quarantines_corrupt() {
    with_temp_backup_dir(|dir| {
        let payload = payload_with(vec![backup_var("JAVA_HOME", "C:\\jdk17", REG_SZ)]);

        // 加密路径：直接调底层读入口并用**同一把**密钥读回
        // （`read_env_backup_any` 走 config_dir()，那是另一把密钥——
        // salt 不同 → payload_hash 必然不匹配，这是正确的拒绝行为）。
        let key_dir = tempfile::TempDir::new().unwrap();
        let key = crate::db::crypto::load_or_create_key(key_dir.path()).unwrap();
        let path = write_env_backup_to_with_key(dir, &payload, &key).expect("写备份失败");
        let back = crate::db::backups::read_env_backup_any(&path, &key).expect("读备份失败");
        assert_eq!(back, payload, "读回的载荷必须与写入的一致");

        // 遗留明文路径也必须仍可读
        let legacy = dir.join("env_backup_20260101_000000_000.json");
        let versioned = Versioned {
            schema_version: PERSIST_SCHEMA_VERSION,
            inner: payload.clone(),
        };
        std::fs::write(&legacy, serde_json::to_string(&versioned).unwrap()).unwrap();
        let back = read_env_backup(&legacy).expect("遗留 .json 必须仍可读");
        assert_eq!(back, payload);

        // 损坏文件：读失败、code=Parse，且被隔离成 .corrupt-*
        let corrupt = dir.join("env_backup_20260101_000000_000.json");
        std::fs::write(&corrupt, "{\"broken\":").unwrap();
        let err = read_env_backup(&corrupt).expect_err("损坏文件必须报错");
        assert_eq!(err.code, ErrorCode::Parse, "损坏文件的错误码必须是 Parse");
        assert!(!corrupt.exists(), "损坏文件必须被隔离（原名不再存在）");
    });
}

/// `preview_restore_file` 必须是「读文件 + 算差异」的组合入口。
///
/// 因为绑定的是真实注册表（`preview_restore` 无存储注入），这里只断言
/// 「链路可用」：写一份备份再读，必须 `Ok`。差异内容本身由注入版测试覆盖。
/// 用只读打开注册表，不写入任何内容。
#[test]
fn preview_restore_file_reads_and_previews() {
    with_temp_backup_dir(|dir| {
        let path = write_env_backup_to(dir, &payload_with(vec![])).expect("写备份失败");
        // 空备份：差异只可能来自真实注册表里「有而备份无」的变量，
        // 无论多少都必须是 Ok（不是 Err）。
        let preview = preview_restore_file(&path).expect("读文件并预览必须成功");
        assert_eq!(
            preview.added, 0,
            "空备份不可能产生新增（备份侧没有任何条目）"
        );
        assert_eq!(preview.modified, 0, "Modified 不可达");
        assert_eq!(
            preview.added + preview.modified + preview.removed + preview.conflicts,
            preview.changes.len(),
            "四个计数之和必须等于差异条数"
        );
    });
}

// ──：恢复执行 ──

/// 构造一个**未配对**的备份条目，revision 必然与注册表当前值不符。
///
/// 与 [`backup_var`]（自洽，配对成功）相对：本函数让备份条目的 revision
/// 由 `revision_value` 算出，而条目携带 `stored_value`，于是
/// `revision_of(name, vtype, stored_value) != revision` 恒成立
/// —— `diff_one_hive` 必然判 `Conflict`。这是唯一能让保存的值与
/// revision 不一致的构造方式，也就是「备份已过期」的真实形态。
fn stale_backup_var(
    name: &str,
    stored_value: &str,
    revision_value: &str,
    kind: EnvValueKind,
) -> EnvBackupVar {
    let vtype = match kind {
        EnvValueKind::String => REG_SZ,
        _ => REG_EXPAND_SZ,
    };
    EnvBackupVar {
        name: name.into(),
        kind,
        value: stored_value.into(),
        revision: revision_of(name, vtype, revision_value),
    }
}

/// `outcome.failures` 里是否存在一条点名 `name` 的条目。
///
/// 失败文本采用 core 全库统一的 `"[{hive标签}] {name}: {message}"` 形态，
/// 因此「点名」可直接由包含 `name` 判定。**失败的具体原因不由本函数判定** ——
/// 判定只认 `CoreError.code`，不得匹配 message 文本；「该输入确实走进了
/// 哪个错误分支」由独立的前置条件测试（如
/// `windir_seed_really_hits_protected_branch`）钉住。
fn failure_names(outcome: &RestoreOutcome, name: &str) -> bool {
    outcome.failures.iter().any(|f| f.contains(name))
}

/// 前置条件核对：确认「保护名单」这一取值真的走进 `ErrorCode::Protected` 分支。
///
/// 存在的理由（断言可证伪性）：`RestoreOutcome.failures` 只承载一条文本，
/// 任何原因（类型不支持、revision 冲突、名称非法）都会让变量出现在 failures 里。
/// 若取值不当，测试会「通过」却根本没测到保护名单。这里用一个**独立于恢复路径**
/// 的同名调用直接取出 `CoreError.code`，把「该输入确实命中保护名单」钉死。
///
/// **覆盖恢复会走的全部三个写函数**：保护名单判定在
/// `create_env_var_in_store` / `update_env_var_force_in_store` /
/// `delete_env_var_force_in_store` 里是**三处独立代码**。只核对一条
/// 会漏掉另外两条 —— 那正是「测试看似通过但测不到东西」的典型形态。
///
/// 可证伪性：把 `windir` 从 `PROTECTED_NAMES`（`core/src/env_var.rs`）删掉，
/// 三个断言全部失败。
#[test]
fn windir_seed_really_hits_protected_branch() {
    let hive = MemoryHive::new(true);
    hive.seed("windir", "C:\\Windows", REG_SZ);

    let created = create_env_var_in_store(
        &hive,
        EnvHive::User,
        "windir",
        "C:\\evil",
        EnvValueKind::String,
    )
    .expect_err("windir 必须被拒绝（create 分支的前置条件）");
    assert_eq!(
        created.code,
        ErrorCode::Protected,
        "create 分支走到的必须是保护名单，而不是类型/名称等其它分支；实际: {created:?}"
    );

    let updated = update_env_var_force_in_store(&hive, EnvHive::User, "windir", "C:\\evil")
        .expect_err("windir 必须被拒绝（update 分支的前置条件）");
    assert_eq!(
        updated.code,
        ErrorCode::Protected,
        "update 分支走到的必须是保护名单；实际: {updated:?}"
    );

    let deleted = delete_env_var_force_in_store(&hive, EnvHive::User, "windir")
        .expect_err("windir 必须被拒绝（delete 分支的前置条件）");
    assert_eq!(
        deleted.code,
        ErrorCode::Protected,
        "delete 分支走到的必须是保护名单；实际: {deleted:?}"
    );
}

/// 默认模式遇到冲突必须中止，且**不做任何写入**（注册表零改动）。
///
/// 可证伪性：
/// - 删掉 `restore_in_stores` 里的冲突中止分支 → 本函数返回 `Ok`，`expect_err` 失败；
/// - 把中止**放在写入之后**（先写后判）→ `NEW_ONE` 会被创建，`!contains` 断言失败。
#[test]
fn restore_aborts_on_conflict_without_writing() {
    with_temp_backup_dir(|dir| {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "changed-by-other-tool", REG_SZ);
        hive.seed("UNTOUCHED", "keep-me", REG_SZ);

        // MY_VAR：注册表值≠备份值 → Conflict。
        // NEW_ONE：备份有、注册表无 → Added（若实现「先写后判」就会写进去）。
        let payload = payload_with(vec![
            stale_backup_var("MY_VAR", "backed-up", "backed-up", EnvValueKind::String),
            backup_var("NEW_ONE", "n", REG_SZ),
        ]);
        let path = write_env_backup_to(dir, &payload).unwrap();

        let err = restore_in_stores(&MemoryHive::new(true), &hive, &path, false)
            .expect_err("冲突必须中止");
        assert_eq!(err.code, ErrorCode::Conflict, "中止的错误码必须是 Conflict");

        assert_eq!(
            String::from_reg_value(&hive.get_raw("MY_VAR").unwrap()).unwrap(),
            "changed-by-other-tool",
            "冲突时不得写入"
        );
        assert!(
            !hive.contains("NEW_ONE"),
            "中止时不得部分写入：NEW_ONE 也不应被创建"
        );
        assert_eq!(
            String::from_reg_value(&hive.get_raw("UNTOUCHED").unwrap()).unwrap(),
            "keep-me",
            "未参与差异的变量必须原样保留"
        );
    });
}

/// `--force` 模式下冲突被覆盖，写入成功（`Conflict` 按 `Modified` 处理）。
///
/// 可证伪性：把 `match` 里 `Modified | Conflict` 的合并写回只处理 `Modified`
/// ，冲突变量的分支会落空
/// ——`applied` 掉到 0 且值停在 `"changed-by-other-tool"`，两条断言同时失败。
#[test]
fn restore_force_overwrites_conflict() {
    with_temp_backup_dir(|dir| {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "changed-by-other-tool", REG_SZ);

        let payload = payload_with(vec![stale_backup_var(
            "MY_VAR",
            "backed-up",
            "backed-up",
            EnvValueKind::String,
        )]);
        let path = write_env_backup_to(dir, &payload).unwrap();

        let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
            .expect("force 恢复必须成功");
        assert_eq!(outcome.applied, 1, "冲突变量在 force 下必须被覆盖写入");
        assert!(
            outcome.failures.is_empty(),
            "force 覆盖冲突不应产生失败: {:?}",
            outcome.failures
        );
        assert_eq!(
            String::from_reg_value(&hive.get_raw("MY_VAR").unwrap()).unwrap(),
            "backed-up"
        );
    });
}

/// S6：`--force` 不豁免保护名单 —— 保护名单变量出现在差异中，
/// 但写入阶段被 core 拒绝并记入 failures，且**不中止其余变量的恢复**。
///
/// 核对轮 E5 裁断：preview 不为保护名单新增枚举变体、不预排除；
/// 拒绝发生在写入阶段（core 写函数内是保护名单的唯一真相源）。
///
/// 可证伪性：把 `restore_in_stores` 的逐条失败改成 `?` 提前返回 → NORMAL_ONE
/// 不会被创建（`hive.contains` 失败）且整个函数返回 `Err`（`expect` 失败）。
#[test]
fn restore_force_does_not_bypass_protected_names() {
    with_temp_backup_dir(|dir| {
        let hive = MemoryHive::new(true);
        hive.seed("windir", "C:\\Windows", REG_SZ);

        // windir：当前值 "C:\\Windows" 与备份 revision（由 "attacker-value" 算）不符
        // → Conflict；force 下按 Modified 处理 → 进 update_env_var_force_in_store
        // → 命中保护名单，返回 ErrorCode::Protected（见上方前置条件测试）。
        // NORMAL_ONE：备份有、注册表无 → Added，用来证明保护名单失败**不中止**其余恢复。
        let payload = payload_with(vec![
            stale_backup_var("windir", "C:\\evil", "attacker-value", EnvValueKind::String),
            backup_var("NORMAL_ONE", "ok", REG_SZ),
        ]);
        let path = write_env_backup_to(dir, &payload).unwrap();

        let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
            .expect("保护名单被拒不得使整个恢复失败");

        assert_eq!(outcome.applied, 1, "只有 NORMAL_ONE 应写入成功");
        assert_eq!(
            outcome.failures.len(),
            1,
            "windir 必须被记为失败，实际: {:?}",
            outcome.failures
        );
        assert!(
            failure_names(&outcome, "windir"),
            "失败原因必须点名 windir，实际: {:?}",
            outcome.failures
        );
        assert_eq!(
            String::from_reg_value(&hive.get_raw("windir").unwrap()).unwrap(),
            "C:\\Windows",
            "保护名单变量的值必须保持原样"
        );
        assert!(hive.contains("NORMAL_ONE"), "其余变量必须照常恢复");
    });
}

/// E5 的对应断言：保护名单变量**必须出现在 preview 差异里**（不预排除）。
#[test]
fn preview_includes_protected_names() {
    let hive = MemoryHive::new(true);
    hive.seed("windir", "C:\\Windows", REG_SZ);

    let payload = payload_with(vec![stale_backup_var(
        "windir",
        "C:\\evil",
        "attacker-value",
        EnvValueKind::String,
    )]);

    let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();
    assert_eq!(
        preview.conflicts, 1,
        "保护名单变量必须出现在差异中，不被预先排除"
    );
    assert!(preview.changes.iter().any(|c| c.name == "windir"));
}

/// 恢复新增与删除两类差异。
///
/// 可证伪性：把 `Added` 分支改调 `update_env_var_force_in_store`（对不存在的
/// 变量必失败）→ `applied` 掉到 1 且 `failures` 非空；把 `Removed` 分支写反
/// （调 create）→ `WILL_BE_REMOVED` 仍在。
#[test]
fn restore_creates_and_removes() {
    with_temp_backup_dir(|dir| {
        let hive = MemoryHive::new(true);
        hive.seed("WILL_BE_REMOVED", "x", REG_SZ);

        let payload = payload_with(vec![backup_var("WILL_BE_CREATED", "created", REG_SZ)]);
        let path = write_env_backup_to(dir, &payload).unwrap();

        let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true).unwrap();
        assert_eq!(outcome.applied, 2, "一个新增 + 一个删除");
        assert!(
            outcome.failures.is_empty(),
            "正常恢复不应有失败: {:?}",
            outcome.failures
        );
        assert!(hive.contains("WILL_BE_CREATED"));
        let created = hive.get_raw("WILL_BE_CREATED").expect("新变量应存在");
        assert_eq!(String::from_reg_value(&created).unwrap(), "created");
        assert!(!hive.contains("WILL_BE_REMOVED"));
    });
}

/// 删除走 `delete_env_var_force_in_store`：**保护名单的删除同样被拒**。
///
/// 与 `restore_force_does_not_bypass_protected_names`（覆盖写入分支）配对 ——
/// 三个 `*_in_store` 的保护名单判定是**三处独立代码**，只测一处会漏掉删除侧。
///
/// 可证伪性：把 `Removed` 分支改调 `store.delete_value` 裸删（S6 禁止的写法）
/// → `windir` 被删掉，`contains` 断言失败。
#[test]
fn restore_removal_does_not_bypass_protected_names() {
    with_temp_backup_dir(|dir| {
        let hive = MemoryHive::new(true);
        hive.seed("windir", "C:\\Windows", REG_SZ);
        hive.seed("NORMAL_ONE", "ok", REG_SZ);

        // 备份里两个变量都没有 → 都应判 Removed
        let payload = payload_with(vec![]);
        let path = write_env_backup_to(dir, &payload).unwrap();

        let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
            .expect("保护名单被拒不得使整个恢复失败");

        assert_eq!(outcome.applied, 1, "只有 NORMAL_ONE 应被删除");
        assert_eq!(
            outcome.failures.len(),
            1,
            "windir 必须被记为失败，实际: {:?}",
            outcome.failures
        );
        assert!(
            failure_names(&outcome, "windir"),
            "失败原因必须点名 windir，实际: {:?}",
            outcome.failures
        );
        assert!(hive.contains("windir"), "保护名单变量不得被删除");
        assert!(!hive.contains("NORMAL_ONE"), "其余变量必须照常删除");
    });
}

/// 单 hive 失败不中止另一 hive，且已完成的 hive **不回滚**（best-effort）。
///
/// 用 `fail_enum` 注入 user 侧枚举失败（`diff_one_hive` 会返回 `Err(Io)`）。
/// 断言的是**顺序契约**：`preview_restore_in_stores` 先算 user 再算 system、
/// 任一失败即整体 `Err`，因此 user 侧注入失败时 system 侧的差异**根本没被算出来**，
/// 恢复必然零写入 —— 这正是「枚举失败必中止」在跨 hive 维度的表现
/// （不产生半个 hive 已被改动的中间状态）。
///
/// 反向那一半（user 差异能算出、system 失败）由
/// `restore_preview_failure_in_second_hive_leaves_first_untouched` 覆盖；
/// 「单变量失败不中止整体」由
/// `restore_collects_per_variable_failure_without_aborting` 覆盖。
#[test]
fn restore_reports_hive_enumeration_failure() {
    with_temp_backup_dir(|dir| {
        let mut usr = MemoryHive::new(true);
        usr.fail_enum = true;
        usr.seed("USER_VAR", "u", REG_SZ);
        let sys = MemoryHive::new(true);

        let payload = payload_with(vec![backup_var("USER_NEW", "n", REG_SZ)]);
        let path = write_env_backup_to(dir, &payload).unwrap();

        let err = restore_in_stores(&sys, &usr, &path, true)
            .expect_err("枚举失败必须中止，不得产出半个 hive 的写入");
        assert_eq!(err.code, ErrorCode::Io, "枚举失败的错误码必须是 Io");
        assert!(
            !usr.contains("USER_NEW"),
            "中止时不得写入 —— user 侧的备份变量一个都不该被创建"
        );
    });
}

/// 差异计算在 user 侧失败时，**system 侧的写入一个都不能发生**。
///
/// 这条与上一条互补：上一条用 `fail_enum`（根本没拿到 user 差异），
/// 本条让 user 的差异**能算出来**（有 Added），再让 system 侧枚举失败 ——
/// 断言的是「绝不因后半段失败而留下前半段已改动的状态」。
#[test]
fn restore_preview_failure_in_second_hive_leaves_first_untouched() {
    with_temp_backup_dir(|dir| {
        let usr = MemoryHive::new(true);
        let mut sys = MemoryHive::new(true);
        sys.fail_enum = true;

        // user 侧有一条真实的 Added 差异；system 侧枚举必然失败
        let payload = payload_with(vec![backup_var("USER_NEW", "n", REG_SZ)]);
        let path = write_env_backup_to(dir, &payload).unwrap();

        let err = restore_in_stores(&sys, &usr, &path, true)
            .expect_err("system 侧枚举失败必须让整次恢复中止");
        assert_eq!(err.code, ErrorCode::Io);
        assert!(
            !usr.contains("USER_NEW"),
            "system 侧失败时，user 侧已算出的差异也不得被写入（不留半个改动的注册表）"
        );
    });
}

/// 单个变量写入失败不中止其余变量 —— 与保护名单用例互补的一格。
///
/// 这里用**同批两个变量、其中一个名称非法**（含 `=`）来触发失败：
/// 名称非法是 `validate_env_name` 的判定，与保护名单是不同分支，
/// 合起来才覆盖「逐条失败被收集而非整体中止」这条契约。
///
/// 可证伪性：把逐条失败改回 `?` → 函数返回 `Err`，`expect` 失败。
#[test]
fn restore_collects_per_variable_failure_without_aborting() {
    with_temp_backup_dir(|dir| {
        let hive = MemoryHive::new(true);

        // 含 `=` 的名称对 `create_env_var_in_store` 非法（InvalidName）。
        // 备份侧能装下这种条目（备份是外部输入，不受写入校验约束）。
        let payload = payload_with(vec![
            backup_var("BAD=NAME", "x", REG_SZ),
            backup_var("GOOD_ONE", "ok", REG_SZ),
        ]);
        let path = write_env_backup_to(dir, &payload).unwrap();

        let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
            .expect("单变量失败不得使整次恢复失败");

        assert_eq!(outcome.applied, 1, "只有 GOOD_ONE 应写入成功");
        assert_eq!(
            outcome.failures.len(),
            1,
            "非法名必须被记为失败，实际: {:?}",
            outcome.failures
        );
        assert!(
            failure_names(&outcome, "BAD=NAME"),
            "失败原因必须点名 BAD=NAME，实际: {:?}",
            outcome.failures
        );
        assert!(hive.contains("GOOD_ONE"), "其余变量必须照常恢复");
        assert!(!hive.contains("BAD=NAME"), "非法名不得写入");
    });
}

/// **空备份的后果被显式固定**：两个 hive 皆空的备份会删光当前可写变量。
///
/// 这是 `EnvBackupHives` 的 `system` / `user` 带 `#[serde(default)]`
/// 的直接后果：形如
/// `{"capturedAt":1,"hives":{}}` 的文件能反序列化成功，得到两个空列表，
/// 差异计算于是把当前注册表里每个变量都判成 `Removed`。
///
/// **裁定理由只写在 [`restore_in_stores`] 的 `# 空备份` 段，此处不复制** ——
/// 同一条论证存两份必然漂移（复审已在同一处风险上抓到过逐字幸存的旧措辞）。
/// 需要理由读那段；本测试只负责把**后果**钉住。
///
/// 特别注意**不要**在这里写「默认模式会保护」之类的说法：
/// [`restore_empty_backup_without_force_also_removes_all`] 断言
/// `force == false` 时**同样删空** —— 空备份零冲突，中止不会触发。
///
/// 若将来有人加了防护，本测试会失败并强制其显式确认语义变更。
///
/// 可证伪性：给恢复层加上「两 hive 皆空则拒绝」，本测试的 `expect` 立即失败。
#[test]
fn restore_empty_backup_removes_all_current_variables() {
    with_temp_backup_dir(|dir| {
        let hive = MemoryHive::new(true);
        hive.seed("KEEP_A", "a", REG_SZ);
        hive.seed("KEEP_B", "b", REG_SZ);

        let path = write_env_backup_to(dir, &payload_with(vec![])).unwrap();

        let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
            .expect("空备份是合法输入，恢复按差异语义执行");

        assert_eq!(outcome.applied, 2, "两个现存变量都应被判为删除");
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        assert!(!hive.contains("KEEP_A"), "空备份 → 当前变量被删除");
        assert!(!hive.contains("KEEP_B"), "空备份 → 当前变量被删除");
    });
}

/// 空备份在**默认模式**下的行为同样被钉住：无冲突 → 不中止，照常删空。
///
/// 与上一条配对：上一条走 `--force`，本条的 `force=false` 说明
/// 「空备份删光变量」**不是 force 专属行为**，因此不能靠「不加 --force 就安全」
/// 来规避 —— 也正因如此，文档必须如实写明这一后果。
#[test]
fn restore_empty_backup_without_force_also_removes_all() {
    with_temp_backup_dir(|dir| {
        let hive = MemoryHive::new(true);
        hive.seed("KEEP_A", "a", REG_SZ);

        let path = write_env_backup_to(dir, &payload_with(vec![])).unwrap();

        let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, false)
            .expect("空备份无冲突，默认模式不得中止");

        assert_eq!(outcome.applied, 1);
        assert!(!hive.contains("KEEP_A"), "默认模式下空备份同样会删空");
    });
}

/// 无操作路径（no-op restore）：备份与注册表完全一致时必须**零写入、零失败**，
/// 且不触发广播门控。
///
/// 存在的理由（复审 Minor）：`RestoreOutcome.skipped` 的恒 0 与
/// [`restore_should_broadcast`] 的门控原先**只靠阅读保证**，没有任何测试覆盖
/// `applied == 0` 这条路径。空备份测试只覆盖「全删」，覆盖不到「无事可做」。
///
/// 构造方式：备份条目由 [`backup_var`] 生成（revision 与自身 name/type/value
/// 自洽），注册表里是**同样的值** → `diff_one_hive` 走「revision 相同 → 不进差异」
/// 分支（`core/src/backup.rs` 的 `Some((_, current_revision)) if ... == &var.revision`），
/// 又因备份侧 `seen` 抑制了 `Removed`，于是 `changes.len() == 0`。
///
/// **可证伪性**：
/// - 把「revision 相同 → 不进差异」改错（例如恒 push `Modified`），`changes` 非空
///   → `applied` 不再为 0，第一条断言失败，广播门控也随之变 true；
/// - 把 `restore_should_broadcast` 改成恒 `true`，最后一条断言失败。
#[test]
fn restore_noop_does_not_broadcast() {
    with_temp_backup_dir(|dir| {
        let hive = MemoryHive::new(true);
        hive.seed("SAME_A", "a", REG_SZ);
        hive.seed("SAME_B", "b", REG_EXPAND_SZ);

        // 两个条目都与注册表当前值/类型一致 → 零差异
        let payload = payload_with(vec![
            backup_var("SAME_A", "a", REG_SZ),
            backup_var("SAME_B", "b", REG_EXPAND_SZ),
        ]);
        let path = write_env_backup_to(dir, &payload).unwrap();

        let before = sorted_dir_names(dir);
        let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
            .expect("零差异的恢复必须成功（不是错误）");

        assert_eq!(outcome.applied, 0, "两端一致 → 不应有任何写入");
        assert_eq!(outcome.skipped, 0, "skipped 恒 0：没有「跳过」这条路径");
        assert!(
            outcome.failures.is_empty(),
            "零差异不应产生失败: {:?}",
            outcome.failures
        );
        // 前置条件：确认走的是「零差异」而不是「碰巧写失败」
        let preview =
            preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).expect("预览失败");
        assert_eq!(
            preview.changes.len(),
            0,
            "前置条件：值全同的备份必须产生零差异，否则本测试测的不是 no-op 路径"
        );
        assert!(
            !restore_should_broadcast(&outcome),
            "applied == 0 时不得广播 —— 什么都没变不该惊动已运行的进程"
        );
        // 顺带钉住 no-op 也不产生备份（与 restore_does_not_create_new_backup 互补）
        assert_eq!(
            sorted_dir_names(dir),
            before,
            "no-op 恢复同样不得产生新备份"
        );
    });
}

/// 广播门控的正向配对：`applied > 0` 时**必须**广播。
///
/// 与 `restore_noop_does_not_broadcast` 合起来才构成完整契约
/// 「有写入 → 广播，无写入 → 不广播」。只测一侧等于把恒 `true`/恒 `false`
/// 这两种退化实现漏掉其一。
#[test]
fn restore_should_broadcast_when_something_applied() {
    let applied = RestoreOutcome {
        applied: 3,
        skipped: 0,
        failures: vec![],
    };
    assert!(
        restore_should_broadcast(&applied),
        "有实际写入时必须广播，否则运行中的进程会持有陈旧变量"
    );

    // 失败但确有写入（partial success）同样必须广播
    let partial = RestoreOutcome {
        applied: 1,
        skipped: 0,
        failures: vec!["[用户] windir: 受保护".into()],
    };
    assert!(
        restore_should_broadcast(&partial),
        "部分成功也算改动过注册表，必须广播"
    );
}

/// 恢复**不产生新备份**：整个恢复过程中备份目录里的文件数不变。
///
/// 设计文档明文要求（「恢复自己不产生新备份」，否则每次恢复都新增文件、
/// 与保留策略互相吞噬）。断言方式是直接的：把备份目录指向可写的临时目录
/// （生产路径下 `backup_before_write()` 必然写进这里），恢复后目录内容必须
/// **逐字节不变** —— 若实现里误接上写前备份，目录会多出一份 `env_backup_*.json`。
///
/// 可证伪性：在 `restore_in_stores` 里加一次 `backup_env_vars()`，本测试失败。
#[test]
fn restore_does_not_create_new_backup() {
    with_temp_backup_dir(|dir| {
        let hive = MemoryHive::new(true);
        hive.seed("OLD_VAR", "old", REG_SZ);

        let payload = payload_with(vec![backup_var("NEW_VAR", "n", REG_SZ)]);
        let path = write_env_backup_to(dir, &payload).unwrap();

        let before: Vec<String> = sorted_dir_names(dir);
        assert_eq!(before.len(), 1, "前置条件：目录里只有我们写的那一份备份");

        restore_in_stores(&MemoryHive::new(true), &hive, &path, true).expect("恢复必须成功");

        let after = sorted_dir_names(dir);
        assert_eq!(
            after, before,
            "恢复不得产生新备份文件（否则与保留策略互相吞噬）"
        );
    });
}

/// 备份目录内的文件名，排序后返回（用于「目录内容不变」的逐字节比较）。
fn sorted_dir_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("读取备份目录失败")
        .map(|e| {
            e.expect("目录项失败")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// `Removed` 条目的顺序必须**可复现**。
///
/// 多个 `Removed` 条目来自 `HashMap` 迭代，`RandomState` 让顺序随进程变化；
/// 排序后必须恒定。断言方式：同一份输入构造**多份独立注册表快照**（每份新建
/// `MemoryHive`，插入顺序不同），差异列表的名字序列必须逐项相同。
///
/// **可证伪性（重要）**：光比对两次结果会因 `HashMap` 的哈希随机化在
/// 单进程内的**种子复用**而漏检 —— 同一进程里 `RandomState` 的密钥是线程局部
/// 缓存后复用的，两次遍历顺序可能恰好一致。因此这里刻意**改变插入顺序**
/// 并把断言放在「跨 hive 的相对位置」上：未排序时 user 段的 `Removed`
/// 会随插入顺序散开，排序后恒为升序 —— 删掉 `sort_changes` 调用即失败。
#[test]
fn preview_removed_entries_are_deterministically_sorted() {
    let payload = payload_with(vec![
        backup_var("A_ADDED", "a", REG_SZ),
        backup_var("Z_ADDED", "z", REG_SZ),
    ]);

    // 两份注册表：同样五个变量，**插入顺序相反**。未排序时
    // `Removed` 的相对顺序会跟着插入顺序（HashMap 迭代受它影响）漂移。
    let forward = MemoryHive::new(true);
    for name in [
        "B_REMOVED",
        "C_REMOVED",
        "D_REMOVED",
        "E_REMOVED",
        "F_REMOVED",
    ] {
        forward.seed(name, "x", REG_SZ);
    }
    let backward = MemoryHive::new(true);
    for name in [
        "F_REMOVED",
        "E_REMOVED",
        "D_REMOVED",
        "C_REMOVED",
        "B_REMOVED",
    ] {
        backward.seed(name, "x", REG_SZ);
    }

    let names = |hive: &MemoryHive| -> Vec<String> {
        preview_restore_in_stores(&MemoryHive::new(true), hive, &payload)
            .expect("预览失败")
            .changes
            .iter()
            .map(|c| format!("{:?}:{:?}", c.kind, c.name))
            .collect()
    };

    let first = names(&forward);
    let second = names(&backward);

    assert_eq!(
        first, second,
        "同一份差异集合必须给出同一顺序（Removed 来自 HashMap，未排序则不可复现）"
    );
    // 顺序本身也钉住：先 Added（按名升序）、再 Removed（按名升序）。
    assert_eq!(
        first,
        vec![
            "Added:\"A_ADDED\"",
            "Added:\"Z_ADDED\"",
            "Removed:\"B_REMOVED\"",
            "Removed:\"C_REMOVED\"",
            "Removed:\"D_REMOVED\"",
            "Removed:\"E_REMOVED\"",
            "Removed:\"F_REMOVED\"",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>(),
        "排序键是 (hive, kind, name)，kind 序为 added → modified → removed → conflict"
    );
}

/// user 段必须整体在 system 段之前，且排序后仍然成立（契约回归）。
///
/// 可证伪性：把 [`hive_rank`] 的取值对调，本测试失败。
#[test]
fn preview_changes_keep_user_before_system_after_sort() {
    let sys = MemoryHive::new(true);
    let usr = MemoryHive::new(true);
    usr.seed("Z_USER", "u", REG_SZ); // user 侧 Removed
    sys.seed("A_SYS", "s", REG_SZ); // system 侧 Removed

    let payload = EnvBackupPayload {
        captured_at: 0,
        hives: EnvBackupHives {
            system: vec![],
            user: vec![],
        },
    };

    let preview = preview_restore_in_stores(&sys, &usr, &payload).expect("预览必须成功");

    assert_eq!(preview.removed, 2);
    assert_eq!(
        preview.changes[0].hive,
        EnvHive::User,
        "user 段必须整体在前（即便其变量名 A_SYS < Z_USER 也不能让 system 插队）"
    );
    assert_eq!(preview.changes[1].hive, EnvHive::System);
}

// ── ：path 备份保留策略 ──

/// path 轮换只删 `path_backup_*.txt`，保留最近 keep 份。
#[test]
fn rotate_path_backups_keeps_exactly_keep_files() {
    with_temp_backup_dir(|dir| {
        for i in 0..25 {
            std::fs::write(
                dir.join(format!("path_backup_202609{:02}_120000_000.txt", i)),
                "PATH",
            )
            .unwrap();
        }
        let removed = rotate_path_backups(dir, PATH_BACKUP_KEEP).expect("轮换失败");
        let remaining = std::fs::read_dir(dir).unwrap().count();
        assert_eq!(remaining, PATH_BACKUP_KEEP, "必须恰好保留 keep 份");
        assert_eq!(removed.len(), 5);
    });
}

/// 对抗性测试：轮换**不得**删除 env 备份或任何非 path_backup 文件。
///
/// 这是对向断言：env 侧既有测试证明「env 轮换不删 path 文件」，
/// 本用例证明「path 轮换不删 env 文件」——两个方向都必须成立。
#[test]
fn rotate_path_backups_never_deletes_foreign_files() {
    with_temp_backup_dir(|dir| {
        for i in 0..25 {
            std::fs::write(
                dir.join(format!("path_backup_202609{:02}_120000_000.txt", i)),
                "PATH",
            )
            .unwrap();
        }
        // 以下全部不得被删
        std::fs::write(dir.join("env_backup_20260901_120000_000.json"), "{}").unwrap();
        std::fs::write(dir.join("path_backup_20260901_120000_000.txt.bak"), "PATH").unwrap();
        std::fs::write(
            dir.join("path_backup_20260901_120000_000.txt.corrupt-123"),
            "x",
        )
        .unwrap();
        std::fs::write(dir.join("notes.txt"), "user file").unwrap();

        rotate_path_backups(dir, PATH_BACKUP_KEEP).expect("轮换失败");

        assert!(
            dir.join("env_backup_20260901_120000_000.json").exists(),
            "env 备份必须保留"
        );
        assert!(
            dir.join("path_backup_20260901_120000_000.txt.bak").exists(),
            ".bak 必须保留"
        );
        assert!(
            dir.join("path_backup_20260901_120000_000.txt.corrupt-123")
                .exists(),
            ".corrupt-* 必须保留"
        );
        assert!(dir.join("notes.txt").exists(), "无关文件必须保留");
    });
}

/// `path_backup_keep` 的四种回落与 env 同口径。
#[test]
fn read_path_backup_keep_falls_back_like_env() {
    let missing = std::env::temp_dir().join("patheditor_no_such_cfg_9f3.ini");
    let _ = std::fs::remove_file(&missing);
    assert_eq!(
        read_backup_keep(&missing, "path_backup_keep", PATH_BACKUP_KEEP),
        PATH_BACKUP_KEEP
    );

    for (i, bad) in [
        "path_backup_keep = abc\n",
        "path_backup_keep = -5\n",
        "path_backup_keep =\n",
    ]
    .iter()
    .enumerate()
    {
        let cfg = std::env::temp_dir().join(format!("patheditor_cfg_path_{}.ini", i));
        std::fs::write(&cfg, bad).unwrap();
        assert_eq!(
            read_backup_keep(&cfg, "path_backup_keep", PATH_BACKUP_KEEP),
            PATH_BACKUP_KEEP,
            "非法值应回落: {bad}"
        );
        let _ = std::fs::remove_file(&cfg);
    }
}

/// `env_backup_keep` 的既有解析经 `read_backup_keep` 通用化后逐字不回归。
#[test]
fn read_env_backup_keep_generic_refactor_preserves_behavior() {
    let cfg = std::env::temp_dir().join("patheditor_cfg_env_9f3.ini");
    std::fs::write(&cfg, "; c\n# c2\nenv_backup_keep   =   7   \n").unwrap();
    assert_eq!(
        read_backup_keep(&cfg, "env_backup_keep", ENV_BACKUP_KEEP),
        7
    );
    let _ = std::fs::remove_file(&cfg);
}
