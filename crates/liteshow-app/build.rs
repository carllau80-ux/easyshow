//! 在面向 Windows 构建时,把图标与版本信息嵌入 exe(交叉编译场景使用 mingw 的 windres)。

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icon.ico");
    res.set("ProductName", "Liteshow");
    res.set("FileDescription", "Liteshow - lightweight, fast image viewer");
    res.set("CompanyName", "Liteshow");
    res.set("LegalCopyright", "MIT OR Apache-2.0");
    if let Err(e) = res.compile() {
        println!("cargo:warning=winresource failed (exe icon skipped): {e}");
    }
}
