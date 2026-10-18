# EasyShow

轻量级跨平台图片查看器(Rust),已到 **M3+** 阶段:浏览、EXIF/位置、地图、动图、扩展格式均已可用。

- 定位:秒开、顺滑、够用 —— 冷启动 < 500 ms,内置 18+ 主流图片格式(含 HEIC/AVIF、JPEG XL、SVG、PSD、HDR/EXR 与 GIF/WebP 动图),中英双语、主题色可换
- **发布形态**:自 0.2.4 起只发布**完整版**(含 HEIC/AVIF、扩展格式与动图);**轻量版 lite 自 0.2.3 起冻结**,不再随新版更新
- 平台:Linux(X11 / Wayland)· Windows · macOS
- 技术栈:eframe(egui)+ wgpu;zune-jpeg / image / fast_image_resize 解码管线;HEIC/AVIF 经运行时加载的 libheif(编译期零 C 依赖);JPEG XL/SVG/PSD 使用纯 Rust 解码器

## 快速开始

```bash
cargo run -p easyshow-app -- <图片文件或文件夹>   # 打开图片 / 浏览目录
cargo test -p easyshow-core                        # 解码 / EXIF / 缩放 / 扩展格式单测
```

键位:← → 翻页 · + / − 缩放 · 0 适应窗口 · 1 实际大小 · R 旋转 · F 镜像 · F11 全屏 · Esc 退出全屏 ·
浏览模式 Space 播放/暂停幻灯片 · 网格 Delete 删除(需在设置中开启)· **M 地图模式(网格↔地图)**;也可直接拖拽图片/文件夹到窗口。

> 国内网络:本项目在 `~/.cargo/config.toml` 使用 USTC crates 镜像(见 PLAN 开发环境说明)。

## 版本与发布

- 版本号唯一来源:`Cargo.toml` 的 `[workspace.package]`(当前 **0.2.6**),每次更新用 `packaging/bump-version.sh` 递增第三位(默认 patch)
- 发布流程与 GitHub 上传步骤见 [docs/PUBLISHING.md](docs/PUBLISHING.md);打 `v*` 标签会触发三平台 CI 出包(完整版)

## 文档

- 📄 [功能清单 + 技术方案 + 里程碑](docs/PLAN.md)

## 部署(用户级本地安装,无需 sudo)

```bash
packaging/install-local.sh                 # 完整版:构建(如缺)+ 安装二进制/图标/桌面条目 + 桌面快捷方式
packaging/install-local.sh --lite          # 轻量版(安装为 easyshow-lite,与完整版可并存)
packaging/install-local.sh --set-default   # 可选:把 EasyShow 设为常见图片格式的默认打开程序
packaging/install-local.sh --uninstall     # 卸载以上全部内容(--lite --uninstall 卸载轻量版)
```

安装位置(均为用户级,不碰系统目录):

| 内容 | 路径 |
|---|---|
| 可执行文件 | `~/.local/bin/easyshow` |
| 桌面条目(应用列表 + "打开方式") | `~/.local/share/applications/easyshow.desktop` |
| 图标(256/128/64) | `~/.local/share/icons/hicolor/*/apps/easyshow.png` |
| 桌面快捷方式 | `~/桌面/easyshow.desktop`(自动识别 xdg 桌面目录) |

## 打包(安装包)

**完整版 full**(含 HEIC/AVIF)与**轻量版 lite**(不含 HEIC/AVIF,不随附任何 DLL)成套产出,两个变体可并存安装:

| 平台 | 产物 | 构建方式 |
|---|---|---|
| Linux | `easyshow_<ver>_amd64.deb`;`easyshow-<ver>-linux-x86_64-full.tar.gz`(便携) | `packaging/build-linux.sh` → `dist/linux/` |
| Windows | `EasyShow-<ver>-Full-Setup.exe`(NSIS 安装包);`easyshow-<ver>-windows-x86_64-full.zip`(便携) | `packaging/windows/make-package.sh`(可在 Linux 上交叉编译) |
| macOS | `EasyShow-<ver>-universal.dmg`(Intel + Apple Silicon 通用) | `packaging/macos/make-bundle.sh`(需 macOS 或 CI) |
| 三平台 | 同上 + AppImage | `.github/workflows/release.yml`(推送 `v*` 标签或手动触发) |

