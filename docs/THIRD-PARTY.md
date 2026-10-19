# Third-Party Components and Licenses

**English** | [简体中文](zh-CN/THIRD-PARTY.md)

EasyShow itself is licensed under MIT OR Apache-2.0. The main third-party components distributed with the binary are listed below (all are pure Rust or loaded at runtime only when needed):

| Component | Purpose | License |
|---|---|---|
| image / zune-jpeg / fast_image_resize | Decoding and resizing | MIT / Apache-2.0 |
| eframe / egui | GUI | MIT / Apache-2.0 |
| kamadak-exif | EXIF | BSD-2-Clause |
| libloading + libheif (runtime-loaded) | HEIC/AVIF | libloading MIT; libheif LGPL-2.1 (dynamic linking, separate component) |
| ureq / rustls / ring / webpki-roots | HTTPS (place names/tiles) | MIT / Apache-2.0 / ISC |
| serde_json | JSON | MIT / Apache-2.0 |
| libc | Local time conversion | MIT / Apache-2.0 |
| crossbeam-channel | Worker communication | MIT / Apache-2.0 |
| webbrowser | Opening external links (About/GitHub) | MIT / Apache-2.0 |
| jxl-oxide | JPEG XL decoding | Apache-2.0 |
| resvg / usvg / tiny-skia / fontdb | SVG rendering and fonts | MPL-2.0 / MIT / BSD-3-Clause |
| psd | PSD composite decoding | MIT |
| exr (image feature) | OpenEXR decoding | BSD-3-Clause |
| image-webp | Animated WebP | MIT / Apache-2.0 |
| gif (image feature) | Animated GIF | MIT / Apache-2.0 |
| Natural Earth / world-atlas | Offline world map | Public Domain / ISC (see assets/map/LICENSE-world-atlas.txt) |
| DataV.GeoAtlas (Alibaba Cloud) / AMap | Offline China province boundaries, nine-dash line and labels | Data copyright belongs to the service providers; used only as a low-resolution basemap with attribution in the UI |
| Phosphor Icons | Toolbar icons | MIT (see assets/fonts/LICENSE-Phosphor.txt) |

- The `resvg` family is MPL-2.0. This project links it as a library and **does not modify its source**; the source is available from crates.io.
- `libheif` is used only in the full build through runtime `dlopen` (DLLs bundled with the Windows installer, system `libheif1` on Linux, bundled inside the macOS app). It is a separate runtime component.
- Online map tiles and geocoding: map data © OpenStreetMap contributors (ODbL) or © AMap; the terms of the respective service apply.

## In-app notices

Settings → About EasyShow → **Open source components** shows the component table and the full MIT/Apache license texts. The Windows installer includes this document (`THIRD-PARTY.md`) and the `licenses/` folder with the bundled DLL licenses.
