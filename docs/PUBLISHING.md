# 发布指南(GitHub)

本机仓库已就绪:提交与标签(v0.2.0 → v0.2.2)均已完成,SSH 身份(加密钥)已验证可推送。

## 首次发布(只差"建仓"这一步)

GitHub 的仓库创建必须在网页完成(SSH 只能推送,不能建仓):

1. 打开 <https://github.com/new>
   - 仓库名:`easyshow`
   - 建议先选 **Private**(确认无误后可随时改为 Public)
   - **不要**勾选 "Add a README / .gitignore / license"(保持空仓库)
2. 回到本机执行(或用该仓库路径告诉我,我来推):

   ```bash
   cd easyshow
   git push -u origin main
   git push origin v0.2.2
   ```

## 之后的每次更新(版本号一起走)

1. 改代码
2. `packaging/bump-version.sh`(不带参数=递增第三位 patch;重大改动用 minor / major)(版本号唯一来源:`Cargo.toml` 的 `[workspace.package]`,会进入安装包文件名、`--version`、设置菜单显示)
3. `git add -A && git commit -m "…" && git tag v<新版本>`
4. `git push && git push --tags`
5. 两种出包方式任选(只发布**完整版**;轻量版 lite 自 0.2.3 起冻结):
   - **本机离线打包**(不依赖网络/CI):
     - Linux:`packaging/build-linux.sh` → `dist/linux/`(deb + tar.gz)
     - Windows:`packaging/windows/make-package.sh` → `dist/windows/`(Setup.exe + 便携 zip;
       交叉编译时先设好 mingw 工具链环境,自动附带 `LIBS_DIR` 中的 libheif DLL 闭包)
     - macOS:`packaging/macos/make-bundle.sh universal`(需在 macOS 上跑)→ `dist/macos/`(.app + .dmg)
   - **GitHub Actions**:`.github/workflows/release.yml` 监听 `v*` 标签,自动构建三平台产物(Artifacts);
     也可把本机 `dist/` 中的安装包直接拖到 GitHub Release 页面上传。

## 备选:从其他机器发布(bundle 移植)

本机可导出完整仓库快照:

```bash
git bundle create dist/easyshow-v<版本>.bundle --all
```

在任意有 GitHub 访问权限的机器上:

```bash
git clone easyshow-v<版本>.bundle easyshow && cd easyshow
git remote set-url origin git@github.com:carllau80-ux/easyshow.git
git push -u origin main --tags
```

## 已内置的发布物料

| 物料 | 位置 |
|---|---|
| 许可(MIT OR Apache-2.0) | `LICENSE-MIT` / `LICENSE-APACHE` |
| 三平台 CI(打 tag 即出包) | `.github/workflows/release.yml` |
| 版本号工具 | `packaging/bump-version.sh` |
| 安装包脚本(Linux/Win/mac) | `packaging/` |
| 本机安装(用户级,含旧名迁移) | `packaging/install-local.sh` |
