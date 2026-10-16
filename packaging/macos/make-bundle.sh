#!/usr/bin/env bash
# 在 macOS 上构建 Liteshow.app 与 .dmg(需要 macOS + Xcode 命令行工具;HEIC 支持需要 Homebrew libheif)。
# 用法:packaging/macos/make-bundle.sh [universal|aarch64-apple-darwin|x86_64-apple-darwin]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; cd "$ROOT"
VERSION="$(awk -F'"' '/^\[workspace.package\]/{f=1} f&&/^version/{print $2; exit}' Cargo.toml)"
ARCH="${1:-universal}"

if [ "$ARCH" = "universal" ]; then
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
    cargo build --release -p liteshow-app --target aarch64-apple-darwin
    cargo build --release -p liteshow-app --target x86_64-apple-darwin
    mkdir -p target/universal/release
    lipo -create \
        target/aarch64-apple-darwin/release/liteshow \
        target/x86_64-apple-darwin/release/liteshow \
        -output target/universal/release/liteshow
    BIN="target/universal/release/liteshow"
else
    cargo build --release -p liteshow-app --target "$ARCH"
    BIN="target/$ARCH/release/liteshow"
fi

APP="build/macos/Liteshow.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
install -m755 "$BIN" "$APP/Contents/MacOS/liteshow"

# 图标 → icns
ICONSET="build/macos/Liteshow.iconset"; rm -rf "$ICONSET"; mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
    sips -z "$s" "$s" assets/icon-256.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2))
    sips -z "$d" "$d" assets/icon-256.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Liteshow.icns"

# Info.plist
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Liteshow</string>
  <key>CFBundleDisplayName</key><string>Liteshow 图片查看器</string>
  <key>CFBundleIdentifier</key><string>dev.liteshow.viewer</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>liteshow</string>
  <key>CFBundleIconFile</key><string>Liteshow</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDocumentTypes</key><array>
    <dict>
      <key>CFBundleTypeName</key><string>Image</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array>
        <string>public.jpeg</string><string>public.png</string><string>com.compuserve.gif</string>
        <string>org.webmproject.webp</string><string>com.microsoft.bmp</string><string>public.tiff</string>
        <string>public.heic</string><string>public.heif</string><string>public.avif</string>
        <string>public.image</string>
      </array>
    </dict>
  </array>
</dict></plist>
EOF

# HEIC 运行时(默认不随包,保持轻量;LITESHOW_BUNDLE_LIBHEIF=1 可打入 Frameworks)
if [ "${LITESHOW_BUNDLE_LIBHEIF:-0}" = "1" ] && command -v brew >/dev/null 2>&1; then
    brew list libheif >/dev/null 2>&1 || brew install libheif
    HEIF_LIB="$(brew --prefix libheif)/lib/libheif.1.dylib"
    cp "$HEIF_LIB" "$APP/Contents/Frameworks/" 2>/dev/null || true
    # 复制 libheif 依赖的第三方 dylib
    otool -L "$HEIF_LIB" 2>/dev/null | awk 'NR>1 {print $1}' | grep -Ev "^/usr/lib|^/System" | while read -r dep; do
        [ -f "$dep" ] && cp "$dep" "$APP/Contents/Frameworks/" 2>/dev/null || true
    done
    # 统一 install_name 到 @executable_path/../Frameworks(与 liteshow 的 dlopen 候选路径一致)
    for f in "$APP/Contents/Frameworks"/*.dylib; do
        base="$(basename "$f")"
        install_name_tool -id "@executable_path/../Frameworks/$base" "$f" 2>/dev/null || true
        for dep in $(otool -L "$f" | awk 'NR>1 {print $1}' | grep -E "libheif|libde265|libaom|libdav1d|libx265|libsharpyuv|libjpeg|libpng|libtiff|libwebp|libz|libbrotli|librsvg" ); do
            install_name_tool -change "$dep" "@executable_path/../Frameworks/$(basename "$dep")" "$f" 2>/dev/null || true
        done
    done
    echo ">> libheif 已打入 Frameworks(HEIC 可用)"
else
    echo ">> 轻量版:未打入 libheif;系统安装 libheif(如 brew install libheif)时仍会自动启用 HEIC"
fi

# 临时签名(未公证;本机直接可运行)
codesign --force --deep --sign - "$APP" 2>/dev/null || true

# .dmg
DIST="dist/macos"; mkdir -p "$DIST"
hdiutil create -volname "Liteshow" -srcfolder "$APP" -ov -format UDZO \
    "$DIST/Liteshow-${VERSION}-${ARCH}.dmg" >/dev/null
echo ">> 产物:"
ls -la "$DIST"
