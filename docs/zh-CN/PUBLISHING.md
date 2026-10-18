# 发布指南(GitHub)

[English](../PUBLISHING.md) | **简体中文**

仓库:<https://github.com/carllau80-ux/easyshow>(SSH 推送已验证)。

> **当前策略:只发布 Windows 完整版**,待 Windows 包验证通过后再发布 Linux/macOS。Linux/macOS 打包脚本保留在仓库中,但暂不出包;GitHub Actions 也暂时只构建 Windows。

## 版本

版本号唯一来源是 `Cargo.toml` 的 `[workspace.package] version`。

```bash
packaging/bump-version.sh          # 0.2.6 → 0.2.7
packaging/bump-version.sh minor    # 0.2.7 → 0.3.0
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

   标签触发 `.github/workflows/release.yml`;当前配置下只在 `windows-latest` 构建完整版,收集 libheif DLL,生成 NSIS 安装包,并自动创建附带 zip 与 Setup.exe 的 GitHub Release。

6. **验证 Release**

   打开 <https://github.com/carllau80-ux/easyshow/releases> 确认:
   - zip 内含 `easyshow.exe` 与 DLL 闭包
   - Setup.exe 可安装并显示新版本号
   - 设置菜单显示的版本正确

## 手动发布

CI 不可用时,把本机 `dist/windows/` 中的 zip 与 Setup.exe 手动上传到 GitHub Release 页面。

## 恢复 Linux/macOS

Windows 包验证通过后:

1. 从 git 历史恢复 `.github/workflows/release.yml` 中的 `linux` / `macos` jobs(0.2.7 之前存在)。
2. 更新 Release 文案与 README 的产物表。
3. 递增一个 patch 版本并重新打标签。

## 许可

仓库包含 `LICENSE-MIT`、`LICENSE-APACHE` 与 `docs/THIRD-PARTY.md`;Windows 包内 `licenses/` 目录附带随包 DLL 的许可。
