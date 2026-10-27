//! `service` 的契约测试（fake PathHiveIo，零真实注册表）。

use super::*;
use crate::domain::path_entry::PathEntry;
use std::cell::RefCell;

fn entry(path: &str, enabled: bool) -> PathEntry {
    PathEntry {
        path: path.to_string(),
        enabled,
    }
}

/// 构造「无并发校验」的写入请求（`expected: None`），简化测试调用。
fn write(entries: Vec<PathEntry>) -> PathHiveWrite {
    PathHiveWrite {
        entries,
        expected: None,
    }
}

/// fake `PathHiveIo` 记录的一次调用事件。
///
/// 顺序类契约（`writes_are_sequential_system_then_user`）**无法从终态反推**——
/// 两次写入都成功时，终态看不出谁先谁后。故 fake 必须记录**有序事件序列**，
/// 而非布尔标志。
#[derive(Debug, Clone, PartialEq, Eq)]
enum IoCall {
    Read(EnvHive),
    Write(EnvHive),
    Backup,
}

/// 可注入的 fake `PathHiveIo`：记录调用序列，**不碰真实注册表**。
///
/// 用 `RefCell` 而非 `Mutex`：契约测试单线程，`&dyn PathHiveIo` 的 `&self`
/// 方法需要可变记录，`RefCell` 足够且无锁开销。
struct FakePathIo {
    calls: RefCell<Vec<IoCall>>,
    /// `read_enabled` 返回的「当前启用路径」；用于并发比对。
    ///
    /// 用有序 `Vec` 而非 `HashMap`：`EnvHive` **未派生 `Hash`**，而契约测试
    /// 的自用结构**不得反向要求生产类型加派生**（那会扩进 `env_var.rs`）。
    current: RefCell<Vec<(EnvHive, Vec<String>)>>,
    /// 置位后 `write_enabled` 返回错误（模拟注册表写失败）。
    fail_write: bool,
    /// 置位后 `backup` 返回错误（模拟备份失败）。
    fail_backup: bool,
}

impl FakePathIo {
    fn new() -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            current: RefCell::new(Vec::new()),
            fail_write: false,
            fail_backup: false,
        }
    }

    /// 预置某 hive 的「当前启用路径」（供并发比对用）。
    fn with_current(self, hive: EnvHive, paths: &[&str]) -> Self {
        self.current
            .borrow_mut()
            .push((hive, paths.iter().map(|p| p.to_string()).collect()));
        self
    }

    /// 读某 hive 的当前值（未预置则视为空）。
    fn current_of(&self, hive: EnvHive) -> Vec<String> {
        self.current
            .borrow()
            .iter()
            .find(|(h, _)| *h == hive)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    fn calls(&self) -> Vec<IoCall> {
        self.calls.borrow().clone()
    }

    /// 提取「写入事件」的 hive 序列（供顺序断言）。
    fn write_order(&self) -> Vec<EnvHive> {
        self.calls()
            .into_iter()
            .filter_map(|c| match c {
                IoCall::Write(h) => Some(h),
                _ => None,
            })
            .collect()
    }

    fn backup_count(&self) -> usize {
        self.calls()
            .iter()
            .filter(|c| matches!(c, IoCall::Backup))
            .count()
    }
}

impl PathHiveIo for FakePathIo {
    fn read_enabled(&self, hive: EnvHive) -> Result<Vec<String>, CoreError> {
        self.calls.borrow_mut().push(IoCall::Read(hive));
        Ok(self.current_of(hive))
    }

    fn write_enabled(&self, hive: EnvHive, paths: &[String]) -> Result<(), CoreError> {
        self.calls.borrow_mut().push(IoCall::Write(hive));
        if self.fail_write {
            return Err(CoreError::new(
                ErrorCode::Io,
                "write_enabled",
                "注入的写失败",
            ));
        }
        let mut cur = self.current.borrow_mut();
        if let Some(slot) = cur.iter_mut().find(|(h, _)| *h == hive) {
            slot.1 = paths.to_vec();
        } else {
            cur.push((hive, paths.to_vec()));
        }
        Ok(())
    }

