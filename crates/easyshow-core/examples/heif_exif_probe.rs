//! 调试:打印 HEIF EXIF 元数据读取结果。
//! 用法:cargo run -p easyshow-core --example heif_exif_probe -- <文件.heic>

fn main() {
    let path = std::env::args().nth(1).expect("usage: heif_exif_probe <file.heic>");
    let path = std::path::Path::new(&path);
    println!("libheif available: {}", easyshow_core::codec::heif::is_available());
    match easyshow_core::codec::heif::exif_bytes(path) {
        Ok(bytes) => {
            println!("exif_bytes: {} bytes", bytes.len());
            let head = &bytes[..bytes.len().min(24)];
            println!("head: {:02x?}", head);
        }
        Err(e) => println!("exif_bytes ERR: {e}"),
    }
    println!("summary: {:?}", easyshow_core::exif::read_summary(path));
}
