// 一文件一测试：见 M0 §5.3。
//
// **D-1 回归守卫**：停写之后，读路径必须改从 DB 来。
//
// 缺口补齐轮审查报告 D-1：首版 P1-4 只关了写（`write_state` 检查
// `v5_writes_disabled`），没关读（`read_state` 照读文件）。停写让
// `disabled.json` 冻结在迁移那一刻，继续读它 = 读一份永不更新的快照。
//
// 方案 A 的修法：`load_path_snapshot()` 在停写模式下走
// `set_current_snapshot_loader` 注入的加载器（从 DB 取），取不到才回落文件。
//
// **本测试不碰 SQLCipher**（只测 `disabled` 模块的注入机制），
// 因此不受「一文件一测试」的 SQLCipher 约束。
use path_editor_core::disabled;
use path_editor_core::PathEntry;

fn entry(path: &str, enabled: bool) -> PathEntry {
    PathEntry {
        path: path.to_string(),
        enabled,
    }
}

#[test]
fn stop_write_switches_read_to_injected_loader() {
    // ① 注入一个「DB 侧」的加载器。**必须用注册表里真实存在的路径**——
    //    `merge_hive` 会过滤掉注册表里没有的条目（审核窗口的探针正是在这里失败的）
    disabled::set_current_snapshot_loader(Box::new(|| {
        Some((
            vec![entry("C:\\WINDOWS", true)],
            vec![entry("C:\\WINDOWS\\system32", false)],
        ))
    }));

    // ② 停写模式：读必须来自加载器，而不是文件
    disabled::set_v5_writes_disabled(true);
    let snap = disabled::load_path_snapshot().expect("读取失败");

    // 加载器的数据必须出现在结果里（说明读路径真的切到了注入源）
    assert!(
        snap.system.iter().any(|e| e.path == "C:\\WINDOWS"),
        "停写模式下 system 必须来自注入的加载器，实际: {:?}",
        snap.system.iter().map(|e| &e.path).collect::<Vec<_>>()
    );
    // `enabled: false` 的标记必须保留（这是 disabled.json 存在的理由）
    let usr = snap
        .user
        .iter()
        .find(|e| e.path == "C:\\WINDOWS\\system32")
        .expect("user 侧必须来自加载器");
    assert!(!usr.enabled, "禁用标记必须从 DB 侧保留");

    // ③ 复位，避免污染同进程其它测试
    disabled::set_v5_writes_disabled(false);
}
