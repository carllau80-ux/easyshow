//! 逆地理编码:把 GPS 坐标解析为「国家-省-市-区县-乡镇」。
//!
//! - 首选高德 Web 服务(需要免费 Key;国内快,直接给到乡镇级);
//! - 无 Key 或高德失败时回退 OSM Nominatim(免 Key,遵守 1 req/s 限速);
//! - 结果按约 11m 网格缓存到 `<cache>/geocode/v1/<provider>/<lat4>_<lon4>.json`,
//!   同一位置重复查看不再发起网络请求;
//! - 全部网络请求在独立工作线程,UI 线程只做 `try_recv`。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::config::NetConfig;
use crate::thumbs::cache_root;

/// 地址层级(缺失的层级为 None)。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Place {
    pub country: Option<String>,
    pub province: Option<String>,
    pub city: Option<String>,
    pub district: Option<String>,
    pub township: Option<String>,
}

impl Place {
    fn parts(&self) -> Vec<&str> {
        let mut parts: Vec<&str> = Vec::new();
        for v in [&self.country, &self.province, &self.city, &self.district, &self.township] {
            if let Some(s) = v {
                let t = s.trim();
                // 跳过空值与连续重复(直辖市的 province/city 往往相同)
                if !t.is_empty() && parts.last().copied() != Some(t) {
                    parts.push(t);
                }
            }
        }
        parts
    }

    /// 展示用文本,如「中国 上海市 黄浦区 南京东路街道」。
    pub fn label(&self) -> Option<String> {
        let parts = self.parts();
        (!parts.is_empty()).then(|| parts.join(" "))
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "country": self.country,
            "province": self.province,
            "city": self.city,
            "district": self.district,
            "township": self.township,
        })
    }

    fn from_json(v: &serde_json::Value) -> Option<Self> {
        let field = |k: &str| {
            v.get(k)
                .and_then(|x| x.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        let place = Place {
            country: field("country"),
            province: field("province"),
            city: field("city"),
            district: field("district"),
            township: field("township"),
        };
        place.label().is_some().then_some(place)
    }
}

fn opt_str(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => {
            let s = s.trim();
            (!s.is_empty()).then(|| s.to_string())
        }
        _ => None,
    }
}

/// 解析高德 `v3/geocode/regeo` 响应。`city` 在直辖市会返回空数组 `[]`。
pub fn parse_amap(text: &str) -> Option<Place> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("status").and_then(|s| s.as_str()) != Some("1") {
        return None;
    }
    let a = v.get("regeocode")?.get("addressComponent")?;
    let place = Place {
        country: opt_str(&a["country"]),
        province: opt_str(&a["province"]),
        city: opt_str(&a["city"]),
        district: opt_str(&a["district"]),
        township: opt_str(&a["township"]),
    };
    place.label().map(|_| place)
}

/// Nominatim 的中国地址常缺省/市级,但带有 ISO 3166-2:CN 代码;
/// 用代码兜底出省份名称(如 CN-BJ → 北京市)。
fn cn_province_from_iso(code: &str) -> Option<&'static str> {
    let code = code.strip_prefix("CN-")?;
    Some(match code {
        "BJ" => "北京市",
        "TJ" => "天津市",
        "HE" => "河北省",
        "SX" => "山西省",
        "NM" => "内蒙古自治区",
        "LN" => "辽宁省",
        "JL" => "吉林省",
        "HL" => "黑龙江省",
        "SH" => "上海市",
        "JS" => "江苏省",
        "ZJ" => "浙江省",
        "AH" => "安徽省",
        "FJ" => "福建省",
        "JX" => "江西省",
        "SD" => "山东省",
        "HA" => "河南省",
        "HB" => "湖北省",
        "HN" => "湖南省",
        "GD" => "广东省",
        "GX" => "广西壮族自治区",
        "HI" => "海南省",
        "CQ" => "重庆市",
        "SC" => "四川省",
        "GZ" => "贵州省",
        "YN" => "云南省",
        "XZ" => "西藏自治区",
        "SN" => "陕西省",
        "GS" => "甘肃省",
        "QH" => "青海省",
        "NX" => "宁夏回族自治区",
        "XJ" => "新疆维吾尔自治区",
        "TW" => "台湾省",
        "HK" => "香港特别行政区",
        "MO" => "澳门特别行政区",
        _ => return None,
    })
}

/// 解析 OSM Nominatim `reverse` 响应(jsonv2 + addressdetails)。
pub fn parse_nominatim(text: &str) -> Option<Place> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let a = v.get("address")?;
    let place = Place {
        country: opt_str(&a["country"]),
        province: opt_str(&a["state"])
            .or_else(|| opt_str(&a["region"]))
            .or_else(|| {
                a.get("ISO3166-2-lvl4")
                    .and_then(|x| x.as_str())
                    .and_then(cn_province_from_iso)
                    .map(str::to_string)
            }),
        city: opt_str(&a["city"]).or_else(|| opt_str(&a["town"])),
        district: opt_str(&a["county"])
            .or_else(|| opt_str(&a["city_district"]))
            .or_else(|| opt_str(&a["district"])),
        township: opt_str(&a["suburb"])
            .or_else(|| opt_str(&a["quarter"]))
            .or_else(|| opt_str(&a["neighbourhood"]))
            .or_else(|| opt_str(&a["village"]))
            .or_else(|| opt_str(&a["hamlet"])),
    };
    place.label().map(|_| place)
}

