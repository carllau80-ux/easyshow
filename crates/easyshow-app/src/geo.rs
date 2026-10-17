//! 地理数学:WGS-84 ↔ GCJ-02、Web Mercator 投影、地图状态与屏幕聚合。
//!
//! 坐标约定:对外一律用 WGS-84(EXIF 的原始坐标);只有在使用高德底图/高德逆地理
//! 编码时才转换成 GCJ-02(中国境内的"火星坐标",偏移约 300–600 m)。

use std::collections::HashMap;

/// WGS-84 坐标(纬度, 经度;十进制度)。
pub type LatLon = (f64, f64);

// ----------------------------------------------------------------------
// WGS-84 ↔ GCJ-02
// ----------------------------------------------------------------------

const A: f64 = 6_378_245.0; // 克拉索夫斯基椭球长半轴
const EE: f64 = 0.006_693_421_622_965_943_23;

/// 中国境外不做偏移(标准判定矩形)。
fn out_of_china(lat: f64, lon: f64) -> bool {
    !(72.004..=137.8347).contains(&lon) || !(0.8293..=55.8271).contains(&lat)
}

fn transform_lat(x: f64, y: f64) -> f64 {
    use std::f64::consts::PI;
    let mut ret = -100.0 + 2.0 * x + 3.0 * y + 0.2 * y * y + 0.1 * x * y + 0.2 * x.abs().sqrt();
    ret += (20.0 * (6.0 * x * PI).sin() + 20.0 * (2.0 * x * PI).sin()) * 2.0 / 3.0;
    ret += (20.0 * (y * PI).sin() + 40.0 * (y / 3.0 * PI).sin()) * 2.0 / 3.0;
    ret += (160.0 * (y / 12.0 * PI).sin() + 320.0 * (y * PI / 30.0).sin()) * 2.0 / 3.0;
    ret
}

fn transform_lon(x: f64, y: f64) -> f64 {
    use std::f64::consts::PI;
    let mut ret = 300.0 + x + 2.0 * y + 0.1 * x * x + 0.1 * x * y + 0.1 * x.abs().sqrt();
    ret += (20.0 * (6.0 * x * PI).sin() + 20.0 * (2.0 * x * PI).sin()) * 2.0 / 3.0;
    ret += (20.0 * (x * PI).sin() + 40.0 * (x / 3.0 * PI).sin()) * 2.0 / 3.0;
    ret += (150.0 * (x / 12.0 * PI).sin() + 300.0 * (x / 30.0 * PI).sin()) * 2.0 / 3.0;
    ret
}

/// WGS-84 → GCJ-02。境外坐标原样返回。
pub fn wgs84_to_gcj02((lat, lon): LatLon) -> LatLon {
    use std::f64::consts::PI;
    if out_of_china(lat, lon) {
        return (lat, lon);
    }
    let mut d_lat = transform_lat(lon - 105.0, lat - 35.0);
    let mut d_lon = transform_lon(lon - 105.0, lat - 35.0);
    let rad_lat = lat / 180.0 * PI;
    let magic = 1.0 - EE * rad_lat.sin() * rad_lat.sin();
    let sqrt_magic = magic.sqrt();
    d_lat = (d_lat * 180.0) / ((A * (1.0 - EE)) / (magic * sqrt_magic) * PI);
    d_lon = (d_lon * 180.0) / (A / sqrt_magic * rad_lat.cos() * PI);
    (lat + d_lat, lon + d_lon)
}

/// GCJ-02 → WGS-84(迭代反解,误差 < 1e-7°)。用于切换底图时保持视野位置。
pub fn gcj02_to_wgs84((lat, lon): LatLon) -> LatLon {
    if out_of_china(lat, lon) {
        return (lat, lon);
    }
    let mut guess = (lat, lon);
    for _ in 0..3 {
        let (glat, glon) = wgs84_to_gcj02(guess);
        guess = (guess.0 - (glat - lat), guess.1 - (glon - lon));
    }
    guess
}

