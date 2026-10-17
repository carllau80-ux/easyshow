//! 在面向 Windows 构建时,把图标与版本信息嵌入 exe(交叉编译场景使用 mingw 的 windres)。
//!
//! 变体由 `heic` feature 决定:关闭时(轻量版)产品名标记为 "EasyShow Lite"。

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let product = if std::env::var_os("CARGO_FEATURE_HEIC").is_some() {
        "EasyShow"
    } else {
        "EasyShow Lite"
    };
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icon.ico");
    let description = format!("{product} - lightweight, fast image viewer");
    res.set("ProductName", product);
    res.set("FileDescription", &description);
    res.set("CompanyName", "EasyShow");
    res.set("LegalCopyright", "MIT OR Apache-2.0");
    if let Err(e) = res.compile() {
        println!("cargo:warning=winresource failed (exe icon skipped): {e}");
    }
}
