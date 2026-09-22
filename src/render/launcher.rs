use tiny_skia::*;
use crate::ipc::WindowInfo;
use crate::render::icon::IconManager;
use crate::render::text::{FontStyle, TextRenderer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwitcherAnim {
    None,
    Opening,
    Closing,
}

pub struct TaskSwitcherRenderer {
    pub windows: Vec<WindowInfo>,
    pub icon_manager: IconManager,
    pub text_renderer: TextRenderer,
    pub anim: SwitcherAnim,
    pub anim_progress: f32,
    pub anim_start_time: Option<std::time::Instant>,
    pub anim_duration: std::time::Duration,
}

impl TaskSwitcherRenderer {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            icon_manager: IconManager::new(),
            text_renderer: TextRenderer::new(),
            anim: SwitcherAnim::None,
            anim_progress: 1.0,
            anim_start_time: None,
            anim_duration: std::time::Duration::from_millis(220),
        }
    }

    pub fn set_windows(&mut self, windows: Vec<WindowInfo>) {
        self.windows = windows;
    }

    /// Подгонка текста под максимальную ширину с добавлением многоточия
    fn fit_text(&self, text: &str, max_width: f32, font_size: f32, style: FontStyle) -> String {
        let full_w = self.text_renderer.measure_text_style(text, font_size, style);
        if full_w <= max_width {
            return text.to_string();
        }

        let ellipsis = "...";
        let el_w = self.text_renderer.measure_text_style(ellipsis, font_size, style);
        let target_w = max_width - el_w;
        if target_w <= 0.0 {
            return ellipsis.to_string();
        }

        let mut result = String::new();
        for ch in text.chars() {
            let candidate = format!("{}{}", result, ch);
            if self.text_renderer.measure_text_style(&candidate, font_size, style) > target_w {
                break;
            }
            result.push(ch);
        }
        result.push_str(ellipsis);
        result
    }

    pub fn is_animating(&self) -> bool {
        self.anim != SwitcherAnim::None
    }

    pub fn start_open(&mut self) {
        self.anim = SwitcherAnim::Opening;
        self.anim_progress = 0.0;
        self.anim_start_time = None;
        self.anim_duration = std::time::Duration::from_millis(220);
    }

    pub fn start_close(&mut self) {
        if self.anim == SwitcherAnim::Closing {
            return;
        }
        self.anim = SwitcherAnim::Closing;
        self.anim_progress = 0.0;
        self.anim_start_time = None;
        self.anim_duration = std::time::Duration::from_millis(180);
    }

    pub fn step_animation(&mut self) -> bool {
        if self.anim == SwitcherAnim::None {
            return false;
        }

        let start = *self.anim_start_time.get_or_insert_with(std::time::Instant::now);
        let elapsed = start.elapsed().as_secs_f32();
        let dur = self.anim_duration.as_secs_f32().max(0.05);
        let linear = (elapsed / dur).min(1.0);

        self.anim_progress = linear;

        if linear >= 1.0 {
            let was_closing = self.anim == SwitcherAnim::Closing;
            self.anim = SwitcherAnim::None;
            self.anim_progress = if was_closing { 0.0 } else { 1.0 };
            self.anim_start_time = None;
            false
        } else {
            true
        }
    }

    /// Определение окна по координатам касания / клика
    pub fn hit_test(&self, x: f32, y: f32, width: u32, height: u32) -> Option<&WindowInfo> {
        let count = self.windows.len();
        if count == 0 {
            return None;
        }

        let item_w = 84.0;
        let spacing = 10.0;
        let total_w = (count as f32) * item_w + ((count - 1) as f32) * spacing;
        let start_x = ((width as f32) - total_w) / 2.0;

        let dock_h = 82.0;
        let pad_bottom = 8.0;
        let base_dock_y = ((height as f32) - dock_h - pad_bottom).max(0.0);

        // Проверяем, попал ли клик по вертикали в область Дока (с небольшим запасом для удобства)
        if y < base_dock_y - 10.0 || y > (height as f32) {
            return None;
        }

        for (i, win) in self.windows.iter().enumerate() {
            let item_x = start_x + (i as f32) * (item_w + spacing);

            if x >= item_x - 4.0 && x <= item_x + item_w + 4.0 {
                return Some(win);
            }
        }

        None
    }

    /// Отрисовка переключателя открытых окон в стиле Dock с анимацией появления и заголовками окон
    pub fn render(&mut self, buffer: &mut [u8], width: u32, height: u32) {
        let mut pixmap = match PixmapMut::from_bytes(buffer, width, height) {
            Some(p) => p,
            None => return,
        };

        pixmap.fill(Color::TRANSPARENT);

        // Расчет параметров анимации появления и закрытия (пружинный slide-up + fade-in / slide-down + fade-out)
        let (slide_y, alpha) = match self.anim {
            SwitcherAnim::Opening => {
                if self.anim_start_time.is_none() {
                    self.anim_start_time = Some(std::time::Instant::now());
                }
                let t = self.anim_progress - 1.0;
                let s = 1.2; // легкий тактильный overshoot
                let eased = (t * t * ((s + 1.0) * t + s) + 1.0).clamp(0.0, 1.05);
                let slide = (1.0 - eased) * 22.0;
                let a = (self.anim_progress * 1.5).min(1.0);
                (slide, a)
            }
            SwitcherAnim::Closing => {
                if self.anim_start_time.is_none() {
                    self.anim_start_time = Some(std::time::Instant::now());
                }
                let p = self.anim_progress;
                // Плавное ускорение вниз (ease-in) и растворение
                let slide = (p * p) * 24.0;
                let a = (1.0 - p).clamp(0.0, 1.0);
                (slide, a)
            }
            SwitcherAnim::None => {
                if self.anim_progress == 0.0 {
                    (24.0, 0.0)
                } else {
                    (0.0, 1.0)
                }
            }
        };

        let count = self.windows.len();
        if count == 0 {
            return;
        }

        let item_w = 84.0;
        let spacing = 10.0;
        let total_w = (count as f32) * item_w + ((count - 1) as f32) * spacing;
        let dock_w = (total_w + 40.0).clamp(180.0, (width as f32 - 32.0).max(180.0));
        let dock_x = ((width as f32) - dock_w) / 2.0;
        let dock_h = 82.0;
        let pad_bottom = 8.0;
        let base_dock_y = ((height as f32) - dock_h - pad_bottom).max(0.0);
        let dock_y = base_dock_y + slide_y;
        let radius = 24.0;

        let build_squircle = |bx: f32, by: f32, bw: f32, bh: f32, rad: f32| -> Option<Path> {
            let mut pb = PathBuilder::new();
            pb.move_to(bx + rad, by);
            pb.line_to(bx + bw - rad, by);
            pb.quad_to(bx + bw, by, bx + bw, by + rad);
            pb.line_to(bx + bw, by + bh - rad);
            pb.quad_to(bx + bw, by + bh, bx + bw - rad, by + bh);
            pb.line_to(bx + rad, by + bh);
            pb.quad_to(bx, by + bh, bx, by + bh - rad);
            pb.line_to(bx, by + rad);
            pb.quad_to(bx, by, bx + rad, by);
            pb.close();
            pb.finish()
        };

        // 1. Мягкая внешняя глубинная тень (Ambient Drop Shadow)
        if let Some(shadow_deep) = build_squircle(dock_x, dock_y + 2.5, dock_w, dock_h, radius) {
            let mut p = Paint::default();
            p.set_color_rgba8(0, 0, 0, (60.0 * alpha) as u8);
            p.anti_alias = true;
            pixmap.fill_path(&shadow_deep, &p, FillRule::Winding, Transform::identity(), None);
        }
        if let Some(shadow_soft) = build_squircle(dock_x, dock_y + 1.2, dock_w, dock_h, radius) {
            let mut p = Paint::default();
            p.set_color_rgba8(0, 0, 0, (80.0 * alpha) as u8);
            p.anti_alias = true;
            pixmap.fill_path(&shadow_soft, &p, FillRule::Winding, Transform::identity(), None);
        }

        // 2. Стеклянный корпус Дока (Frosted Glass Vibrancy)
        if let Some(dock_path) = build_squircle(dock_x, dock_y, dock_w, dock_h, radius) {
            let mut bg_paint = Paint::default();
            bg_paint.set_color_rgba8(28, 30, 36, (235.0 * alpha) as u8);
            bg_paint.anti_alias = true;
            pixmap.fill_path(&dock_path, &bg_paint, FillRule::Winding, Transform::identity(), None);

            // Тончайшая стеклянная внутренняя фаска (Highlight Inner Border)
            let mut border_paint = Paint::default();
            border_paint.set_color_rgba8(255, 255, 255, (40.0 * alpha) as u8);
            border_paint.anti_alias = true;
            let stroke = Stroke { width: 1.0, ..Stroke::default() };
            pixmap.stroke_path(&dock_path, &border_paint, &stroke, Transform::identity(), None);
        }

        let icon_size = 42.0;
        let start_x = ((width as f32) - total_w) / 2.0;
        let icon_y = dock_y + 8.0;

        for i in 0..count {
            let win = &self.windows[i];
            let item_x = start_x + (i as f32) * (item_w + spacing);
            let icon_x = item_x + (item_w - icon_size) / 2.0;

            // Запрашиваем настоящую системную иконку (PNG/SVG)
            let loaded_icon = self.icon_manager.get_icon(&win.app_id, 42).cloned();

            // 1. Иконка приложения (круглая подложка полностью удалена)
            if let Some(icon_pixmap) = loaded_icon {
                let px = icon_x + (icon_size - icon_pixmap.width() as f32) / 2.0;
                let py = icon_y + (icon_size - icon_pixmap.height() as f32) / 2.0;
                let mut ppaint = PixmapPaint::default();
                ppaint.opacity = alpha;
                pixmap.draw_pixmap(
                    px as i32,
                    py as i32,
                    icon_pixmap.as_ref(),
                    &ppaint,
                    Transform::identity(),
                    None,
                );
            } else {
                // Векторный фоллбэк: элегантная плашка squircle со стилизованным глифом
                let app_lower = win.app_id.to_lowercase();
                let bg_color = if app_lower.contains("code") || app_lower.contains("dev") {
                    [35, 120, 240, 245]
                } else if app_lower.contains("foot") || app_lower.contains("term") {
                    [32, 34, 40, 245]
                } else if app_lower.contains("firefox") || app_lower.contains("browser") {
                    [230, 95, 30, 245]
                } else if app_lower.contains("thunar") || app_lower.contains("file") {
                    [30, 150, 230, 245]
                } else {
                    [75, 85, 100, 245]
                };

                let irad = 12.0;
                if let Some(icon_path) = build_squircle(icon_x, icon_y, icon_size, icon_size, irad) {
                    let mut icon_paint = Paint::default();
                    icon_paint.set_color_rgba8(
                        bg_color[0],
                        bg_color[1],
                        bg_color[2],
                        (bg_color[3] as f32 * alpha) as u8,
                    );
                    icon_paint.anti_alias = true;
                    pixmap.fill_path(&icon_path, &icon_paint, FillRule::Winding, Transform::identity(), None);

                    let mut border_paint = Paint::default();
                    border_paint.set_color_rgba8(255, 255, 255, (35.0 * alpha) as u8);
                    border_paint.anti_alias = true;
                    let stroke = Stroke { width: 1.0, ..Stroke::default() };
                    pixmap.stroke_path(&icon_path, &border_paint, &stroke, Transform::identity(), None);
                }

                let cx = icon_x + icon_size / 2.0;
                let cy = icon_y + icon_size / 2.0;

                if app_lower.contains("code") || app_lower.contains("dev") {
                    // Скобки </>
                    let mut b = PathBuilder::new();
                    b.move_to(cx - 4.0, cy - 6.0);
                    b.line_to(cx - 9.0, cy);
                    b.line_to(cx - 4.0, cy + 6.0);
                    b.move_to(cx + 4.0, cy - 6.0);
                    b.line_to(cx + 9.0, cy);
                    b.line_to(cx + 4.0, cy + 6.0);
                    if let Some(p) = b.finish() {
                        let mut bp = Paint::default();
                        bp.set_color_rgba8(255, 255, 255, (240.0 * alpha) as u8);
                        bp.anti_alias = true;
                        let strk = Stroke { width: 2.0, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
                        pixmap.stroke_path(&p, &bp, &strk, Transform::identity(), None);
                    }
                } else if app_lower.contains("foot") || app_lower.contains("term") {
                    // Промпт >_
                    let mut tb = PathBuilder::new();
                    // '>'
                    tb.move_to(cx - 8.0, cy - 6.0);
                    tb.line_to(cx - 2.0, cy);
                    tb.line_to(cx - 8.0, cy + 6.0);
                    // '_'
                    tb.move_to(cx + 1.0, cy + 6.0);
                    tb.line_to(cx + 8.0, cy + 6.0);
                    if let Some(tp) = tb.finish() {
                        let mut tpaint = Paint::default();
                        tpaint.set_color_rgba8(110, 240, 140, (240.0 * alpha) as u8);
                        tpaint.anti_alias = true;
                        let tstroke = Stroke { width: 2.2, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
                        pixmap.stroke_path(&tp, &tpaint, &tstroke, Transform::identity(), None);
                    }
                } else {
                    if let Some(p) = build_squircle(cx - 8.0, cy - 8.0, 16.0, 16.0, 4.0) {
                        let mut sym_paint = Paint::default();
                        sym_paint.set_color_rgba8(255, 255, 255, (210.0 * alpha) as u8);
                        sym_paint.anti_alias = true;
                        let stroke = Stroke { width: 1.5, ..Stroke::default() };
                        pixmap.stroke_path(&p, &sym_paint, &stroke, Transform::identity(), None);
                    }
                }
            }

            // 2. Название / заголовок окна приложения под иконкой
            let title_raw = if !win.title.is_empty() {
                &win.title
            } else {
                &win.app_id
            };
            let style = if win.is_active {
                FontStyle::Bold
            } else {
                FontStyle::Regular
            };
            let font_size = 11.0;
            let label = self.fit_text(title_raw, item_w - 6.0, font_size, style);
            let label_w = self.text_renderer.measure_text_style(&label, font_size, style);
            let text_x = item_x + (item_w - label_w) / 2.0;
            let text_y = icon_y + icon_size + 4.0 + 10.0;

            let text_color = if win.is_active {
                [255, 255, 255, (250.0 * alpha) as u8]
            } else {
                [190, 196, 210, (200.0 * alpha) as u8]
            };

            self.text_renderer.draw_text_styled_clipped(
                &mut pixmap,
                &label,
                text_x,
                text_y,
                font_size,
                text_color,
                style,
                None,
            );

            // 3. Светящаяся белая точка активности (ТОЛЬКО под активным окном)
            if win.is_active {
                let dot_cx = item_x + item_w / 2.0;
                let dot_cy = dock_y + dock_h - 4.5;

                // Внешний мягкий ореол свечения
                let mut halo_builder = PathBuilder::new();
                halo_builder.push_circle(dot_cx, dot_cy, 3.5);
                if let Some(halo_path) = halo_builder.finish() {
                    let mut halo_paint = Paint::default();
                    halo_paint.set_color_rgba8(255, 255, 255, (60.0 * alpha) as u8);
                    halo_paint.anti_alias = true;
                    pixmap.fill_path(&halo_path, &halo_paint, FillRule::Winding, Transform::identity(), None);
                }

                // Яркая белая точка-сердцевина
                let mut dot_builder = PathBuilder::new();
                dot_builder.push_circle(dot_cx, dot_cy, 1.8);
                if let Some(dot_path) = dot_builder.finish() {
                    let mut dot_paint = Paint::default();
                    dot_paint.set_color_rgba8(255, 255, 255, (250.0 * alpha) as u8);
                    dot_paint.anti_alias = true;
                    pixmap.fill_path(&dot_path, &dot_paint, FillRule::Winding, Transform::identity(), None);
                }
            }
        }
    }
}
