# Publishing Guide (GitHub)

**English** | [简体中文](zh-CN/PUBLISHING.md)

Repository: <https://github.com/carllau80-ux/easyshow> (SSH push access verified).

> **Current policy:** publish **Windows full builds only** until the Windows package has been validated. Linux and macOS packaging scripts stay in the repository, but they are not shipped yet. The GitHub Actions workflow is temporarily Windows-only for the same reason; restore the other jobs after validation.

## Versioning

The only version source is `[workspace.package] version` in `Cargo.toml`.

```bash
packaging/bump-version.sh          # patch bump (0.2.8 → 0.2.9)
packaging/bump-version.sh minor    # 0.2.9 → 0.3.0
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

   The tag starts `.github/workflows/release.yml`, which (in the current Windows-only configuration) builds the full Windows package on `windows-latest`, collects the HEIC DLL closure, creates the NSIS installer, and publishes a GitHub Release with the zip and Setup executable.

6. **Verify the release**

   Open <https://github.com/carllau80-ux/easyshow/releases> and confirm:
   - the Windows zip contains `easyshow.exe` and the DLL closure
   - the NSIS Setup runs and installs the expected version
   - the Settings dialog reports the new version

## Manual alternative

If CI is unavailable, upload the locally built Windows artifacts to the GitHub Release page. The release should contain at least:

- `easyshow-<ver>-windows-x86_64-full.zip`
- `EasyShow-<ver>-Full-Setup.exe`

## Re-enabling Linux/macOS

Once the Windows build has been validated:

1. Restore the `linux` and `macos` jobs in `.github/workflows/release.yml` from Git history (they existed before the 0.2.7 Windows-only change).
2. Update the release-body text and the README build table.
3. Tag a new patch version and push it.

## Licenses

The repository ships `LICENSE-MIT`, `LICENSE-APACHE` and `docs/THIRD-PARTY.md`. Windows packages include a `licenses/` folder with the bundled DLL licenses.
