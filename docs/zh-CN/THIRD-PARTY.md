# 第三方组件与许可

[English](../THIRD-PARTY.md) | **简体中文**

EasyShow 本体采用 MIT OR Apache-2.0。以下为随二进制分发的主要第三方组件(均为纯 Rust 或仅按需运行时加载):

| 组件 | 用途 | 许可 |
|---|---|---|
| image / zune-jpeg / fast_image_resize | 解码与缩放 | MIT / Apache-2.0 |
| eframe / egui | GUI | MIT / Apache-2.0 |
| kamadak-exif | EXIF | BSD-2-Clause |
| libloading + libheif(运行时加载) | HEIC/AVIF | libloading MIT;libheif LGPL-2.1(动态链接,独立组件) |
| ureq / rustls / ring / webpki-roots | HTTPS(地名/瓦片) | MIT / Apache-2.0 / ISC |
| serde_json | JSON | MIT / Apache-2.0 |
| libc | 本地时间转换 | MIT / Apache-2.0 |
| crossbeam-channel | 工作线程通信 | MIT / Apache-2.0 |
| jxl-oxide | JPEG XL 解码 | Apache-2.0 |
| resvg / usvg / tiny-skia / fontdb | SVG 渲染与字体 | MPL-2.0 / MIT / BSD-3-Clause |
| psd | PSD 合成图 | MIT |
| exr(image 的 exr 特性) | OpenEXR 解码 | BSD-3-Clause |
| image-webp | 动画 WebP | MIT / Apache-2.0 |
| gif(image 的 gif 特性) | 动画 GIF | MIT / Apache-2.0 |
| Natural Earth / world-atlas | 离线世界地图 | Public Domain / ISC(见 assets/map/LICENSE-world-atlas.txt) |
| DataV.GeoAtlas(阿里云)/ 高德地图 | 离线中国省界与九段线蒙版、省名标注 | 数据版权归原服务方,仅作低分辨率底图展示;地图界面标注来源 |
| Phosphor Icons | 工具栏图标 | MIT(见 assets/fonts/LICENSE-Phosphor.txt) |

- `resvg` 系列为 MPL-2.0:本项目以库的形式静态链接使用,**未修改其源码**;其源码可从 crates.io 获取。
- `libheif` 仅在完整版中通过运行时 `dlopen` 使用(Windows 安装包随附 DLL,Linux 依赖系统 `libheif1`,macOS 打包内置),属于独立进程外组件。
- 在线地图瓦片/地名服务:地图数据 © OpenStreetMap contributors(ODbL)或 © 高德地图,使用条款由对应服务方规定。

## 应用内说明

设置 → 关于 EasyShow →「开源组件」可查看组件清单与 MIT/Apache 许可证全文。Windows 安装包内包含本文档(`THIRD-PARTY.md`)及随包 DLL 的 `licenses/` 目录。
