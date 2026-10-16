#!/usr/bin/env bash
# 产出 Linux 安装包 → dist/linux/
#   liteshow_<ver>_amd64.deb           系统级安装(/usr/bin、应用菜单、图标、文档)
#   liteshow-<ver>-linux-x86_64.tar.gz 便携包(解压后执行 packaging/install-local.sh 装到用户目录)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

VERSION="$(awk -F'"' '/^\[workspace.package\]/{f=1} f&&/^version/{print $2; exit}' Cargo.toml)"
BIN="target/release/liteshow"
[ -x "$BIN" ] || { echo ">> 构建 release…"; cargo build --release -p liteshow-app; }

DIST="dist/linux"; mkdir -p "$DIST"
DEBROOT="build/deb/liteshow_${VERSION}_amd64"
TARSTAGE="build/tar/liteshow-${VERSION}-linux-x86_64"
rm -rf "$DEBROOT" "$TARSTAGE"

# ---------------- .deb ----------------
mkdir -p "$DEBROOT"/DEBIAN \
         "$DEBROOT"/usr/bin \
         "$DEBROOT"/usr/share/applications \
         "$DEBROOT"/usr/share/doc/liteshow \
         "$DEBROOT"/usr/share/icons/hicolor/{256x256,128x128,64x64}/apps

install -m755 "$BIN" "$DEBROOT/usr/bin/liteshow"
for s in 256 128 64; do
    install -m644 "assets/icon-$s.png" "$DEBROOT/usr/share/icons/hicolor/${s}x${s}/apps/liteshow.png"
done

cat > "$DEBROOT/usr/share/applications/liteshow.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Version=1.0
Name=Liteshow
Name[zh_CN]=Liteshow 图片查看器
GenericName=Image Viewer
GenericName[zh_CN]=图片查看器
Comment=Lightweight, fast image viewer
Comment[zh_CN]=轻量级快速图片查看器
Exec=/usr/bin/liteshow %F
Icon=liteshow
Terminal=false
Categories=Graphics;Viewer;
MimeType=image/jpeg;image/png;image/gif;image/webp;image/bmp;image/tiff;image/heic;image/heif;image/avif;image/qoi;image/x-tga;image/x-portable-pixmap;
StartupNotify=true
StartupWMClass=liteshow
Keywords=image;viewer;photo;picture;
Keywords[zh_CN]=图片;看图;浏览;照片;
EOF

cat > "$DEBROOT/usr/share/doc/liteshow/copyright" <<'EOF'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: Liteshow

Files: *
Copyright: 2026 Liteshow contributors
License: MIT or Apache-2.0
 本项目采用 MIT OR Apache-2.0 双许可,完整文本见上游仓库 LICENSE-MIT 与 LICENSE-APACHE。
EOF

cat > "$DEBROOT/DEBIAN/control" <<EOF
Package: liteshow
Version: $VERSION
Architecture: amd64
Maintainer: Liteshow contributors <liteshow@localhost>
Depends: libc6 (>= 2.34), libgcc-s1
Recommends: libheif1
Section: graphics
Priority: optional
Installed-Size: $(du -sk "$DEBROOT" | cut -f1)
Description: Lightweight, fast image viewer (Liteshow)
 Liteshow 是一款轻量级、快速的图片查看器:支持 JPEG/PNG/GIF/WebP/HEIC/AVIF 等格式,
 缩放平移、旋转镜像、文件夹内翻页浏览。
 HEIC/AVIF 通过运行时加载 libheif 支持(建议安装 libheif1 以启用)。
EOF

cat > "$DEBROOT/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database -q || true; fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then gtk-update-icon-cache -q -t /usr/share/icons/hicolor 2>/dev/null || true; fi
exit 0
EOF
cp "$DEBROOT/DEBIAN/postinst" "$DEBROOT/DEBIAN/postrm"
chmod 755 "$DEBROOT/DEBIAN/postinst" "$DEBROOT/DEBIAN/postrm"

chmod 644 "$DEBROOT/usr/share/applications/liteshow.desktop" "$DEBROOT/usr/share/doc/liteshow/copyright"
dpkg-deb --build --root-owner-group "$DEBROOT" "$DIST/liteshow_${VERSION}_amd64.deb" >/dev/null

# ---------------- 便携 tar.gz ----------------
mkdir -p "$TARSTAGE/packaging"
install -m755 "$BIN" "$TARSTAGE/liteshow"
cp -r assets "$TARSTAGE/assets"
install -m755 packaging/install-local.sh "$TARSTAGE/packaging/install-local.sh"
cp README.md "$TARSTAGE/"
for f in LICENSE-MIT LICENSE-APACHE; do [ -f "$f" ] && cp "$f" "$TARSTAGE/"; done
tar -C build/tar -czf "$DIST/liteshow-${VERSION}-linux-x86_64.tar.gz" "liteshow-${VERSION}-linux-x86_64"

echo ">> 产物:"
ls -la "$DIST"
echo ">> deb 信息:"
dpkg-deb -I "$DIST/liteshow_${VERSION}_amd64.deb" | sed -n '1,14p'
