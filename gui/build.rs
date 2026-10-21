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
/// 只在 Windows 目标生效。非 Windows 没有 comctl32。
///
/// **MSVC 与 GNU 走两条不同的路**：
/// - GNU：用 MinGW 的 `windres` 把 `test.rc` 编成 COFF 对象再链进去。
/// - MSVC：不编译资源，直接把 `test.manifest` 交给链接器的
///   `/MANIFESTINPUT`。MSVC 的 runner 上通常没有 `windres`，
///   而“没有清单”正是 0xC0000139 的成因，所以这里必须有一条不依赖 MinGW 的路径。
///
/// 两条路的资源身份不同（GNU 侧的 `.rc` 用中性语言 0），因此不会与 Tauri 的
/// `libresource.a` 撞车；MSVC 侧则是把两份清单**合并**，不是替换。
fn embed_manifest_for_test_binaries() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=test.rc");
    println!("cargo:rerun-if-changed=test.manifest");

    // MSVC：直接用链接器的清单开关，不依赖 MinGW。
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 必须存在"),
        )
        .join("test.manifest");
        // `/MANIFESTINPUT` 要绝对路径：链接时的工作目录不是 crate 根。
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        return;
    }

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
        Err(e) => println!(
            "cargo:warning=找不到 windres（{e}）：测试二进制将缺少 comctl32 v6 清单，\
             运行时会以 0xC0000139 (STATUS_ENTRYPOINT_NOT_FOUND) 退出。\
             修复：把 MinGW 的 bin 目录加入 PATH（本机为 \
             `D:\\settings\\Language\\C\\mingw64\\bin`），或安装 mingw-w64 工具链后重新构建。\
             （MSVC 目标不走这条分支，它用链接器的 /MANIFESTINPUT，不需要 windres。）"
        ),
    }
}
