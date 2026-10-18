# EasyShow · Lightweight Cross-Platform Image Viewer — Plan and Technical Design

**English** | [简体中文](zh-CN/PLAN.md)

> **EasyShow** is the project name (earlier working names: Lumen, Liteshow).
> One-line goal: a desktop image viewer that is fast, smooth and sufficient, written in Rust, with one codebase for Linux, Windows and macOS.
> References: qimgv (very fast), ImageGlass (feature-rich), nomacs — take the "fast and beautiful" intersection.
> Current stage: M1/M2 and the M3+ map/format/UX tracks are delivered; maintenance and release work continues.

---

## 1. Goals and non-goals

**Core goals**

- ⚡ Fast: cold start < 500 ms; 24MP JPEG preview in tens of milliseconds; zoom/pan at ≥ 60 fps
- 🪶 Small: no Electron/WebView, modest memory use and package size
- 🖼 Format coverage: 18+ common formats built in, including phone HEIC/AVIF
- 💻 Consistent across platforms: Linux (X11), Windows, macOS; installers and "Open with" integration
- 🧭 Convenient: remembers the last browsed folder and restores it on launch

**Explicit non-goals**

- ❌ Library management (tags, faces, cloud sync) — digiKam / Lightroom territory
- ❌ Professional editing (layers, masks) — only light editing is planned
- ❌ Video playback

---

## 2. Feature list

### P0 — first release

**A. Opening and formats**

| ID | Feature | Notes |
|---|---|---|
| A1 | Open file / folder | CLI argument, drag and drop, dialogs; optional default-viewer registration |
| A2 | Built-in decoding | JPEG/PNG/GIF/WebP/BMP/TIFF/ICO/TGA/PNM/QOI; HEIC/AVIF via runtime libheif; JPEG XL/SVG/PSD/HDR/EXR through format features |
| A3 | Robustness | Corrupt/oversized/unknown files fail gracefully; decoding runs off the UI thread |
| A4 | State | Restore the last directory on launch (`state.json`); remember network settings (`config.json`) |
| A5 | Live Photos | Recognized in principle; still images are shown, video playback is out of scope |

**B. Browsing and navigation**

| ID | Feature | Notes |
|---|---|---|
| B1 | Previous/next/first/last | Toolbar and keyboard |
| B2 | Thumbnail grid + subfolder cards | Current folder plus direct subfolders that contain photos; folder covers are stable random photos |
| B3 | Sorting | Modified time (default, descending), EXIF capture time, name, created time, size, type |
| B4 | Timeline | Year/month/day headers and a year/month navigator; click to jump; sticky headers |
| B5 | Background scanning | Directory scans, EXIF times, GPS and folder summaries run in workers |

**C. Viewing interaction**

| ID | Feature | Notes |
|---|---|---|
| C1 | Zoom | Fit to window by default for every image; 1:1; cursor-anchored wheel zoom; double-click fit↔1:1 |
| C2 | Pan and transform | Drag pan; 90° rotation; horizontal mirror; EXIF orientation corrected at decode |
| C3 | Immersion | Fullscreen, slideshow (whole folder or selected batch), light/dark themes |
| C4 | Info panel | File info plus full EXIF, GPS, place name and one-click text copy |
| C5 | Shortcuts and gestures | Full keyboard shortcuts; trackpad pinch; drag and drop |

**D. Performance**

| ID | Feature | Notes |
|---|---|---|
| D1 | Display-size decoding | Decode for the physical viewport × DPR; full resolution on demand |
| D2 | Neighbor prefetch | Prefetch adjacent images in navigation order |
| D3 | Multi-level caching | Thumbnail disk cache (path + mtime + size), in-memory bitmap LRU, GPU textures |
| D4 | Cancellable async work | Generation IDs discard stale decode results; workers never block the UI |

**E. Details**

- English/Chinese UI with a one-click toggle; default follows the system language
- Light/dark themes following the system, with a manual override
- HiDPI and multi-monitor friendly
- Single-instance mode: still on the roadmap

### P1 — enhancements

- Light editing: crop, arbitrary rotation, mirror; save as PNG/JPEG/WebP
- File operations: rename (F2), delete to trash, move, copy image to clipboard
- Folder watcher with live refresh
- Histogram panel; set as wallpaper; improved SVG support