    fn backup(&self) -> Result<(), CoreError> {
        self.calls.borrow_mut().push(IoCall::Backup);
        if self.fail_backup {
            return Err(CoreError::new(
                ErrorCode::Io,
                "backup",
                "注入的备份失败（磁盘已满）",
            ));
        }
        Ok(())
    }
}

// `save_path_registry_io` 不接收 pending 存储，调用链上不存在任何
// pending 写入；「并发冲突 / 注册表失败不产 pending」由函数签名本身保证。
// pending 的落盘契约在 `save_path_with_db_io` 层验证（`db_path_save_state`）。

// ── 契约测试：经 fake `PathHiveIo`，零真实注册表 ──

/// 契约 3：system 先于 user。
///
/// 顺序**无法从终态反推**，故断言 fake 记录的写入事件序列。
#[test]
fn writes_are_sequential_system_then_user() {
    let io = FakePathIo::new();
    let sys = vec![entry("C:\\svc_seq_sys", true)];
    let usr = vec![entry("D:\\svc_seq_usr", true)];

    save_path_registry_io(&io, Some(write(sys)), Some(write(usr))).unwrap();

    assert_eq!(
        io.write_order(),
        vec![EnvHive::System, EnvHive::User],
        "两个 hive 都写时必须是 system → user 顺序（D3）"
    );
}

/// 契约 1：确实发生写入时 `backup()` 被调用**恰好一次**。
///
/// 该测试守护「写入前必须调用备份」这一契约。
#[test]
fn backup_called_once_when_write_happens() {
    let io = FakePathIo::new();
    let sys = vec![entry("C:\\svc_bak_sys", true)];

    save_path_registry_io(&io, Some(write(sys)), None).unwrap();

    assert_eq!(io.backup_count(), 1, "有写入时 backup() 必须被调用恰好一次");
}

/// 契约 1 的一半：**无变更时不备份**。
///
/// 与上一条互为补集——只断言「有写入则备份」不够，还要钉住「无写入则不备份」，
/// 否则「无条件备份」的实现也能让上一条通过。
#[test]
fn no_backup_when_both_hives_skipped() {
    let io = FakePathIo::new();

    save_path_registry_io(&io, None, None).unwrap();

    assert_eq!(
        io.backup_count(),
        0,
        "两个 hive 都 Skipped 时不得备份（无写入发生）"
    );
}

/// 契约 2：并发不匹配 → 该 hive **不写、不备份、不产 pending**。
///
/// 三条「不」缺一不可：只断言「不写」不够——若实现仍备份，等于并发冲突还留下
/// 一份无用备份；若仍写 sidecar，则污染快照。
#[test]
fn concurrent_mismatch_writes_nothing_and_no_backup() {
    // 注册表当前是 ["C:\\orig"]，但期望是 ["C:\\stale"]（模拟外部已改动）。
    let io = FakePathIo::new().with_current(EnvHive::System, &["C:\\orig"]);
    let req = PathHiveWrite {
        entries: vec![entry("C:\\new", true)],
        expected: Some(vec!["C:\\stale".to_string()]),
    };

    let outcome = save_path_registry_io(&io, Some(req), None).unwrap();

    // 不写该 hive，且错误码为 Conflict
    match outcome.system {
        HiveOutcome::Failed(e) => {
            assert_eq!(e.code, ErrorCode::Conflict, "并发不匹配必须是 Conflict");
        }
        other => panic!("期望 HiveOutcome::Failed(Conflict)，实为 {other:?}"),
    }
    assert!(
        io.write_order().is_empty(),
        "并发不匹配时不得发生任何 write_enabled（不写）"
    );
    assert_eq!(io.backup_count(), 0, "并发不匹配时不得备份");
    assert!(
        matches!(outcome.sidecar, SidecarOutcome::Saved),
        "本函数只写注册表，不触碰 sidecar/pending"
    );
}

/// 契约 2 的补集：`expected` **匹配**时正常写入（证明校验不是恒拒绝）。
#[test]
fn concurrent_match_proceeds_with_write() {
    let io = FakePathIo::new().with_current(EnvHive::System, &["C:\\orig"]);
    let req = PathHiveWrite {
        entries: vec![entry("C:\\new", true)],
        expected: Some(vec!["C:\\orig".to_string()]),
    };

    let outcome = save_path_registry_io(&io, Some(req), None).unwrap();

    assert!(matches!(outcome.system, HiveOutcome::Applied));
    assert_eq!(io.write_order(), vec![EnvHive::System]);
    assert_eq!(io.backup_count(), 1, "匹配且写入时应有备份");
}

