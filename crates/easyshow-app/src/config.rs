//! 最小化配置持久化:联网开关 / 高德 Key / 地图底图来源。
//!
//! 存放于平台配置目录(而非缓存目录):
//! - Windows: `%APPDATA%\EasyShow\config.json`
//! - macOS:   `~/Library/Application Support/easyshow/config.json`
//! - 其他:    `$XDG_CONFIG_HOME/easyshow/config.json` 或 `~/.config/easyshow/config.json`
//!
//! 其余界面设置(主题、语言、删除开关等)仍只存在于内存中,不在此文件。

use std::path::PathBuf;

/// 地图底图来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TileSource {
    /// 自动:中文界面用高德,英文界面用 OSM。
    #[default]
    Auto,
    Amap,
    Osm,
}

impl TileSource {
    pub fn key(self) -> &'static str {
        match self {
            TileSource::Auto => "auto",
            TileSource::Amap => "amap",
            TileSource::Osm => "osm",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "amap" => TileSource::Amap,
            "osm" => TileSource::Osm,
            _ => TileSource::Auto,
        }
    }
}

/// 联网相关配置。`online` 默认 **false**(隐私优先,需在设置中显式开启)。
#[derive(Debug, Clone)]
pub struct NetConfig {
    pub online: bool,
    /// 高德开放平台 Web 服务 Key;环境变量 `EASYSHOW_AMAP_KEY` 优先。
    pub amap_key: String,
    pub tile_source: TileSource,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self { online: false, amap_key: String::new(), tile_source: TileSource::Auto }
    }
}

impl NetConfig {
    /// 实际生效的高德 Key(环境变量优先)。
    pub fn effective_key(&self) -> String {
        if let Ok(key) = std::env::var("EASYSHOW_AMAP_KEY") {
            let key = key.trim().to_string();
            if !key.is_empty() {
                return key;
            }
        }
        self.amap_key.trim().to_string()
    }

    /// 实际使用的瓦片源(Auto 按界面语言解析)。
    pub fn resolved_tile_source(&self, chinese_ui: bool) -> TileSource {
        match self.tile_source {
            TileSource::Auto => {
                if chinese_ui {
                    TileSource::Amap
                } else {
                    TileSource::Osm
                }
            }
            other => other,
        }
    }
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("EasyShow"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/easyshow"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join("easyshow"))
    }
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}

pub fn load() -> NetConfig {
    let Some(path) = config_path() else { return NetConfig::default() };
    let Ok(text) = std::fs::read_to_string(&path) else { return NetConfig::default() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        log::warn!("config: {} 解析失败,使用默认值", path.display());
        return NetConfig::default();
    };
    NetConfig {
        online: v.get("online").and_then(|x| x.as_bool()).unwrap_or(false),
        amap_key: v.get("amap_key").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        tile_source: v
            .get("tile_source")
            .and_then(|x| x.as_str())
            .map(TileSource::from_key)
            .unwrap_or(TileSource::Auto),
    }
}

pub fn save(cfg: &NetConfig) {
    let Some(path) = config_path() else { return };
    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            log::warn!("config: 无法创建目录 {}: {e}", dir.display());
            return;
        }
    }
    let body = serde_json::json!({
        "online": cfg.online,
        "amap_key": cfg.amap_key,
        "tile_source": cfg.tile_source.key(),
    });
    let tmp = path.with_extension("json.tmp");
    let result = (|| -> std::io::Result<()> {
        std::fs::write(&tmp, serde_json::to_vec_pretty(&body).unwrap_or_default())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    })();
    if let Err(e) = result {
        log::warn!("config: 写入 {} 失败: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_source_round_trips() {
        for s in [TileSource::Auto, TileSource::Amap, TileSource::Osm] {
            assert_eq!(TileSource::from_key(s.key()), s);
        }
        assert_eq!(TileSource::from_key("nonsense"), TileSource::Auto);
    }

    #[test]
    fn auto_source_follows_language() {
        let cfg = NetConfig { tile_source: TileSource::Auto, ..Default::default() };
        assert_eq!(cfg.resolved_tile_source(true), TileSource::Amap);
        assert_eq!(cfg.resolved_tile_source(false), TileSource::Osm);
        let cfg = NetConfig { tile_source: TileSource::Osm, ..Default::default() };
        assert_eq!(cfg.resolved_tile_source(true), TileSource::Osm);
    }

    #[test]
    fn default_is_offline() {
        assert!(!NetConfig::default().online);
    }
}
