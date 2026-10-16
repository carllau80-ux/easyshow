#!/usr/bin/env bash
# Liteshow 用户级本地安装(无需 sudo):
#   1. 二进制            → ~/.local/bin/liteshow
#   2. 图标(3 种尺寸)  → ~/.local/share/icons/hicolor/{256x256,128x128,64x64}/apps/liteshow.png
#   3. 桌面条目          → ~/.local/share/applications/liteshow.desktop(应用列表 + "打开方式")
#   4. 桌面快捷方式      → 用户桌面目录(自动识别 xdg-user-dir,如中文系统的 ~/桌面)
#
# 用法:
#   packaging/install-local.sh                 # 构建(如缺 release 二进制)+ 安装 + 桌面快捷方式
#   packaging/install-local.sh --set-default   # 额外:把 Liteshow 设为常见图片格式的默认打开程序
#   packaging/install-local.sh --uninstall     # 卸载以上全部内容
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# 仓库布局 target/release/liteshow;便携 tar 包布局 ./liteshow
if [ -x "$ROOT/target/release/liteshow" ]; then
    BIN_SRC="$ROOT/target/release/liteshow"
else
    BIN_SRC="$ROOT/liteshow"
fi
BIN_DST="$HOME/.local/bin/liteshow"
ICONS_SRC="$ROOT/assets"
ICON_BASE="$HOME/.local/share/icons/hicolor"
ICON_SIZES=(256 128 64)
APPS_DIR="$HOME/.local/share/applications"
DESKTOP_FILE="$APPS_DIR/liteshow.desktop"

MIMES="image/jpeg;image/png;image/gif;image/webp;image/bmp;image/tiff;image/heic;image/heif;image/avif;image/qoi;image/x-tga;image/x-portable-pixmap;"

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
    [ -n "$ddir" ] && rm -f "$ddir/liteshow.desktop"
    local s
    for s in "${ICON_SIZES[@]}"; do rm -f "$ICON_BASE/${s}x${s}/apps/liteshow.png"; done
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
    echo ">> Liteshow 已卸载(仅用户级文件,原图与配置不受影响)"
    exit 0
}

SET_DEFAULT=0
for arg in "$@"; do
    case "$arg" in
        --uninstall) uninstall ;;
        --set-default) SET_DEFAULT=1 ;;
        *) echo "未知参数: $arg" >&2; exit 2 ;;
    esac
done

if [ ! -x "$BIN_SRC" ]; then
    echo ">> 未找到 release 二进制,先构建…"
    (cd "$ROOT" && cargo build --release -p liteshow-app)
fi

# 迁移:清理旧名(Lumen)的用户级安装残留
rm -f "$HOME/.local/bin/lumen" "$HOME/.local/share/applications/lumen.desktop"
legacy_dir="$(desktop_dir)"
[ -n "$legacy_dir" ] && rm -f "$legacy_dir/lumen.desktop"
for s in 256 128 64; do rm -f "$HOME/.local/share/icons/hicolor/${s}x${s}/apps/lumen.png"; done

echo ">> 安装二进制 → $BIN_DST"
install -Dm755 "$BIN_SRC" "$BIN_DST"

echo ">> 安装图标 → $ICON_BASE"
for s in "${ICON_SIZES[@]}"; do
    install -Dm644 "$ICONS_SRC/icon-${s}.png" "$ICON_BASE/${s}x${s}/apps/liteshow.png"
done

echo ">> 写入桌面条目 → $DESKTOP_FILE"
mkdir -p "$APPS_DIR"
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=Liteshow
Name[zh_CN]=Liteshow 图片查看器
GenericName=Image Viewer
GenericName[zh_CN]=图片查看器
Comment=Lightweight, fast image viewer
Comment[zh_CN]=轻量级快速图片查看器
Exec=$BIN_DST %F
Icon=liteshow
Terminal=false
Categories=Graphics;Viewer;
MimeType=$MIMES
StartupNotify=true
StartupWMClass=liteshow
Keywords=image;viewer;photo;picture;
Keywords[zh_CN]=图片;看图;浏览;照片;
EOF

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true

DDIR="$(desktop_dir)"
if [ -n "$DDIR" ] && [ -d "$DDIR" ]; then
    echo ">> 创建桌面快捷方式 → $DDIR/liteshow.desktop"
    cp "$DESKTOP_FILE" "$DDIR/liteshow.desktop"
    chmod +x "$DDIR/liteshow.desktop"
    # GNOME 需要 trusted 元数据,双击才能直接启动(Prompt "Allow Launching" 一次即可,这里预置)
    command -v gio >/dev/null 2>&1 && gio set "$DDIR/liteshow.desktop" metadata::trusted true 2>/dev/null || true
else
    echo "!! 桌面目录不存在,已跳过快捷方式(应用列表仍可用)"
fi

if [ "$SET_DEFAULT" = "1" ]; then
    echo ">> 设为默认看图应用:"
    IFS=';' read -ra LIST <<< "$MIMES"
    for m in "${LIST[@]}"; do
        [ -n "$m" ] && xdg-mime default liteshow.desktop "$m" 2>/dev/null || true
    done
fi

echo ">> 完成。命令行可用 'liteshow <图片或文件夹>',应用列表/桌面快捷方式:'Liteshow 图片查看器'"
