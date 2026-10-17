#!/usr/bin/env bash
# 组装 Windows 分发产物(两个变体)→ dist/windows/
#   easyshow-<ver>-windows-x86_64-full.zip  便携·完整版(exe + HEIC/AVIF 运行时 DLL)
#   easyshow-<ver>-windows-x86_64-lite.zip  便携·轻量版(exe,无 DLL,不含 HEIC/AVIF)
#   EasyShow-<ver>-Full-Setup.exe           NSIS 安装包·完整版
#   EasyShow-<ver>-Lite-Setup.exe           NSIS 安装包·轻量版(与完整版可共存)
#
# 两个 exe 由本脚本自行编译(full 用默认特性;lite 用 --no-default-features)。
# 交叉编译时请先设好工具链环境:
#   export PATH="$HOME/.local/mingw64/root/usr/bin:$PATH"
#   export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$PWD/packaging/windows/mingw-linker.sh"
#   export WINDRES="$HOME/.local/mingw64/root/usr/bin/x86_64-w64-mingw32-windres"
#
# 环境变量:
#   TARGET     目标三元组(默认宿主;交叉编译设 x86_64-pc-windows-gnu)
#   LIBS_DIR   libheif DLL 闭包(默认 /tmp/msys2/pruned;缺失则完整版不含 DLL,给出警告)
#   NSIS_ROOT  NSIS 解包位置(默认 $HOME/.local/nsis/root;缺失只产 zip)
#   MAKENSIS   直接指定 makensis 可执行文件(如 Windows CI)
#   EXE_FULL / EXE_LITE  跳过编译,直接使用已有 exe
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; cd "$ROOT"
VERSION="$(awk -F'"' '/^\[workspace.package\]/{f=1} f&&/^version/{print $2; exit}' Cargo.toml)"

TARGET="${TARGET:-}"
LIBS_DIR="${LIBS_DIR:-/tmp/msys2/pruned}"
NSIS_ROOT="${NSIS_ROOT:-$HOME/.local/nsis/root}"

# makensis 定位:优先 $MAKENSIS(如 Windows CI 上的 C:\Program Files (x86)\NSIS\makensis.exe),
# 其次本机解包的 $NSIS_ROOT/usr/bin/makensis
MAKENSIS="${MAKENSIS:-}"
if [ -z "$MAKENSIS" ] && [ -x "$NSIS_ROOT/usr/bin/makensis" ]; then
    MAKENSIS="$NSIS_ROOT/usr/bin/makensis"
    export NSISDIR="$NSIS_ROOT/usr/share/nsis"
fi

CARGO_TARGET_ARG=()
EXE_DIR="target/release"
if [ -n "$TARGET" ]; then
    CARGO_TARGET_ARG=(--target "$TARGET")
    EXE_DIR="target/$TARGET/release"
fi

DIST="dist/windows"; mkdir -p "$DIST"
STAGE_FULL="build/win/EasyShow-$VERSION-windows-x86_64-full"
STAGE_LITE="build/win/EasyShow-$VERSION-windows-x86_64-lite"
rm -rf "$STAGE_FULL" "$STAGE_LITE"; mkdir -p "$STAGE_FULL" "$STAGE_LITE"

# 注意:两次编译共用同一个输出路径,必须"编译完立即复制",否则会被下一变体覆盖
if [ -n "${EXE_FULL:-}" ]; then
    echo ">> 使用已有完整版 exe: $EXE_FULL"
else
    echo ">> 编译完整版(full, 含 HEIC/AVIF)…"
    cargo build --release -j4 -p easyshow-app "${CARGO_TARGET_ARG[@]}" 2>&1 | tail -2
    EXE_FULL="$EXE_DIR/easyshow.exe"
fi
[ -f "$EXE_FULL" ] || { echo "!! 缺少 $EXE_FULL"; exit 1; }
install -m755 "$EXE_FULL" "$STAGE_FULL/easyshow.exe"

if [ -n "${EXE_LITE:-}" ]; then
    echo ">> 使用已有轻量版 exe: $EXE_LITE"
else
    echo ">> 编译轻量版(lite, --no-default-features)…"
    cargo build --release -j4 -p easyshow-app --no-default-features "${CARGO_TARGET_ARG[@]}" 2>&1 | tail -2
    EXE_LITE="$EXE_DIR/easyshow.exe"
fi
[ -f "$EXE_LITE" ] || { echo "!! 缺少 $EXE_LITE"; exit 1; }
install -m755 "$EXE_LITE" "$STAGE_LITE/easyshow-lite.exe"