### P2 — optional extensions

- RAW decoding, ICC color management (lcms2), live-photo video
- Batch processing, custom shortcuts, portable mode

---

## 3. Technical design

### 3.1 GUI framework

| Candidate | Verdict |
|---|---|
| **eframe (egui) + glow** | ✅ Chosen: pure Rust, no WebView, immediate-mode canvas is a good fit, consistent GPU rendering across platforms |
| iced | Fallback: larger canvas/custom-interaction effort |
| Slint | Fallback: license considerations |
| GTK4 | Rejected: poor Windows/macOS distribution |
| Tauri | Rejected: system WebView conflicts with the lightweight goal |

The UI is custom-drawn in a modern, minimal style; the image canvas is fully GPU-rendered.

### 3.2 Workspace layout

```
easyshow/
├── Cargo.toml                  # workspace; release: thin LTO, strip, codegen-units=1
├── crates/
│   ├── easyshow-core/          # no UI: formats, resize, EXIF, thumbnails, time utilities
│   │   ├── codec/              # format probing and decoders (image, libheif, JXL, SVG, PSD...)
│   │   ├── resize/             # fast_image_resize wrapper (SIMD)
│   │   ├── exif.rs             # EXIF parsing, orientation, GPS
│   │   ├── thumb.rs            # thumbnail pipeline (embedded HEIC thumbnail fast path)
│   │   └── timeutil.rs         # EXIF capture time and local wall-clock conversion
│   └── easyshow-app/           # eframe application
│       ├── app.rs              # state, modes, worker polling, navigation
│       ├── ui.rs               # toolbar, tree, grid/timeline, info panel, status
│       ├── map.rs              # map view, clusters, cluster gallery
│       ├── thumbs.rs / decode.rs / time_scan.rs / folders.rs / geo_scan.rs
│       └── config.rs           # config.json + state.json
└── packaging/                  # installer scripts, CI helpers, map assets
```

### 3.3 Key dependencies

| Purpose | Crate | Notes |
|---|---|---|
| Window/UI/GPU | eframe/egui (glow) | X11 on Linux; no wayland-client dependency in the current build |
| Decoding | image, zune-jpeg, image-webp | image covers most formats; zune-jpeg is a fast pure-Rust JPEG path |
| HEIC/AVIF | runtime libheif via libloading, feature `heic` | No compile-time C dependency; friendly degradation if the library is missing |
| Resize | fast_image_resize | AVX2/NEON SIMD |
| EXIF | kamadak-exif | Orientation, camera data, GPS |
| Extended formats | jxl-oxide, resvg/usvg/fontdb, psd, image hdr/exr | Pure Rust, feature-gated by `formats` |
| Animation | image GifDecoder, image-webp | Streaming frames with a bounded channel |
| Networking | ureq 3 + rustls/ring + webpki-roots | Blocking HTTP in workers; online features off by default |
| JSON/config | serde_json | Platform config directory |
| Clipboard/UI | egui built-ins | Info copy and language/theme toggles |
| Packaging | shell scripts + GitHub Actions | Windows NSIS/zip; Linux deb/tar; macOS app/dmg |

The feature flags:
- `heic` (default in full): HEIC/AVIF support and libloading
- `formats` (default in full): JPEG XL, SVG, PSD, HDR/EXR, animation
- Full = default features; lite = `--no-default-features` (frozen since 0.2.3)

### 3.4 Why it is fast

1. **Decode tiers**
   - Thumbnails are decoded at display size (or from an embedded HEIC thumbnail) and downsampled immediately.
   - The viewer decodes at physical viewport × DPR, then requests full resolution only when zoomed beyond it.
2. **Cancellable concurrency**: every image change carries a generation ID; stale results are discarded, so rapid navigation never catches up with old work.
3. **Caching**: disk thumbnails (cross-session), in-memory bitmap LRU, GPU textures with viewport-aware eviction.
4. **GPU transforms**: the image is uploaded once; zoom/pan/rotation are transform operations.
5. **Viewport priority**: huge folders decode only the visible area (±2 rows), with map and gallery thumbnails scheduled under a per-screen budget.