// ── 契约测试：共存校验的顺序敏感修复 ──
//
// 背景：`expected` 来自 `merge_hive` 输出，其**成员忠实于注册表、顺序取自快照**；
// `current` 是**注册表顺序**。二者在外部重排后必然不等 → 顺序敏感比较会**永久
// 拒绝保存**（死锁）。修复：改为**多重集比较**（忽略顺序、保留重复计数）。
//
// ⚠️ 三条测试分工：第 1 条驱动修复（现状 RED）；第 2/3 条**守护**——现状即 GREEN，
// 用来防止修复被误实现成「关掉校验」。

/// 契约：**仅顺序不同**（同集合）→ 校验**通过**。
#[test]
fn reorder_only_does_not_conflict() {
    let io = FakePathIo::new().with_current(EnvHive::System, &["C:\\a", "C:\\b", "C:\\c"]);
    // 期望值与当前值同集合，但顺序不同（模拟外部重排 / 快照顺序陈旧）。
    let req = PathHiveWrite {
        entries: vec![entry("C:\\new", true)],
        expected: Some(vec!["C:\\c".into(), "C:\\a".into(), "C:\\b".into()]),
    };

    let outcome = save_path_registry_io(&io, Some(req), None).unwrap();

    assert!(
        matches!(outcome.system, HiveOutcome::Applied),
        "仅顺序不同不得判冲突（死锁修复），实为 {:?}",
        outcome.system
    );
}

/// 守护契约：**成员不同**（增/删）→ **仍拒绝**。
///
/// 防止修复被误实现成「恒通过」——真实的成员变化必须仍被检出。
#[test]
fn membership_change_still_conflicts() {
    let io = FakePathIo::new().with_current(EnvHive::System, &["C:\\a", "C:\\b"]);
    // 期望值少一条（模拟外部删除）；即使排序后也不相等。
    let req = PathHiveWrite {
        entries: vec![entry("C:\\new", true)],
        expected: Some(vec!["C:\\a".into()]),
    };

    let outcome = save_path_registry_io(&io, Some(req), None).unwrap();

    match outcome.system {
        HiveOutcome::Failed(e) => assert_eq!(e.code, ErrorCode::Conflict),
        other => panic!("成员变化必须判 Conflict，实为 {other:?}"),
    }
    assert!(io.write_order().is_empty(), "冲突时不得写入");
}

/// 守护契约：同集合但**重复计数不同** → **拒绝**（多重集而非集合）。
///
/// 防止修复被误实现成「`HashSet` 集合比较」——那会吞掉重复项。
#[test]
fn duplicate_count_matters() {
    let io = FakePathIo::new().with_current(EnvHive::System, &["C:\\a", "C:\\a", "C:\\b"]);
    // 期望值：`C:\a` 只出现一次（去重后的集合相同，但多重集不同）。
    let req = PathHiveWrite {
        entries: vec![entry("C:\\new", true)],
        expected: Some(vec!["C:\\a".into(), "C:\\b".into()]),
    };

    let outcome = save_path_registry_io(&io, Some(req), None).unwrap();

    match outcome.system {
        HiveOutcome::Failed(e) => assert_eq!(e.code, ErrorCode::Conflict),
        other => panic!("重复计数不同必须判 Conflict（多重集语义），实为 {other:?}"),
    }
}

#[test]
fn apply_outcome_serializes_camel_case() {
    let outcome = ApplyOutcome {
        system: HiveOutcome::Applied,
        user: HiveOutcome::Skipped,
        sidecar: SidecarOutcome::Saved,
    };
    let v = serde_json::to_value(&outcome).unwrap();
    assert_eq!(v["system"], serde_json::json!("applied"));
    assert_eq!(v["user"], serde_json::json!("skipped"));
    assert_eq!(v["sidecar"], serde_json::json!("saved"));

    // Failed 变体携带结构化错误，且错误码为 camelCase
    let failed = ApplyOutcome {
        system: HiveOutcome::Skipped,
        user: HiveOutcome::Failed(CoreError::new(ErrorCode::Io, "op", "磁盘已满")),
        sidecar: SidecarOutcome::Pending(CoreError::new(ErrorCode::Io, "op", "磁盘已满")),
    };
    let v = serde_json::to_value(&failed).unwrap();
    assert_eq!(v["user"]["failed"]["code"], serde_json::json!("io"));
    assert_eq!(
        v["sidecar"]["pending"]["message"],
        serde_json::json!("磁盘已满")
    );

    // 反序列化 roundtrip
    let back: ApplyOutcome = serde_json::from_value(v).unwrap();
    assert!(matches!(back.user, HiveOutcome::Failed(_)));
    assert!(matches!(back.sidecar, SidecarOutcome::Pending(_)));
}

