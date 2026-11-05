use std::path::PathBuf;
use std::process::Command;

fn main() {
    tauri_build::build();
    embed_manifest_for_test_binaries();
    stage_webview2_loader_for_local_runs();
}

/// 给 **GNU** 本地构建补齐运行期的 `WebView2Loader.dll`。
///
/// ## 为什么只有 GNU 需要
///
/// `webview2-com-sys` 按目标环境二选一（见该 crate `src/lib.rs` 的
/// `link_webview2!` 宏）：
/// - `target_env = "msvc"` → `link(name = "WebView2LoaderStatic", kind = "static")`，
///   加载器被静态链进 exe，**不需要**外部 DLL（发布产物即此路径）；
/// - 其它（本机 GNU）→ `link(name = "WebView2Loader.dll")`，走导入库，
///   exe 会真实导入 `WebView2Loader.dll`。
///
/// 导入库和 DLL 都由该 crate 的构建脚本放进
/// `target/<profile>/build/webview2-com-sys-*/out/x64/`，但 cargo 不会把它复制到
/// exe 旁边。仓库里曾直接跟踪一份 160 KB 的 DLL 来顶这个坑；那份副本对 MSVC 发布
/// 是死文件，且是经典的 DLL 劫持目标名，已删除。改为构建时从依赖的 OUT_DIR 取样，
/// 只动 `target/`，不进索引。
///
/// 失败只告警不中断：DLL 缺失只影响本机直接双击 `target/debug/PathEditor.exe`，
/// 不影响编译产物本身的正确性。
fn stage_webview2_loader_for_local_runs() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // MSVC 静态链接，不需要外部 DLL
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        return;
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR 必须存在"));
    // OUT_DIR = target/<profile>/build/<pkg>-<hash>/out
    let Some(profile_dir) = out_dir.ancestors().nth(3).map(PathBuf::from) else {
        return;
    };
    let build_root = profile_dir.join("build");

    let Ok(entries) = std::fs::read_dir(&build_root) else {
        return;
    };
    let source = entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("webview2-com-sys-")
        })
        .map(|e| e.path().join("out").join("x64").join("WebView2Loader.dll"))
        .find(|p| p.is_file());

    let Some(source) = source else {
        println!(
            "cargo:warning=未找到 webview2-com-sys 的 WebView2Loader.dll（GNU 构建运行时需要它）；\
             `target/<profile>/PathEditor.exe` 可能启动失败"
        );
        return;
    };

    let target = profile_dir.join("WebView2Loader.dll");
    // 已是同一份就跳过：避免每次增量构建都改动文件时间戳
    if std::fs::read(&target).ok() == std::fs::read(&source).ok() {
        return;
    }
    if let Err(e) = std::fs::copy(&source, &target) {
        println!("cargo:warning=复制 WebView2Loader.dll 失败（{e}）");
    }
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
/// - MSVC：**在 `PATHEDITOR_MSVC_TEST_MANIFEST=1` 时**把 `test.manifest` 交给链接器的
///   `/MANIFESTINPUT`。不能无条件注入——bin 已经有 Tauri 的 `resource.lib` 清单，
///   再叠一份会 CVT1100 重复资源。详见下方分支处的注释。
///
/// 两条路的资源身份不同（GNU 侧的 `.rc` 用中性语言 0），因此不会与 Tauri 的
/// `libresource.a` 撞车；MSVC 侧**不参与那份清单**——它只在 bin 不参与链接的那次
/// 调用里，给 lib 的 test harness 补一份。
fn embed_manifest_for_test_binaries() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=test.rc");
    println!("cargo:rerun-if-changed=test.manifest");

    // MSVC：**只在显式要求时注入**。
    //
    // 为什么不能无条件注入：Tauri 通过 `rustc-link-arg-bins=resource.lib` 已经给
    // **bin** 嵌了 MANIFEST 资源；再叠一个 `/MANIFEST:EMBED` 会让 CVTRES 报
    // `CVT1100: duplicate resource. type:MANIFEST, name:1, language:0x0409`。
    //
    // 缺清单的只有 **lib 的 test harness**（拿不到 `-bins` 的参数）。因此约定：
    // 单独跑 `cargo test -p patheditor --lib` 时设置环境变量
    // `PATHEDITOR_MSVC_TEST_MANIFEST=1`——那次调用不链接 bin，也就没有 resource.lib，
    // 不会撞车。CI 的 ci.yml 就是按这个两步跑法写的。
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rerun-if-env-changed=PATHEDITOR_MSVC_TEST_MANIFEST");
        if std::env::var_os("PATHEDITOR_MSVC_TEST_MANIFEST").is_some() {
            let manifest = PathBuf::from(
                std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 必须存在"),
            )
            .join("test.manifest");
            // `/MANIFESTINPUT` 要绝对路径：链接时的工作目录不是 crate 根。
            println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
            println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        }
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
