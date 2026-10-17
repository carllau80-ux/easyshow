#!/usr/bin/env bash
# 产出 Linux 分发产物(完整版 + 轻量版)→ dist/linux/
#   easyshow_<ver>_amd64.deb               完整版 .deb(含 HEIC/AVIF,运行时加载系统 libheif)
#   easyshow-lite_<ver>_amd64.deb          轻量版 .deb(不含 HEIC/AVIF 相关代码)
#   easyshow-<ver>-linux-x86_64-full.tar.gz 便携·完整版(解压后执行 packaging/install-local.sh)
#   easyshow-<ver>-linux-x86_64-lite.tar.gz 便携·轻量版(同理,自动识别为 easyshow-lite)
#
# 两个变体可并存安装(easyshow / easyshow-lite 两个包、两个桌面条目)。
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

VERSION="$(awk -F'"' '/^\[workspace.package\]/{f=1} f&&/^version/{print $2; exit}' Cargo.toml)"
DIST="dist/linux"; mkdir -p "$DIST" build/lin

echo ">> 编译轻量版(lite, --no-default-features)…"
cargo build --release -j4 -p easyshow-app --no-default-features 2>&1 | tail -1
install -m755 target/release/easyshow build/lin/easyshow-lite

echo ">> 编译完整版(full, 含 HEIC/AVIF)…"
cargo build --release -j4 -p easyshow-app 2>&1 | tail -1
install -m755 target/release/easyshow build/lin/easyshow

# ---------------- .deb ----------------
# make_deb <包名> <二进制> <英文名> <中文名> <MimeType 集合> <Recommends> <一句描述> <详细描述追加行>
make_deb() {
    local pkg="$1" bin="$2" en_name="$3" display="$4" mimes="$5" recommends="$6" blurb="$7" extra="$8"
    local debroot="build/deb/${pkg}_${VERSION}_amd64"
    rm -rf "$debroot"
    mkdir -p "$debroot"/DEBIAN \
             "$debroot"/usr/bin \
             "$debroot"/usr/share/applications \
             "$debroot"/usr/share/doc/"$pkg" \
             "$debroot"/usr/share/icons/hicolor/{256x256,128x128,64x64}/apps

    install -m755 "$bin" "$debroot/usr/bin/$pkg"
    local s
    for s in 256 128 64; do
        install -m644 "assets/icon-$s.png" "$debroot/usr/share/icons/hicolor/${s}x${s}/apps/$pkg.png"
    done

    cat > "$debroot/usr/share/applications/$pkg.desktop" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=$en_name
Name[zh_CN]=$display
GenericName=Image Viewer
GenericName[zh_CN]=图片查看器
Comment=Lightweight, fast image viewer
Comment[zh_CN]=轻量级快速图片查看器
Exec=/usr/bin/$pkg %F
Icon=$pkg
Terminal=false
Categories=Graphics;Viewer;
MimeType=$mimes
StartupNotify=true
StartupWMClass=$pkg
Keywords=image;viewer;photo;picture;
Keywords[zh_CN]=图片;看图;浏览;照片;
EOF

    cat > "$debroot/usr/share/doc/$pkg/copyright" <<'EOF'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: EasyShow

Files: *
Copyright: 2026 EasyShow contributors
License: MIT or Apache-2.0
 本项目采用 MIT OR Apache-2.0 双许可,完整文本见上游仓库 LICENSE-MIT 与 LICENSE-APACHE。
EOF

    cat > "$debroot/DEBIAN/control" <<EOF
Package: $pkg
Version: $VERSION
Architecture: amd64
Maintainer: EasyShow contributors <easyshow@localhost>
Depends: libc6 (>= 2.34), libgcc-s1
Recommends: $recommends
Section: graphics
Priority: optional
Installed-Size: $(du -sk "$debroot" | cut -f1)
Description: $blurb
 EasyShow 是一款轻量级、快速的图片查看器:支持 JPEG/PNG/GIF/WebP 等格式,
 缩放平移、旋转镜像、目录树与缩略图网格、批量删除(回收站)、幻灯片。
$extra
EOF

    cat > "$debroot/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database -q || true; fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then gtk-update-icon-cache -q -t /usr/share/icons/hicolor 2>/dev/null || true; fi
exit 0
EOF
    cp "$debroot/DEBIAN/postinst" "$debroot/DEBIAN/postrm"
    chmod 755 "$debroot/DEBIAN/postinst" "$debroot/DEBIAN/postrm"
    chmod 644 "$debroot/usr/share/applications/$pkg.desktop" "$debroot/usr/share/doc/$pkg/copyright"

    dpkg-deb --build --root-owner-group "$debroot" "$DIST/${pkg}_${VERSION}_amd64.deb" >/dev/null
    echo "deb: $DIST/${pkg}_${VERSION}_amd64.deb"
}

MIMES_FULL="image/jpeg;image/png;image/gif;image/webp;image/bmp;image/tiff;image/heic;image/heif;image/avif;image/qoi;image/x-tga;image/x-portable-pixmap;"
MIMES_LITE="image/jpeg;image/png;image/gif;image/webp;image/bmp;image/tiff;image/qoi;image/x-tga;image/x-portable-pixmap;"

make_deb easyshow build/lin/easyshow "EasyShow" "EasyShow 图片查看器" "$MIMES_FULL" "libheif1" \
    "Lightweight, fast image viewer (EasyShow)" \
    " 完整版:HEIC/AVIF 通过运行时加载 libheif 支持(建议安装 libheif1 以启用)。"

make_deb easyshow-lite build/lin/easyshow-lite "EasyShow Lite" "EasyShow Lite 图片查看器" "$MIMES_LITE" "" \
    "Lightweight, fast image viewer (EasyShow Lite)" \
    " 轻量版:不含 HEIC/AVIF 相关代码,零外部依赖;需要 HEIC/AVIF 请安装 easyshow。"

# ---------------- 便携 tar.gz ----------------
# make_tar <tag> <二进制源> <包内二进制名>
make_tar() {
    local tag="$1" bin="$2" binname="$3"
    local stage="build/tar/easyshow-${VERSION}-linux-x86_64-$tag"
    rm -rf "$stage"; mkdir -p "$stage/packaging"
    install -m755 "$bin" "$stage/$binname"
    cp -r assets "$stage/assets"
    install -m755 packaging/install-local.sh "$stage/packaging/install-local.sh"
    cp README.md "$stage/"
    local f
    for f in LICENSE-MIT LICENSE-APACHE; do [ -f "$f" ] && cp "$f" "$stage/"; done
    tar -C build/tar -czf "$DIST/easyshow-${VERSION}-linux-x86_64-$tag.tar.gz" "$(basename "$stage")"
    echo "tar: $DIST/easyshow-${VERSION}-linux-x86_64-$tag.tar.gz"
}

make_tar full build/lin/easyshow easyshow
make_tar lite build/lin/easyshow-lite easyshow-lite

echo ">> 产物:"
ls -la "$DIST"
echo ">> deb 信息(full):"
dpkg-deb -I "$DIST/easyshow_${VERSION}_amd64.deb" | sed -n '1,12p'