Windows 安装包特性:用户级安装(无需管理员)、开始菜单/桌面快捷方式、加入常见图片格式"打开方式"、控制面板卸载项,含中文界面;完整版与轻量版使用**不同的安装目录与注册表键,可同时安装**。

> **变体说明**
> - **完整版**:含 HEIC/AVIF(运行时 dlopen libheif,编译期零 C 依赖)、JPEG XL/SVG/PSD/HDR/EXR 与 GIF/WebP 动图。Windows 随包附带 libheif DLL 闭包;Linux 经系统 `libheif1`(deb 中 Recommends);macOS 默认把 libheif 打入 App 内。
> - **轻量版 lite(冻结)**:自 0.2.3 起不再更新,也不随新版发布;需要旧 lite 包可从 v0.2.3 的 Release 下载。

## 功能(当前版本)

- **目录树浏览**:左侧目录树逐级展开,点击进入即显示该目录全部图片的缩略图网格;**Windows 下为资源管理器式(「此电脑」+ 常用位置 + 盘符),图标直接取自系统(与资源管理器一致)**
- **缩略图网格**:双击缩略图进入浏览模式,**Esc 返回网格**;方向键移动选择,Enter 打开
- **子目录卡片**:当前目录下含照片的子目录会以卡片形式排在网格最前(递归 2 层统计),封面从子目录照片中稳定随机选取,显示照片数量;**双击进入**,Enter 同样可进入
- **时间轴**:按时间排序时显示年 / 月 / 日分组标题与右侧时间轴导航(年/月 + 每月张数),点击跳转,滚动时年月标题吸顶;缺省按**文件修改时间降序**(打开快),可切换为 **EXIF 拍摄时间**(首次需后台扫描全目录 EXIF,状态栏显示进度)
- **单图浏览**:每张图打开时默认**适应窗口**(缩放不带到下一张)/ 1:1 / 滚轮以光标为锚点缩放 / 拖动平移 / 双击切换 / 旋转与镜像
- **幻灯片**:工具栏 ▶ 或浏览模式 Space 播放;播放**整个目录**或**选中的批量照片**(网格中多选 ≥2 张后启动即只在选中集内循环);间隔 3 / 5 / 10 秒可设
- **批量删除(默认只读)**:缺省不开启删除;设置中勾选「允许删除」后,网格里 Ctrl/Shift 多选、Delete 键或垃圾桶按钮 → **二次确认弹窗** → 移入系统回收站(三平台,可恢复)
- **信息面板**(右侧):文件尺寸、大小、格式、修改时间 + **完整 EXIF**(相机、镜头、光圈、快门、ISO、焦距、拍摄时间、**GPS 坐标**;HEIC/AVIF 亦可读);**一键复制**为纯文本(含 EXIF、地名与坐标)
- **照片位置**:信息面板把 GPS 显示为「国家-省-市-区县-乡镇」——高德为主(设置里填免费 Web 服务 Key)、OSM Nominatim 兜底(免 Key);结果按位置缓存,重复查看不联网;无网或关闭联网时回退显示坐标
- **地图模式**:工具栏地图按钮(或 **M** 键)把当前目录带 GPS 的照片聚合显示在图上——**聚合点直接显示照片微缩图**(最多 4 张,随缩放展示 1/4 张,超出显示 +N),**单击聚合打开查看页,展示该聚合的全部照片**(虚拟滚动;点缩略图选中、双击进入浏览,`Esc` 逐级退回查看页→原地图位置),单击单张选中、双击查看;内置**离线底图(世界轮廓 + 中国省界,省名随缩放显示)**,开启联网后叠加高德/OSM 在线瓦片(按视口下载并缓存,自动处理高德 GCJ-02 坐标偏移)
- **联网默认关闭**:设置中开启「联网功能」后才会请求地名/瓦片;可填高德 Key、选择底图来源(自动 / 高德 / OpenStreetMap)
- **格式**:JPEG / PNG / GIF / WebP(含动图)/ BMP / TIFF / ICO / TGA / PNM / QOI;**HEIC / AVIF**(运行时 libheif)、**JPEG XL**(jxl-oxide)、**SVG**(resvg 按需栅格化)、**PSD**(合成图)、**HDR / EXR**(色调映射)
- **动图**:GIF / WebP 完整播放(后台流式解码,内存有界),工具栏播放/暂停、显示总帧数
- **速度**:缩略图磁盘缓存(二次打开秒出)、相邻图预取(翻页零等待)、按显示尺寸解码、头部读尺寸不整图解码;**视口优先**——上万文件的大目录只解析"滚动到哪里就显示哪里"的可见区域(±2 行预读),拖动不卡;**HEIC 缩略图优先使用文件内嵌的小图**,不再为每张缩略图解码全尺寸主图;地图与聚合查看页的缩略图统一调度,大聚合优先、限制每屏请求量,避免大量聚合时空白
- **排序**:修改时间(缺省,降序)/ **拍摄时间** / 名称(自然序,IMG_2 在 IMG_10 前)/ 创建时间 / 大小 / 类型,升降序一键切换
- **鼠标**:滚轮按"格"缩放(每格 1.15×,光标为锚,平台一致手感)、触控板捏合缩放、左/中键拖动平移、双击适应↔1:1;可在设置中把滚轮改为"翻页"
- **底部状态栏**:文件名、序号、尺寸、格式、**文件大小、修改日期**、当前缩放
- **界面**:Apple 风格细线图标工具栏(悬停提示)、左侧目录树、右侧信息面板(含 EXIF)、**中/英文一键切换**、**深/浅主题(缺省跟随系统,设置中可锁定)**