// ----------------------------------------------------------------------
// Web Mercator
// ----------------------------------------------------------------------

/// Web Mercator 的最大纬度(保证世界坐标落在 [0,1]²)。
pub const MAX_LAT: f64 = 85.051_128_779_806_6;

/// 经纬度 → 世界坐标(Web Mercator;x/y ∈ [0,1],y 向下)。
pub fn lonlat_to_world((lat, lon): LatLon) -> (f64, f64) {
    use std::f64::consts::PI;
    let lat_c = lat.clamp(-MAX_LAT, MAX_LAT);
    let x = ((lon + 180.0) / 360.0).clamp(0.0, 1.0);
    let y = (0.5 - ((PI / 4.0 + lat_c * PI / 360.0).tan().ln()) / (2.0 * PI)).clamp(0.0, 1.0);
    (x, y)
}

/// 世界坐标 → 经纬度。
pub fn world_to_lonlat((x, y): (f64, f64)) -> LatLon {
    use std::f64::consts::PI;
    let lon = x * 360.0 - 180.0;
    let n = PI - 2.0 * PI * y;
    let lat = n.sinh().atan().to_degrees();
    (lat, lon)
}

/// 缩放级别 → 每个世界单位对应的屏幕像素数(256px 瓦片)。
pub fn world_px(zoom: f64) -> f64 {
    256.0 * 2f64.powf(zoom)
}

// ----------------------------------------------------------------------
// 地图状态(平移/缩放/适配)
// ----------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct MapState {
    /// 视图中心(WGS-84)。
    pub center: LatLon,
    /// Web Mercator 缩放级别(整数为瓦片级,可带小数)。
    pub zoom: f64,
}

impl Default for MapState {
    fn default() -> Self {
        // 默认世界视角,略偏北半球(中国/欧洲/北美都在视野内)。
        Self { center: (30.0, 20.0), zoom: 1.6 }
    }
}

impl MapState {
    pub fn center_world(&self) -> (f64, f64) {
        lonlat_to_world(self.center)
    }

    fn world_per_px(&self) -> f64 {
        1.0 / world_px(self.zoom)
    }

    /// 屏幕坐标(相对地图区域左上角)→ 世界坐标。
    pub fn screen_to_world(&self, size: (f64, f64), screen: (f64, f64)) -> (f64, f64) {
        let cw = self.center_world();
        let dw = self.world_per_px();
        (cw.0 + (screen.0 - size.0 / 2.0) * dw, cw.1 + (screen.1 - size.1 / 2.0) * dw)
    }

    /// 世界坐标 → 屏幕坐标。
    pub fn world_to_screen(&self, size: (f64, f64), world: (f64, f64)) -> (f64, f64) {
        let cw = self.center_world();
        let px = world_px(self.zoom);
        ((world.0 - cw.0) * px + size.0 / 2.0, (world.1 - cw.1) * px + size.1 / 2.0)
    }

    /// 拖动平移(增量以屏幕像素计,方向与指针移动一致)。
    pub fn pan_px(&mut self, dx: f64, dy: f64) {
        let dw = self.world_per_px();
        let cw = self.center_world();
        let nx = (cw.0 - dx * dw).clamp(0.0, 1.0);
        let ny = (cw.1 - dy * dw).clamp(0.0, 1.0);
        self.center = world_to_lonlat((nx, ny));
    }

    /// 以屏幕某点为锚缩放:`steps` 为滚轮格数(每格 1.15 倍,与浏览模式一致)。
    pub fn zoom_by_steps(
        &mut self,
        cursor: (f64, f64),
        size: (f64, f64),
        steps: f32,
        min_zoom: f64,
        max_zoom: f64,
    ) {
        if steps == 0.0 || size.0 <= 1.0 || size.1 <= 1.0 {
            return;
        }
        let anchor = self.screen_to_world(size, cursor);
        let new_zoom = (self.zoom + steps as f64 * 1.15f64.log2()).clamp(min_zoom, max_zoom);
        if (new_zoom - self.zoom).abs() < 1e-9 {
            return;
        }
        self.zoom = new_zoom;
        let px = world_px(self.zoom);
        let cw = (
            anchor.0 - (cursor.0 - size.0 / 2.0) / px,
            anchor.1 - (cursor.1 - size.1 / 2.0) / px,
        );
        self.center = world_to_lonlat((cw.0.clamp(0.0, 1.0), cw.1.clamp(0.0, 1.0)));
    }