### 3.5 Cross-platform and release

- Build targets: Linux x86_64, Windows x86_64, macOS universal (scripts; only Windows is published at the moment)
- CI: `.github/workflows/release.yml`; currently Windows-only until the Windows package has been validated. Restore the Linux/macOS jobs after validation.
- libheif is a runtime dependency: Windows bundles the DLL closure; Linux uses system `libheif1`; macOS bundles it in the app.
- File associations: `.desktop`, Windows registry, macOS Info.plist.
- Config/cache directories follow platform conventions; the bundled CJK font fallback is selected from system fonts.

### 3.6 Performance targets

| Metric | Target |
|---|---|
| Cold start to first frame | < 500 ms |
| 24MP JPEG: preview → sharp | tens of ms → < 150 ms |
| 100 thumbnails of 24MP photos (cold cache) | a few seconds |
| Zoom/pan | ≥ 60 fps |
| Idle memory / installer size | modest; no WebView |

Measured on release builds: 24MP JPEG full decode ~104 ms; display-size texture 2100×1400 for a 24MP source.

### 3.7 Risks and mitigations

| Risk | Mitigation |
|---|---|
| Huge images exceed GPU texture limits | Downsample for display; tiling is a future enhancement |
| HEIC needs libheif | Runtime loading with graceful degradation; Windows bundles the DLL closure |
| Many clusters on a map overwhelm the thumbnail queue | Per-screen budget, larger clusters first, virtualized cluster gallery |
| Wide-gamut photos look wrong without color management | Documented limitation; ICC support is future work |
| Reading phone photos (MTP/PTP) needs platform work | Planned as a device import/browse feature; not part of the current release |

---

## 4. Milestones

| Stage | Content | Status |
|---|---|---|
| M1 foundation | Window, open file/folder, drag & drop, single image, zoom/pan, shortcuts | ✅ Delivered (including runtime HEIC decoding) |
| M2 browsing | Folder index, thumbnail grid, sorting, disk cache, progressive loading, EXIF orientation | ✅ Delivered (folder tree, subfolder cards, timeline, viewport priority) |
| M3 polish | Animation, slideshow, fullscreen, themes and bilingual UI, config, info panel | ✅ Mostly delivered (single instance pending) |
| M3+ location and map | Reverse geocoding (AMap/OSM), map mode with clusters, online toggle and caches | ✅ Delivered (offline world map, GCJ-02, online tiles) |
| M3+ extended formats | HDR/EXR, JPEG XL, SVG, PSD, GIF/WebP animation | ✅ Delivered |
| M3++ folders and timeline | Subfolder cards, year/month/day timeline, EXIF capture time, HEIC embedded thumbnails | ✅ Delivered (0.2.5) |
| M3++ offline province map | China province boundaries (including the nine-dash line) and labels | ✅ Delivered (0.2.5) |
| M3++ UX and map fixes | Fit-to-window default, copy info, thumbnail texture upload/scheduling, full cluster gallery | ✅ Delivered (0.2.6) |
| State and documentation | Last-directory restore, English-default docs with Chinese versions | ✅ Delivered (0.2.7) |
| Navigation and safety | Explorer-style address bar (history/refresh), selection bar, mandatory delete confirmation, About/contact, open-source notices | ✅ Delivered (0.2.8) |
| M4 release | Windows full builds published; Linux/macOS after validation | 🔄 Windows done, other platforms pending |

---

## 5. UI notes

- Top: toolbar (open / folder │ tree/info │ map │ prev/next │ viewer controls │ fullscreen · language · sort · slideshow · settings)
- Center: grid with subfolder cards and timeline, viewer canvas, or map
- Left: folder tree; right: info panel with EXIF and copy
- Bottom: status bar (file name, index, dimensions, format, size, modified date, zoom)
- Dark theme by default, light theme switchable; accent color follows the system palette
- Bilingual UI: English/Chinese toggle in the toolbar

---

## 6. Development notes

- Mainland China mirror: `~/.cargo/config.toml` uses the USTC crates mirror.
- Tests: `cargo test --workspace` (decoder/format fixtures in `testdata/`); UI smoke tests are run on Xvfb during development.
- Packaging scripts are under `packaging/`; see the publishing guide.
