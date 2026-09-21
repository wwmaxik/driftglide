pub mod layer_surface;
pub mod screencopy;
pub mod shm;
pub mod touch;

use wayland_client::protocol::{
    wl_buffer::WlBuffer,
    wl_compositor::WlCompositor,
    wl_keyboard::{self, WlKeyboard},
    wl_output::{self, WlOutput},
    wl_pointer::{self, WlPointer},
    wl_region::{self, WlRegion},
    wl_registry::{self, WlRegistry},
    wl_seat::{self, WlSeat},
    wl_shm::{Format, WlShm},
    wl_shm_pool::WlShmPool,
    wl_surface::WlSurface,
    wl_touch::{self, WlTouch},
};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::ZwlrLayerShellV1,
    zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1},
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

use tiny_skia::PixmapMut;
use tracing::{error, info, warn};
use xkbcommon::xkb;

use crate::config::Config;
use crate::gemini::GeminiWindow;
use crate::gestures::{GestureAction, GestureDetector};
use crate::ipc::{IpcClient, IpcCommand};
use crate::render::{render_gemini, LassoRenderer, PillRenderer, TaskSwitcherRenderer};
use crate::search::CircleToSearch;
use layer_surface::LayerSurface;
use screencopy::ScreencopyCapture;
use shm::DoubleBufferedShm;

pub struct AppState {
    pub config: Config,
    pub running: bool,

    // Wayland Globals
    pub compositor: Option<WlCompositor>,
    pub shm: Option<WlShm>,
    pub layer_shell: Option<ZwlrLayerShellV1>,
    pub screencopy_manager: Option<ZwlrScreencopyManagerV1>,
    pub seat: Option<WlSeat>,
    pub touch: Option<WlTouch>,
    pub pointer: Option<WlPointer>,
    pub keyboard: Option<WlKeyboard>,
    pub xkb_context: xkb::Context,
    pub xkb_state: Option<xkb::State>,
    pub outputs: Vec<WlOutput>,

    // Pointer state (мышь / тачпад)
    pub pointer_surface: Option<WlSurface>,
    pub pointer_x: f64,
    pub pointer_y: f64,
    pub pointer_pressed: bool,

    // Surfaces
    pub pill_surface: Option<LayerSurface>,
    pub switcher_surface: Option<LayerSurface>,
    pub search_surface: Option<LayerSurface>,

    // SHM Buffers
    pub pill_shm: DoubleBufferedShm,
    pub switcher_shm: DoubleBufferedShm,
    pub search_shm: DoubleBufferedShm,

    // Logic & Rendering
    pub gesture_detector: GestureDetector,
    pub pill_renderer: PillRenderer,
    pub lasso_renderer: LassoRenderer,
    pub task_switcher: TaskSwitcherRenderer,
    pub circle_to_search: CircleToSearch,
    pub gemini_window: GeminiWindow,
    pub screencopy: Option<ScreencopyCapture>,
    pub ipc: IpcClient,

    // Cursor (мышь / указатель)
    pub cursor_theme: Option<wayland_cursor::CursorTheme>,
    pub cursor_surface: Option<WlSurface>,
    pub current_cursor_name: &'static str,
    pub pointer_enter_serial: u32,
    pub cursor_needs_set: bool,

