use tiny_skia::*;
use crate::config::Config;
use crate::gestures::PillVisualState;

pub struct PillRenderer {
    config: Config,
}

impl PillRenderer {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Отрисовывает навигационную пилюлю
    pub fn render(
        &self,
        pixmap_data: &mut [u8],
        surface_width: u32,
        surface_height: u32,
        state: &PillVisualState,
    ) {
        let mut pixmap = match PixmapMut::from_bytes(pixmap_data, surface_width, surface_height) {
            Some(p) => p,
            None => return,
        };

        // Полная очистка буфера прозрачным цветом
        pixmap.fill(Color::TRANSPARENT);

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
        let center_y = (surface_height as f32) / 2.0 + offset_y;

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

        // 1. Мягкая нижняя тень для идеального контраста на любом фоне
        if let Some(shadow_path) = build_pill_path(x, y + 1.2, width, height, radius) {
            let mut shadow_paint = Paint::default();
            shadow_paint.set_color_rgba8(0, 0, 0, 75);
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
            paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
            paint.anti_alias = true;

            pixmap.fill_path(&pill_path, &paint, FillRule::Winding, Transform::identity(), None);

            // Тончайший верхний световой блик (Top highlight)
            let mut highlight_paint = Paint::default();
            highlight_paint.set_color_rgba8(255, 255, 255, 60);
            highlight_paint.anti_alias = true;
            let stroke = Stroke { width: 0.7, ..Stroke::default() };
            pixmap.stroke_path(&pill_path, &highlight_paint, &stroke, Transform::identity(), None);
        }
    }
}