// ----------------------------------------------------------------------
// 提供服务
// ----------------------------------------------------------------------

#[derive(Clone)]
struct GeoMode {
    online: bool,
    amap_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    Amap,
    Nominatim,
}

impl Provider {
    fn tag(self) -> &'static str {
        match self {
            Provider::Amap => "amap",
            Provider::Nominatim => "nominatim",
        }
    }
}

pub struct GeoRequest {
    pub generation: u64,
    pub path: PathBuf,
    pub lat: f64,
    pub lon: f64,
}

pub struct GeoReply {
    pub generation: u64,
    pub path: PathBuf,
    pub place: Option<Place>,
}

pub struct GeocodeService {
    tx: Sender<GeoRequest>,
    rx: Receiver<GeoReply>,
    latest: Arc<AtomicU64>,
    mode: Arc<RwLock<GeoMode>>,
}

impl GeocodeService {
    pub fn new() -> Self {
        let (tx, req_rx) = unbounded::<GeoRequest>();
        let (reply_tx, rx) = unbounded::<GeoReply>();
        let latest = Arc::new(AtomicU64::new(0));
        let mode = Arc::new(RwLock::new(GeoMode { online: false, amap_key: String::new() }));

        let latest_worker = latest.clone();
        let mode_worker = mode.clone();
        let spawned = std::thread::Builder::new()
            .name("easyshow-geocode".into())
            .spawn(move || {
                let agent = build_agent();
                let mut last_fetch = Instant::now()
                    .checked_sub(Duration::from_secs(2))
                    .unwrap_or_else(Instant::now);
                for req in req_rx {
                    if req.generation < latest_worker.load(Ordering::Relaxed) {
                        continue; // 已经切到别的照片
                    }
                    let mode = mode_worker.read().unwrap().clone();
                    if !mode.online {
                        continue;
                    }
                    let Some(place) = lookup(&agent, &mode, req.lat, req.lon, &mut last_fetch) else {
                        continue;
                    };
                    if req.generation < latest_worker.load(Ordering::Relaxed) {
                        continue;
                    }
                    if reply_tx
                        .send(GeoReply { generation: req.generation, path: req.path, place: Some(place) })
                        .is_err()
                    {
                        break;
                    }
                }
            });
        if let Err(e) = spawned {
            log::warn!("geocode: 工作线程创建失败: {e}");
        }

        Self { tx, rx, latest, mode }
    }

    /// 联网开关或 Key 变化时调用。
    pub fn set_mode(&self, online: bool, amap_key: String) {
        if let Ok(mut mode) = self.mode.write() {
            *mode = GeoMode { online, amap_key };
        }
    }

    /// 请求解析某张照片的位置;返回本次请求的代号。
    pub fn request(&self, path: PathBuf, lat: f64, lon: f64) -> u64 {
        let generation = self.latest.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.tx.send(GeoRequest { generation, path, lat, lon });
        generation
    }

    pub fn try_recv(&self) -> Option<GeoReply> {
        self.rx.try_recv().ok()
    }
}

fn user_agent() -> String {
    format!("EasyShow/{} (https://github.com/carllau80-ux/easyshow)", env!("CARGO_PKG_VERSION"))
}

fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .user_agent(user_agent())
        .build()
        .into()
}

/// 先查缓存(两种 provider),再依次请求;成功后写缓存。
fn lookup(
    agent: &ureq::Agent,
    mode: &GeoMode,
    lat: f64,
    lon: f64,
    last_fetch: &mut Instant,
) -> Option<Place> {
    let providers: &[Provider] = if mode.amap_key.is_empty() {
        &[Provider::Nominatim]
    } else {
        &[Provider::Amap, Provider::Nominatim]
    };

    for provider in providers {
        if let Some(place) = load_cache(*provider, lat, lon) {
            return Some(place);
        }
    }

    for provider in providers {
        // 限速:统一 1 req/s(Nominatim 的要求;高德也足够宽松)
        let elapsed = last_fetch.elapsed();
        if elapsed < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_secs(1) - elapsed);
        }
        let result = match provider {
            Provider::Amap => fetch_amap(agent, &mode.amap_key, lat, lon),
            Provider::Nominatim => fetch_nominatim(agent, lat, lon),
        };
        *last_fetch = Instant::now();
        if let Ok(Some(place)) = result {
            save_cache(*provider, lat, lon, &place);
            return Some(place);
        }
    }
    None
}