    // UI state
    pub switcher_open: bool,
    pub search_needs_redraw: bool,
    pub last_switcher_activity: Option<std::time::Instant>,
    pub pending_screencopy_frame: Option<ZwlrScreencopyFrameV1>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let ipc = IpcClient::new(&config.ipc_socket_path);
        let gesture_detector = GestureDetector::new(config.clone());
        let pill_renderer = PillRenderer::new(config.clone());
        let lasso_renderer = LassoRenderer::new();
        let task_switcher = TaskSwitcherRenderer::new();
        let circle_to_search = CircleToSearch::new(config.clone());
        let gemini_window = GeminiWindow::new(config.clone());
        let xkb_context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);

        Self {
            pill_shm: DoubleBufferedShm::new(1920, config.bar_height),
            switcher_shm: DoubleBufferedShm::new(720, 110),
            search_shm: DoubleBufferedShm::new(1920, 1080),
            config,
            running: true,

            compositor: None,
            shm: None,
            layer_shell: None,
            screencopy_manager: None,
            seat: None,
            touch: None,
            pointer: None,
            keyboard: None,
            xkb_context,
            xkb_state: None,
            outputs: Vec::new(),

            pointer_surface: None,
            pointer_x: 0.0,
            pointer_y: 0.0,
            pointer_pressed: false,

            pill_surface: None,
            switcher_surface: None,
            search_surface: None,

            gesture_detector,
            pill_renderer,
            lasso_renderer,
            task_switcher,
            circle_to_search,
            gemini_window,
            screencopy: None,
            ipc,

            cursor_theme: None,
            cursor_surface: None,
            current_cursor_name: "default",
            pointer_enter_serial: 0,
            cursor_needs_set: true,

            switcher_open: false,
            search_needs_redraw: false,
            last_switcher_activity: None,
            pending_screencopy_frame: None,
        }
    }

    /// Инициализация нижней полоски с пилюлей
    pub fn setup_pill_surface(&mut self, qh: &QueueHandle<Self>) {
        if let (Some(compositor), Some(layer_shell)) = (&self.compositor, &self.layer_shell) {
            let primary_output = self.outputs.first();
            let pill_surface = LayerSurface::new_pill_bar(
                compositor,
                layer_shell,
                primary_output,
                self.config.bar_height,
                qh,
            );
            info!("Создан Layer Surface для навигационной пилюли");
            self.pill_surface = Some(pill_surface);
        }
    }

    /// Перерисовка пилюли
    pub fn redraw_pill(&mut self, qh: &QueueHandle<Self>) {
        let (surface, width, height) = match &self.pill_surface {
            Some(p) if p.configured => (&p.surface, p.configured_width, p.configured_height),
            _ => return,
        };

        let shm = match &self.shm {
            Some(s) => s,
            None => return,
        };

        self.pill_shm.resize(width, height);
        if let Ok(buf) = self.pill_shm.next_buffer(shm, qh) {
            self.pill_renderer.render(
                buf.as_mut_slice(),
                width,
                height,
                &self.gesture_detector.visual_state,
            );
            buf.rgba_to_argb8888();

            surface.attach(Some(&buf.buffer), 0, 0);
            surface.damage(0, 0, width as i32, height as i32);
            surface.commit();
        }
    }

    /// Шаг анимации возвращения пилюли (spring physics)
    pub fn step_pill_animation(&mut self, qh: &QueueHandle<Self>) -> bool {
        if self.gesture_detector.pill_animating {
            if self.gesture_detector.step_animation() {
                self.redraw_pill(qh);
                return self.gesture_detector.pill_animating;
            }
        }
        false
    }

    /// Открытие всплывающего Dock переключателя открытых приложений
    pub fn open_task_switcher(&mut self, qh: &QueueHandle<Self>) {
        if self.switcher_surface.is_some() {
            return;
        }

        let windows = self.ipc.query_open_windows();
        let count = windows.len();
        self.task_switcher.set_windows(windows);

        let item_w = 68.0;
        let spacing = 12.0;
        let width = (((count as f32) * (item_w + spacing) + 24.0).clamp(160.0, 960.0)) as u32;
        let height = 76u32;

        if let (Some(compositor), Some(layer_shell)) = (&self.compositor, &self.layer_shell) {
            let primary_output = self.outputs.first();
            let switcher = LayerSurface::new_task_switcher_dock(
                compositor,
                layer_shell,
                primary_output,
                width,
                height,
                36,
                qh,
            );
            self.switcher_surface = Some(switcher);
            self.switcher_open = true;
            self.last_switcher_activity = Some(std::time::Instant::now());
            info!("Открыт Task Switcher Dock: {} открытых окон", count);
        }
    }

    /// Закрытие переключателя приложений
    pub fn close_task_switcher(&mut self) {
        if let Some(switcher) = self.switcher_surface.take() {
            switcher.layer_surface.destroy();
            switcher.surface.destroy();
            self.switcher_open = false;
            self.last_switcher_activity = None;
            info!("Task Switcher закрыт");
        }
    }

    /// Проверка таймаута бездействия: если меню открыто и не используется, скрываем его
    pub fn check_idle_timeout(&mut self, qh: &QueueHandle<Self>) {
        if self.switcher_open {
            if let Some(last) = self.last_switcher_activity {
                if last.elapsed() >= std::time::Duration::from_millis(self.config.switcher_idle_timeout_ms) {
                    info!("Task Switcher автоматически скрыт по таймауту неактивности ({} мс)", self.config.switcher_idle_timeout_ms);
                    self.close_task_switcher();
                    self.redraw_pill(qh);
                }
            }
        }
    }

    /// Инициализация темы курсора Wayland
    pub fn init_cursor_theme(&mut self, conn: &Connection, qh: &QueueHandle<Self>) {
        if let (Some(shm), Some(comp)) = (&self.shm, &self.compositor) {
            if self.cursor_theme.is_none() {
                self.cursor_theme = wayland_cursor::CursorTheme::load(conn, shm.clone(), 24).ok();
            }
            if self.cursor_surface.is_none() {
                self.cursor_surface = Some(comp.create_surface(qh, ()));
            }
        }
    }

    /// Определение типа курсора в зависимости от координат мыши
    pub fn determine_cursor(&self, fx: f32, fy: f32) -> &'static str {
        let is_search = self.search_surface.as_ref().map_or(false, |o| {
            self.pointer_surface.as_ref() == Some(&o.surface)
        });
        if is_search && self.circle_to_search.active && self.gemini_window.visible && !self.gemini_window.minimized {
            if self.gemini_window.settings_open {
                if self.gemini_window.hit_test_settings_input(fx, fy) {
                    return "text";
                }
                if self.gemini_window.hit_test_close(fx, fy)
                    || self.gemini_window.hit_test_minimize(fx, fy)
                    || self.gemini_window.hit_test_settings(fx, fy)
                    || self.gemini_window.hit_test_send(fx, fy)
                    || self.gemini_window.hit_test_model_chip(fx, fy).is_some()
                {
                    return "pointer";
                }
                if self.gemini_window.hit_test_header(fx, fy) {
                    return "grab";
                }
            } else {
                if self.gemini_window.hit_test_copy(fx, fy) {
                    return "pointer";
                }
                if self.gemini_window.hit_test_input(fx, fy) || self.gemini_window.hit_test_chat(fx, fy) {
                    return "text";
                }
                if self.gemini_window.hit_test_close(fx, fy)
                    || self.gemini_window.hit_test_minimize(fx, fy)
                    || self.gemini_window.hit_test_settings(fx, fy)
                    || self.gemini_window.hit_test_reset(fx, fy)
                    || self.gemini_window.hit_test_send(fx, fy)
                    || self.gemini_window.hit_test_scrollbar(fx, fy)
                {
                    return "pointer";
                }
                if self.gemini_window.hit_test_header(fx, fy) {
                    return "grab";
                }
            }
        }
        "default"
    }

    /// Обновление графического курсора мыши
    pub fn update_pointer_cursor(&mut self, fx: f32, fy: f32) {
        let cursor_name = self.determine_cursor(fx, fy);
        if self.current_cursor_name == cursor_name && !self.cursor_needs_set {
            return;
        }
        self.current_cursor_name = cursor_name;
        self.cursor_needs_set = false;

        let pointer = match &self.pointer {
            Some(p) => p,
            None => return,
        };
        let theme = match &mut self.cursor_theme {
            Some(t) => t,
            None => return,
        };
        let surface = match &self.cursor_surface {
            Some(s) => s,
            None => return,
        };

        let names: &[&str] = match cursor_name {
            "text" => &["text", "xterm", "ibeam"],
            "pointer" => &["pointer", "hand2", "pointing_hand"],
            "grab" => &["grab", "openhand", "fleur"],
            _ => &["default", "left_ptr"],
        };

        let mut found_name = None;
        for &name in names {
            if theme.get_cursor(name).is_some() {
                found_name = Some(name);
                break;
            }
        }

        if let Some(name) = found_name {
            if let Some(cursor) = theme.get_cursor(name) {
                let buffer = &cursor[0];
                let (w, h) = buffer.dimensions();
                let (hx, hy) = buffer.hotspot();
                surface.attach(Some(&**buffer), 0, 0);
                surface.damage(0, 0, w as i32, h as i32);
                surface.commit();
                pointer.set_cursor(self.pointer_enter_serial, Some(surface), hx as i32, hy as i32);
            }
        }
    }

    /// Перерисовка переключателя открытых приложений
    pub fn redraw_switcher(&mut self, qh: &QueueHandle<Self>) {
        let (surface, width, height) = match &self.switcher_surface {
            Some(s) if s.configured => (&s.surface, s.configured_width, s.configured_height),
            _ => return,
        };

        let shm = match &self.shm {
            Some(s) => s,
            None => return,
        };

        self.switcher_shm.resize(width, height);
        if let Ok(buf) = self.switcher_shm.next_buffer(shm, qh) {
            self.task_switcher.render(buf.as_mut_slice(), width, height);
            buf.rgba_to_argb8888();

            surface.attach(Some(&buf.buffer), 0, 0);
            surface.damage(0, 0, width as i32, height as i32);
            surface.commit();
        }
    }

    /// Открытие полноэкранного оверлея Circle to Search
    pub fn open_search_overlay(&mut self, qh: &QueueHandle<Self>) {
        if self.search_surface.is_some() {
            return;
        }

        if let (Some(compositor), Some(layer_shell)) = (&self.compositor, &self.layer_shell) {
            let primary_output = self.outputs.first();
            let search = LayerSurface::new_fullscreen_overlay(
                compositor,
                layer_shell,
                primary_output,
                "driftglide_search",
                qh,
            );
            self.search_surface = Some(search);
            info!("Создан полноэкранный Layer Surface для Circle to Search");
        }
    }

    /// Закрытие оверлея Circle to Search
    pub fn close_search_overlay(&mut self) {
        if let Some(search) = self.search_surface.take() {
            search.layer_surface.destroy();
            search.surface.destroy();
            info!("Оверлей Circle to Search закрыт");
        }
    }

    /// Ограничивает интерактивную область оверлея Circle to Search:
    /// - Когда окно Gemini открыто — клики принимает ТОЛЬКО окно Gemini (остальной экран полностью кликабелен!).
    /// - Когда Gemini свернут — клики принимает ТОЛЬКО плавающий кружок в углу (остальной экран полностью кликабелен!).
    /// - Когда идет обводка — интерактивен весь экран.
    pub fn update_search_input_region(&self, qh: &QueueHandle<Self>) {
        let (surface, width, height) = match &self.search_surface {
            Some(s) if s.configured => (&s.surface, s.configured_width, s.configured_height),
            _ => return,
        };
        let compositor = match &self.compositor {
            Some(c) => c,
            None => return,
        };

        if self.gemini_window.visible {
            let region = compositor.create_region(qh, ());
            if self.gemini_window.minimized {
                // Только плавающий кружок в правом нижнем углу
                let (cx, cy, r) = self.gemini_window.minimized_bubble_rect(width, height);
                let x = (cx - r - 4.0).max(0.0) as i32;
                let y = (cy - r - 4.0).max(0.0) as i32;
                let size = ((r + 4.0) * 2.0) as i32;
                region.add(x, y, size, size);
            } else {
                // Область окна Gemini с запасом для тени
                let x = (self.gemini_window.x - 8.0).max(0.0) as i32;
                let y = (self.gemini_window.y - 8.0).max(0.0) as i32;
                let w = (self.gemini_window.width + 16.0) as i32;
                let h = (self.gemini_window.height + 16.0) as i32;
                region.add(x, y, w, h);
            }
            surface.set_input_region(Some(&region));
            surface.commit();
            region.destroy();
        } else {
            // Для обводки лассо интерактивен весь экран
            surface.set_input_region(None);
            surface.commit();
        }
    }

    /// Перерисовка полноэкранного оверлея Circle to Search
    pub fn redraw_search(&mut self, qh: &QueueHandle<Self>) {
        let (surface, width, height) = match &self.search_surface {
            Some(s) if s.configured => (&s.surface, s.configured_width, s.configured_height),
            _ => return,
        };

        let shm = match &self.shm {
            Some(s) => s,
            None => return,
        };

        self.search_shm.resize(width, height);
        if let Ok(buf) = self.search_shm.next_buffer(shm, qh) {
            if self.gemini_window.visible {
                // После обводки оверлей скриншота, лассо и рамка исчезают, остается только окно Gemini!
                buf.as_mut_slice().fill(0);
                if let Some(mut pixmap) = PixmapMut::from_bytes(buf.as_mut_slice(), width, height) {
                    render_gemini(&mut pixmap, &mut self.gemini_window, width, height);
                }
            } else {
                self.lasso_renderer.render(
                    buf.as_mut_slice(),
                    width,
                    height,
                    self.circle_to_search.screen_width,
                    self.circle_to_search.screen_height,
                    self.circle_to_search.base_scrim_argb.as_deref(),
                    self.circle_to_search.screenshot_argb.as_deref(),
                    &self.circle_to_search.lasso_points,
                    self.circle_to_search.current_bbox.as_ref(),
                );
            }

            surface.attach(Some(&buf.buffer), 0, 0);
            surface.damage(0, 0, width as i32, height as i32);
            surface.commit();
        }
    }

    /// Обработка результата жеста
    pub fn handle_gesture_action(&mut self, action: GestureAction, qh: &QueueHandle<Self>) {
        match action {
            GestureAction::None => {}
            GestureAction::RedrawPill => {
                self.redraw_pill(qh);
            }
            GestureAction::FocusNext => {
                info!("Жест: свайп вправо -> focus_next");
                let _ = self.ipc.send_command(&IpcCommand::FocusNext);
                self.redraw_pill(qh);
            }
            GestureAction::FocusPrev => {
                info!("Жест: свайп влево -> focus_prev");
                let _ = self.ipc.send_command(&IpcCommand::FocusPrev);
                self.redraw_pill(qh);
            }
            GestureAction::OpenLauncher => {
                info!("Жест: вытягивание вверх -> открытие Task Switcher");
                self.open_task_switcher(qh);
                self.redraw_pill(qh);
            }
            GestureAction::CloseLauncher => {
                info!("Закрытие Task Switcher");
                self.close_task_switcher();
                self.redraw_pill(qh);
            }
            GestureAction::StartCircleToSearch => {
                info!("Жест: долгое нажатие -> активация Circle to Search!");
                self.trigger_circle_to_search(qh);
            }
        }
    }

    /// Запуск скриншота и переход в Circle to Search
    pub fn trigger_circle_to_search(&mut self, qh: &QueueHandle<Self>) {
        if let (Some(manager), Some(output)) = (&self.screencopy_manager, self.outputs.first()) {
            let mut capture = ScreencopyCapture::new(manager.clone());
            let frame = capture.capture_output(output, qh);
            self.screencopy = Some(capture);
            self.pending_screencopy_frame = Some(frame);
            info!("Инициирован захват экрана через zwlr_screencopy_v1");
        } else {
            info!("Screencopy manager недоступен (KDE Plasma). Захват экрана через CLI (spectacle/grim)...");
            if let Some((rgba, w, h)) = capture_fallback_screenshot() {
                self.circle_to_search.start(rgba, w, h);
            } else {
                warn!("Не удалось захватить экран через CLI. Используем нейтральный фон.");
                let w = 1920;
                let h = 1080;
                self.circle_to_search.start(vec![30; (w * h * 4) as usize], w, h);
            }
            self.open_search_overlay(qh);
            self.redraw_search(qh);
        }
        self.redraw_pill(qh);
    }
}

