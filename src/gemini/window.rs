use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{info, error};
use tiny_skia::*;
use crate::config::Config;
use crate::gemini::client::{call_gemini, ChatTurn};
use crate::render::markdown::{layout_markdown, MarkdownLayout};
use crate::render::text::TextRenderer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelDef {
    pub display_name: &'static str,
    pub api_id: &'static str,
}

pub const GEMINI_MODELS: &[ModelDef] = &[
    ModelDef { display_name: "Gemini 3.1 Flash Lite", api_id: "gemini-3.1-flash-lite" },
    ModelDef { display_name: "Gemini 3.5 Flash Lite", api_id: "gemini-3.5-flash-lite" },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowAnim {
    None,
    Opening,
    Closing,
    Minimizing,
    Unminimizing,
}

pub struct GeminiWindow {
    pub visible: bool,
    pub minimized: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,

    pub is_dragging: bool,
    pub drag_offset_x: f32,
    pub drag_offset_y: f32,

    pub scroll_offset: f32,
    pub max_scroll: f32,
    pub is_mouse_scrolling: bool,
    pub mouse_last_y: f32,
    pub is_touch_scrolling: bool,
    pub touch_last_y: f32,
    pub is_scrollbar_dragging: bool,

    pub markdown_layout: Option<MarkdownLayout>,
    pub layout_text: String,
    pub layout_width: f32,

    pub cursor_pos: usize,
    pub selection_anchor: usize,
    pub is_mouse_selecting: bool,
    pub last_click_time: Option<std::time::Instant>,
    pub last_click_pos: (f32, f32),
    pub click_count: u32,

    pub chat_select_start: Option<(f32, f32)>,
    pub chat_select_end: Option<(f32, f32)>,
    pub is_chat_selecting: bool,
    pub chat_copied_toast_time: Option<std::time::Instant>,
    pub chat_last_click_time: Option<std::time::Instant>,
    pub chat_last_click_pos: (f32, f32),
    pub chat_click_count: u32,

    pub cursor_blink_start: std::time::Instant,
    pub cursor_last_blink_state: bool,

    pub anim: WindowAnim,
    pub anim_progress: f32,
    pub anim_start_time: Option<std::time::Instant>,
    pub anim_duration: f32,

    pub crop_path: Option<PathBuf>,
    pub crop_pixmap: Option<Pixmap>,

    pub chat_history: Vec<ChatTurn>,
    pub current_response: String,
    pub input_text: String,
    pub is_loading: bool,

    pub settings_open: bool,
    pub api_key: String,
    pub model: String,
    pub status_msg: Option<String>,

    pub text_renderer: TextRenderer,
    pub config: Config,

    // Для межпоточного получения ответа Gemini
    pub pending_response: Arc<Mutex<Option<Result<String, String>>>>,
}

impl GeminiWindow {
    pub fn new(config: Config) -> Self {
        let api_key = config.gemini_api_key.clone().unwrap_or_default();
        let mut model = config.gemini_model.clone();

        // Читаем ранее сохраненную модель из файла
        let conf_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".config/driftglide");
        let model_file = conf_dir.join("gemini_model");
        if let Ok(m) = std::fs::read_to_string(&model_file) {
            let m_trim = m.trim().to_string();
            if !m_trim.is_empty() {
                model = m_trim;
            }
        }

        // Если сохраненная модель начинается на 1 или 2 (устаревшие) — сбрасываем на 3.1
        if model.starts_with("gemini-1") || model.starts_with("gemini-2") {
            model = "gemini-3.1-flash-lite".into();
        }

        Self {
            visible: false,
            minimized: false,
            x: 100.0,
            y: 100.0,
            width: 480.0,
            height: 560.0,

            is_dragging: false,
            drag_offset_x: 0.0,
            drag_offset_y: 0.0,

            scroll_offset: 0.0,
            max_scroll: 0.0,
            is_mouse_scrolling: false,
            mouse_last_y: 0.0,
            is_touch_scrolling: false,
            touch_last_y: 0.0,
            is_scrollbar_dragging: false,

            markdown_layout: None,
            layout_text: String::new(),
            layout_width: 0.0,

            cursor_pos: 0,
            selection_anchor: 0,
            is_mouse_selecting: false,
            last_click_time: None,
            last_click_pos: (0.0, 0.0),
            click_count: 0,
            chat_select_start: None,
            chat_select_end: None,
            is_chat_selecting: false,
            chat_copied_toast_time: None,
            chat_last_click_time: None,
            chat_last_click_pos: (0.0, 0.0),
            chat_click_count: 0,

            cursor_blink_start: std::time::Instant::now(),
            cursor_last_blink_state: true,

            anim: WindowAnim::None,
            anim_progress: 1.0,
            anim_start_time: None,
            anim_duration: 0.16,

            crop_path: None,
            crop_pixmap: None,

            chat_history: Vec::new(),
            current_response: String::new(),
            input_text: String::new(),
            is_loading: false,

            settings_open: false,
            api_key,
            model,
            status_msg: None,

            text_renderer: TextRenderer::new(),
            config,
            pending_response: Arc::new(Mutex::new(None)),
        }
    }

    /// Открытие окна с новым кропом
    pub fn open(&mut self, crop_path: PathBuf, screen_w: u32, screen_h: u32) {
        self.visible = true;
        self.minimized = false;
        self.is_dragging = false;
        self.scroll_offset = 0.0;
        self.max_scroll = 0.0;
        self.is_mouse_scrolling = false;
        self.is_touch_scrolling = false;
        self.is_scrollbar_dragging = false;
        self.markdown_layout = None;
        self.layout_text.clear();
        self.cursor_pos = 0;
        self.selection_anchor = 0;
        self.is_mouse_selecting = false;
        self.chat_select_start = None;
        self.chat_select_end = None;
        self.is_chat_selecting = false;
        self.chat_copied_toast_time = None;
        self.last_click_time = None;
        self.last_click_pos = (0.0, 0.0);
        self.click_count = 0;
        self.chat_last_click_time = None;
        self.chat_last_click_pos = (0.0, 0.0);
        self.chat_click_count = 0;
        *self.pending_response.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.cursor_blink_start = std::time::Instant::now();
        self.cursor_last_blink_state = true;
        self.anim = WindowAnim::Opening;
        self.anim_progress = 0.0;
        self.anim_start_time = Some(std::time::Instant::now());

        self.crop_path = Some(crop_path.clone());
        self.chat_history.clear();
        self.current_response.clear();
        self.input_text.clear();
        self.settings_open = false;
        self.status_msg = None;

        // Позиционируем окно по центру экрана
        self.x = ((screen_w as f32) - self.width) / 2.0;
        self.y = ((screen_h as f32) - self.height) / 2.0;
        if self.x < 20.0 { self.x = 20.0; }
        if self.y < 20.0 { self.y = 20.0; }

        // Загружаем миниатюру кропа и масштабируем её под размер контейнера превью (106 x 70)
        let target_tw = 106u32;
        let target_th = 70u32;
        if let Ok(bytes) = std::fs::read(&crop_path) {
            if let Ok(img) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) {
                let rgba = img.to_rgba8();
                let (orig_w, orig_h) = rgba.dimensions();
                if orig_w > 0 && orig_h > 0 {
                    let scale_w = target_tw as f32 / orig_w as f32;
                    let scale_h = target_th as f32 / orig_h as f32;
                    let scale = scale_w.min(scale_h);
                    let rw = ((orig_w as f32 * scale).round() as u32).max(1);
                    let rh = ((orig_h as f32 * scale).round() as u32).max(1);

                    let resized = image::imageops::resize(
                        &rgba,
                        rw,
                        rh,
                        image::imageops::FilterType::Triangle,
                    );

                    if let Some(mut pix) = Pixmap::new(rw, rh) {
                        for (i, chunk) in resized.into_raw().chunks_exact(4).enumerate() {
                            let r = chunk[0];
                            let g = chunk[1];
                            let b = chunk[2];
                            let a = 255u8; // Превью всегда непрозрачно
                            let pm = PremultipliedColorU8::from_rgba(r, g, b, a)
                                .unwrap_or(PremultipliedColorU8::TRANSPARENT);
                            pix.pixels_mut()[i] = pm;
                        }
                        info!("Превью кропа успешно создано: {}x{}", rw, rh);
                        self.crop_pixmap = Some(pix);
                    }
                }
            } else {
                error!("Не удалось декодировать PNG кропа из {:?}", crop_path);
            }
        } else {
            error!("Не удалось прочитать файл кропа {:?}", crop_path);
        }

        // Не отправляем запрос автоматически — ожидаем ввод вопроса от пользователя
        if self.api_key.trim().is_empty() {
            self.current_response = "Нажмите ⚙ в верхнем правом углу, чтобы указать ваш Gemini API Key.".into();
        } else {
            self.current_response = "Задайте вопрос о выделенной области экрана в поле ниже...".into();
        }
        self.update_markdown_layout();
    }

    pub fn is_animating(&self) -> bool {
        self.anim != WindowAnim::None
    }

    pub fn start_close(&mut self) {
        if !self.visible {
            return;
        }
        self.anim = WindowAnim::Closing;
        self.anim_progress = 0.0;
        self.anim_start_time = Some(std::time::Instant::now());
    }

    pub fn start_minimize(&mut self) {
        if !self.visible {
            return;
        }
        if self.minimized {
            self.start_unminimize();
        } else {
            self.anim = WindowAnim::Minimizing;
            self.anim_progress = 0.0;
            self.anim_start_time = Some(std::time::Instant::now());
        }
    }

    pub fn start_unminimize(&mut self) {
        if !self.visible {
            return;
        }
        self.minimized = false;
        self.anim = WindowAnim::Unminimizing;
        self.anim_progress = 0.0;
        self.anim_start_time = Some(std::time::Instant::now());
    }

    pub fn step_animation(&mut self) -> bool {
        if self.anim == WindowAnim::None {
            return false;
        }

        let start = match self.anim_start_time {
            Some(t) => t,
            None => {
                self.anim = WindowAnim::None;
                return false;
            }
        };

        let elapsed = start.elapsed().as_secs_f32();
        let dur = self.anim_duration.max(0.05);
        let linear = (elapsed / dur).min(1.0);

        // Cubic ease-out
        let ease = 1.0 - (1.0 - linear).powi(3);
        self.anim_progress = ease.clamp(0.0, 1.0);

        if linear >= 1.0 {
            match self.anim {
                WindowAnim::Opening => {
                    self.anim = WindowAnim::None;
                    self.anim_progress = 1.0;
                }
                WindowAnim::Closing => {
                    self.anim = WindowAnim::None;
                    self.close();
                }
                WindowAnim::Minimizing => {
                    self.anim = WindowAnim::None;
                    self.minimized = true;
                    self.anim_progress = 1.0;
                }
                WindowAnim::Unminimizing => {
                    self.anim = WindowAnim::None;
                    self.minimized = false;
                    self.anim_progress = 1.0;
                }
                WindowAnim::None => {}
            }
            false
        } else {
            true
        }
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.minimized = false;
        self.is_dragging = false;
        self.crop_path = None;
        self.crop_pixmap = None;
        self.is_loading = false;
        self.chat_history.clear();
        self.current_response.clear();
        self.markdown_layout = None;
        self.layout_text.clear();
        self.input_text.clear();
        self.cursor_pos = 0;
        self.selection_anchor = 0;
        self.is_mouse_selecting = false;
        self.chat_select_start = None;
        self.chat_select_end = None;
        self.is_chat_selecting = false;
        self.chat_copied_toast_time = None;
        self.last_click_time = None;
        self.last_click_pos = (0.0, 0.0);
        self.click_count = 0;
        self.chat_last_click_time = None;
        self.chat_last_click_pos = (0.0, 0.0);
        self.chat_click_count = 0;
        *self.pending_response.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.scroll_offset = 0.0;
        self.max_scroll = 0.0;
    }

    #[allow(dead_code)]
    pub fn toggle_minimize(&mut self) {
        self.minimized = !self.minimized;
        self.is_dragging = false;
    }

    /// Отправка промпта в фоновом потоке
    pub fn send_prompt(&mut self, prompt: String) {
        let trimmed = prompt.trim().to_string();
        if trimmed.is_empty() || self.is_loading {
            return;
        }

        self.chat_history.push(ChatTurn {
            role: "user".into(),
            text: trimmed,
        });

        self.is_loading = true;
        self.update_markdown_layout();
        self.scroll_offset = self.max_scroll;

        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let crop_path = self.crop_path.clone();
        let pending = self.pending_response.clone();
        let history = self.chat_history.clone();

        std::thread::spawn(move || {
            let res = call_gemini(&api_key, &model, &history, crop_path.as_deref());
            let mut lock = pending.lock().unwrap_or_else(|e| e.into_inner());
            *lock = Some(res);
        });
    }

    /// Проверка завершения асинхронного ответа Gemini
    pub fn check_pending_response(&mut self) -> bool {
        let res = {
            let mut lock = self.pending_response.lock().unwrap_or_else(|e| e.into_inner());
            lock.take()
        };
        if let Some(res) = res {
            self.is_loading = false;
            match res {
                Ok(text) => {
                    self.chat_history.push(ChatTurn {
                        role: "model".into(),
                        text,
                    });
                }
                Err(err) => {
                    self.chat_history.push(ChatTurn {
                        role: "model".into(),
                        text: format!("⚠️ Ошибка: {}", err),
                    });
                }
            }
            self.update_markdown_layout();
            self.scroll_offset = self.max_scroll;
            true
        } else {
            false
        }
    }

    /// Сброс диалога пользователем (очистка истории сообщений)
    pub fn reset_chat(&mut self) {
        self.chat_history.clear();
        self.current_response.clear();
        self.scroll_offset = 0.0;
        self.max_scroll = 0.0;
        self.markdown_layout = None;
        self.layout_text.clear();
        self.input_text.clear();
        self.cursor_pos = 0;
        self.selection_anchor = 0;
        self.is_mouse_selecting = false;
        self.chat_select_start = None;
        self.chat_select_end = None;
        self.is_chat_selecting = false;
        self.chat_copied_toast_time = None;
        self.last_click_time = None;
        self.last_click_pos = (0.0, 0.0);
        self.click_count = 0;
        self.chat_last_click_time = None;
        self.chat_last_click_pos = (0.0, 0.0);
        self.chat_click_count = 0;
        *self.pending_response.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.update_markdown_layout();
        info!("Диалог Gemini сброшен пользователем");
    }

    /// Сохранение API ключа в файл и конфиг
    pub fn save_api_key(&mut self, key: String) {
        self.api_key = key.trim().to_string();
        self.config.gemini_api_key = Some(self.api_key.clone());

        let conf_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".config/driftglide");

        let _ = std::fs::create_dir_all(&conf_dir);
        let key_file = conf_dir.join("gemini_key");
        if let Err(e) = std::fs::write(&key_file, &self.api_key) {
            error!("Не удалось сохранить API ключ в {:?}: {}", key_file, e);
            self.status_msg = Some("Ошибка сохранения ключа".into());
        } else {
            info!("API ключ Gemini успешно сохранен в {:?}", key_file);
            self.status_msg = Some("Ключ сохранен!".into());
        }
    }

    /// Выбор и сохранение модели Gemini
    pub fn set_model(&mut self, model: String) {
        self.model = model.clone();
        self.config.gemini_model = model.clone();
        let display_name = self.get_model_display_name();
        self.status_msg = Some(format!("Модель: {}", display_name));

        let conf_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".config/driftglide");

        let _ = std::fs::create_dir_all(&conf_dir);
        let model_file = conf_dir.join("gemini_model");
        let _ = std::fs::write(&model_file, &self.model);
        info!("Выбрана модель Gemini: {} ({})", self.model, display_name);
    }

    pub fn get_model_display_name(&self) -> String {
        for m in GEMINI_MODELS {
            if m.api_id == self.model {
                return m.display_name.to_string();
            }
        }
        self.model.clone()
    }

    /// Вычисление координат плашек моделей в настройках: (ModelDef, x, y, w, h)
    pub fn model_chips_layout(&self) -> Vec<(ModelDef, f32, f32, f32, f32)> {
        let sy = self.y + 60.0;
        let in_y = sy + 48.0;
        let in_h = 38.0;
        let chips_y = in_y + in_h + 34.0;

        let mut res = Vec::new();
        let mut cur_x = self.x + 20.0;
        let mut cur_y = chips_y;
        let chip_h = 28.0;
        let spacing_x = 8.0;
        let spacing_y = 8.0;
        let max_x = self.x + self.width - 20.0;

        for &m in GEMINI_MODELS {
            let chip_w = (m.display_name.len() as f32) * 7.2 + 20.0;
            if cur_x + chip_w > max_x && cur_x > self.x + 20.0 {
                cur_x = self.x + 20.0;
                cur_y += chip_h + spacing_y;
            }
            res.push((m, cur_x, cur_y, chip_w, chip_h));
            cur_x += chip_w + spacing_x;
        }

        res
    }

    pub fn hit_test_model_chip(&self, x: f32, y: f32) -> Option<String> {
        if !self.visible || self.minimized || !self.settings_open {
            return None;
        }
        for (m, cx, cy, cw, ch) in self.model_chips_layout() {
            if x >= cx && x <= cx + cw && y >= cy && y <= cy + ch {
                return Some(m.api_id.to_string());
            }
        }
        None
    }

    // --- Hit-testing ---

    pub fn minimized_bubble_rect(&self, screen_w: u32, screen_h: u32) -> (f32, f32, f32) {
        // (center_x, center_y, radius)
        ((screen_w as f32) - 60.0, (screen_h as f32) - 72.0, 26.0)
    }

    pub fn hit_test_minimized(&self, x: f32, y: f32, screen_w: u32, screen_h: u32) -> bool {
        if !self.visible || !self.minimized {
            return false;
        }
        let (cx, cy, r) = self.minimized_bubble_rect(screen_w, screen_h);
        let dx = x - cx;
        let dy = y - cy;
        (dx * dx + dy * dy) <= (r * r)
    }

    pub fn hit_test_close(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized {
            return false;
        }
        // Кнопка закрытия (в правом углу шапки): x in [self.x + self.width - 36, self.x + self.width - 10], y in [self.y + 10, self.y + 34]
        let bx = self.x + self.width - 34.0;
        let by = self.y + 10.0;
        x >= bx && x <= bx + 24.0 && y >= by && y <= by + 24.0
    }

    pub fn hit_test_minimize(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized {
            return false;
        }
        let bx = self.x + self.width - 66.0;
        let by = self.y + 10.0;
        x >= bx && x <= bx + 24.0 && y >= by && y <= by + 24.0
    }

    pub fn hit_test_settings(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized {
            return false;
        }
        let bx = self.x + self.width - 90.0;
        let by = self.y + 11.0;
        x >= bx && x <= bx + 22.0 && y >= by && y <= by + 22.0
    }

    pub fn hit_test_reset(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized || self.settings_open {
            return false;
        }
        let bx = self.x + self.width - 118.0;
        let by = self.y + 11.0;
        x >= bx && x <= bx + 22.0 && y >= by && y <= by + 22.0
    }

    pub fn hit_test_header(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized {
            return false;
        }
        if self.hit_test_close(x, y)
            || self.hit_test_minimize(x, y)
            || self.hit_test_settings(x, y)
            || self.hit_test_reset(x, y)
        {
            return false;
        }
        // Шапка окна (высота 44px)
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + 44.0
    }

    #[allow(dead_code)]
    pub fn hit_test_input(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized {
            return false;
        }
        let ix = self.x + 16.0;
        let iy = self.y + self.height - 54.0;
        let iw = self.width - 32.0 - 52.0;
        let ih = 40.0;
        x >= ix && x <= ix + iw && y >= iy && y <= iy + ih
    }

    pub fn hit_test_send(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized {
            return false;
        }
        let sx = self.x + self.width - 16.0 - 44.0;
        let sy = self.y + self.height - 54.0;
        x >= sx && x <= sx + 44.0 && y >= sy && y <= sy + 40.0
    }

    pub fn hit_test_window(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized {
            return false;
        }
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    pub fn scroll_by(&mut self, delta: f32) {
        if self.max_scroll <= 0.0 {
            self.scroll_offset = 0.0;
            return;
        }
        self.scroll_offset = (self.scroll_offset + delta).clamp(0.0, self.max_scroll);
    }

    pub fn chat_rect(&self) -> (f32, f32, f32, f32) {
        let chat_x = self.x + 18.0;
        let chat_y = self.y + 140.0;
        let chat_w = self.width - 36.0;
        let chat_h = self.height - 140.0 - 64.0;
        (chat_x, chat_y, chat_w, chat_h)
    }

    pub fn hit_test_chat(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized || self.settings_open {
            return false;
        }
        let (cx, cy, cw, ch) = self.chat_rect();
        x >= cx && x <= cx + cw && y >= cy && y <= cy + ch
    }

    pub fn hit_test_scrollbar(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized || self.settings_open || self.max_scroll <= 0.0 {
            return false;
        }
        let (cx, cy, cw, ch) = self.chat_rect();
        let sx = cx + cw - 18.0;
        let sy = cy + 4.0;
        let sw = 16.0;
        let sh = ch - 8.0;
        x >= sx && x <= sx + sw && y >= sy && y <= sy + sh
    }

    pub fn build_chat_markdown(&self) -> String {
        if self.chat_history.is_empty() {
            if self.api_key.trim().is_empty() {
                return "Нажмите ⚙ в верхнем правом углу, чтобы указать ваш Gemini API Key.".to_string();
            } else {
                return "Задайте вопрос о выделенной области экрана в поле ниже...".to_string();
            }
        }

        let mut md = String::new();
        for (i, turn) in self.chat_history.iter().enumerate() {
            if i > 0 {
                md.push_str("\n\n---\n\n");
            }
            if turn.role == "user" {
                md.push_str("> **👤 Вы:**\n");
                for line in turn.text.lines() {
                    md.push_str("> ");
                    md.push_str(line);
                    md.push('\n');
                }
            } else {
                md.push_str(&turn.text);
                md.push('\n');
            }
        }

        if self.is_loading {
            md.push_str("\n\n*Gemini думает...*");
        }

        md
    }

    pub fn update_markdown_layout(&mut self) {
        let chat_w = self.width - 36.0;
        let content_w = (chat_w - 24.0).max(100.0);
        let text = self.build_chat_markdown();

        if self.markdown_layout.is_none()
            || self.layout_text != text
            || (self.layout_width - content_w).abs() > 1.0
        {
            let layout = layout_markdown(
                &self.text_renderer,
                &text,
                content_w,
                &self.config.theme,
            );
            let chat_h = self.height - 140.0 - 64.0;
            let content_h = chat_h - 16.0;
            self.max_scroll = (layout.total_height - content_h).max(0.0);
            self.scroll_offset = self.scroll_offset.clamp(0.0, self.max_scroll);
            self.layout_text = text;
            self.layout_width = content_w;
            self.markdown_layout = Some(layout);
        }
    }

    pub fn is_cursor_visible(&self) -> bool {
        let ms = self.cursor_blink_start.elapsed().as_millis();
        (ms % 1000) < 550
    }

    pub fn tick_cursor(&mut self) -> bool {
        let visible = self.is_cursor_visible();
        if visible != self.cursor_last_blink_state {
            self.cursor_last_blink_state = visible;
            true
        } else {
            false
        }
    }

    pub fn hit_test_copy(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized || self.settings_open {
            return false;
        }
        let (cx, cy, cw, _) = self.chat_rect();
        let bx = cx + cw - 28.0;
        let by = cy + 8.0;
        x >= bx && x <= bx + 22.0 && y >= by && y <= by + 22.0
    }

    pub fn cursor_byte_offset(&self) -> usize {
        self.input_text
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(idx, _)| idx)
            .unwrap_or(self.input_text.len())
    }

    pub fn has_selection(&self) -> bool {
        self.selection_anchor != self.cursor_pos
    }

    pub fn selection_range(&self) -> (usize, usize) {
        let a = self.selection_anchor.min(self.cursor_pos);
        let b = self.selection_anchor.max(self.cursor_pos);
        let char_count = self.input_text.chars().count();
        (a.min(char_count), b.min(char_count))
    }

    pub fn selected_text(&self) -> Option<String> {
        if !self.has_selection() {
            return None;
        }
        let (start, end) = self.selection_range();
        let start_byte = self.input_text.char_indices().nth(start).map(|(i, _)| i).unwrap_or(self.input_text.len());
        let end_byte = self.input_text.char_indices().nth(end).map(|(i, _)| i).unwrap_or(self.input_text.len());
        Some(self.input_text[start_byte..end_byte].to_string())
    }

    pub fn delete_selection(&mut self) -> bool {
        if !self.has_selection() {
            return false;
        }
        let (start, end) = self.selection_range();
        let start_byte = self.input_text.char_indices().nth(start).map(|(i, _)| i).unwrap_or(self.input_text.len());
        let end_byte = self.input_text.char_indices().nth(end).map(|(i, _)| i).unwrap_or(self.input_text.len());
        self.input_text.replace_range(start_byte..end_byte, "");
        self.cursor_pos = start;
        self.selection_anchor = start;
        true
    }

    pub fn select_all(&mut self) {
        self.selection_anchor = 0;
        self.cursor_pos = self.input_text.chars().count();
    }

    pub fn select_word_at_cursor(&mut self) {
        if self.input_text.is_empty() {
            return;
        }
        let char_count = self.input_text.chars().count();
        let pos = self.cursor_pos.min(char_count);
        let chars: Vec<char> = self.input_text.chars().collect();

        let mut start = pos;
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }

        let mut end = pos;
        while end < char_count && !chars[end].is_whitespace() {
            end += 1;
        }

        if start < end {
            self.selection_anchor = start;
            self.cursor_pos = end;
        } else {
            self.selection_anchor = 0;
            self.cursor_pos = char_count;
        }
    }

    pub fn get_char_pos_from_click(&self, rel_x: f32) -> usize {
        if self.input_text.is_empty() {
            return 0;
        }
        let mut best_pos = 0;
        let mut min_diff = f32::MAX;
        for (i, (idx, _)) in self.input_text.char_indices().enumerate() {
            let w = self.text_renderer.measure_text(&self.input_text[..idx], 12.0);
            let diff = (w - rel_x).abs();
            if diff < min_diff {
                min_diff = diff;
                best_pos = i;
            }
        }
        let total_w = self.text_renderer.measure_text(&self.input_text, 12.0);
        if (total_w - rel_x).abs() < min_diff {
            best_pos = self.input_text.chars().count();
        }
        best_pos
    }

    pub fn handle_input_click(&mut self, rel_x: f32, fx: f32, fy: f32) {
        self.cursor_blink_start = std::time::Instant::now();
        self.cursor_last_blink_state = true;

        let now = std::time::Instant::now();
        let is_double_click = if let Some(last_time) = self.last_click_time {
            let elapsed = last_time.elapsed().as_millis();
            let (lx, ly) = self.last_click_pos;
            let dist = ((fx - lx).powi(2) + (fy - ly).powi(2)).sqrt();
            elapsed < 400 && dist < 12.0
        } else {
            false
        };

        self.last_click_time = Some(now);
        self.last_click_pos = (fx, fy);

        if is_double_click {
            self.click_count = (self.click_count + 1) % 3;
            if self.click_count == 1 {
                self.cursor_pos = self.get_char_pos_from_click(rel_x);
                self.select_word_at_cursor();
                return;
            } else if self.click_count == 2 {
                self.select_all();
                return;
            }
        } else {
            self.click_count = 0;
        }

        let pos = self.get_char_pos_from_click(rel_x);
        self.cursor_pos = pos;
        self.selection_anchor = pos;
        self.is_mouse_selecting = true;
    }

    pub fn set_cursor_from_click(&mut self, rel_x: f32) {
        self.cursor_blink_start = std::time::Instant::now();
        self.cursor_last_blink_state = true;
        let pos = self.get_char_pos_from_click(rel_x);
        self.cursor_pos = pos;
        self.selection_anchor = pos;
    }

    pub fn screen_to_chat_content(&self, fx: f32, fy: f32) -> (f32, f32) {
        let (cx, cy, _, _) = self.chat_rect();
        let origin_x = cx + 12.0;
        let origin_y = cy + 10.0;
        (fx - origin_x, fy - origin_y + self.scroll_offset)
    }

    pub fn select_all_chat(&mut self) {
        let w = self.layout_width.max(600.0);
        let total_h = self.markdown_layout.as_ref().map_or(10000.0, |l| l.total_height + 20.0);
        self.chat_select_start = Some((0.0, 0.0));
        self.chat_select_end = Some((w, total_h));
    }

    pub fn handle_chat_click(&mut self, fx: f32, fy: f32) {
        let now = std::time::Instant::now();
        let is_double_click = if let Some(last_time) = self.chat_last_click_time {
            let elapsed = last_time.elapsed().as_millis();
            let (lx, ly) = self.chat_last_click_pos;
            let dist = ((fx - lx).powi(2) + (fy - ly).powi(2)).sqrt();
            elapsed < 400 && dist < 12.0
        } else {
            false
        };

        self.chat_last_click_time = Some(now);
        self.chat_last_click_pos = (fx, fy);

        let (ctx, cty) = self.screen_to_chat_content(fx, fy);

        if is_double_click {
            self.chat_click_count = (self.chat_click_count + 1) % 3;
            if self.chat_click_count == 1 {
                if let Some(layout) = &self.markdown_layout {
                    for span in &layout.spans {
                        let line_top = span.y - span.size - 3.0;
                        let line_bot = span.y + 5.0;
                        if ctx >= span.x && ctx <= span.x + span.width && cty >= line_top && cty <= line_bot {
                            self.chat_select_start = Some((span.x, line_top));
                            self.chat_select_end = Some((span.x + span.width, line_bot));
                            self.is_chat_selecting = false;
                            return;
                        }
                    }
                }
            } else if self.chat_click_count == 2 {
                self.select_all_chat();
                self.is_chat_selecting = false;
                return;
            }
        } else {
            self.chat_click_count = 0;
        }

        self.chat_select_start = Some((ctx, cty));
        self.chat_select_end = Some((ctx, cty));
        self.is_chat_selecting = true;
    }

    pub fn get_chat_selected_text(&self) -> Option<String> {
        let (s, e) = match (self.chat_select_start, self.chat_select_end) {
            (Some(s), Some(e)) => (s, e),
            _ => return None,
        };

        if (s.1 - e.1).abs() < 3.0 && (s.0 - e.0).abs() < 3.0 {
            return None;
        }

        let layout = self.markdown_layout.as_ref()?;
        let forward = s.1 < e.1 || ((s.1 - e.1).abs() <= 12.0 && s.0 <= e.0);
        let (start_pt, end_pt) = if forward { (s, e) } else { (e, s) };

        let mut selected_words = Vec::new();
        let mut last_y = -1000.0f32;

        for span in &layout.spans {
            let line_top = span.y - span.size - 3.0;
            let line_bot = span.y + 5.0;
            let span_left = span.x;
            let span_right = span.x + span.width;

            let is_in_selection = if line_bot < start_pt.1 || line_top > end_pt.1 {
                false
            } else if start_pt.1 >= line_top && start_pt.1 <= line_bot {
                if end_pt.1 <= line_bot {
                    span_right >= start_pt.0 && span_left <= end_pt.0
                } else {
                    span_right >= start_pt.0
                }
            } else if end_pt.1 >= line_top && end_pt.1 <= line_bot {
                span_left <= end_pt.0
            } else {
                true
            };

            if is_in_selection {
                if (span.y - last_y).abs() > 4.0 && last_y > -500.0 {
                    selected_words.push("\n".to_string());
                } else if !selected_words.is_empty() && selected_words.last().map_or(false, |w| w != "\n") {
                    selected_words.push(" ".to_string());
                }
                selected_words.push(span.text.clone());
                last_y = span.y;
            }
        }

        if selected_words.is_empty() {
            None
        } else {
            Some(selected_words.concat().trim().to_string())
        }
    }

    pub fn hit_test_settings_input(&self, x: f32, y: f32) -> bool {
        if !self.visible || self.minimized || !self.settings_open {
            return false;
        }
        let in_x = self.x + 20.0;
        let in_y = self.y + 60.0 + 46.0;
        let in_w = self.width - 40.0;
        let in_h = 38.0;
        x >= in_x && x <= in_x + in_w && y >= in_y && y <= in_y + in_h
    }

    /// Обработка клавиатурного ввода
    pub fn on_key(
        &mut self,
        utf8: &str,
        is_backspace: bool,
        is_delete: bool,
        is_left: bool,
        is_right: bool,
        is_home: bool,
        is_end: bool,
        is_enter: bool,
        is_escape: bool,
        is_select_all: bool,
        is_copy: bool,
        is_cut: bool,
        is_paste: bool,
        is_shift: bool,
    ) -> bool {
        if !self.visible {
            return false;
        }

        self.cursor_blink_start = std::time::Instant::now();
        self.cursor_last_blink_state = true;

        if is_escape {
            if self.settings_open {
                self.settings_open = false;
            } else {
                self.start_close();
            }
            return true;
        }

        let char_count = self.input_text.chars().count();
        if self.cursor_pos > char_count {
            self.cursor_pos = char_count;
        }
        if self.selection_anchor > char_count {
            self.selection_anchor = char_count;
        }

        // Ctrl + A: Выделить всё
        if is_select_all {
            if self.chat_select_start.is_some() || (self.input_text.is_empty() && !self.settings_open) {
                self.select_all_chat();
            } else {
                self.select_all();
            }
            return true;
        }

        // Ctrl + C: Копировать выделенный текст (или ответ Gemini, если поле ввода пустое)
        if is_copy {
            if self.has_selection() {
                if let Some(txt) = self.selected_text() {
                    set_clipboard_text(&txt);
                }
            } else if let Some(chat_txt) = self.get_chat_selected_text() {
                set_clipboard_text(&chat_txt);
                self.chat_copied_toast_time = Some(std::time::Instant::now());
            } else if !self.settings_open && !self.input_text.is_empty() {
                set_clipboard_text(&self.input_text);
            } else if !self.settings_open {
                if let Some(last_turn) = self.chat_history.iter().rev().find(|t| t.role == "model") {
                    set_clipboard_text(&last_turn.text);
                    self.chat_copied_toast_time = Some(std::time::Instant::now());
                } else if !self.current_response.is_empty() {
                    set_clipboard_text(&self.current_response);
                    self.chat_copied_toast_time = Some(std::time::Instant::now());
                }
            }
            return true;
        }

        // Ctrl + X: Вырезать выделенный текст
        if is_cut {
            if self.has_selection() {
                if let Some(txt) = self.selected_text() {
                    set_clipboard_text(&txt);
                }
                self.delete_selection();
                return true;
            }
            return false;
        }

        // Ctrl + V: Вставить из буфера обмена (заменяя выделение при наличии)
        if is_paste {
            if let Some(text) = get_clipboard_text() {
                self.delete_selection();
                let off = self.cursor_byte_offset();
                self.input_text.insert_str(off, &text);
                self.cursor_pos += text.chars().count();
                self.selection_anchor = self.cursor_pos;
                return true;
            }
            return false;
        }

        // Навигация со Shift / без Shift
        if is_left {
            if is_shift {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
            } else if self.has_selection() {
                self.cursor_pos = self.selection_range().0;
                self.selection_anchor = self.cursor_pos;
            } else {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
                self.selection_anchor = self.cursor_pos;
            }
            return true;
        }
        if is_right {
            if is_shift {
                self.cursor_pos = (self.cursor_pos + 1).min(char_count);
            } else if self.has_selection() {
                self.cursor_pos = self.selection_range().1;
                self.selection_anchor = self.cursor_pos;
            } else {
                self.cursor_pos = (self.cursor_pos + 1).min(char_count);
                self.selection_anchor = self.cursor_pos;
            }
            return true;
        }
        if is_home {
            self.cursor_pos = 0;
            if !is_shift {
                self.selection_anchor = 0;
            }
            return true;
        }
        if is_end {
            self.cursor_pos = char_count;
            if !is_shift {
                self.selection_anchor = char_count;
            }
            return true;
        }

        if self.settings_open {
            if is_enter {
                let key = self.input_text.clone();
                self.save_api_key(key);
                self.input_text.clear();
                self.cursor_pos = 0;
                self.selection_anchor = 0;
                self.settings_open = false;
                return true;
            }

            if is_backspace {
                if self.has_selection() {
                    self.delete_selection();
                    return true;
                }
                if self.cursor_pos > 0 {
                    let prev_off = self.input_text
                        .char_indices()
                        .nth(self.cursor_pos - 1)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.input_text.remove(prev_off);
                    self.cursor_pos -= 1;
                    self.selection_anchor = self.cursor_pos;
                    return true;
                }
                return false;
            }

            if is_delete {
                if self.has_selection() {
                    self.delete_selection();
                    return true;
                }
                if self.cursor_pos < char_count {
                    let cur_off = self.cursor_byte_offset();
                    self.input_text.remove(cur_off);
                    return true;
                }
                return false;
            }

            if !utf8.is_empty() {
                self.delete_selection();
                for c in utf8.chars() {
                    if !c.is_control() {
                        let off = self.cursor_byte_offset();
                        self.input_text.insert(off, c);
                        self.cursor_pos += 1;
                    }
                }
                self.selection_anchor = self.cursor_pos;
                return true;
            }
        } else {
            if is_enter {
                let prompt = std::mem::take(&mut self.input_text);
                self.cursor_pos = 0;
                self.selection_anchor = 0;
                self.send_prompt(prompt);
                return true;
            }

            if is_backspace {
                if self.has_selection() {
                    self.delete_selection();
                    return true;
                }
                if self.cursor_pos > 0 {
                    let prev_off = self.input_text
                        .char_indices()
                        .nth(self.cursor_pos - 1)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.input_text.remove(prev_off);
                    self.cursor_pos -= 1;
                    self.selection_anchor = self.cursor_pos;
                    return true;
                }
                return false;
            }

            if is_delete {
                if self.has_selection() {
                    self.delete_selection();
                    return true;
                }
                if self.cursor_pos < char_count {
                    let cur_off = self.cursor_byte_offset();
                    self.input_text.remove(cur_off);
                    return true;
                }
                return false;
            }

            if !utf8.is_empty() {
                self.delete_selection();
                for c in utf8.chars() {
                    if !c.is_control() {
                        let off = self.cursor_byte_offset();
                        self.input_text.insert(off, c);
                        self.cursor_pos += 1;
                    }
                }
                self.selection_anchor = self.cursor_pos;
                return true;
            }
        }

        false
    }
}

/// Чтение текста из буфера обмена через wl-paste (Wayland)
pub fn get_clipboard_text() -> Option<String> {
    let output = std::process::Command::new("wl-paste")
        .arg("--no-newline")
        .output()
        .ok()?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

/// Запись текста в буфер обмена через wl-copy (Wayland)
pub fn set_clipboard_text(text: &str) {
    if text.is_empty() {
        return;
    }
    use std::io::Write;
    if let Ok(mut child) = std::process::Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
            // stdin закрывается при drop, wl-copy завершится сам
        }
        // НЕ вызываем child.wait() — это блокировало бы event loop
    }
}

