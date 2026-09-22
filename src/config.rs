use std::path::PathBuf;

/// Конфигурация внешнего вида и поведения DriftGlide
#[derive(Debug, Clone)]
pub struct Config {
    /// Высота нижней панели Layer Shell (px)
    pub bar_height: u32,
    /// Ширина пилюли в состоянии покоя (px)
    pub pill_width: f32,
    /// Высота пилюли в состоянии покоя (px)
    pub pill_height: f32,
    /// Радиус скругления углов пилюли (px)
    pub pill_radius: f32,
    /// Цвет пилюли в idle состоянии (RGBA)
    pub color_idle: [u8; 4],
    /// Цвет пилюли при нажатии (RGBA)
    pub color_pressed: [u8; 4],
    /// Цвет пилюли при перемещении/свайпе (RGBA)
    pub color_dragging: [u8; 4],
    /// Цвет пилюли при срабатывании долгого нажатия (RGBA)
    pub color_trigger: [u8; 4],

    /// Порог горизонтального свайпа (px)
    pub swipe_x_threshold: f32,
    /// Порог вертикального свайпа вверх для открытия лончера (px)
    pub swipe_y_threshold: f32,
    /// Минимальная скорость флика (px/ms) для быстрого переключения
    pub flick_velocity_threshold: f32,
    /// Длительность удержания для активации Circle to Search (мс)
    pub long_press_duration_ms: u64,
    /// Зона нечувствительности (deadzone) перед началом жеста (px)
    pub deadzone_threshold: f32,

    /// Время неактивности меню открытых окон до автоматического скрытия (мс)
    pub switcher_idle_timeout_ms: u64,
    /// Время неактивности навигационной пилюли до автоматического скрытия (мс, 0 = не скрывать)
    pub pill_idle_timeout_ms: u64,
    /// Максимальный интервал между нажатиями для двойного клика (мс)
    pub double_click_timeout_ms: u64,

    /// Путь к Unix-сокету driftwm
    pub ipc_socket_path: PathBuf,

    /// Путь к пользовательскому скрипту/хуку Circle to Search (опционально)
    pub search_hook_script: Option<PathBuf>,

    /// API ключ Gemini (или из GEMINI_API_KEY / ~/.config/driftglide/gemini_key)
    pub gemini_api_key: Option<String>,
    /// Модель Gemini (по умолчанию gemini-2.0-flash)
    pub gemini_model: String,

    /// Тема оформления (настраиваемая)
    pub theme: ThemeConfig,
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub window_bg: [u8; 4],
    pub header_bg: [u8; 4],
    pub border_color: [u8; 4],
    pub accent_color: [u8; 4],
    pub text_primary: [u8; 4],
    pub text_secondary: [u8; 4],
    pub input_bg: [u8; 4],
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            window_bg: [26, 28, 33, 255],       // Глубокий темно-графитовый
            header_bg: [33, 35, 42, 255],       // Шапка окна
            border_color: [52, 56, 68, 255],    // Четкая 1px рамка
            accent_color: [64, 134, 244, 255],  // Сапфировый синий
            text_primary: [242, 244, 248, 255], // Высококонтрастный текст
            text_secondary: [150, 156, 172, 255],// Вторичный серый текст
            input_bg: [18, 19, 23, 255],        // Поле ввода
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let socket_path = std::env::var_os("DRIFTWM_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let xdg = std::env::var_os("XDG_RUNTIME_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/tmp"));
                let wayland_display = std::env::var("WAYLAND_DISPLAY")
                    .unwrap_or_else(|_| "wayland-0".into());
                xdg.join("driftwm").join(format!("ipc-{}.sock", wayland_display))
            });

        let gemini_key = std::env::var("GEMINI_API_KEY").ok().or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".config/driftglide/gemini_key"))
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });

        Self {
            bar_height: 52,
            pill_width: 140.0,
            pill_height: 5.0,
            pill_radius: 2.5,
            color_idle: [255, 255, 255, 175],      // матовый полупрозрачный белый
            color_pressed: [255, 255, 255, 250],   // чистый белый с высокой яркостью
            color_dragging: [195, 220, 255, 250],  // премиальный лазурный оттенок
            color_trigger: [130, 245, 170, 255],   // нежно-изумрудный акцент
            swipe_x_threshold: 55.0,
            swipe_y_threshold: 35.0,
            flick_velocity_threshold: 0.55,
            long_press_duration_ms: 420,
            deadzone_threshold: 7.0,
            switcher_idle_timeout_ms: 3500, // 3.5 секунды авто-скрытия при бездействии
            pill_idle_timeout_ms: 2500,     // 2.5 секунды до скрытия пилюли в простое
            double_click_timeout_ms: 280,   // 280 мс окно для двойного клика (Mod+W)
            ipc_socket_path: socket_path,
            search_hook_script: std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".config/driftglide/search-hook.sh")),
            gemini_api_key: gemini_key,
            gemini_model: "gemini-3.1-flash-lite".into(),
            theme: ThemeConfig::default(),
        }
    }
}