/// Захват экрана через CLI-утилиты (spectacle для KDE Plasma, grim для wlroots)
pub fn capture_fallback_screenshot() -> Option<(Vec<u8>, u32, u32)> {
    let tmp_path = std::env::temp_dir().join(format!("driftglide_shot_{}.png", std::process::id()));

    // 1. Попытка через spectacle (KDE Plasma)
    let spectacle_status = std::process::Command::new("spectacle")
        .args(["-b", "-n", "-f", "-o"])
        .arg(&tmp_path)
        .status();

    let success = if let Ok(status) = spectacle_status {
        status.success()
    } else {
        // 2. Попытка через grim (wlroots)
        let grim_status = std::process::Command::new("grim")
            .arg(&tmp_path)
            .status();
        grim_status.map(|s| s.success()).unwrap_or(false)
    };

    if success && tmp_path.exists() {
        if let Ok(bytes) = std::fs::read(&tmp_path) {
            let _ = std::fs::remove_file(&tmp_path);
            if let Ok(img) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let mut raw = rgba.into_raw();
                for chunk in raw.chunks_exact_mut(4) {
                    chunk[3] = 255;
                }
                info!("Успешный захват экрана через CLI ({}x{})", w, h);
                return Some((raw, w, h));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Wayland Dispatch Implementations
// ---------------------------------------------------------------------------

impl Dispatch<WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    let compositor = registry.bind::<WlCompositor, _, _>(name, version.min(6), qh, ());
                    state.compositor = Some(compositor);
                }
                "wl_shm" => {
                    let shm = registry.bind::<WlShm, _, _>(name, version.min(1), qh, ());
                    state.shm = Some(shm);
                }
                "wl_output" => {
                    let output = registry.bind::<WlOutput, _, _>(name, version.min(4), qh, ());
                    state.outputs.push(output);
                }
                "wl_seat" => {
                    let seat = registry.bind::<WlSeat, _, _>(name, version.min(7), qh, ());
                    state.seat = Some(seat);
                }
                "zwlr_layer_shell_v1" => {
                    let ls = registry.bind::<ZwlrLayerShellV1, _, _>(name, version.min(4), qh, ());
                    state.layer_shell = Some(ls);
                }
                "zwlr_screencopy_manager_v1" => {
                    let sm = registry.bind::<ZwlrScreencopyManagerV1, _, _>(name, version.min(3), qh, ());
                    state.screencopy_manager = Some(sm);
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<WlCompositor, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlCompositor,
        _: wayland_client::protocol::wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<WlSurface, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlSurface,
        _: wayland_client::protocol::wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<WlShm, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: wayland_client::protocol::wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<WlShmPool, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlShmPool,
        _: wayland_client::protocol::wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<WlBuffer, ()> for AppState {
    fn event(
        state: &mut Self,
        buffer: &WlBuffer,
        event: wayland_client::protocol::wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wayland_client::protocol::wl_buffer::Event::Release = event {
            state.pill_shm.mark_released(buffer);
            state.switcher_shm.mark_released(buffer);
            state.search_shm.mark_released(buffer);
        }
    }
}

impl Dispatch<WlOutput, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlOutput,
        _: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<WlSeat, ()> for AppState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            let has_touch = match capabilities {
                WEnum::Value(caps) => caps.contains(wl_seat::Capability::Touch),
                _ => false,
            };
            if has_touch && state.touch.is_none() {
                let touch = seat.get_touch(qh, ());
                info!("Подключено сенсорное устройство через wl_touch");
                state.touch = Some(touch);
            }

            let has_pointer = match capabilities {
                WEnum::Value(caps) => caps.contains(wl_seat::Capability::Pointer),
                _ => false,
            };
            if has_pointer && state.pointer.is_none() {
                let pointer = seat.get_pointer(qh, ());
                info!("Подключено устройство указателя через wl_pointer (мышь/тачпад)");
                state.pointer = Some(pointer);
            }

            let has_keyboard = match capabilities {
                WEnum::Value(caps) => caps.contains(wl_seat::Capability::Keyboard),
                _ => false,
            };
            if has_keyboard && state.keyboard.is_none() {
                let keyboard = seat.get_keyboard(qh, ());
                info!("Подключена клавиатура через wl_keyboard");
                state.keyboard = Some(keyboard);
            }
        }
    }
}

