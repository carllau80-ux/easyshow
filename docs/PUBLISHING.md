# Publishing Guide (GitHub)

**English** | [简体中文](zh-CN/PUBLISHING.md)

Repository: <https://github.com/carllau80-ux/easyshow> (SSH push access verified).

> **Release policy:** publish full builds for **Windows, Linux and macOS**. A `v*` tag runs all three CI jobs and publishes the installers in one GitHub Release. Lite builds remain frozen at 0.2.3.

## Versioning

The only version source is `[workspace.package] version` in `Cargo.toml`.

```bash
packaging/bump-version.sh          # patch bump (0.2.9 → 0.2.10)
packaging/bump-version.sh minor    # 0.2.10 → 0.3.0
```

The version flows into the window title/CLI, installer filenames, the Settings dialog and release assets.

## Release checklist

1. **Implement and test**

   ```bash
   cargo test --workspace
   cargo build --release -p easyshow-app
   ```

2. **Update docs**
   - `README.md` (English, default) and `README.zh-CN.md`
   - `docs/PLAN.md` if the feature set or roadmap changed
   - Screenshots under `docs/screenshots/` when the UI changes

3. **Build the Windows artifacts locally (recommended sanity check)**

   On Linux with the mingw-w64 toolchain and a libheif DLL closure:

   ```bash
   export PATH="$HOME/.local/mingw64/root/usr/bin:$PATH"
   export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$PWD/packaging/windows/mingw-linker.sh"
   export WINDRES="$HOME/.local/mingw64/root/usr/bin/x86_64-w64-mingw32-windres"
   TARGET=x86_64-pc-windows-gnu LIBS_DIR=/tmp/msys2/pruned \
       packaging/windows/make-package.sh
   ```

   Outputs: `dist/windows/EasyShow-<ver>-Full-Setup.exe` and
   `dist/windows/easyshow-<ver>-windows-x86_64-full.zip`.

4. **Commit and push**

   ```bash
   git add -A
   git commit -m "release: v<version>"
   git push origin main
   ```

5. **Tag and publish**

   ```bash
   git tag v<version>
   git push origin v<version>
   ```

   The tag starts `.github/workflows/release.yml`, which builds Linux (deb/tar.gz/AppImage), Windows (zip/Setup) and macOS (universal dmg), then publishes a GitHub Release with all artifacts.

6. **Verify the release**

   Open <https://github.com/carllau80-ux/easyshow/releases> and confirm all platform assets are present:
   - Windows zip contains `easyshow.exe` and the DLL closure; the NSIS Setup installs
   - Linux .deb, tar.gz and AppImage are uploaded
   - macOS universal .dmg is uploaded
   - the Settings dialog reports the new version

## Manual alternative

If CI is unavailable, upload locally built artifacts to the GitHub Release page. Windows and Linux can be built on Linux; macOS packages require macOS or CI.

## Licenses

The repository ships `LICENSE-MIT`, `LICENSE-APACHE` and `docs/THIRD-PARTY.md`. Windows packages include a `licenses/` folder with the bundled DLL licenses.