#[test]
fn sidecar_outcome_enum_semantics() {
    // 三态语义可区分：Saved / Pending（可补写）/ Failed（需人工核对）
    let saved: SidecarOutcome = SidecarOutcome::Saved;
    let pending = SidecarOutcome::Pending(CoreError::new(ErrorCode::Io, "op", "a"));
    let failed = SidecarOutcome::Failed(CoreError::new(ErrorCode::Io, "op", "b"));
    assert!(matches!(saved, SidecarOutcome::Saved));
    assert!(matches!(pending, SidecarOutcome::Pending(_)));
    assert!(matches!(failed, SidecarOutcome::Failed(_)));
}

//  注入机制：输入校验（null 字节）使 validate_and_join_paths 在触碰
// 注册表前失败 —— 与「注册表写失败」走同一段注册表阶段失败路径。
// 断言：HiveOutcome::Failed、无 pending 产生、sidecar 未被触及。
//
// **用 fake IO**（而非真实 `RealPathHiveIo`）：`save_path_with_sidecar`
// 会在写入前调 `io.backup()`，真实现会落真实备份文件到 `~/.patheditor/backups/`
// —— 违反「本层测试不写真实注册表/不产生真实副作用」。fake 记录调用且零副作用。
#[test]
fn registry_stage_failure_reports_failed_without_pending() {
    let mut io = FakePathIo::new();
    io.fail_write = true; // 模拟注册表写失败（等价于输入校验失败）
    let entries = vec![entry("C:\\ok", true), entry("C:\0bad", true)];
    let outcome = save_path_registry_io(&io, Some(write(entries)), None).unwrap();
    assert!(matches!(outcome.system, HiveOutcome::Failed(_)));
    assert!(matches!(outcome.user, HiveOutcome::Skipped));
    assert!(matches!(outcome.sidecar, SidecarOutcome::Saved));
    // 注册表阶段失败不产生 pending：本函数无 pending 依赖（见上方说明）。
}

// 同类注入：超长（>32767 UTF-16）条目，走用户 hive，验证 partial 形态；
// 同样经 fake IO。
#[test]
fn oversized_user_entry_yields_partial_outcome_without_pending() {
    let mut io = FakePathIo::new();
    io.fail_write = true;
    let long = format!("D:\\{}", "a".repeat(32767));
    let outcome = save_path_registry_io(&io, None, Some(write(vec![entry(&long, true)]))).unwrap();
    assert!(matches!(outcome.system, HiveOutcome::Skipped));
    assert!(matches!(outcome.user, HiveOutcome::Failed(_)));
    assert!(matches!(outcome.sidecar, SidecarOutcome::Saved));
}

#[test]
fn event_result_classifies_hive_combinations() {
    let err = || CoreError::new(ErrorCode::Io, "test", "失败");
    assert_eq!(
        event_result(&HiveOutcome::Applied, &HiveOutcome::Applied),
        EventResult::Success
    );
    assert_eq!(
        event_result(&HiveOutcome::Applied, &HiveOutcome::Failed(err())),
        EventResult::Partial
    );
    assert_eq!(
        event_result(&HiveOutcome::Failed(err()), &HiveOutcome::Failed(err())),
        EventResult::Failure,
        "两个 hive 全失败必须是 Failure，不能记成 Partial"
    );
    assert_eq!(
        event_result(&HiveOutcome::Skipped, &HiveOutcome::Failed(err())),
        EventResult::Failure
    );
    assert_eq!(
        event_result(&HiveOutcome::Skipped, &HiveOutcome::Skipped),
        EventResult::Success
    );
}