impl Dispatch<WlTouch, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &WlTouch,
        event: wl_touch::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_touch::Event::Down { surface, id, x, y, .. } => {
                let is_pill = state.pill_surface.as_ref().map_or(false, |p| p.surface == surface);
                let is_switcher = state.switcher_surface.as_ref().map_or(false, |s| s.surface == surface);
                let is_search = state.search_surface.as_ref().map_or(false, |o| o.surface == surface);

                if is_pill {
                    let action = state.gesture_detector.on_touch_down(id, x as f32, y as f32);
                    state.handle_gesture_action(action, qh);
                } else if is_switcher {
                    if let Some(switcher) = &state.switcher_surface {
                        if let Some(win) = state.task_switcher.hit_test(
                            x as f32,
                            y as f32,
                            switcher.configured_width,
                            switcher.configured_height,
                        ) {
                            info!("Тап по окну: {} ({})", win.title, win.id);
                            let _ = state.ipc.send_command(&IpcCommand::FocusWindow(win.id));
                        }
                        state.close_task_switcher();
                        state.redraw_pill(qh);
                    }
                } else if is_search && state.circle_to_search.active {
                    let fx = x as f32;
                    let fy = y as f32;

                    // 1. Проверяем взаимодействие с окном Gemini
                    if state.gemini_window.visible {
                        if state.gemini_window.minimized {
                            let sw = state.circle_to_search.screen_width;
                            let sh = state.circle_to_search.screen_height;
                            if state.gemini_window.hit_test_minimized(fx, fy, sw, sh) {
                                state.gemini_window.start_unminimize();
                                state.update_search_input_region(qh);
                                state.redraw_search(qh);
                            }
                        } else {
                            if state.gemini_window.hit_test_close(fx, fy) {
                                state.gemini_window.start_close();
                                state.redraw_search(qh);
                                return;
                            }
                            if state.gemini_window.hit_test_minimize(fx, fy) {
                                state.gemini_window.start_minimize();
                                state.update_search_input_region(qh);
                                state.redraw_search(qh);
                                return;
                            }
                            if state.gemini_window.hit_test_settings(fx, fy) {
                                state.gemini_window.settings_open = !state.gemini_window.settings_open;
                                state.gemini_window.status_msg = None;
                                state.redraw_search(qh);
                                return;
                            }
                            if state.gemini_window.hit_test_reset(fx, fy) {
                                state.gemini_window.reset_chat();
                                state.redraw_search(qh);
                                return;
                            }
                            if state.gemini_window.settings_open {
                                if let Some(sel_model) = state.gemini_window.hit_test_model_chip(fx, fy) {
                                    state.gemini_window.set_model(sel_model);
                                    state.redraw_search(qh);
                                    return;
                                }
                                if state.gemini_window.hit_test_settings_input(fx, fy) {
                                    let in_x = state.gemini_window.x + 20.0;
                                    state.gemini_window.set_cursor_from_click(fx - (in_x + 12.0));
                                    state.redraw_search(qh);
                                    return;
                                }
                            } else {
                                if state.gemini_window.hit_test_input(fx, fy) {
                                    let in_x = state.gemini_window.x + 16.0;
                                    state.gemini_window.set_cursor_from_click(fx - (in_x + 12.0));
                                    state.redraw_search(qh);
                                    return;
                                }
                            }
                            if state.gemini_window.hit_test_send(fx, fy) {
                                if state.gemini_window.settings_open {
                                    let key = state.gemini_window.input_text.clone();
                                    state.gemini_window.save_api_key(key);
                                    state.gemini_window.input_text.clear();
                                    state.gemini_window.settings_open = false;
                                } else {
                                    let prompt = std::mem::take(&mut state.gemini_window.input_text);
                                    state.gemini_window.send_prompt(prompt);
                                }
                                state.redraw_search(qh);
                                return;
                            }
                            if state.gemini_window.hit_test_header(fx, fy) {
                                state.gemini_window.is_dragging = true;
                                state.gemini_window.drag_offset_x = fx - state.gemini_window.x;
                                state.gemini_window.drag_offset_y = fy - state.gemini_window.y;
                                return;
                            }
                            if state.gemini_window.hit_test_chat(fx, fy) {
                                state.gemini_window.is_touch_scrolling = true;
                                state.gemini_window.touch_last_y = fy;
                                return;
                            }
                            if state.gemini_window.hit_test_window(fx, fy) {
                                return;
                            }
                        }
                        // Если окно Gemini активно (открыто или свернуто), ни в коем случае не начинаем лассо!
                        return;
                    }

                    // 2. Иначе начинаем обводку лассо
                    state.circle_to_search.is_drawing = true;
                    state.circle_to_search.lasso_points.clear();
                    state.circle_to_search.add_point(fx, fy);
                    state.redraw_search(qh);
                }
            }
            wl_touch::Event::Motion { id, x, y, .. } => {
                let fx = x as f32;
                let fy = y as f32;

                if state.gemini_window.is_dragging {
                    state.gemini_window.x = fx - state.gemini_window.drag_offset_x;
                    state.gemini_window.y = fy - state.gemini_window.drag_offset_y;
                    state.search_needs_redraw = true;
                } else if state.gemini_window.is_touch_scrolling {
                    let dy = state.gemini_window.touch_last_y - fy;
                    state.gemini_window.scroll_by(dy);
                    state.gemini_window.touch_last_y = fy;
                    state.search_needs_redraw = true;
                } else if state.circle_to_search.active && state.circle_to_search.is_drawing {
                    if state.circle_to_search.add_point(fx, fy) {
                        state.search_needs_redraw = true;
                    }
                } else {
                    let action = state.gesture_detector.on_touch_motion(id, fx, fy);
                    state.handle_gesture_action(action, qh);
                }
            }
            wl_touch::Event::Up { id, .. } => {
                state.search_needs_redraw = false;
                if state.gemini_window.is_dragging {
                    state.gemini_window.is_dragging = false;
                    state.update_search_input_region(qh);
                } else if state.gemini_window.is_touch_scrolling {
                    state.gemini_window.is_touch_scrolling = false;
                } else if state.circle_to_search.active && state.circle_to_search.is_drawing {
                    state.circle_to_search.is_drawing = false;
                    info!("Завершение обводки пальцем в Circle to Search");
                    if state.circle_to_search.lasso_points.len() >= 3 {
                        if let Some(crop_path) = state.circle_to_search.finish_selection() {
                            let sw = state.circle_to_search.screen_width;
                            let sh = state.circle_to_search.screen_height;
                            state.gemini_window.open(crop_path, sw, sh);
                            state.update_search_input_region(qh);
                            state.redraw_search(qh);
                        }
                    } else {
                        state.circle_to_search.lasso_points.clear();
                        state.redraw_search(qh);
                    }
                } else {
                    let action = state.gesture_detector.on_touch_up(id);
                    state.handle_gesture_action(action, qh);
                }
            }
            wl_touch::Event::Cancel => {
                state.search_needs_redraw = false;
                if state.gemini_window.is_dragging {
                    state.gemini_window.is_dragging = false;
                }
                state.gemini_window.is_touch_scrolling = false;
                if state.circle_to_search.active {
                    state.circle_to_search.is_drawing = false;
                }
                let action = state.gesture_detector.on_touch_cancel(0);
                state.handle_gesture_action(action, qh);
            }
            _ => {}
        }
    }
}

