#![cfg(windows)]
// 密钥文件 DACL 校验器的主体判定。
//
// 背景：校验器曾经**按 SDDL 文本**比较主体，而 SDDL 会把众所周知的 SID 写成别名
// （`SY` / `LA` / `BA`…）。CI runner 的进程令牌是 `S-1-5-114`（Local Administrator），
// 转成 SDDL 是 `LA`，与 `current_user_sid_string()` 返回的 `S-1-5-114` 文本不等，
// 于是「只有 SYSTEM + 当前用户」的合法 DACL 被判成「存在非预期主体」。
//
// 现在的实现遍历 ACE、把每个主体的 SID 转成规范字符串再比较。本用例覆盖它的
// **可证伪方向**：额外授予一个真实主体（BUILTIN\Users）后，必须返回 false。
use path_editor_core::db::dacl::key_dacl_is_restricted;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// 用 icacls 给文件追加一条非白名单主体的授权。
///
/// 用系统自带工具而不是复刻一遍 SetFileSecurityW：本用例要验证的是**读取侧**
/// 的判定，写入侧越简单越不容易把自己的 bug 写进夹具。
fn grant_users_read(path: &Path) -> bool {
    Command::new("icacls")
        .arg(path)
        .args(["/grant", "*S-1-5-32-545:(R)"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[test]
fn dacl_checker_rejects_extra_principal() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("dacl_probe.bin");
    std::fs::write(&file, b"probe").unwrap();

    // 先把 DACL 收敛到与密钥文件相同的形态（SYSTEM + 当前用户），
    // 否则默认继承的 ACL 里本来就有 Users，测不出「追加后变宽」的差异。
    let user_sid = String::from_utf16_lossy(
        &path_editor_core::db::dacl::current_user_sid_for_test().expect("读取当前用户 SID 失败"),
    );
    let sddl = format!("D:P(A;;GA;;;SY)(A;;GA;;;{user_sid})");
    assert!(
        path_editor_core::db::dacl::set_dacl_for_test(&file, &sddl).is_ok(),
        "设置夹具 DACL 失败"
    );

    assert!(
        key_dacl_is_restricted(&file).expect("读取 DACL 失败"),
        "只授权 SYSTEM + 当前用户时必须判定为受限"
    );

    // 正例之后必须变负例，否则「永远返回 true」也会通过上面的断言。
    if !grant_users_read(&file) {
        // 极少数环境没有 icacls（或受限），此时如实跳过可证伪的一半，
        // 但不能静默通过：用 panic 提醒该用例在当前环境覆盖不足。
        panic!("icacls 不可用或授权失败，无法构造「多一个主体」的负例");
    }

    assert!(
        !key_dacl_is_restricted(&file).expect("读取 DACL 失败"),
        "额外授予 BUILTIN\\Users 后必须判定为不受限"
    );
}
