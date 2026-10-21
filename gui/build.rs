use std::path::PathBuf;
use std::process::Command;

fn main() {
    tauri_build::build();
    embed_manifest_for_test_binaries();
}

/// 给测试二进制嵌入 comctl32 v6 清单。
///
/// ## 问题
///
/// 依赖链里的 `windows-sys` 启用了 `Win32_UI_Controls`，导入
/// `comctl32.dll!TaskDialogIndirect`。该符号**只在 comctl32 v6 里存在**，
/// 而 `C:\Windows\System32\comctl32.dll` 是 v5.82——只有通过 side-by-side
/// 清单请求 v6，加载器才会去 WinSxS 取 v6。
///
/// Tauri 通过 `cargo:rustc-link-arg-bins=libresource.a` 给 **bin** 嵌了清单，
/// 但 `cargo test -p patheditor --lib` 构建的是 **lib 的 test harness**，
/// 拿不到那份清单，于是进程以 `0xC0000139 STATUS_ENTRYPOINT_NOT_FOUND` 退出，
/// 一个测试都没跑。
///
/// ## 为什么用 `rustc-link-arg` 而不是 `-tests`
///
/// Cargo 的 `rustc-link-arg-tests` 只覆盖 `tests/*.rs` 集成测试；本 crate 的
/// 单元测试是 **lib target 以 `--test` 编译**，只有 `rustc-link-arg`（全目标）
/// 才能覆盖。代价是 bin 也会链上这个 `.o`——所以 `test.rc` 用中性语言（0），
/// 与 Tauri 的 1033 清单资源身份不同，链接器不会报重复。
///
/// ## 平台边界
///
/// 只在 Windows 目标生效。非 Windows 没有 comctl32；MSVC 工具链下由链接器
/// 按默认规则处理。
fn embed_manifest_for_test_binaries() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=test.rc");
    println!("cargo:rerun-if-changed=test.manifest");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR 必须存在"));
    let obj = out_dir.join("test_manifest.o");

    // windres 来自 MinGW；本机 Windows 构建时其 bin 已在 PATH
    // （CLAUDE.md 记录：构建期 Tauri GUI 硬依赖 MinGW 的 windres）。
    let status = Command::new("windres")
        .arg("-i")
        .arg("test.rc")
        .arg("-o")
        .arg(&obj)
        .args(["--input-format=rc", "--output-format=coff"])
        .status();

    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg={}", obj.display()),
        Ok(s) => println!(
            "cargo:warning=windres 退出码 {:?}：测试二进制可能缺 v6 清单，\
             会以 0xC0000139 (STATUS_ENTRYPOINT_NOT_FOUND) 退出",
            s.code()
        ),
        Err(e) => {
            println!("cargo:warning=找不到 windres（{e}）：测试二进制可能缺 v6 清单而无法启动")
        }
    }
}
