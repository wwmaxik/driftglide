use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Описание окна из ответа driftwm
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriftWindow {
    pub id: u64,
    pub app_id: String,
    pub title: String,
    #[serde(default)]
    pub is_focused: bool,
    #[serde(default)]
    pub suspended: bool,
    #[serde(default)]
    pub is_widget: bool,
    #[serde(default)]
    pub position: Option<[f64; 2]>,
    #[serde(default)]
    pub size: Option<[f64; 2]>,
    #[serde(default)]
    pub mode: Option<String>,
}

/// Снимок состояния driftwm
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct DriftState {
    #[serde(default)]
    pub windows: Vec<DriftWindow>,
    #[serde(default)]
    pub layout: Option<String>,
    #[serde(default)]
    pub layout_short: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct DriftReply {
    #[serde(rename = "Ok")]
    ok: Option<DriftStateWrapper>,
    #[serde(rename = "Err")]
    err: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DriftStateWrapper {
    #[serde(rename = "State")]
    state: Option<DriftState>,
}

/// Информация об открытом окне приложения для UI
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    #[serde(default)]
    pub is_active: bool,
}

/// Команды для driftwm
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcCommand {
    FocusNext,
    FocusPrev,
    FocusWindow(u64),
    GetWindows,
    LaunchApp(String),
    CenterWindow,
    ZoomToFit,
}

/// Клиент Unix Domain Socket IPC для driftwm
pub struct IpcClient {
    socket_path: PathBuf,
    state_file_path: PathBuf,
    stream: Option<UnixStream>,
}

impl IpcClient {
    pub fn new(socket_path: impl AsRef<Path>) -> Self {
        let path = socket_path.as_ref().to_path_buf();

        // Путь к файлу состояния $XDG_RUNTIME_DIR/driftwm/state
        let state_file = std::env::var_os("XDG_RUNTIME_DIR")
            .map(|dir| PathBuf::from(dir).join("driftwm").join("state"))
            .unwrap_or_else(|| PathBuf::from("/tmp/driftwm-state"));

        let mut client = Self {
            socket_path: path,
            state_file_path: state_file,
            stream: None,
        };
        client.try_connect();
        client
    }

    fn try_connect(&mut self) -> bool {
        if self.stream.is_some() {
            return true;
        }

        match UnixStream::connect(&self.socket_path) {
            Ok(stream) => {
                let _ = stream.set_write_timeout(Some(Duration::from_millis(150)));
                let _ = stream.set_read_timeout(Some(Duration::from_millis(60)));
                info!("Успешное подключение к IPC сокету driftwm: {:?}", self.socket_path);
                self.stream = Some(stream);
                true
            }
            Err(e) => {
                debug!(
                    "Не удалось подключиться к сокету {:?}: {}. Повторная попытка будет при обращении.",
                    self.socket_path, e
                );
                self.stream = None;
                false
            }
        }
    }

    /// Отправляет сырой запрос в сокет driftwm и возвращает первую строку ответа
    fn send_raw_request(&mut self, request_line: &str) -> Result<String, String> {
        if !self.try_connect() {
            return Err(format!("Сокет {:?} недоступен", self.socket_path));
        }

        let stream = self.stream.as_mut().ok_or("Соединение разорвано")?;
        let payload = format!("{}\n", request_line.trim());

        if let Err(e) = stream.write_all(payload.as_bytes()) {
            self.stream = None;
            return Err(format!("Ошибка записи: {e}"));
        }
        let _ = stream.flush();

        // Читаем одну строку ответа
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut response = String::new();
        match reader.read_line(&mut response) {
            Ok(n) if n > 0 => Ok(response),
            Ok(_) => {
                self.stream = None;
                Err("Пустой ответ от сокета".into())
            }
            Err(e) => {
                self.stream = None;
                Err(format!("Ошибка чтения: {e}"))
            }
        }
    }

    /// Запрос полного состояния driftwm через "State"
    pub fn query_state(&mut self) -> Option<DriftState> {
        // 1. Попытка через сокет
        if let Ok(raw) = self.send_raw_request("\"State\"") {
            if let Ok(reply) = serde_json::from_str::<DriftReply>(&raw) {
                if let Some(wrapper) = reply.ok {
                    if let Some(state) = wrapper.state {
                        return Some(state);
                    }
                }
            }
        }

        // 2. Фоллбэк: чтение из $XDG_RUNTIME_DIR/driftwm/state
        if self.state_file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.state_file_path) {
                for line in content.lines() {
                    if let Some(json_part) = line.strip_prefix("windows=") {
                        if let Ok(windows) = serde_json::from_str::<Vec<DriftWindow>>(json_part) {
                            return Some(DriftState {
                                windows,
                                layout: None,
                                layout_short: None,
                            });
                        }
                    }
                }
            }
        }

        None
    }

    /// Запрашивает у driftwm актуальный список открытых окон
    pub fn query_open_windows(&mut self) -> Vec<WindowInfo> {
        if let Some(state) = self.query_state() {
            let windows: Vec<WindowInfo> = state
                .windows
                .into_iter()
                .filter(|w| !w.is_widget && !w.suspended)
                .map(|w| WindowInfo {
                    id: w.id,
                    title: if w.title.is_empty() { w.app_id.clone() } else { w.title },
                    app_id: w.app_id,
                    is_active: w.is_focused,
                })
                .collect();

            if !windows.is_empty() {
                return windows;
            }
        }

        // Фоллбэк: демонстрационный список приложений, если driftwm еще не запущен
        vec![
            WindowInfo {
                id: 1,
                title: "driftglide – main.rs".into(),
                app_id: "code".into(),
                is_active: true,
            },
            WindowInfo {
                id: 2,
                title: "Terminal".into(),
                app_id: "foot".into(),
                is_active: false,
            },
            WindowInfo {
                id: 3,
                title: "Firefox".into(),
                app_id: "firefox".into(),
                is_active: false,
            },
            WindowInfo {
                id: 4,
                title: "Файлы".into(),
                app_id: "thunar".into(),
                is_active: false,
            },
        ]
    }

    /// Фокусировка окна по id: {"Focus": <id>}
    pub fn focus_window(&mut self, id: u64) -> Result<(), String> {
        let req = format!("{{\"Focus\":{}}}", id);
        let resp = self.send_raw_request(&req)?;
        debug!("Ответ на Focus {}: {}", id, resp.trim());
        Ok(())
    }

    /// Переключение фокуса на следующее окно
    pub fn focus_next(&mut self) -> Result<(), String> {
        let windows = self.query_open_windows();
        if windows.is_empty() {
            return Ok(());
        }

        let active_idx = windows.iter().position(|w| w.is_active).unwrap_or(0);
        let next_idx = (active_idx + 1) % windows.len();
        let target_id = windows[next_idx].id;

        info!("Свайп вправо: переключение на следующее окно (id: {}, title: {})", target_id, windows[next_idx].title);
        self.focus_window(target_id)
    }

    /// Переключение фокуса на предыдущее окно
    pub fn focus_prev(&mut self) -> Result<(), String> {
        let windows = self.query_open_windows();
        if windows.is_empty() {
            return Ok(());
        }

        let active_idx = windows.iter().position(|w| w.is_active).unwrap_or(0);
        let prev_idx = (active_idx + windows.len() - 1) % windows.len();
        let target_id = windows[prev_idx].id;

        info!("Свайп влево: переключение на предыдущее окно (id: {}, title: {})", target_id, windows[prev_idx].title);
        self.focus_window(target_id)
    }

    /// Приближение / центрирование активного окна (аналог Mod+C в driftwm: action center-window)
    pub fn center_window(&mut self) -> Result<(), String> {
        info!("Вызов center-window для активного окна (Mod+C)");
        let req = "{\"Action\":\"center-window\"}";
        if let Ok(resp) = self.send_raw_request(req) {
            debug!("Ответ на Action center-window: {}", resp.trim());
            return Ok(());
        }

        // Фоллбэк: прямой вызов driftwm msg action center-window через CLI
        let _ = std::process::Command::new("driftwm")
            .args(["msg", "action", "center-window"])
            .spawn();
        Ok(())
    }

    /// Масштабирование холста для отображения всех окон (аналог Mod+W в driftwm: action zoom-to-fit)
    pub fn zoom_to_fit(&mut self) -> Result<(), String> {
        info!("Вызов zoom-to-fit для обзора всех окон (Mod+W)");
        let req = "{\"Action\":\"zoom-to-fit\"}";
        if let Ok(resp) = self.send_raw_request(req) {
            debug!("Ответ на Action zoom-to-fit: {}", resp.trim());
            return Ok(());
        }

        // Фоллбэк: прямой вызов driftwm msg action zoom-to-fit через CLI
        let _ = std::process::Command::new("driftwm")
            .args(["msg", "action", "zoom-to-fit"])
            .spawn();
        Ok(())
    }

    /// Отправка общей команды
    pub fn send_command(&mut self, cmd: &IpcCommand) -> Result<(), String> {
        match cmd {
            IpcCommand::FocusNext => self.focus_next(),
            IpcCommand::FocusPrev => self.focus_prev(),
            IpcCommand::FocusWindow(id) => self.focus_window(*id),
            IpcCommand::GetWindows => {
                let _ = self.query_open_windows();
                Ok(())
            }
            IpcCommand::LaunchApp(cmd_str) => {
                // Запуск приложения через Command::new или через action spawn
                info!("Запуск приложения: {}", cmd_str);
                let _ = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd_str)
                    .spawn();
                Ok(())
            }
            IpcCommand::CenterWindow => self.center_window(),
            IpcCommand::ZoomToFit => self.zoom_to_fit(),
        }
    }
}
