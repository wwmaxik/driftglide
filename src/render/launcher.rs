use tiny_skia::*;
use crate::ipc::WindowInfo;
use crate::render::icon::IconManager;

pub struct TaskSwitcherRenderer {
    pub windows: Vec<WindowInfo>,
    pub icon_manager: IconManager,
}

impl TaskSwitcherRenderer {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            icon_manager: IconManager::new(),
        }
    }

    pub fn set_windows(&mut self, windows: Vec<WindowInfo>) {
        self.windows = windows;
    }

    /// Определение окна по координатам касания / клика
    pub fn hit_test(&self, x: f32, y: f32, width: u32, height: u32) -> Option<&WindowInfo> {
        let count = self.windows.len();
        if count == 0 {
            return None;
        }

        let item_w = 68.0;
        let spacing = 12.0;
        let total_w = (count as f32) * item_w + ((count - 1) as f32) * spacing;
        let start_x = ((width as f32) - total_w) / 2.0;

        for (i, win) in self.windows.iter().enumerate() {
            let item_x = start_x + (i as f32) * (item_w + spacing);

            if x >= item_x && x <= item_x + item_w && y >= 0.0 && y <= height as f32 {
                return Some(win);
            }
        }

        None
    }

    /// Отрисовка переключателя открытых окон в стиле Dock
    pub fn render(&mut self, buffer: &mut [u8], width: u32, height: u32) {
        let mut pixmap = match PixmapMut::from_bytes(buffer, width, height) {
            Some(p) => p,
            None => return,
        };

        pixmap.fill(Color::TRANSPARENT);

        let pad_x = 8.0;
        let pad_y = 6.0;
        let dock_w = (width as f32) - pad_x * 2.0;
        let dock_h = (height as f32) - pad_y * 2.0;
        let radius = 22.0;

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
        if let Some(shadow_deep) = build_squircle(pad_x, pad_y + 2.5, dock_w, dock_h, radius) {
            let mut p = Paint::default();
            p.set_color_rgba8(0, 0, 0, 60);
            p.anti_alias = true;
            pixmap.fill_path(&shadow_deep, &p, FillRule::Winding, Transform::identity(), None);
        }
        if let Some(shadow_soft) = build_squircle(pad_x, pad_y + 1.2, dock_w, dock_h, radius) {
            let mut p = Paint::default();
            p.set_color_rgba8(0, 0, 0, 80);
            p.anti_alias = true;
            pixmap.fill_path(&shadow_soft, &p, FillRule::Winding, Transform::identity(), None);
        }

        // 2. Стеклянный корпус Дока (Frosted Glass Vibrancy)
        if let Some(dock_path) = build_squircle(pad_x, pad_y, dock_w, dock_h, radius) {
            let mut bg_paint = Paint::default();
            bg_paint.set_color_rgba8(28, 30, 36, 235);
            bg_paint.anti_alias = true;
            pixmap.fill_path(&dock_path, &bg_paint, FillRule::Winding, Transform::identity(), None);

            // Тончайшая стеклянная внутренняя фаска (Highlight Inner Border)
            let mut border_paint = Paint::default();
            border_paint.set_color_rgba8(255, 255, 255, 40);
            border_paint.anti_alias = true;
            let stroke = Stroke { width: 1.0, ..Stroke::default() };
            pixmap.stroke_path(&dock_path, &border_paint, &stroke, Transform::identity(), None);
        }

        let count = self.windows.len();
        if count == 0 {
            return;
        }

        let item_w = 68.0;
        let icon_size = 48.0;
        let spacing = 12.0;
        let total_w = (count as f32) * item_w + ((count - 1) as f32) * spacing;
        let start_x = ((width as f32) - total_w) / 2.0;
        // Центрируем иконку по вертикали с небольшим смещением вверх под точку-индикатор
        let icon_y = ((height as f32) - icon_size) / 2.0 - 4.0;

        for i in 0..count {
            let win = &self.windows[i];
            let item_x = start_x + (i as f32) * (item_w + spacing);
            let icon_x = item_x + (item_w - icon_size) / 2.0;

            // Запрашиваем настоящую системную иконку (PNG)
            let loaded_icon = self.icon_manager.get_icon(&win.app_id, 48).cloned();

            // Если окно активно: мягкий ореол свечения позади иконки
            if win.is_active {
                let halo_cx = icon_x + icon_size / 2.0;
                let halo_cy = icon_y + icon_size / 2.0;
                let mut halo_b = PathBuilder::new();
                halo_b.push_circle(halo_cx, halo_cy, icon_size / 2.0 + 4.0);
                if let Some(hp) = halo_b.finish() {
                    let mut hpaint = Paint::default();
                    hpaint.set_color_rgba8(255, 255, 255, 28);
                    hpaint.anti_alias = true;
                    pixmap.fill_path(&hp, &hpaint, FillRule::Winding, Transform::identity(), None);
                }
            }

            if let Some(icon_pixmap) = loaded_icon {
                // Настоящая системная иконка отображается напрямую без искусственных квадратных рамок!
                let px = icon_x + (icon_size - icon_pixmap.width() as f32) / 2.0;
                let py = icon_y + (icon_size - icon_pixmap.height() as f32) / 2.0;
                pixmap.draw_pixmap(
                    px as i32,
                    py as i32,
                    icon_pixmap.as_ref(),
                    &PixmapPaint::default(),
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
                    icon_paint.set_color_rgba8(bg_color[0], bg_color[1], bg_color[2], bg_color[3]);
                    icon_paint.anti_alias = true;
                    pixmap.fill_path(&icon_path, &icon_paint, FillRule::Winding, Transform::identity(), None);

                    let mut border_paint = Paint::default();
                    border_paint.set_color_rgba8(255, 255, 255, 35);
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
                        bp.set_color_rgba8(255, 255, 255, 240);
                        bp.anti_alias = true;
                        let strk = Stroke { width: 2.0, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
                        pixmap.stroke_path(&p, &bp, &strk, Transform::identity(), None);
                    }
                } else if app_lower.contains("foot") || app_lower.contains("term") {
                    // Промпт >_
                    let mut tb = PathBuilder::new();
                    tb.move_to(cx - 8.0, cy - 6.0);
                    tb.line_to(cx - 2.0, cy);
                    tb.line_to(cx - 8.0, cy + 6.0);
                    if let Some(tp) = tb.finish() {
                        let mut tpaint = Paint::default();
                        tpaint.set_color_rgba8(110, 240, 140, 240);
                        tpaint.anti_alias = true;
                        let tstroke = Stroke { width: 2.2, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
                        pixmap.stroke_path(&tp, &tpaint, &tstroke, Transform::identity(), None);
                    }
                } else {
                    let sym_rect = Rect::from_xywh(cx - 6.0, cy - 6.0, 12.0, 12.0).unwrap();
                    let mut sym_paint = Paint::default();
                    sym_paint.set_color_rgba8(255, 255, 255, 210);
                    sym_paint.anti_alias = true;
                    pixmap.fill_rect(sym_rect, &sym_paint, Transform::identity(), None);
                }
            }

            // 3. Светящаяся белая точка активности (ТОЛЬКО под активным окном)
            if win.is_active {
                let dot_cx = icon_x + icon_size / 2.0;
                let dot_cy = icon_y + icon_size + 6.5;

                // Внешний мягкий ореол свечения
                let mut halo_builder = PathBuilder::new();
                halo_builder.push_circle(dot_cx, dot_cy, 4.0);
                if let Some(halo_path) = halo_builder.finish() {
                    let mut halo_paint = Paint::default();
                    halo_paint.set_color_rgba8(255, 255, 255, 60);
                    halo_paint.anti_alias = true;
                    pixmap.fill_path(&halo_path, &halo_paint, FillRule::Winding, Transform::identity(), None);
                }

                // Яркая белая точка-сердцевина
                let mut dot_builder = PathBuilder::new();
                dot_builder.push_circle(dot_cx, dot_cy, 2.2);
                if let Some(dot_path) = dot_builder.finish() {
                    let mut dot_paint = Paint::default();
                    dot_paint.set_color_rgba8(255, 255, 255, 250);
                    dot_paint.anti_alias = true;
                    pixmap.fill_path(&dot_path, &dot_paint, FillRule::Winding, Transform::identity(), None);
                }
            }
        }
    }
}
