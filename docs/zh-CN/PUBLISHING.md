# 发布指南(GitHub)

[English](../PUBLISHING.md) | **简体中文**

仓库:<https://github.com/carllau80-ux/easyshow>(SSH 推送已验证)。

> **发布策略:Windows / Linux / macOS 三平台完整版**。打 `v*` 标签会触发三平台 CI,并在同一个 GitHub Release 中发布安装包;lite 仍冻结在 0.2.3。

## 版本

版本号唯一来源是 `Cargo.toml` 的 `[workspace.package] version`。

```bash
packaging/bump-version.sh          # 0.2.8 → 0.2.9
packaging/bump-version.sh minor    # 0.2.9 → 0.3.0
```

版本号会进入窗口/CLI、安装包文件名、设置菜单和 Release 资产名。

## 发布步骤

1. **实现与测试**

   ```bash
   cargo test --workspace
   cargo build --release -p easyshow-app
   ```

2. **更新文档**
   - `README.md`(英文默认)与 `README.zh-CN.md`
   - 功能或路线变化时更新 `docs/PLAN.md`
   - 界面变化时更新 `docs/screenshots/`

3. **本机构建 Windows 包(建议做一次 sanity check)**

   ```bash
   export PATH="$HOME/.local/mingw64/root/usr/bin:$PATH"
   export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$PWD/packaging/windows/mingw-linker.sh"
   export WINDRES="$HOME/.local/mingw64/root/usr/bin/x86_64-w64-mingw32-windres"
   TARGET=x86_64-pc-windows-gnu LIBS_DIR=/tmp/msys2/pruned        packaging/windows/make-package.sh
   ```

   产物:`dist/windows/EasyShow-<ver>-Full-Setup.exe`、`dist/windows/easyshow-<ver>-windows-x86_64-full.zip`。

4. **提交并推送**

   ```bash
   git add -A
   git commit -m "release: v<version>"
   git push origin main
   ```

5. **打标签并发布**

   ```bash
   git tag v<version>
   git push origin v<version>
   ```

   标签触发 `.github/workflows/release.yml`,构建 Linux(deb/tar.gz/AppImage)、Windows(zip/Setup)与 macOS(通用 dmg),并在同一个 GitHub Release 中发布全部产物。

6. **验证 Release**

   打开 <https://github.com/carllau80-ux/easyshow/releases> 确认三平台产物齐全:
   - Windows:zip 内含 exe 与 DLL 闭包,Setup.exe 可安装
   - Linux:deb、tar.gz、AppImage
   - macOS:通用 dmg
   - 设置菜单显示的版本正确

## 手动发布

CI 不可用时,可上传本机构建产物;Linux/Windows 可在 Linux 上构建,macOS 需要 macOS 或 CI。

## 许可

仓库包含 `LICENSE-MIT`、`LICENSE-APACHE` 与 `docs/THIRD-PARTY.md`;Windows 包内 `licenses/` 目录附带随包 DLL 的许可。