# ---- 完整版:附带 HEIC/AVIF 运行时 DLL ----
if [ -d "$LIBS_DIR" ] && ls "$LIBS_DIR"/*.dll >/dev/null 2>&1; then
    echo ">> 完整版:附带 HEIC 运行时 DLL($LIBS_DIR)"
    cp "$LIBS_DIR"/*.dll "$STAGE_FULL/" 2>/dev/null || true
    if [ -d /tmp/msys2/out/mingw64/share/licenses ]; then
        cp -r /tmp/msys2/out/mingw64/share/licenses "$STAGE_FULL/licenses"
    fi
else
    echo "!! 警告:未找到 DLL 闭包($LIBS_DIR),完整版将不含 HEIC 运行时 DLL"
fi

for stage in "$STAGE_FULL" "$STAGE_LITE"; do
    cp README.md "$stage/README.md" 2>/dev/null || true
    for f in LICENSE-MIT LICENSE-APACHE; do [ -f "$f" ] && cp "$f" "$stage/"; done
done

cat > "$STAGE_FULL/使用说明.txt" <<'EOF'
EasyShow 完整版 使用说明
========================
1. 双击 easyshow.exe 运行;可直接打开图片,或把图片/文件夹拖进窗口
2. 支持 JPEG/PNG/GIF/WebP/BMP/TIFF/ICO/QOI/TGA/PNM 与 HEIC/AVIF
3. 本版已附带 HEIC/AVIF 解码库(libheif 相关 DLL),iPhone 的 HEIC 照片开箱即用;
   同目录下的 DLL 请勿删除或移动
4. 如需开始菜单/桌面快捷方式/卸载入口,请使用 EasyShow-*-Full-Setup.exe 安装包
EOF

cat > "$STAGE_LITE/使用说明.txt" <<'EOF'
EasyShow 轻量版 使用说明
========================
1. 双击 easyshow-lite.exe 运行;可直接打开图片,或把图片/文件夹拖进窗口
2. 支持 JPEG/PNG/GIF/WebP/BMP/TIFF/ICO/QOI/TGA/PNM,零外部依赖,免安装
3. 轻量版不含 HEIC/AVIF 支持(不附带任何 DLL);需要看 HEIC/AVIF 请使用完整版
4. 如需开始菜单/桌面快捷方式/卸载入口,请使用 EasyShow-*-Lite-Setup.exe 安装包
EOF

echo ">> 打包 zip…"
# 可移植 zip(zip → 7z → PowerShell → python3 → python),避免依赖单一工具
make_zip() { # <out.zip> <待打包目录>
    local out="$1" dir="$2"
    local out_abs; out_abs="$(cd "$(dirname "$out")" && pwd)/$(basename "$out")"
    rm -f "$out"
    if command -v zip >/dev/null 2>&1; then
        (cd "$(dirname "$dir")" && zip -qr "$out_abs" "$(basename "$dir")")
    elif command -v 7z >/dev/null 2>&1; then
        (cd "$(dirname "$dir")" && 7z a -tzip "$out_abs" "$(basename "$dir")" >/dev/null)
    elif command -v pwsh >/dev/null 2>&1 || command -v powershell >/dev/null 2>&1; then
        local ps="powershell"
        command -v pwsh >/dev/null 2>&1 && ps="pwsh"
        "$ps" -NoProfile -Command "Compress-Archive -Path '$dir' -DestinationPath '$out_abs' -Force"
    else
        local py=""
        command -v python3 >/dev/null 2>&1 && py="python3"
        [ -z "$py" ] && command -v python >/dev/null 2>&1 && py="python"
        [ -n "$py" ] || { echo "!! 没有可用的 zip 工具(zip/7z/powershell/python)"; return 1; }
        "$py" - "$dir" "$out_abs" <<'PY'
import os, sys, zipfile
stage, out = sys.argv[1], sys.argv[2]
base = os.path.dirname(stage)
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for root, _, files in os.walk(stage):
        for f in files:
            p = os.path.join(root, f)
            z.write(p, os.path.relpath(p, base))
PY
    fi
    echo "zip: $out"
}

make_zip "dist/windows/easyshow-$VERSION-windows-x86_64-full.zip" "$STAGE_FULL"
make_zip "dist/windows/easyshow-$VERSION-windows-x86_64-lite.zip" "$STAGE_LITE"

if [ -n "$MAKENSIS" ]; then
    for variant in full lite; do
        case "$variant" in
            full) SRC="$ROOT/$STAGE_FULL"; OUT="$ROOT/$DIST/EasyShow-$VERSION-Full-Setup.exe" ;;
            lite) SRC="$ROOT/$STAGE_LITE"; OUT="$ROOT/$DIST/EasyShow-$VERSION-Lite-Setup.exe" ;;
        esac
        echo ">> 生成 NSIS 安装包($variant)…"
        "$MAKENSIS" -V2 -INPUTCHARSET UTF8 \
            -DVERSION="$VERSION" \
            -DVARIANT="$variant" \
            -DSRC="$SRC" \
            -DOUT="$OUT" \
            -DICON="$ROOT/assets/icon.ico" \
            "$ROOT/packaging/windows/easyshow.nsi" | tail -3
    done
else
    echo "!! 未找到 makensis,跳过安装包(可用便携 zip)"
fi

echo ">> 产物:"
ls -la "$DIST"
