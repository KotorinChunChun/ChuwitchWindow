fn main() {
    tauri_build::build();
    // Tauri標準のアプリ資源はCargo exampleに伝播しないため、撮影ランナーにも
    // TaskDialogIndirectを提供するCommon Controls v6を明示する。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg-examples=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-examples=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
    }
}
