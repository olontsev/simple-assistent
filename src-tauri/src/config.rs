use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const CONFIG_VERSION: u32 = 1;

pub const DEFAULT_PROFILE_ARGS: &str = r#"-ngl 99 -c 131072 -ctk q4_0 -ctv q4_0 -ctkd q4_0 -ctvd q4_0 --merge-qkv -muge -t 1 -tb 1 -tm 16 --parallel 1 --ctx-checkpoints 32 -cram 65536 --spec-type mtp:n_max=4,p_min=0.0 -fa on -b 2048 -ub 512 --jinja --reasoning auto --chat-template-kwargs "{\"reasoning_effort\":\"low\"}" --host 0.0.0.0 --port 8080"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub args: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub llama_cpp_path: String,
    pub models_dir: String,
    pub autostart: bool,
    pub active_profile_id: String,
    pub active_model_path: Option<String>,
    pub profiles: Vec<Profile>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_id = "default".to_string();
        Self {
            version: CONFIG_VERSION,
            llama_cpp_path: String::new(),
            models_dir: String::new(),
            autostart: false,
            active_profile_id: default_id.clone(),
            active_model_path: None,
            profiles: vec![Profile {
                id: default_id,
                name: "Qwen 27B".to_string(),
                args: DEFAULT_PROFILE_ARGS.to_string(),
            }],
        }
    }
}

impl AppConfig {
    pub fn migrate(mut self) -> Self {
        if self.version < CONFIG_VERSION {
            self.version = CONFIG_VERSION;
        }
        if self.profiles.is_empty() {
            let default = AppConfig::default();
            self.profiles = default.profiles;
            self.active_profile_id = default.active_profile_id;
        }
        if !self
            .profiles
            .iter()
            .any(|p| p.id == self.active_profile_id)
        {
            self.active_profile_id = self.profiles[0].id.clone();
        }
        self
    }

    pub fn active_profile(&self) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.id == self.active_profile_id)
            .or_else(|| self.profiles.first())
    }

    pub fn resolve_llama_binary(&self) -> Result<PathBuf, String> {
        let raw = self.llama_cpp_path.trim();
        if raw.is_empty() {
            return Err("Путь к llama.cpp не задан".into());
        }
        let path = PathBuf::from(raw);
        if path.is_file() {
            return Ok(path);
        }
        if path.is_dir() {
            let candidate = path.join("llama-server.exe");
            if candidate.is_file() {
                return Ok(candidate);
            }
            let candidate = path.join("llama-server");
            if candidate.is_file() {
                return Ok(candidate);
            }
            return Err(format!(
                "В папке {} не найден llama-server.exe",
                path.display()
            ));
        }
        Err(format!("Путь к llama.cpp не существует: {raw}"))
    }
}

pub fn config_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.json")
}

pub fn load_config(app_data_dir: &Path) -> Result<AppConfig, String> {
    let path = config_path(app_data_dir);
    if !path.exists() {
        let cfg = AppConfig::default();
        save_config(app_data_dir, &cfg)?;
        return Ok(cfg);
    }
    let data = fs::read_to_string(&path).map_err(|e| format!("Не удалось прочитать конфиг: {e}"))?;
    let cfg: AppConfig =
        serde_json::from_str(&data).map_err(|e| format!("Некорректный конфиг: {e}"))?;
    Ok(cfg.migrate())
}

pub fn save_config(app_data_dir: &Path, config: &AppConfig) -> Result<(), String> {
    fs::create_dir_all(app_data_dir)
        .map_err(|e| format!("Не удалось создать каталог данных: {e}"))?;
    let path = config_path(app_data_dir);
    let data = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Не удалось сериализовать конфиг: {e}"))?;
    fs::write(&path, data).map_err(|e| format!("Не удалось сохранить конфиг: {e}"))?;
    Ok(())
}

pub fn new_profile_id() -> String {
    Uuid::new_v4().to_string()
}