fn fetch_amap(agent: &ureq::Agent, key: &str, lat: f64, lon: f64) -> Result<Option<Place>, String> {
    // 高德使用 GCJ-02 坐标
    let (glat, glon) = crate::geo::wgs84_to_gcj02((lat, lon));
    let url = format!(
        "https://restapi.amap.com/v3/geocode/regeo?key={key}&location={glon:.6},{glat:.6}&extensions=base&output=JSON"
    );
    let mut response = agent.get(&url).call().map_err(|e| e.to_string())?;
    let text = response.body_mut().read_to_string().map_err(|e| e.to_string())?;
    Ok(parse_amap(&text))
}

fn fetch_nominatim(agent: &ureq::Agent, lat: f64, lon: f64) -> Result<Option<Place>, String> {
    let url = format!(
        "https://nominatim.openstreetmap.org/reverse?format=jsonv2&lat={lat:.6}&lon={lon:.6}&zoom=18&addressdetails=1&accept-language=zh-CN"
    );
    let mut response = agent.get(&url).call().map_err(|e| e.to_string())?;
    let text = response.body_mut().read_to_string().map_err(|e| e.to_string())?;
    Ok(parse_nominatim(&text))
}

fn cache_path(provider: Provider, lat: f64, lon: f64) -> Option<PathBuf> {
    Some(
        cache_root()?
            .join("geocode")
            .join("v1")
            .join(provider.tag())
            .join(format!("{lat:.4}_{lon:.4}.json")),
    )
}

fn load_cache(provider: Provider, lat: f64, lon: f64) -> Option<Place> {
    let path = cache_path(provider, lat, lon)?;
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    Place::from_json(&v)
}

fn save_cache(provider: Provider, lat: f64, lon: f64, place: &Place) {
    let Some(path) = cache_path(provider, lat, lon) else { return };
    if let Some(dir) = path.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
    }
    let Ok(body) = serde_json::to_vec_pretty(&place.to_json()) else { return };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, body).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// 供 UI 侧初始化:把配置同步到服务。
impl GeocodeService {
    pub fn sync_config(&self, cfg: &NetConfig) {
        self.set_mode(cfg.online, cfg.effective_key());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amap_municipality_has_empty_city_array() {
        // 上海市:city 为空数组,province 与 district 齐全
        let json = r#"{
            "status":"1","info":"OK",
            "regeocode":{"addressComponent":{
                "country":"中国","province":"上海市","city":[],"district":"黄浦区",
                "township":"南京东路街道","adcode":"310101"
            }}
        }"#;
        let place = parse_amap(json).unwrap();
        assert_eq!(place.label().as_deref(), Some("中国 上海市 黄浦区 南京东路街道"));
    }

    #[test]
    fn amap_regular_levels() {
        let json = r#"{
            "status":"1",
            "regeocode":{"addressComponent":{
                "country":"中国","province":"浙江省","city":"杭州市","district":"西湖区",
                "township":"北山街道"
            }}
        }"#;
        let place = parse_amap(json).unwrap();
        assert_eq!(place.label().as_deref(), Some("中国 浙江省 杭州市 西湖区 北山街道"));
    }

    #[test]
    fn amap_error_status_is_none() {
        assert!(parse_amap(r#"{"status":"0","info":"INVALID_USER_SCODE"}"#).is_none());
        assert!(parse_amap("not json").is_none());
    }

    #[test]
    fn nominatim_levels_and_dedup() {
        let json = r#"{
            "address":{
                "country":"中国","state":"上海市","city":"上海市","county":"黄浦区",
                "suburb":"南京东路街道"
            }
        }"#;
        let place = parse_nominatim(json).unwrap();
        // province/city 相同 → 去重
        assert_eq!(place.label().as_deref(), Some("中国 上海市 黄浦区 南京东路街道"));
    }

    #[test]
    fn nominatim_overseas() {
        let json = r#"{
            "address":{
                "country":"France","state":"Île-de-France","city":"Paris",
                "suburb":"Quartier de la Sorbonne"
            }
        }"#;
        let place = parse_nominatim(json).unwrap();
        assert_eq!(place.label().as_deref(), Some("France Île-de-France Paris Quartier de la Sorbonne"));
    }

    #[test]
    fn nominatim_china_iso_province_fallback() {
        // 北京天安门一带的真实返回:没有 state,区县填在 city,需用 ISO 代码补出北京市
        let json = r#"{
            "address":{
                "country":"中国","city":"东城区","suburb":"东华门街道",
                "ISO3166-2-lvl4":"CN-BJ"
            }
        }"#;
        let place = parse_nominatim(json).unwrap();
        assert_eq!(place.province.as_deref(), Some("北京市"));
        assert_eq!(place.label().as_deref(), Some("中国 北京市 东城区 东华门街道"));
        assert!(cn_province_from_iso("CN-ZJ") == Some("浙江省"));
        assert!(cn_province_from_iso("FR-IF").is_none());
    }

    #[test]
    fn place_label_skips_empty_and_dedupes_consecutive() {
        let place = Place {
            country: Some("中国".into()),
            province: Some("  ".into()),
            city: Some("北京市".into()),
            district: Some("北京市".into()),
            township: None,
        };
        assert_eq!(place.label().as_deref(), Some("中国 北京市"));
        assert!(Place::default().label().is_none());
    }
}
