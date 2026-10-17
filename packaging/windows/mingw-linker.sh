#!/usr/bin/env bash
# 交叉链接器包装:指向解包到用户目录的 mingw-w64 工具链(无需 sudo)。
# EASYSHOW_MINGW_ROOT 可覆盖默认路径。
set -euo pipefail
MG_ROOT="${EASYSHOW_MINGW_ROOT:-$HOME/.local/mingw64/root}"
export GCC_EXEC_PREFIX="$MG_ROOT/usr/lib/gcc/"
export PATH="$MG_ROOT/usr/bin:$PATH"
exec "$MG_ROOT/usr/bin/x86_64-w64-mingw32-gcc-posix" "$@"
