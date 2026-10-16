#!/usr/bin/env bash
# 组装 Windows 分发产物 → dist/windows/
#   liteshow-<ver>-windows-x86_64.zip     便携版(exe + HEIC 运行时 DLL)
#   Liteshow-<ver>-Setup.exe              NSIS 安装包(开始菜单/桌面快捷方式/打开方式/卸载)
#
# 前置条件:
#   - 交叉编译产物(默认 target/x86_64-pc-windows-gnu/release/liteshow.exe,可用 EXE= 覆盖)
#   - libheif DLL 闭包(默认 /tmp/msys2/out/mingw64/bin,可用 LIBS_DIR= 覆盖;缺失则打无 HEIC 的包)
#   - NSIS(默认 $HOME/.local/nsis/root,可用 NSIS_ROOT= 覆盖;缺失只产 zip)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; cd "$ROOT"
VERSION="$(awk -F'"' '/^\[workspace.package\]/{f=1} f&&/^version/{print $2; exit}' Cargo.toml)"

EXE="${EXE:-target/x86_64-pc-windows-gnu/release/liteshow.exe}"
# 默认轻量版:不随包附带 HEIC 运行时 DLL;SHIP_LIBHEIF=1 时从 LIBS_DIR 取完整版
SHIP_LIBHEIF="${SHIP_LIBHEIF:-0}"
LIBS_DIR="${LIBS_DIR:-/tmp/msys2/pruned}"
NSIS_ROOT="${NSIS_ROOT:-$HOME/.local/nsis/root}"

# makensis 定位:优先 $MAKENSIS(如 Windows CI 上的 C:\Program Files (x86)\NSIS\makensis.exe),
# 其次本机解包的 $NSIS_ROOT/usr/bin/makensis
MAKENSIS="${MAKENSIS:-}"
if [ -z "$MAKENSIS" ] && [ -x "$NSIS_ROOT/usr/bin/makensis" ]; then
    MAKENSIS="$NSIS_ROOT/usr/bin/makensis"
    export NSISDIR="$NSIS_ROOT/usr/share/nsis"
fi

[ -f "$EXE" ] || { echo "!! 缺少 $EXE,请先交叉构建"; exit 1; }

DIST="dist/windows"; STAGE="build/win/liteshow-$VERSION-windows-x86_64"
rm -rf "$STAGE"; mkdir -p "$DIST" "$STAGE"

install -m755 "$EXE" "$STAGE/liteshow.exe"

if [ "$SHIP_LIBHEIF" = "1" ] && [ -d "$LIBS_DIR" ]; then
    echo ">> 完整版:附带 HEIC 运行时 DLL($LIBS_DIR)"
    cp "$LIBS_DIR"/*.dll "$STAGE/" 2>/dev/null || true
    if [ -d /tmp/msys2/out/mingw64/share/licenses ]; then
        cp -r /tmp/msys2/out/mingw64/share/licenses "$STAGE/licenses"
    fi
else
    echo ">> 轻量版:不附带 HEIC 运行时 DLL(系统或程序目录提供 libheif.dll 时自动启用)"
fi

cp README.md "$STAGE/README.md" 2>/dev/null || true
for f in LICENSE-MIT LICENSE-APACHE; do [ -f "$f" ] && cp "$f" "$STAGE/"; done
if [ "$SHIP_LIBHEIF" = "1" ]; then
cat > "$STAGE/使用说明.txt" <<'EOF'
Liteshow 使用说明(完整版)
======================
1. 双击 liteshow.exe 运行;可直接打开图片,或把图片/文件夹拖进窗口
2. 支持 JPEG/PNG/GIF/WebP/BMP/TIFF/ICO/QOI/TGA/PNM 等格式
3. 本版已附带 HEIC/AVIF 解码库(libheif 相关 DLL),可直接打开 iPhone 的 HEIC 照片;
   同目录下的 DLL 请勿删除或移动
4. 如需开始菜单/桌面快捷方式/卸载入口,请使用 Liteshow-Setup.exe 安装包
EOF
else
cat > "$STAGE/使用说明.txt" <<'EOF'
Liteshow 便携版使用说明
====================
1. 双击 liteshow.exe 运行;可直接打开图片,或把图片/文件夹拖进窗口
2. 支持 JPEG/PNG/GIF/WebP/BMP/TIFF/ICO/QOI/TGA/PNM 等格式(无需任何外部依赖)
3. HEIC/AVIF:默认未附带解码库。若系统 PATH 或本目录中存在 libheif.dll(及依赖),
   会自动启用;不存在时 HEIC 文件会提示"HEIC 支持不可用",其余格式不受影响
4. 如需开始菜单/桌面快捷方式/卸载入口,请改用 Liteshow-Setup.exe 安装包
EOF
fi

python3 - "$STAGE" "$DIST/liteshow-$VERSION-windows-x86_64.zip" <<'PY'
import os, sys, zipfile
stage, out = sys.argv[1], sys.argv[2]
base = os.path.dirname(stage)
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for root, _, files in os.walk(stage):
        for f in files:
            p = os.path.join(root, f)
            z.write(p, os.path.relpath(p, base))
print("zip:", out)
PY

if [ -n "$MAKENSIS" ]; then
    echo ">> 生成 NSIS 安装包…"
    "$MAKENSIS" -V2 -INPUTCHARSET UTF8 \
        -DVERSION="$VERSION" \
        -DSRC="$ROOT/$STAGE" \
        -DOUT="$ROOT/$DIST/Liteshow-$VERSION-Setup.exe" \
        -DICON="$ROOT/assets/icon.ico" \
        "$ROOT/packaging/windows/liteshow.nsi" | tail -8
else
    echo "!! 未找到 makensis,跳过安装包(可用便携 zip)"
fi

echo ">> 产物:"
ls -la "$DIST"
