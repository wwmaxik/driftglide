use tiny_skia::*;
use crate::config::Config;
use crate::gestures::PillVisualState;
use crate::render::text::{FontStyle, TextRenderer};

#[derive(Debug, Clone)]
pub struct PillSwipePreview {
    pub title: String,
    pub is_next: bool,
    pub progress: f32,
    pub offset_x: f32,
}

pub struct PillRenderer {
    config: Config,
    text_renderer: TextRenderer,
}

impl PillRenderer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            text_renderer: TextRenderer::new(),
        }
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

    /// Отрисовывает навигационную пилюлю с поддержкой плавного скрытия и подсказки целевого окна при свайпе
    pub fn render(
        &self,
        pixmap_data: &mut [u8],
        surface_width: u32,
        surface_height: u32,
        state: &PillVisualState,
        opacity: f32,
        target_preview: Option<&PillSwipePreview>,
    ) {
        let mut pixmap = match PixmapMut::from_bytes(pixmap_data, surface_width, surface_height) {
            Some(p) => p,
            None => return,
        };

        // Полная очистка буфера прозрачным цветом
        pixmap.fill(Color::TRANSPARENT);

        if opacity <= 0.001 {
            return;
        }

        let base_width = self.config.pill_width;
        let base_height = self.config.pill_height;
        let radius = self.config.pill_radius;

        // Динамическое изменение пропорций (spring response)
        let (width, height) = match state {
            PillVisualState::Pressed => (base_width * 1.06, base_height * 1.25),
            PillVisualState::Dragging { offset_x, offset_y }
            | PillVisualState::Returning { offset_x, offset_y, .. } => {
                let stretch_x = (offset_x.abs() * 0.2).min(30.0);
                let stretch_y = (offset_y.abs() * 0.15).min(2.0);
                (base_width + stretch_x, base_height + stretch_y)
            }
            PillVisualState::Triggered => (base_width * 1.15, base_height * 1.5),
            PillVisualState::Idle => (base_width, base_height),
        };

        // Смещение пилюли
        let (offset_x, offset_y) = match state {
            PillVisualState::Dragging { offset_x, offset_y }
            | PillVisualState::Returning { offset_x, offset_y, .. } => (*offset_x * 0.35, *offset_y * 0.3),
            _ => (0.0, 0.0),
        };

        let center_x = (surface_width as f32) / 2.0 + offset_x;
        // Полоска располагается у самого нижнего края экрана (3.0px от низа)
        let center_y = (surface_height as f32) - 5.5 + offset_y;

        let x = (center_x - width / 2.0).max(0.0);
        let y = (center_y - height / 2.0).max(0.0);

        // Функция построения скругленного пути
        let build_pill_path = |px: f32, py: f32, pw: f32, ph: f32, pr: f32| -> Option<Path> {
            let mut pb = PathBuilder::new();
            let r = pr.min(ph / 2.0).min(pw / 2.0);
            pb.move_to(px + r, py);
            pb.line_to(px + pw - r, py);
            pb.quad_to(px + pw, py, px + pw, py + r);
            pb.line_to(px + pw, py + ph - r);
            pb.quad_to(px + pw, py + ph, px + pw - r, py + ph);
            pb.line_to(px + r, py + ph);
            pb.quad_to(px, py + ph, px, py + ph - r);
            pb.line_to(px, py + r);
            pb.quad_to(px, py, px + r, py);
            pb.close();
            pb.finish()
        };

        let op = opacity.clamp(0.0, 1.0);

        // 1. Мягкая нижняя тень для идеального контраста на любом фоне
        if let Some(shadow_path) = build_pill_path(x, y + 1.2, width, height, radius) {
            let mut shadow_paint = Paint::default();
            shadow_paint.set_color_rgba8(0, 0, 0, (75.0 * op) as u8);
            shadow_paint.anti_alias = true;
            pixmap.fill_path(&shadow_path, &shadow_paint, FillRule::Winding, Transform::identity(), None);
        }

        // 2. Основное тело пилюли
        if let Some(pill_path) = build_pill_path(x, y, width, height, radius) {
            let color = match state {
                PillVisualState::Idle => self.config.color_idle,
                PillVisualState::Pressed => self.config.color_pressed,
                PillVisualState::Dragging { .. }
                | PillVisualState::Returning { .. } => self.config.color_dragging,
                PillVisualState::Triggered => self.config.color_trigger,
            };

            let mut paint = Paint::default();
            paint.set_color_rgba8(
                color[0],
                color[1],
                color[2],
                (color[3] as f32 * op) as u8,
            );
            paint.anti_alias = true;

            pixmap.fill_path(&pill_path, &paint, FillRule::Winding, Transform::identity(), None);

            // Тончайший верхний световой блик (Top highlight)
            let mut highlight_paint = Paint::default();
            highlight_paint.set_color_rgba8(255, 255, 255, (60.0 * op) as u8);
            highlight_paint.anti_alias = true;
            let stroke = Stroke { width: 0.7, ..Stroke::default() };
            pixmap.stroke_path(&pill_path, &highlight_paint, &stroke, Transform::identity(), None);
        }

        // 3. Плавающий бейдж с названием целевого приложения при свайпе влево/вправо
        if let Some(preview) = target_preview {
            let font_size = 11.5;
            let max_text_w = 260.0;
            let fitted_title = self.fit_text(&preview.title, max_text_w, font_size, FontStyle::Regular);

            let arrow = if preview.is_next { "→" } else { "←" };
            let arrow_w = self.text_renderer.measure_text_style(arrow, font_size, FontStyle::Bold);
            let title_w = self.text_renderer.measure_text_style(&fitted_title, font_size, FontStyle::Regular);

            let pad_h = 12.0; // горизонтальный отступ внутри бейджа
            let badge_w = arrow_w + 6.0 + title_w + pad_h * 2.0;
            let badge_h = 24.0;
            let badge_radius = 12.0;

            // Центрируем бейдж над пилюлей с легким параллакс-смещением за пальцем
            let badge_offset_x = preview.offset_x * 0.35;
            let badge_x = (center_x - badge_w / 2.0 + badge_offset_x).clamp(8.0, (surface_width as f32) - badge_w - 8.0);
            let badge_y = (center_y - 30.0).max(2.0);

            // Плавное появление бейджа по мере вытягивания свайпа
            let badge_alpha = ((preview.progress * 1.6).clamp(0.0, 1.0)) * op;

            if badge_alpha > 0.01 {
                // Тень бейджа
                if let Some(shadow_p) = build_pill_path(badge_x, badge_y + 1.5, badge_w, badge_h, badge_radius) {
                    let mut spaint = Paint::default();
                    spaint.set_color_rgba8(0, 0, 0, (90.0 * badge_alpha) as u8);
                    spaint.anti_alias = true;
                    pixmap.fill_path(&shadow_p, &spaint, FillRule::Winding, Transform::identity(), None);
                }

                // Корпус бейджа (темный полупрозрачный акрил)
                if let Some(bg_p) = build_pill_path(badge_x, badge_y, badge_w, badge_h, badge_radius) {
                    let mut bg_paint = Paint::default();
                    let bg_color = if preview.progress >= 1.0 {
                        // Подсветка при достижении порога срабатывания жеста
                        [30, 38, 50, (245.0 * badge_alpha) as u8]
                    } else {
                        [24, 26, 32, (230.0 * badge_alpha) as u8]
                    };
                    bg_paint.set_color_rgba8(bg_color[0], bg_color[1], bg_color[2], bg_color[3]);
                    bg_paint.anti_alias = true;
                    pixmap.fill_path(&bg_p, &bg_paint, FillRule::Winding, Transform::identity(), None);

                    // Рамка: при достижении порога подсвечивается изумрудно-голубым акцентом
                    let mut border_paint = Paint::default();
                    let border_color = if preview.progress >= 1.0 {
                        [100, 220, 170, (230.0 * badge_alpha) as u8] // Готов к переключению!
                    } else {
                        [255, 255, 255, (45.0 * badge_alpha) as u8]
                    };
                    border_paint.set_color_rgba8(border_color[0], border_color[1], border_color[2], border_color[3]);
                    border_paint.anti_alias = true;
                    let stroke = Stroke { width: 1.0, ..Stroke::default() };
                    pixmap.stroke_path(&bg_p, &border_paint, &stroke, Transform::identity(), None);
                }

                // Текст и стрелка
                let baseline_y = badge_y + 16.5;
                let text_start_x = badge_x + pad_h;

                if preview.is_next {
                    // "Title  →"
                    let t_color = if preview.progress >= 1.0 {
                        [255, 255, 255, (255.0 * badge_alpha) as u8]
                    } else {
                        [220, 225, 235, (230.0 * badge_alpha) as u8]
                    };
                    self.text_renderer.draw_text_styled_clipped(
                        &mut pixmap,
                        &fitted_title,
                        text_start_x,
                        baseline_y,
                        font_size,
                        t_color,
                        if preview.progress >= 1.0 { FontStyle::Bold } else { FontStyle::Regular },
                        None,
                    );

                    let arrow_color = if preview.progress >= 1.0 {
                        [110, 240, 180, (255.0 * badge_alpha) as u8]
                    } else {
                        [160, 170, 190, (220.0 * badge_alpha) as u8]
                    };
                    self.text_renderer.draw_text_styled_clipped(
                        &mut pixmap,
                        arrow,
                        text_start_x + title_w + 6.0,
                        baseline_y,
                        font_size,
                        arrow_color,
                        FontStyle::Bold,
                        None,
                    );
                } else {
                    // "←  Title"
                    let arrow_color = if preview.progress >= 1.0 {
                        [110, 240, 180, (255.0 * badge_alpha) as u8]
                    } else {
                        [160, 170, 190, (220.0 * badge_alpha) as u8]
                    };
                    self.text_renderer.draw_text_styled_clipped(
                        &mut pixmap,
                        arrow,
                        text_start_x,
                        baseline_y,
                        font_size,
                        arrow_color,
                        FontStyle::Bold,
                        None,
                    );

                    let t_color = if preview.progress >= 1.0 {
                        [255, 255, 255, (255.0 * badge_alpha) as u8]
                    } else {
                        [220, 225, 235, (230.0 * badge_alpha) as u8]
                    };
                    self.text_renderer.draw_text_styled_clipped(
                        &mut pixmap,
                        &fitted_title,
                        text_start_x + arrow_w + 6.0,
                        baseline_y,
                        font_size,
                        t_color,
                        if preview.progress >= 1.0 { FontStyle::Bold } else { FontStyle::Regular },
                        None,
                    );
                }
            }
        }
    }
}
