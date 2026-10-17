#!/usr/bin/env python3
"""从 world-atlas TopoJSON 生成离线世界底图蒙版(land + borders)。

用法: python3 packaging/tools/worldmap.py <countries-110m.json> <输出目录>

产物(白色 RGB + alpha 覆盖率,便于运行时用主题色 tint):
  world-land.png     陆地
  world-borders.png  国界线

投影:Web Mercator,覆盖整个世界,输出 2048×2048。
数据来源: Natural Earth 110m(public domain)/ world-atlas 2.0.2(ISC)。
"""
import json
import math
import sys

from PIL import Image, ImageDraw

SIZE = 2048
SS = 2  # 超采样:4096 绘制后降采样,边缘更平滑


def load_topo(path):
    with open(path) as f:
        topo = json.load(f)
    tr = topo.get("transform")
    arcs = []
    for arc in topo["arcs"]:
        pts = []
        x = y = 0
        for dx, dy in arc:
            x += dx
            y += dy
            if tr:
                lon = x * tr["scale"][0] + tr["translate"][0]
                lat = y * tr["scale"][1] + tr["translate"][1]
            else:
                lon, lat = x, y
            pts.append((lon, lat))
        arcs.append(pts)

    def ring(idx_list):
        pts = []
        for i in idx_list:
            arc = arcs[~i][::-1] if i < 0 else arcs[i]
            pts.extend(arc if not pts else arc[1:])
        return pts

    polygons = []
    for g in topo["objects"]["countries"]["geometries"]:
        if g["type"] == "Polygon":
            polygons.append([ring(r) for r in g["arcs"]])
        elif g["type"] == "MultiPolygon":
            for poly in g["arcs"]:
                polygons.append([ring(r) for r in poly])
    return polygons


def merc(lon, lat):
    lat = max(-85.05112878, min(85.05112878, lat))
    x = (lon + 180.0) / 360.0
    y = 0.5 - math.log(math.tan(math.pi / 4 + math.radians(lat) / 2)) / (2 * math.pi)
    return (x * SIZE * SS, y * SIZE * SS)


def unwrap_ring(pts):
    """展开跨 ±180° 经线的环,避免出现横贯全图的直线。"""
    out = []
    prev = None
    for lon, lat in pts:
        if prev is not None:
            while lon - prev > 180.0:
                lon -= 360.0
            while lon - prev < -180.0:
                lon += 360.0
        out.append((lon, lat))
        prev = lon
    return out


def render(polygons, fill_land):
    img = Image.new("L", (SIZE * SS, SIZE * SS), 0)
    draw = ImageDraw.Draw(img)
    for poly in polygons:
        for ring in poly:
            ring = unwrap_ring(ring)
            if len(ring) < 3:
                continue
            # 展开后的经度可能超出 ±180°,三份偏移绘制由画布自动裁剪
            for offset in (-360.0, 0.0, 360.0):
                pts = [merc(lon + offset, lat) for lon, lat in ring]
                if fill_land:
                    draw.polygon(pts, fill=255)
                else:
                    draw.line(pts + [pts[0]], fill=255, width=3, joint="curve")
    return img.resize((SIZE, SIZE), Image.LANCZOS)


def save_mask(img, path):
    rgba = Image.new("RGBA", img.size, (255, 255, 255, 0))
    rgba.putalpha(img)
    rgba.save(path, optimize=True)


def main():
    src, out = sys.argv[1], sys.argv[2]
    polygons = load_topo(src)
    save_mask(render(polygons, True), f"{out}/world-land.png")
    save_mask(render(polygons, False), f"{out}/world-borders.png")
    print("ok")


if __name__ == "__main__":
    main()
