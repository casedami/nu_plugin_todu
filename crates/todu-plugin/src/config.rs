use nu_plugin::EngineInterface;
use std::path::PathBuf;

pub struct Config {
    pub db_path: PathBuf,
    pub default_global: bool,
}

impl Config {
    fn default_db_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".local/share/nu_plugin_todu/todu.db")
    }

    pub fn from_engine(engine: &EngineInterface) -> Self {
        let cfg = engine.get_plugin_config().ok().flatten();
        let get_cfg_val = |key: &str| {
            cfg.as_ref()
                .and_then(|config_val| config_val.as_record().ok())
                .and_then(|record| record.get(key))
                .cloned()
        };

        let db_path = get_cfg_val("db_path")
            .and_then(|val| val.as_str().ok().map(|path_str| path_str.to_string()))
            .map(|path_str| {
                if path_str.starts_with("~/") {
                    let home = std::env::var("HOME").unwrap_or_default();
                    PathBuf::from(format!("{home}{}", &path_str[1..]))
                } else {
                    PathBuf::from(path_str)
                }
            })
            .unwrap_or_else(Self::default_db_path);

        let default_global = get_cfg_val("default_global")
            .and_then(|val| val.as_bool().ok())
            .unwrap_or(false);

        Config {
            db_path,
            default_global,
        }
    }
}
