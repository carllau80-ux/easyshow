#!/usr/bin/env python3
"""从阿里云 DataV 中国省级 GeoJSON 生成离线省界蒙版与省名标注。

用法: python3 packaging/tools/chinamap.py <100000_full.json> <输出目录>

产物(白色 + alpha 蒙版,运行时按主题 tint):
  china-land.png      中国陆地(省域填充)
  china-borders.png   省界 + 九段线
  china-provinces.txt 省名标注 "简称|经度|纬度"(每行一个)

投影:Web Mercator,覆盖中国及南海(约 73.4°E–135.2°E,2.6°N–54.0°N),
输出宽 4096,约 1.7 px/km,可支撑到地图缩放 z6。

数据来源:DataV.GeoAtlas 行政区划边界(© 阿里云 DataV / 高德地图),
下载地址 https://geo.datav.aliyun.com/areas_v3/bound/100000_full.json
"""
import json
import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw

WIDTH = 3072
SS = 2  # 超采样
LON0, LON1, LAT0, LAT1 = 73.4, 135.2, 2.6, 54.0
BORDER_W = 1.6 * SS


def merc_y(lat):
    lat = max(-85.05112878, min(85.05112878, lat))
    return 0.5 - math.log(math.tan(math.pi / 4 + math.radians(lat) / 2)) / (2 * math.pi)


TOP = merc_y(LAT1)
BOTTOM = merc_y(LAT0)
HEIGHT = round(WIDTH * (BOTTOM - TOP) * 360.0 / (LON1 - LON0))


def project(lon, lat):
    x = (lon - LON0) / (LON1 - LON0) * WIDTH * SS
    y = (merc_y(lat) - TOP) / (BOTTOM - TOP) * HEIGHT * SS
    return (x, y)


def rings(geometry):
    """返回 [(点集, 是否外环)]。"""
    out = []
    if geometry["type"] == "Polygon":
        for i, ring in enumerate(geometry["coordinates"]):
            out.append((ring, i == 0))
    else:
        for poly in geometry["coordinates"]:
            for i, ring in enumerate(poly):
                out.append((ring, i == 0))
    return out


def main():
    src, outdir = sys.argv[1], Path(sys.argv[2])
    outdir.mkdir(parents=True, exist_ok=True)
    with open(src) as f:
        data = json.load(f)

    size = (WIDTH * SS, HEIGHT * SS)
    land = Image.new("L", size, 0)
    draw = ImageDraw.Draw(land)
    provinces = [f for f in data["features"] if f["properties"].get("name")]
    sea = [f for f in data["features"] if not f["properties"].get("name")]

    for feature in provinces:
        for ring, outer in rings(feature["geometry"]):
            pts = [project(lon, lat) for lon, lat in ring]
            if len(pts) >= 3:
                draw.polygon(pts, fill=255 if outer else 0)

    borders = Image.new("L", size, 0)
    bdraw = ImageDraw.Draw(borders)
    for feature in data["features"]:
        for ring, _ in rings(feature["geometry"]):
            pts = [project(lon, lat) for lon, lat in ring]
            if len(pts) >= 2:
                bdraw.line(pts + [pts[0]], fill=255, width=max(1, round(BORDER_W)),
                           joint="curve")

    target = (WIDTH, HEIGHT)
    land = land.resize(target, Image.LANCZOS)
    borders = borders.resize(target, Image.LANCZOS)

    def save(mask, name):
        # 灰度 → 白色 + alpha,运行时用任意主题色 tint
        rgba = Image.merge("LA", (Image.new("L", target, 255), mask)).convert("RGBA")
        rgba.save(outdir / name, optimize=True)

    save(land, "china-land.png")
    save(borders, "china-borders.png")

    short = {
        "内蒙古自治区": "内蒙古", "广西壮族自治区": "广西", "西藏自治区": "西藏",
        "宁夏回族自治区": "宁夏", "新疆维吾尔自治区": "新疆",
        "香港特别行政区": "香港", "澳门特别行政区": "澳门",
    }
    lines = []
    for feature in provinces:
        p = feature["properties"]
        name = p["name"]
        label = short.get(name)
        if label is None:
            label = name[:-1] if name.endswith(("省", "市")) else name
        point = p.get("centroid") or p.get("center")
        if not point:
            continue
        lines.append(f"{label}|{point[0]:.5f}|{point[1]:.5f}")
    (outdir / "china-provinces.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")

    print(f"china map: {WIDTH}x{HEIGHT}, {len(provinces)} provinces, "
          f"{len(sea)} sea features, {len(lines)} labels")
    print(f"wrote {outdir}/china-land.png, china-borders.png, china-provinces.txt")


if __name__ == "__main__":
    main()