impl Dispatch<WlPointer, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter { serial, surface, surface_x, surface_y, .. } => {
                state.pointer_surface = Some(surface);
                state.pointer_x = surface_x;
                state.pointer_y = surface_y;
                state.pointer_enter_serial = serial;
                state.cursor_needs_set = true;
                state.update_pointer_cursor(surface_x as f32, surface_y as f32);
            }
            wl_pointer::Event::Leave { .. } => {
                if !state.pointer_pressed {
                    state.pointer_surface = None;
                }
            }
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                state.pointer_x = surface_x;
                state.pointer_y = surface_y;
                let fx = surface_x as f32;
                let fy = surface_y as f32;

                state.update_pointer_cursor(fx, fy);

                if state.switcher_open && state.switcher_surface.as_ref().map_or(false, |s| state.pointer_surface.as_ref() == Some(&s.surface)) {
                    state.last_switcher_activity = Some(std::time::Instant::now());
                }

                if state.pointer_pressed {
                    if state.gemini_window.is_dragging {
                        state.gemini_window.x = fx - state.gemini_window.drag_offset_x;
                        state.gemini_window.y = fy - state.gemini_window.drag_offset_y;
                        state.search_needs_redraw = true;
                    } else if state.gemini_window.is_mouse_selecting {
                        let in_x = if state.gemini_window.settings_open {
                            state.gemini_window.x + 20.0
                        } else {
                            state.gemini_window.x + 16.0
                        };
                        state.gemini_window.cursor_pos = state.gemini_window.get_char_pos_from_click(fx - (in_x + 12.0));
                        state.search_needs_redraw = true;
                    } else if state.gemini_window.is_chat_selecting {
                        let (_cx, cy, _cw, ch) = state.gemini_window.chat_rect();
                        if fy < cy + 20.0 {
                            state.gemini_window.scroll_by(-10.0);
                        } else if fy > cy + ch - 20.0 {
                            state.gemini_window.scroll_by(10.0);
                        }
                        let c_pt = state.gemini_window.screen_to_chat_content(fx, fy);
                        state.gemini_window.chat_select_end = Some(c_pt);
                        state.search_needs_redraw = true;
                    } else if state.gemini_window.is_scrollbar_dragging {
                        let (_cx, cy, _cw, ch) = state.gemini_window.chat_rect();
                        let track_y = cy + 6.0;
                        let track_h = (ch - 12.0).max(1.0);
                        let ratio = ((fy - track_y) / track_h).clamp(0.0, 1.0);
                        state.gemini_window.scroll_offset = ratio * state.gemini_window.max_scroll;
                        state.search_needs_redraw = true;
                    } else if state.gemini_window.is_mouse_scrolling {
                        let dy = state.gemini_window.mouse_last_y - fy;
                        state.gemini_window.scroll_by(dy);
                        state.gemini_window.mouse_last_y = fy;
                        state.search_needs_redraw = true;
                    } else if state.circle_to_search.active && state.circle_to_search.is_drawing {
                        if state.circle_to_search.add_point(fx, fy) {
                            state.search_needs_redraw = true;
                        }
                    } else {
                        let action = state.gesture_detector.on_touch_motion(0, fx, fy);
                        state.handle_gesture_action(action, qh);
                    }
                }
            }
            wl_pointer::Event::Axis { value, .. } => {
                let is_search = state.search_surface.as_ref().map_or(false, |o| {
                    state.pointer_surface.as_ref() == Some(&o.surface)
                });
                if is_search
                    && state.circle_to_search.active
                    && state.gemini_window.visible
                    && !state.gemini_window.minimized
                    && !state.gemini_window.settings_open
                {
                    let fx = state.pointer_x as f32;
                    let fy = state.pointer_y as f32;
                    if state.gemini_window.hit_test_window(fx, fy) {
                        let delta = (value as f32) * 1.5;
                        state.gemini_window.scroll_by(delta);
                        state.redraw_search(qh);
                    }
                }
            }
            wl_pointer::Event::Button { button, state: btn_state, .. } => {
                let is_left = button == 0x110 || button == 272;
                let is_right = button == 0x111 || button == 273;

                let pressed = match btn_state {
                    WEnum::Value(wl_pointer::ButtonState::Pressed) => true,
                    _ => false,
                };

                if is_left {
                    state.pointer_pressed = pressed;
                    let fx = state.pointer_x as f32;
                    let fy = state.pointer_y as f32;

                    let is_pill = state.pill_surface.as_ref().map_or(false, |p| {
                        state.pointer_surface.as_ref() == Some(&p.surface)
                    });
                    let is_switcher = state.switcher_surface.as_ref().map_or(false, |s| {
                        state.pointer_surface.as_ref() == Some(&s.surface)
                    });
                    let is_search = state.search_surface.as_ref().map_or(false, |o| {
                        state.pointer_surface.as_ref() == Some(&o.surface)
                    });

                    if pressed {
                        if is_pill {
                            let action = state.gesture_detector.on_touch_down(0, fx, fy);
                            state.handle_gesture_action(action, qh);
                        } else if is_switcher {
                            if let Some(switcher) = &state.switcher_surface {
                                if let Some(win) = state.task_switcher.hit_test(
                                    fx,
                                    fy,
                                    switcher.configured_width,
                                    switcher.configured_height,
                                ) {
                                    info!("Клик по окну в Switcher: {} ({})", win.title, win.id);
                                    let _ = state.ipc.send_command(&IpcCommand::FocusWindow(win.id));
                                }
                                state.close_task_switcher();
                                state.redraw_pill(qh);
                            }
                        } else if is_search && state.circle_to_search.active {
                            // 1. Проверяем взаимодействие с окном Gemini
                            if state.gemini_window.visible {
                                if state.gemini_window.minimized {
                                    let sw = state.circle_to_search.screen_width;
                                    let sh = state.circle_to_search.screen_height;
                                    if state.gemini_window.hit_test_minimized(fx, fy, sw, sh) {
                                        state.gemini_window.start_unminimize();
                                        state.update_search_input_region(qh);
                                        state.redraw_search(qh);
                                        return;
                                    }
                                } else {
                                    if state.gemini_window.hit_test_close(fx, fy) {
                                        state.gemini_window.start_close();
                                        state.redraw_search(qh);
                                        return;
                                    }
                                    if state.gemini_window.hit_test_minimize(fx, fy) {
                                        state.gemini_window.start_minimize();
                                        state.update_search_input_region(qh);
                                        state.redraw_search(qh);
                                        return;
                                    }
                                    if state.gemini_window.hit_test_settings(fx, fy) {
                                        state.gemini_window.settings_open = !state.gemini_window.settings_open;
                                        state.gemini_window.status_msg = None;
                                        state.redraw_search(qh);
                                        return;
                                    }
                                    if state.gemini_window.hit_test_reset(fx, fy) {
                                        state.gemini_window.reset_chat();
                                        state.redraw_search(qh);
                                        return;
                                    }
                                    if state.gemini_window.hit_test_copy(fx, fy) {
                                        if let Some(chat_txt) = state.gemini_window.get_chat_selected_text() {
                                            crate::gemini::window::set_clipboard_text(&chat_txt);
                                        } else if let Some(last_turn) = state.gemini_window.chat_history.iter().rev().find(|t| t.role == "model") {
                                            crate::gemini::window::set_clipboard_text(&last_turn.text);
                                        } else if !state.gemini_window.current_response.is_empty() {
                                            crate::gemini::window::set_clipboard_text(&state.gemini_window.current_response);
                                        }
                                        state.gemini_window.chat_copied_toast_time = Some(std::time::Instant::now());
                                        state.redraw_search(qh);
                                        return;
                                    }
                                    if state.gemini_window.settings_open {
                                        if let Some(sel_model) = state.gemini_window.hit_test_model_chip(fx, fy) {
                                            state.gemini_window.set_model(sel_model);
                                            state.redraw_search(qh);
                                            return;
                                        }
                                        if state.gemini_window.hit_test_settings_input(fx, fy) {
                                            state.gemini_window.chat_select_start = None;
                                            state.gemini_window.chat_select_end = None;
                                            let in_x = state.gemini_window.x + 20.0;
                                            state.gemini_window.handle_input_click(fx - (in_x + 12.0), fx, fy);
                                            state.redraw_search(qh);
                                            return;
                                        }
                                    } else {
                                        if state.gemini_window.hit_test_input(fx, fy) {
                                            state.gemini_window.chat_select_start = None;
                                            state.gemini_window.chat_select_end = None;
                                            let in_x = state.gemini_window.x + 16.0;
                                            state.gemini_window.handle_input_click(fx - (in_x + 12.0), fx, fy);
                                            state.redraw_search(qh);
                                            return;
                                        }
                                    }
                                    if state.gemini_window.hit_test_send(fx, fy) {
                                        if state.gemini_window.settings_open {
                                            let key = state.gemini_window.input_text.clone();
                                            state.gemini_window.save_api_key(key);
                                            state.gemini_window.input_text.clear();
                                            state.gemini_window.settings_open = false;
                                        } else {
                                            let prompt = std::mem::take(&mut state.gemini_window.input_text);
                                            state.gemini_window.send_prompt(prompt);
                                        }
                                        state.redraw_search(qh);
                                        return;
                                    }
                                    if state.gemini_window.hit_test_header(fx, fy) {
                                        state.gemini_window.is_dragging = true;
                                        state.gemini_window.drag_offset_x = fx - state.gemini_window.x;
                                        state.gemini_window.drag_offset_y = fy - state.gemini_window.y;
                                        return;
                                    }
                                    if state.gemini_window.hit_test_scrollbar(fx, fy) {
                                        state.gemini_window.is_scrollbar_dragging = true;
                                        let (_cx, cy, _cw, ch) = state.gemini_window.chat_rect();
                                        let track_y = cy + 6.0;
                                        let track_h = (ch - 12.0).max(1.0);
                                        let ratio = ((fy - track_y) / track_h).clamp(0.0, 1.0);
                                        state.gemini_window.scroll_offset = ratio * state.gemini_window.max_scroll;
                                        state.redraw_search(qh);
                                        return;
                                    }
                                    if state.gemini_window.hit_test_chat(fx, fy) {
                                        state.gemini_window.selection_anchor = state.gemini_window.cursor_pos;
                                        state.gemini_window.handle_chat_click(fx, fy);
                                        state.redraw_search(qh);
                                        return;
                                    }
                                    if state.gemini_window.hit_test_window(fx, fy) {
                                        return;
                                    }
                                }
                                // Если окно Gemini активно (открыто или свернуто), ни в коем случае не начинаем лассо!
                                return;
                            }

                            // 2. Иначе начинаем обводку лассо
                            state.circle_to_search.is_drawing = true;
                            state.circle_to_search.lasso_points.clear();
                            state.circle_to_search.add_point(fx, fy);
                            state.redraw_search(qh);
                        }
                    } else {
                        // Отпускание ЛКМ
                        if state.gemini_window.is_dragging {
                            state.gemini_window.is_dragging = false;
                            state.update_search_input_region(qh);
                        } else if state.gemini_window.is_scrollbar_dragging {
                            state.gemini_window.is_scrollbar_dragging = false;
                            state.redraw_search(qh);
                        } else if state.gemini_window.is_mouse_selecting {
                            state.gemini_window.is_mouse_selecting = false;
                        } else if state.gemini_window.is_chat_selecting {
                            state.gemini_window.is_chat_selecting = false;
                        } else if state.gemini_window.is_mouse_scrolling {
                            state.gemini_window.is_mouse_scrolling = false;
                        } else if state.circle_to_search.active && state.circle_to_search.is_drawing {
                            state.circle_to_search.is_drawing = false;
                            info!("Завершение обводки мышью в Circle to Search");
                            if state.circle_to_search.lasso_points.len() >= 3 {
                                if let Some(crop_path) = state.circle_to_search.finish_selection() {
                                    let sw = state.circle_to_search.screen_width;
                                    let sh = state.circle_to_search.screen_height;
                                    state.gemini_window.open(crop_path, sw, sh);
                                    state.update_search_input_region(qh);
                                    state.redraw_search(qh);
                                }
                            } else {
                                state.circle_to_search.lasso_points.clear();
                                state.redraw_search(qh);
                            }
                        } else {
                            let action = state.gesture_detector.on_touch_up(0);
                            state.handle_gesture_action(action, qh);
                        }
                    }
                } else if is_right && pressed {
                    if state.circle_to_search.active {
                        info!("Отмена/закрытие Circle to Search по нажатию ПКМ");
                        state.gemini_window.start_close();
                        state.redraw_search(qh);
                    } else if state.switcher_open {
                        info!("Закрытие Task Switcher по нажатию ПКМ");
                        state.close_task_switcher();
                        state.redraw_pill(qh);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if format == WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
                    unsafe {
                        if let Ok(Some(keymap)) = xkb::Keymap::new_from_fd(
                            &state.xkb_context,
                            fd,
                            size as usize,
                            xkb::KEYMAP_FORMAT_TEXT_V1,
                            xkb::KEYMAP_COMPILE_NO_FLAGS,
                        ) {
                            state.xkb_state = Some(xkb::State::new(&keymap));
                        }
                    }
                }
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(xkb_state) = &mut state.xkb_state {
                    xkb_state.update_mask(
                        mods_depressed,
                        mods_latched,
                        mods_locked,
                        0,
                        0,
                        group,
                    );
                }
            }
            wl_keyboard::Event::Key { key, state: key_state, .. } => {
                let pressed = match key_state {
                    WEnum::Value(wl_keyboard::KeyState::Pressed) => true,
                    _ => false,
                };
                if pressed && state.gemini_window.visible {
                    let keycode = xkb::Keycode::new(key + 8);
                    let mut utf8 = String::new();
                    let mut is_backspace = false;
                    let mut is_delete = false;
                    let mut is_left = false;
                    let mut is_right = false;
                    let mut is_home = false;
                    let mut is_end = false;
                    let mut is_enter = false;
                    let mut is_escape = false;
                    let mut is_select_all = false;
                    let mut is_copy = false;
                    let mut is_cut = false;
                    let mut is_paste = false;
                    let mut is_shift = false;

                    if let Some(xkb_state) = &state.xkb_state {
                        let sym = xkb_state.key_get_one_sym(keycode);
                        let ctrl = xkb_state.mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE);
                        is_shift = xkb_state.mod_name_is_active(xkb::MOD_NAME_SHIFT, xkb::STATE_MODS_EFFECTIVE);

                        if ctrl && (sym == xkb::Keysym::a || sym == xkb::Keysym::A || sym == xkb::Keysym::Cyrillic_ef || sym == xkb::Keysym::Cyrillic_EF) {
                            is_select_all = true;
                        } else if ctrl && (sym == xkb::Keysym::c || sym == xkb::Keysym::C || sym == xkb::Keysym::Cyrillic_es || sym == xkb::Keysym::Cyrillic_ES) {
                            is_copy = true;
                        } else if ctrl && (sym == xkb::Keysym::x || sym == xkb::Keysym::X || sym == xkb::Keysym::Cyrillic_che || sym == xkb::Keysym::Cyrillic_CHE) {
                            is_cut = true;
                        } else if ctrl && (sym == xkb::Keysym::v || sym == xkb::Keysym::V || sym == xkb::Keysym::Cyrillic_em || sym == xkb::Keysym::Cyrillic_EM) {
                            is_paste = true;
                        } else if sym == xkb::Keysym::BackSpace {
                            is_backspace = true;
                        } else if sym == xkb::Keysym::Delete {
                            is_delete = true;
                        } else if sym == xkb::Keysym::Left {
                            is_left = true;
                        } else if sym == xkb::Keysym::Right {
                            is_right = true;
                        } else if sym == xkb::Keysym::Home {
                            is_home = true;
                        } else if sym == xkb::Keysym::End {
                            is_end = true;
                        } else if sym == xkb::Keysym::Return || sym == xkb::Keysym::KP_Enter {
                            is_enter = true;
                        } else if sym == xkb::Keysym::Escape {
                            is_escape = true;
                        } else {
                            utf8 = xkb_state.key_get_utf8(keycode);
                            if utf8 == "\u{1}" {
                                is_select_all = true;
                                utf8.clear();
                            } else if utf8 == "\u{3}" {
                                is_copy = true;
                                utf8.clear();
                            } else if utf8 == "\u{18}" {
                                is_cut = true;
                                utf8.clear();
                            } else if utf8 == "\u{16}" {
                                is_paste = true;
                                utf8.clear();
                            }
                        }
                    }

                    if is_escape {
                        if state.gemini_window.settings_open {
                            state.gemini_window.settings_open = false;
                            state.search_needs_redraw = true;
                        } else {
                            state.gemini_window.start_close();
                            state.search_needs_redraw = true;
                        }
                    } else if state.gemini_window.on_key(
                        &utf8,
                        is_backspace,
                        is_delete,
                        is_left,
                        is_right,
                        is_home,
                        is_end,
                        is_enter,
                        is_escape,
                        is_select_all,
                        is_copy,
                        is_cut,
                        is_paste,
                        is_shift,
                    ) {
                        state.search_needs_redraw = true;
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &ZwlrLayerShellV1,
        _: wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        surface: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                surface.ack_configure(serial);

                if let Some(pill) = &mut state.pill_surface {
                    if &pill.layer_surface == surface {
                        pill.configured_width = width;
                        pill.configured_height = height;
                        pill.configured = true;
                        state.redraw_pill(qh);
                        return;
                    }
                }

                if let Some(switcher) = &mut state.switcher_surface {
                    if &switcher.layer_surface == surface {
                        switcher.configured_width = width;
                        switcher.configured_height = height;
                        switcher.configured = true;
                        state.redraw_switcher(qh);
                        return;
                    }
                }

                if let Some(search) = &mut state.search_surface {
                    if &search.layer_surface == surface {
                        search.configured_width = width;
                        search.configured_height = height;
                        search.configured = true;
                        state.update_search_input_region(qh);
                        state.redraw_search(qh);
                    }
                }
            }
            zwlr_layer_surface_v1::Event::Closed => {
                if let Some(pill) = &state.pill_surface {
                    if &pill.layer_surface == surface {
                        info!("Pill Layer surface закрыт композитором");
                        state.running = false;
                    }
                }
                if let Some(switcher) = &state.switcher_surface {
                    if &switcher.layer_surface == surface {
                        info!("Switcher surface закрыт композитором");
                        state.switcher_surface = None;
                        state.switcher_open = false;
                    }
                }
                if let Some(search) = &state.search_surface {
                    if &search.layer_surface == surface {
                        info!("Search surface закрыт композитором");
                        state.search_surface = None;
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &ZwlrScreencopyManagerV1,
        _: wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for AppState {
    fn event(
        state: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                let fmt = match format {
                    WEnum::Value(f) => f,
                    _ => Format::Argb8888,
                };
                if let (Some(capture), Some(shm)) = (&mut state.screencopy, &state.shm) {
                    if let Err(e) = capture.init_buffer(shm, qh, frame, fmt, width, height, stride) {
                        error!("Не удалось создать буфер для screencopy: {}", e);
                    }
                }
            }
            zwlr_screencopy_frame_v1::Event::Flags { .. } => {}
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                info!("Кадр screencopy успешно захвачен!");
                if let Some(capture) = &mut state.screencopy {
                    capture.state.ready = true;
                    if let Some(rgba) = capture.get_rgba_data() {
                        let w = capture.state.width;
                        let h = capture.state.height;
                        state.circle_to_search.start(rgba, w, h);
                        state.open_search_overlay(qh);
                        state.redraw_search(qh);
                    }
                }
                state.pending_screencopy_frame = None;
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                warn!("Захват кадра screencopy завершился неудачей, пробуем CLI fallback...");
                state.pending_screencopy_frame = None;
                if let Some((rgba, w, h)) = capture_fallback_screenshot() {
                    state.circle_to_search.start(rgba, w, h);
                } else {
                    let w = 1920;
                    let h = 1080;
                    state.circle_to_search.start(vec![30; (w * h * 4) as usize], w, h);
                }
                state.open_search_overlay(qh);
                state.redraw_search(qh);
            }
            _ => {}
        }
    }
}

impl Dispatch<WlRegion, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
