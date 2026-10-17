#!/usr/bin/env bash
# 更新版本号(单一来源:workspace Cargo.toml 的 [workspace.package] version)。
# 每次发布/更新都应先运行本脚本,版本号随构建进入:窗口 CLI(--version)、
# 安装包文件名、设置菜单显示等。
#
# 默认策略:不带参数 = patch(每次更新递增第三位);重大改动可手动 minor / major。
#
# 用法:
#   packaging/bump-version.sh           # 0.2.1 → 0.2.2(默认)
#   packaging/bump-version.sh 0.3.0     # 指定版本
#   packaging/bump-version.sh minor     # 0.2.1 → 0.3.0
#   packaging/bump-version.sh major     # 0.3.0 → 1.0.0
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="$ROOT/Cargo.toml"

current="$(awk -F'"' '/^\[workspace.package\]/{f=1} f&&/^version/{print $2; exit}' "$MANIFEST")"
target="${1:-patch}"

case "$target" in
    patch | minor | major)
        new="$(python3 - "$current" "$target" <<'EOF'
import sys
cur, kind = sys.argv[1], sys.argv[2]
major, minor, patch = (int(x) for x in cur.split("."))
if kind == "patch":
    patch += 1
elif kind == "minor":
    minor += 1
    patch = 0
else:
    major += 1
    minor = patch = 0
print(f"{major}.{minor}.{patch}")
EOF
)" ;;
    *)
        new="$target" ;;
esac

python3 - "$MANIFEST" "$new" <<'EOF'
import re, sys
path, new = sys.argv[1], sys.argv[2]
text = open(path).read()
text, n = re.subn(r'(?m)^(version = )"[0-9]+\.[0-9]+\.[0-9]+"', rf'\g<1>"{new}"', text, count=1)
if n != 1:
    sys.exit("未找到 [workspace.package] version 行")
open(path, "w").write(text)
EOF

echo "版本:$current → $new"
echo "提示:Cargo.lock 会在下次构建时自动更新;发布时请 git tag v$new 触发 CI 出包。"
