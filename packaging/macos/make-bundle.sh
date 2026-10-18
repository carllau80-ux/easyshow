#!/usr/bin/env bash
# 在 macOS 上构建 EasyShow.app 与 .dmg(需要 macOS + Xcode 命令行工具)。
# 默认产出两个变体:
#   完整版 EasyShow.app / EasyShow-<ver>-<arch>.dmg      (含 HEIC/AVIF;默认打入 libheif,可用 EASYSHOW_BUNDLE_LIBHEIF=0 关闭)
#   轻量版 EasyShow Lite.app / EasyShow-Lite-<ver>-<arch>.dmg (不含 HEIC/AVIF,零附加依赖)
# 用法:packaging/macos/make-bundle.sh [universal|aarch64-apple-darwin|x86_64-apple-darwin]
# 轻量版(lite)自 0.2.3 起冻结,不再随新版打包。
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; cd "$ROOT"
VERSION="$(awk -F'"' '/^\[workspace.package\]/{f=1} f&&/^version/{print $2; exit}' Cargo.toml)"
ARCH="${1:-universal}"
WHICH="${2:-full}"

build_bin() { # $1 = full|lite → 输出二进制路径
    local variant="$1"
    # 注意:macOS 自带 /bin/bash 3.2,空数组展开在 set -u 下会报 unbound,
    # 因此这里用普通字符串而非数组传递 flag。
    local extra=""
    [ "$variant" = "lite" ] && extra="--no-default-features"
    if [ "$ARCH" = "universal" ]; then
        rustup target add aarch64-apple-darwin x86_64-apple-darwin
        cargo build --release -j4 -p easyshow-app $extra --target aarch64-apple-darwin
        cargo build --release -j4 -p easyshow-app $extra --target x86_64-apple-darwin
        mkdir -p target/universal/release
        lipo -create \
            target/aarch64-apple-darwin/release/easyshow \
            target/x86_64-apple-darwin/release/easyshow \
            -output "target/universal/release/easyshow-$variant"
        echo "target/universal/release/easyshow-$variant"
    else
        cargo build --release -j4 -p easyshow-app $extra --target "$ARCH"
        echo "target/$ARCH/release/easyshow"
    fi
}

bundle() { # $1 = full|lite
    local variant="$1"
    local app_name exe_name bundle_id dmg_name
    if [ "$variant" = "lite" ]; then
        app_name="EasyShow Lite"; exe_name="easyshow-lite"
        bundle_id="dev.easyshow.viewer.lite"; dmg_name="EasyShow-Lite-${VERSION}-${ARCH}.dmg"
    else
        app_name="EasyShow"; exe_name="easyshow"
        bundle_id="dev.easyshow.viewer"; dmg_name="EasyShow-${VERSION}-${ARCH}.dmg"
    fi

    local bin; bin="$(build_bin "$variant")"
    local app="build/macos/$app_name.app"
    rm -rf "$app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Frameworks"
    install -m755 "$bin" "$app/Contents/MacOS/$exe_name"

    # 图标 → icns
    local iconset="build/macos/EasyShow.iconset"; rm -rf "$iconset"; mkdir -p "$iconset"
    local s d
    for s in 16 32 128 256 512; do
        sips -z "$s" "$s" assets/icon-256.png --out "$iconset/icon_${s}x${s}.png" >/dev/null
        d=$((s * 2))
        sips -z "$d" "$d" assets/icon-256.png --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null
    done
    iconutil -c icns "$iconset" -o "$app/Contents/Resources/EasyShow.icns"

    # Info.plist(轻量版去掉 HEIC/HEIF/AVIF 文档类型)
    local heic_types=""
    if [ "$variant" = "full" ]; then
        heic_types='        <string>public.heic</string><string>public.heif</string><string>public.avif</string>'
    fi
    cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>${app_name}</string>
  <key>CFBundleDisplayName</key><string>${app_name} 图片查看器</string>
  <key>CFBundleIdentifier</key><string>${bundle_id}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>${exe_name}</string>
  <key>CFBundleIconFile</key><string>EasyShow</string>
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
${heic_types}
        <string>public.image</string>
      </array>
    </dict>
  </array>
</dict></plist>
EOF

    # HEIC 运行时:完整版默认打入 Frameworks(lite 不涉及);EASYSHOW_BUNDLE_LIBHEIF=0 可关闭
    local want_heif="${EASYSHOW_BUNDLE_LIBHEIF:-$([ "$variant" = "full" ] && echo 1 || echo 0)}"
    if [ "$variant" = "full" ] && [ "$want_heif" = "1" ] && command -v brew >/dev/null 2>&1; then
        brew list libheif >/dev/null 2>&1 || brew install libheif
        local heif_lib; heif_lib="$(brew --prefix libheif)/lib/libheif.1.dylib"
        cp "$heif_lib" "$app/Contents/Frameworks/" 2>/dev/null || true
        # 复制 libheif 依赖的第三方 dylib(注意:无匹配时 grep 返回 1,pipefail 下需容错)
        local deps
        deps="$(otool -L "$heif_lib" 2>/dev/null | awk 'NR>1 {print $1}' | grep -Ev "^/usr/lib|^/System" || true)"
        while read -r dep; do
            [ -n "$dep" ] && [ -f "$dep" ] && cp "$dep" "$app/Contents/Frameworks/" 2>/dev/null || true
        done <<< "$deps"
        # 统一 install_name 到 @executable_path/../Frameworks(与 easyshow 的 dlopen 候选路径一致)
        local f base dep
        for f in "$app/Contents/Frameworks"/*.dylib; do
            base="$(basename "$f")"
            install_name_tool -id "@executable_path/../Frameworks/$base" "$f" 2>/dev/null || true
            for dep in $(otool -L "$f" | awk 'NR>1 {print $1}' | grep -E "libheif|libde265|libaom|libdav1d|libx265|libsharpyuv|libjpeg|libpng|libtiff|libwebp|libz|libbrotli|librsvg" || true); do
                install_name_tool -change "$dep" "@executable_path/../Frameworks/$(basename "$dep")" "$f" 2>/dev/null || true
            done
        done
        echo ">> [$variant] libheif 已打入 Frameworks(HEIC 可用)"
    elif [ "$variant" = "full" ]; then
        echo ">> [$variant] 未打入 libheif;系统安装 libheif(如 brew install libheif)时仍会自动启用 HEIC"
    fi

    # 临时签名(未公证;本机直接可运行)
    codesign --force --deep --sign - "$app" 2>/dev/null || true

    # .dmg
    local dist="dist/macos"; mkdir -p "$dist"
    hdiutil create -volname "$app_name" -srcfolder "$app" -ov -format UDZO \
        "$dist/$dmg_name" >/dev/null
    echo "dmg: $dist/$dmg_name"
}

case "$WHICH" in
    full) bundle full ;;
    *) echo "轻量版自 0.2.3 起冻结;用法:make-bundle.sh [arch] [full]" >&2; exit 2 ;;
esac

echo ">> 产物:"
ls -la dist/macos
