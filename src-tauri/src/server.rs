use crate::args::{extract_host_port, model_alias, split_args, validate_profile_args};
use crate::config::AppConfig;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerPhase {
    Stopped,
    Starting,
    RunningEmpty,
    RunningLoaded,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub phase: ServerPhase,
    pub pid: Option<u32>,
    pub url: Option<String>,
    pub loaded_model: Option<String>,
    pub last_error: Option<String>,
    pub wants_model: bool,
}

impl Default for ServerStatus {
    fn default() -> Self {
        Self {
            phase: ServerPhase::Stopped,
            pid: None,
            url: None,
            loaded_model: None,
            last_error: None,
            wants_model: false,
        }
    }
}

pub struct ServerManager {
    child: Option<Child>,
    status: ServerStatus,
    started_at: Option<Instant>,
    /// Model path that was requested for the current/last spawn
    spawn_model: Option<String>,
}

impl Default for ServerManager {
    fn default() -> Self {
        Self {
            child: None,
            status: ServerStatus::default(),
            started_at: None,
            spawn_model: None,
        }
    }
}

impl ServerManager {
    pub fn status(&self) -> ServerStatus {
        self.status.clone()
    }

    pub fn set_last_error(&mut self, msg: String) {
        self.status.last_error = Some(msg);
    }

    pub fn log_path(app_data_dir: &Path) -> PathBuf {
        app_data_dir.join("llama-server.log")
    }

