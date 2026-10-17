#!/usr/bin/env bash
# EasyShow 用户级本地安装(无需 sudo):
#   1. 二进制            → ~/.local/bin/easyshow[|easyshow-lite]
#   2. 图标(3 种尺寸)  → ~/.local/share/icons/hicolor/{256x256,128x128,64x64}/apps/
#   3. 桌面条目          → ~/.local/share/applications/(应用列表 + "打开方式")
#   4. 桌面快捷方式      → 用户桌面目录(自动识别 xdg-user-dir,如中文系统的 ~/桌面)
#
# 变体:
#   完整版(缺省) easyshow       — 含 HEIC/AVIF(运行时加载系统 libheif)
#   轻量版 --lite easyshow-lite — 不含 HEIC/AVIF 相关代码,零运行时依赖
#
# 用法:
#   packaging/install-local.sh                 # 完整版:构建(如缺)+ 安装 + 桌面快捷方式
#   packaging/install-local.sh --lite          # 轻量版
#   packaging/install-local.sh --set-default   # 额外:设为常见图片格式的默认打开程序
#   packaging/install-local.sh --uninstall     # 卸载完整版(--lite --uninstall 卸载轻量版)
# 便携 tar 包内直接运行本脚本时,目录中存在 easyshow-lite 二进制会自动按轻量版安装。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

LITE=0
SET_DEFAULT=0
UNINSTALL=0
for arg in "$@"; do
    case "$arg" in
        --lite) LITE=1 ;;
        --set-default) SET_DEFAULT=1 ;;
        --uninstall) UNINSTALL=1 ;;
        *) echo "未知参数: $arg" >&2; exit 2 ;;
    esac
done

# 便携包布局:根目录直接有 easyshow-lite 二进制 → 自动按轻量版处理
if [ -x "$ROOT/easyshow-lite" ] && [ ! -d "$ROOT/target" ]; then
    LITE=1
fi
# 仓库布局 target/release/easyshow;便携 tar 包布局 ./easyshow(-lite)
if [ -x "$ROOT/target/release/easyshow" ]; then
    BIN_SRC="$ROOT/target/release/easyshow"
elif [ -x "$ROOT/easyshow-lite" ]; then
    BIN_SRC="$ROOT/easyshow-lite"
elif [ -x "$ROOT/easyshow" ]; then
    BIN_SRC="$ROOT/easyshow"
else
    BIN_SRC=""
fi

if [ "$LITE" = "1" ]; then
    APP="easyshow-lite"
    APP_NAME="EasyShow Lite"
    MIMES="image/jpeg;image/png;image/gif;image/webp;image/bmp;image/tiff;image/qoi;image/x-tga;image/x-portable-pixmap;"
else
    APP="easyshow"
    APP_NAME="EasyShow"
    MIMES="image/jpeg;image/png;image/gif;image/webp;image/bmp;image/tiff;image/heic;image/heif;image/avif;image/qoi;image/x-tga;image/x-portable-pixmap;"
fi
DISPLAY_NAME="$APP_NAME 图片查看器"

BIN_DST="$HOME/.local/bin/$APP"
ICONS_SRC="$ROOT/assets"
ICON_BASE="$HOME/.local/share/icons/hicolor"
ICON_SIZES=(256 128 64)
APPS_DIR="$HOME/.local/share/applications"
DESKTOP_FILE="$APPS_DIR/$APP.desktop"

desktop_dir() {
    if command -v xdg-user-dir >/dev/null 2>&1; then
        xdg-user-dir DESKTOP 2>/dev/null || echo "$HOME/Desktop"
    else
        echo "$HOME/Desktop"
    fi
}

uninstall() {
    rm -f "$BIN_DST" "$DESKTOP_FILE"
    local ddir; ddir="$(desktop_dir)"
    [ -n "$ddir" ] && rm -f "$ddir/$APP.desktop"
    local s
    for s in "${ICON_SIZES[@]}"; do rm -f "$ICON_BASE/${s}x${s}/apps/$APP.png"; done
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
    echo ">> $APP_NAME 已卸载(仅用户级文件,原图与配置不受影响)"
    exit 0
}

if [ "$UNINSTALL" = "1" ]; then
    uninstall
fi

if [ -z "$BIN_SRC" ]; then
    echo ">> 未找到二进制,先构建($APP_NAME)…"
    BUILD_FLAGS=(--release -p easyshow-app)
    [ "$LITE" = "1" ] && BUILD_FLAGS+=(--no-default-features)
    (cd "$ROOT" && cargo build "${BUILD_FLAGS[@]}")
    BIN_SRC="$ROOT/target/release/easyshow"
fi

# 迁移:清理旧名(Lumen → Liteshow → EasyShow)的用户级安装残留
legacy_dir="$(desktop_dir)"
for old in lumen liteshow; do
    rm -f "$HOME/.local/bin/$old" "$HOME/.local/share/applications/$old.desktop"
    [ -n "$legacy_dir" ] && rm -f "$legacy_dir/$old.desktop"
    for s in 256 128 64; do rm -f "$HOME/.local/share/icons/hicolor/${s}x${s}/apps/$old.png"; done
done

echo ">> 安装二进制($APP_NAME) → $BIN_DST"
install -Dm755 "$BIN_SRC" "$BIN_DST"

echo ">> 安装图标 → $ICON_BASE"
for s in "${ICON_SIZES[@]}"; do
    install -Dm644 "$ICONS_SRC/icon-${s}.png" "$ICON_BASE/${s}x${s}/apps/$APP.png"
done

echo ">> 写入桌面条目 → $DESKTOP_FILE"
mkdir -p "$APPS_DIR"
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=$APP_NAME
Name[zh_CN]=$DISPLAY_NAME
GenericName=Image Viewer
GenericName[zh_CN]=图片查看器
Comment=Lightweight, fast image viewer
Comment[zh_CN]=轻量级快速图片查看器
Exec=$BIN_DST %F
Icon=$APP
Terminal=false
Categories=Graphics;Viewer;
MimeType=$MIMES
StartupNotify=true
StartupWMClass=$APP
Keywords=image;viewer;photo;picture;
Keywords[zh_CN]=图片;看图;浏览;照片;
EOF

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true

DDIR="$(desktop_dir)"
if [ -n "$DDIR" ] && [ -d "$DDIR" ]; then
    echo ">> 创建桌面快捷方式 → $DDIR/$APP.desktop"
    cp "$DESKTOP_FILE" "$DDIR/$APP.desktop"
    chmod +x "$DDIR/$APP.desktop"
    # GNOME 需要 trusted 元数据,双击才能直接启动(Prompt "Allow Launching" 一次即可,这里预置)
    command -v gio >/dev/null 2>&1 && gio set "$DDIR/$APP.desktop" metadata::trusted true 2>/dev/null || true
else
    echo "!! 桌面目录不存在,已跳过快捷方式(应用列表仍可用)"
fi

if [ "$SET_DEFAULT" = "1" ]; then
    echo ">> 设为默认看图应用:"
    IFS=';' read -ra LIST <<< "$MIMES"
    for m in "${LIST[@]}"; do
        [ -n "$m" ] && xdg-mime default "$APP.desktop" "$m" 2>/dev/null || true
    done
fi

echo ">> 完成。命令行可用 '$APP <图片或文件夹>',应用列表/桌面快捷方式:'$DISPLAY_NAME'"
