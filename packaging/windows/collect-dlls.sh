#!/usr/bin/env bash
# 计算 Windows DLL 依赖闭包(用于随包附带 libheif 运行时)。
#
# 从入口 DLL 出发递归解析 PE 导入表,把 search_dir 中实际存在的依赖复制到 out_dir;
# 系统 DLL(API Set / System32 常见项)自动跳过。缺依赖时打印警告但不失败(便于 CI).
#
# 用法: collect-dlls.sh <search_dir> <out_dir> <entry.dll> [更多入口...]
# 例:   collect-dlls.sh C:/msys64/mingw64/bin build/win-dlls libheif.dll
set -euo pipefail
SEARCH="$1"; OUT="$2"; shift 2
OBJDUMP="${OBJDUMP:-objdump}"
mkdir -p "$OUT"

is_system() {
    case "$1" in
        api-ms-win-*|ext-ms-*) return 0 ;;
        kernel32.dll|kernelbase.dll|ntdll.dll|user32.dll|gdi32.dll|advapi32.dll|shell32.dll|\
        ole32.dll|oleaut32.dll|comdlg32.dll|comctl32.dll|ws2_32.dll|winmm.dll|msvcrt.dll|\
        bcrypt.dll|crypt32.dll|dwmapi.dll|uxtheme.dll|imm32.dll|setupapi.dll|shlwapi.dll|\
        version.dll|dwrite.dll|d3d11.dll|dxgi.dll|opengl32.dll|secur32.dll|netapi32.dll|\
        userenv.dll|wtsapi32.dll|dnsapi.dll|iphlpapi.dll|psapi.dll|powrprof.dll|dbghelp.dll|\
        msimg32.dll|usp10.dll|winhttp.dll|wininet.dll|wldap32.dll|rpcrt4.dll|winspool.drv)
            return 0 ;;
    esac
    return 1
}

declare -A seen=()
queue=("$@")
while [ ${#queue[@]} -gt 0 ]; do
    name="$(basename "${queue[0]}")"; queue=("${queue[@]:1}")
    lower="$(printf '%s' "$name" | tr 'A-Z' 'a-z')"
    [ -n "${seen[$lower]:-}" ] && continue
    if is_system "$lower"; then continue; fi
    src="$SEARCH/$name"
    if [ ! -f "$src" ]; then
        echo "!! 依赖未找到: $name(跳过)"
        continue
    fi
    seen[$lower]="$src"
    while IFS= read -r dep; do
        [ -n "$dep" ] && queue+=("$dep")
    done < <("$OBJDUMP" -p "$src" 2>/dev/null | sed -n 's/.*DLL Name: //p')
done

for lower in "${!seen[@]}"; do
    cp -f "${seen[$lower]}" "$OUT/"
done
for lower in "${!seen[@]}"; do
    echo "  $(basename "${seen[$lower]}")"
done
echo ">> 依赖闭包: ${#seen[@]} 个 DLL → $OUT"