## 界面

![目录树 + 缩略图网格](docs/screenshots/m2-grid-tree.png)
![浏览模式 + EXIF 信息](docs/screenshots/m2-viewer-exif.png)
![扩展格式缩略图(SVG/PSD/HDR/EXR/JXL/动图)](docs/screenshots/m7-formats-grid.png)
![SVG 矢量渲染](docs/screenshots/m7-svg.png)
![PSD 合成图](docs/screenshots/m7-psd.png)
![GIF 动图播放(3 帧)](docs/screenshots/m7-anim-gif.png)
![地图聚合缩略图](docs/screenshots/m7-map-thumbnails.png)
![聚合查看页:全部照片 + 虚拟滚动](docs/screenshots/m9-map-cluster.png)
![地图模式 · 离线世界图 + 照片聚合](docs/screenshots/m6-map-offline.png)
![地图模式 · 在线瓦片(高德)+ 标记对齐](docs/screenshots/m6-map-online.png)
![照片位置:国家-省-市-区县-乡镇](docs/screenshots/m6-map-location.png)
![子目录卡片 + 年/月/日时间轴](docs/screenshots/m8-grid-timeline.png)
![离线底图:中国省界 + 省名](docs/screenshots/m8-map-provinces.png)
![批量删除二次确认(默认只读)](docs/screenshots/m5-delete-confirm.png)
![幻灯片(所选批量,2/2)](docs/screenshots/m5-slideshow.png)
![大目录视口优先缩略图](docs/screenshots/m5-viewport-priority.png)
![设置:删除开关默认关闭 + 幻灯片间隔](docs/screenshots/m5-settings-readonly.png)
![JPEG 查看](docs/screenshots/m1-jpeg.png)
![HEIC 解码](docs/screenshots/m1-heic.png)
![24MP 目录浏览](docs/screenshots/m1-directory-24mp.png)

实测(release):24MP JPEG 解码 ~104 ms;24MP 按显示尺寸降采样后纹理仅 2100×1400。

## 进度

- [x] 功能规划与技术方案
- [x] UI 设计稿(见会话内可交互演示)
- [x] M1 骨架:窗口、打开与显示、缩放平移、HEIC 运行时解码
- [x] M2 浏览:目录树、缩略图网格 + 磁盘缓存、视口优先解析、相邻预取、排序
- [x] M3 打磨(部分):深浅主题(随系统)、中英双语、EXIF 信息面板、全屏、批量删除(回收站)、幻灯片
- [x] M3+ 位置与地图:逆地理编码(高德/OSM,可配 Key、结果缓存)、地图模式(离线世界图 + 在线瓦片、聚合缩略图/弹层)
- [x] M3+ 扩展格式与动图:HDR/EXR、JPEG XL、SVG、PSD、GIF/WebP 动图播放
- [ ] M3 余项:配置持久化(目前仅联网设置落盘)、单实例
- [ ] M4 发布:三平台打包与文件关联

License(计划):MIT OR Apache-2.0