    fn open_log(app_data_dir: &Path) -> Result<(Stdio, Stdio), String> {
        let path = Self::log_path(app_data_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create log directory: {e}"))?;
        }
        let mut header = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("Failed to open log: {e}"))?;
        let _ = writeln!(
            header,
            "\n===== {} =====",
            chrono_like_now()
        );
        drop(header);

        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("Failed to open log (stdout): {e}"))?;
        let stderr = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("Failed to open log (stderr): {e}"))?;
        Ok((Stdio::from(stdout), Stdio::from(stderr)))
    }

    pub fn start(
        &mut self,
        config: &AppConfig,
        app_data_dir: &Path,
        with_model: bool,
    ) -> Result<(), String> {
        if matches!(
            self.status.phase,
            ServerPhase::Starting | ServerPhase::RunningEmpty | ServerPhase::RunningLoaded
        ) {
            return Err("Server is already running or starting".into());
        }

        let binary = config.resolve_llama_binary()?;
        let profile = config
            .active_profile()
            .ok_or_else(|| "No active profile".to_string())?;
        validate_profile_args(&profile.args)?;
        let profile_args = split_args(&profile.args)?;

        let model_path = if with_model {
            let path = config
                .active_model_path
                .as_ref()
                .ok_or_else(|| "No model selected".to_string())?;
            if !Path::new(path).is_file() {
                return Err(format!("Model file not found: {path}"));
            }
            Some(path.clone())
        } else {
            None
        };

        let (host, port) = extract_host_port(Some(profile));
        let url = format!("http://{host}:{port}");

        let (stdout, stderr) = Self::open_log(app_data_dir)?;

        let mut cmd = Command::new(&binary);
        cmd.stdout(stdout).stderr(stderr).stdin(Stdio::null());

        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
            // Profile tokens keep original quotes (needed for JSON kwargs).
            for arg in &profile_args {
                cmd.raw_arg(arg);
            }
        }

        #[cfg(not(windows))]
        {
            for arg in &profile_args {
                let bare = if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
                    &arg[1..arg.len() - 1]
                } else {
                    arg.as_str()
                };
                cmd.arg(bare);
            }
        }

        // Model path / alias via normal escaping (paths may contain spaces).
        if let Some(ref m) = model_path {
            cmd.arg("-m").arg(m).arg("--alias").arg(model_alias(m));
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start llama-server: {e}"))?;

        let pid = child.id();
        self.child = Some(child);
        self.spawn_model = model_path.clone();
        self.started_at = Some(Instant::now());
        self.status = ServerStatus {
            phase: ServerPhase::Starting,
            pid: Some(pid),
            url: Some(url),
            loaded_model: None,
            last_error: None,
            wants_model: with_model,
        };
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.started_at = None;
        self.spawn_model = None;
        self.status = ServerStatus {
            phase: ServerPhase::Stopped,
            pid: None,
            url: None,
            loaded_model: None,
            last_error: None,
            wants_model: false,
        };
        Ok(())
    }

    /// Restart with or without model (used for Load / Unload).
    pub fn restart(
        &mut self,
        config: &AppConfig,
        app_data_dir: &Path,
        with_model: bool,
    ) -> Result<(), String> {
        let _ = self.stop();
        self.start(config, app_data_dir, with_model)
    }

    pub fn poll(&mut self) {
        // Check if process exited
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code().unwrap_or(-1);
                    self.child = None;
                    self.started_at = None;
                    self.status.phase = ServerPhase::Error;
                    self.status.pid = None;
                    self.status.loaded_model = None;
                    self.status.last_error =
                        Some(format!("llama-server process exited (code {code})"));
                    return;
                }
                Ok(None) => {}
                Err(e) => {
                    self.status.last_error = Some(format!("Process check error: {e}"));
                }
            }
        } else if matches!(
            self.status.phase,
            ServerPhase::Starting | ServerPhase::RunningEmpty | ServerPhase::RunningLoaded
        ) {
            self.status.phase = ServerPhase::Error;
            self.status.last_error = Some("Process lost".into());
            return;
        }

        if !matches!(
            self.status.phase,
            ServerPhase::Starting | ServerPhase::RunningEmpty | ServerPhase::RunningLoaded
        ) {
            return;
        }

        let Some(url) = self.status.url.clone() else {
            return;
        };
        let health_url = format!("{url}/health");

        match check_health(&health_url) {
            Ok(true) => {
                if self.status.wants_model {
                    self.status.phase = ServerPhase::RunningLoaded;
                    self.status.loaded_model = self.spawn_model.clone();
                } else {
                    self.status.phase = ServerPhase::RunningEmpty;
                    self.status.loaded_model = None;
                }
                self.status.last_error = None;
            }
            Ok(false) => {
                // still starting or temporarily unhealthy
                if self.status.phase == ServerPhase::Starting {
                    if let Some(started) = self.started_at {
                        if started.elapsed() > Duration::from_secs(120) {
                            self.status.phase = ServerPhase::Error;
                            self.status.last_error =
                                Some("/health wait timeout (120s)".into());
                            let _ = self.stop_keep_error();
                        }
                    }
                }
            }
            Err(e) => {
                if self.status.phase == ServerPhase::Starting {
                    if let Some(started) = self.started_at {
                        if started.elapsed() > Duration::from_secs(120) {
                            self.status.phase = ServerPhase::Error;
                            self.status.last_error = Some(format!("Health check: {e}"));
                            let _ = self.stop_keep_error();
                        }
                    }
                } else if matches!(
                    self.status.phase,
                    ServerPhase::RunningEmpty | ServerPhase::RunningLoaded
                ) {
                    // transient — keep phase, note error lightly
                    self.status.last_error = Some(format!("Health: {e}"));
                }
            }
        }
    }

    fn stop_keep_error(&mut self) -> Result<(), String> {
        let err = self.status.last_error.clone();
        let phase = self.status.phase;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.started_at = None;
        self.spawn_model = None;
        self.status = ServerStatus {
            phase: if phase == ServerPhase::Error {
                ServerPhase::Error
            } else {
                ServerPhase::Stopped
            },
            pid: None,
            url: None,
            loaded_model: None,
            last_error: err,
            wants_model: false,
        };
        if self.status.phase != ServerPhase::Error && self.status.last_error.is_some() {
            self.status.phase = ServerPhase::Error;
        }
        Ok(())
    }
}

fn check_health(url: &str) -> Result<bool, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    Ok(resp.status().is_success())
}

fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

pub type SharedServer = Mutex<ServerManager>;