    /// 适配给定世界坐标点集(先投影再传入)。空集返回 false。
    pub fn fit_world(
        &mut self,
        points: &[(f64, f64)],
        size: (f64, f64),
        padding: f64,
        min_zoom: f64,
        max_zoom: f64,
    ) -> bool {
        let Some((center, zoom)) = fit_bounds(points, size, padding, min_zoom, max_zoom) else {
            return false;
        };
        self.center = world_to_lonlat(center);
        self.zoom = zoom;
        true
    }
}

/// 计算能容纳全部点的中心(世界坐标)与缩放级别。
pub fn fit_bounds(
    points: &[(f64, f64)],
    size: (f64, f64),
    padding: f64,
    min_zoom: f64,
    max_zoom: f64,
) -> Option<((f64, f64), f64)> {
    if points.is_empty() || size.0 <= 1.0 || size.1 <= 1.0 {
        return None;
    }
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in points {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let center = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
    let avail_w = (size.0 - 2.0 * padding).max(1.0);
    let avail_h = (size.1 - 2.0 * padding).max(1.0);
    let dx = max_x - min_x;
    let dy = max_y - min_y;
    let zoom = if dx <= 1e-12 && dy <= 1e-12 {
        // 单点(或全部重合):城市级视角
        10.0_f64.min(max_zoom).max(min_zoom)
    } else {
        let sx = if dx > 1e-12 { avail_w / dx } else { f64::INFINITY };
        let sy = if dy > 1e-12 { avail_h / dy } else { f64::INFINITY };
        (sx.min(sy)).max(1e-9).log2() - 8.0
    };
    Some((center, zoom.clamp(min_zoom, max_zoom)))
}

// ----------------------------------------------------------------------
// 屏幕聚合
// ----------------------------------------------------------------------

/// 一个屏幕聚合簇:成员为照片在条目列表中的索引。
#[derive(Debug, Clone)]
pub struct ScreenCluster {
    pub members: Vec<usize>,
    pub x: f32,
    pub y: f32,
}

/// 按屏幕网格聚合:同一 `cell` 像素格内的点合成一簇,坐标取平均值。
/// 输出按首个成员索引排序,保证渲染顺序稳定。
pub fn cluster_points(points: &[(usize, f32, f32)], cell: f32) -> Vec<ScreenCluster> {
    let cell = cell.max(1.0);
    let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (pos, &(_, x, y)) in points.iter().enumerate() {
        let key = ((x / cell).floor() as i32, (y / cell).floor() as i32);
        buckets.entry(key).or_default().push(pos);
    }
    let mut out: Vec<ScreenCluster> = buckets
        .into_values()
        .map(|positions| {
            let n = positions.len() as f32;
            let (mut sx, mut sy) = (0.0f32, 0.0f32);
            for &p in &positions {
                sx += points[p].1;
                sy += points[p].2;
            }
            let mut members: Vec<usize> = positions.into_iter().map(|p| points[p].0).collect();
            members.sort_unstable();
            ScreenCluster { members, x: sx / n, y: sy / n }
        })
        .collect();
    out.sort_by_key(|c| c.members.first().copied().unwrap_or(usize::MAX));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn gcj02_matches_known_offsets() {
        // 上海人民广场、北京天安门、深圳福田(用标准算法算出的期望值)
        let sh = wgs84_to_gcj02((31.230416, 121.473701));
        assert!(close(sh.0, 31.228_473_742, 1e-6), "sh lat {sh:?}");
        assert!(close(sh.1, 121.478_224_057, 1e-6), "sh lon {sh:?}");

        let bj = wgs84_to_gcj02((39.9042, 116.4074));
        assert!(close(bj.0, 39.905_603_343, 1e-6), "bj {bj:?}");

        let sz = wgs84_to_gcj02((22.5431, 114.0579));
        assert!(close(sz.0, 22.540_382_814, 1e-6), "sz {sz:?}");
    }

    #[test]
    fn gcj02_leaves_overseas_unchanged() {
        let paris = (48.8566, 2.3522);
        assert_eq!(wgs84_to_gcj02(paris), paris);
        let tokyo = (35.6762, 139.6503);
        assert_eq!(wgs84_to_gcj02(tokyo), tokyo);
    }

    #[test]
    fn gcj02_inverse_round_trips() {
        for p in [(31.230416, 121.473701), (39.9042, 116.4074), (22.5431, 114.0579)] {
            let back = gcj02_to_wgs84(wgs84_to_gcj02(p));
            assert!(close(p.0, back.0, 1e-6) && close(p.1, back.1, 1e-6), "{p:?} -> {back:?}");
        }
    }

    #[test]
    fn mercator_round_trips() {
        for p in [(31.230416, 121.473701), (0.0, 0.0), (-33.8688, 151.2093), (60.0, -120.0)] {
            let w = lonlat_to_world(p);
            let back = world_to_lonlat(w);
            assert!(close(p.0, back.0, 1e-9), "{p:?} -> {w:?} -> {back:?}");
            assert!(close(p.1, back.1, 1e-9), "{p:?} -> {w:?} -> {back:?}");
        }
        // 极点被夹到 Web Mercator 的极限纬度
        let pole = lonlat_to_world((89.0, 0.0));
        assert!(pole.1 >= 0.0 && pole.1 <= 1.0);
        assert!(close(pole.1, 0.0, 1e-9), "north clamp {pole:?}");
    }

    #[test]
    fn screen_world_round_trip() {
        let size = (800.0, 600.0);
        let mut map = MapState { center: (31.2, 121.4), zoom: 9.0 };
        let screen = (640.0, 110.0);
        let w = map.screen_to_world(size, screen);
        let back = map.world_to_screen(size, w);
        assert!(close(back.0, screen.0, 1e-6) && close(back.1, screen.1, 1e-6));

        // 缩放锚点:光标下的世界坐标保持不变
        let before = map.screen_to_world(size, screen);
        map.zoom_by_steps(screen, size, 2.0, 0.0, 18.0);
        let after = map.screen_to_world(size, screen);
        assert!(close(before.0, after.0, 1e-9) && close(before.1, after.1, 1e-9), "{before:?} {after:?}");
    }

    #[test]
    fn fit_includes_all_points() {
        let size = (1000.0, 800.0);
        let pts = vec![lonlat_to_world((31.23, 121.47)), lonlat_to_world((31.24, 121.48))];
        let (center, zoom) = fit_bounds(&pts, size, 40.0, 0.0, 17.0).unwrap();
        let map = MapState { center: world_to_lonlat(center), zoom };
        for p in &pts {
            let (x, y) = map.world_to_screen(size, *p);
            assert!(x >= -1.0 && x <= size.0 + 1.0 && y >= -1.0 && y <= size.1 + 1.0, "outside: {x},{y}");
        }
    }

    #[test]
    fn clustering_groups_by_screen_distance() {
        let points = [(0usize, 10.0, 10.0), (1, 14.0, 12.0), (2, 200.0, 200.0), (3, 205.0, 201.0)];
        let clusters = cluster_points(&points, 40.0);
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].members, vec![0, 1]);
        assert_eq!(clusters[1].members, vec![2, 3]);
        // 单点
        let single = cluster_points(&[(7usize, 5.0, 5.0)], 40.0);
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].members, vec![7]);
        // 空
        assert!(cluster_points(&[], 40.0).is_empty());
    }
}
