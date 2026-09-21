use tiny_skia::*;
use crate::gemini::window::{GeminiWindow, WindowAnim};
use crate::render::markdown::render_markdown;

/// Отрисовка сквиркла со скругленными углами
fn build_rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let mut pb = PathBuilder::new();
    let rad = r.min(w / 2.0).min(h / 2.0);
    pb.move_to(x + rad, y);
    pb.line_to(x + w - rad, y);
    pb.quad_to(x + w, y, x + w, y + rad);
    pb.line_to(x + w, y + h - rad);
    pb.quad_to(x + w, y + h, x + w - rad, y + h);
    pb.line_to(x + rad, y + h);
    pb.quad_to(x, y + h, x, y + h - rad);
    pb.line_to(x, y + rad);
    pb.quad_to(x, y, x + rad, y);
    pb.close();
    pb.finish()
}

/// Отрисовка векторной лупы (Magnifying Glass)
fn draw_magnifier_icon(pixmap: &mut PixmapMut, cx: f32, cy: f32, radius: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;

    // Круг линзы
    let mut pb = PathBuilder::new();
    pb.push_circle(cx - 2.0, cy - 2.0, radius);
    if let Some(path) = pb.finish() {
        let stroke = Stroke { width: 2.5, ..Stroke::default() };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    // Ручка лупы
    let mut handle_b = PathBuilder::new();
    handle_b.move_to(cx + radius * 0.7 - 2.0, cy + radius * 0.7 - 2.0);
    handle_b.line_to(cx + radius * 1.5, cy + radius * 1.5);
    if let Some(path) = handle_b.finish() {
        let stroke = Stroke {
            width: 3.0,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

/// Отрисовка векторной шестеренки настроек
fn draw_gear_icon(pixmap: &mut PixmapMut, cx: f32, cy: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;

    // Внутреннее отверстие
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, 2.8);
    if let Some(p) = pb.finish() {
        let stroke = Stroke { width: 1.4, ..Stroke::default() };
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
    }

    // 6 радиальных зубцов
    let mut tb = PathBuilder::new();
    for i in 0..6 {
        let angle = (i as f32) * std::f32::consts::PI / 3.0;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        tb.move_to(cx + cos_a * 3.5, cy + sin_a * 3.5);
        tb.line_to(cx + cos_a * 6.5, cy + sin_a * 6.5);
    }
    if let Some(tp) = tb.finish() {
        let stroke = Stroke { width: 1.8, line_cap: LineCap::Round, ..Stroke::default() };
        pixmap.stroke_path(&tp, &paint, &stroke, Transform::identity(), None);
    }
}

/// Отрисовка векторной иконки сброса/нового диалога (круговая стрелка ↺)
fn draw_reset_icon(pixmap: &mut PixmapMut, cx: f32, cy: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;

    // Круговая стрелка (↺)
    let r = 4.6;
    let mut pb = PathBuilder::new();
    let start_angle = 0.85f32; // ~50 deg
    let end_angle = 5.35f32;   // ~306 deg
    let steps = 18;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let angle = start_angle + t * (end_angle - start_angle);
        let x = cx + r * angle.cos();
        let y = cy + r * angle.sin();
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    if let Some(p) = pb.finish() {
        let stroke = Stroke { width: 1.5, line_cap: LineCap::Round, ..Stroke::default() };
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
    }

    // Наконечник стрелки на конце дуги (в точке start_angle)
    let ax = cx + r * start_angle.cos();
    let ay = cy + r * start_angle.sin();
    let mut ab = PathBuilder::new();
    ab.move_to(ax - 0.5, ay - 3.5);
    ab.line_to(ax + 2.0, ay + 0.5);
    ab.line_to(ax - 2.5, ay + 1.2);
    ab.close();
    if let Some(ap) = ab.finish() {
        pixmap.fill_path(&ap, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Отрисовка векторной иконки копирования (два прямоугольника)
fn draw_copy_icon(pixmap: &mut PixmapMut, cx: f32, cy: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;
    let stroke = Stroke { width: 1.2, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };

    // Задний прямоугольник
    let mut pb1 = PathBuilder::new();
    pb1.move_to(cx - 1.0, cy - 5.0);
    pb1.line_to(cx + 4.5, cy - 5.0);
    pb1.line_to(cx + 4.5, cy + 2.0);
    pb1.line_to(cx + 2.0, cy + 2.0);
    if let Some(p) = pb1.finish() {
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
    }

    // Передний прямоугольник
    let mut pb2 = PathBuilder::new();
    pb2.move_to(cx - 4.5, cy - 2.5);
    pb2.line_to(cx + 2.0, cy - 2.5);
    pb2.line_to(cx + 2.0, cy + 5.0);
    pb2.line_to(cx - 4.5, cy + 5.0);
    pb2.close();
    if let Some(p) = pb2.finish() {
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
    }
}

/// Отрисовка векторной иконки галочки (успешное копирование)
fn draw_check_icon(pixmap: &mut PixmapMut, cx: f32, cy: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;
    let stroke = Stroke { width: 1.6, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };

    let mut pb = PathBuilder::new();
    pb.move_to(cx - 4.0, cy + 0.5);
    pb.line_to(cx - 1.0, cy + 3.5);
    pb.line_to(cx + 4.5, cy - 3.5);
    if let Some(p) = pb.finish() {
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
    }
}

/// Отрисовка векторной четырехконечной звезды Gemini
fn draw_sparkle_icon(pixmap: &mut PixmapMut, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;

    let mut pb = PathBuilder::new();
    pb.move_to(cx, cy - r);
    pb.quad_to(cx, cy, cx + r, cy);
    pb.quad_to(cx, cy, cx, cy + r);
    pb.quad_to(cx, cy, cx - r, cy);
    pb.quad_to(cx, cy, cx, cy - r);
    pb.close();
    if let Some(p) = pb.finish() {
        pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Отрисовка окна Gemini и свернутого кружочка с анимацией
pub fn render_gemini(
    pixmap: &mut PixmapMut,
    window: &mut GeminiWindow,
    screen_w: u32,
    screen_h: u32,
) {
    if !window.visible {
        return;
    }

    window.update_markdown_layout();

    let theme = &window.config.theme;

    // --- СВЕРНУТЫЙ РЕЖИМ (Плавающий кружочек в правом нижнем углу) ---
    if window.minimized && window.anim == WindowAnim::None {
        let (cx, cy, r) = window.minimized_bubble_rect(screen_w, screen_h);

        // 1. Тень
        let mut shadow_b = PathBuilder::new();
        shadow_b.push_circle(cx, cy + 3.0, r + 1.0);
        if let Some(sp) = shadow_b.finish() {
            let mut spaint = Paint::default();
            spaint.set_color_rgba8(0, 0, 0, 90);
            spaint.anti_alias = true;
            pixmap.fill_path(&sp, &spaint, FillRule::Winding, Transform::identity(), None);
        }

        // 2. Тело кнопки
        let mut circle_b = PathBuilder::new();
        circle_b.push_circle(cx, cy, r);
        if let Some(cp) = circle_b.finish() {
            let mut cpaint = Paint::default();
            cpaint.set_color_rgba8(theme.header_bg[0], theme.header_bg[1], theme.header_bg[2], 255);
            cpaint.anti_alias = true;
            pixmap.fill_path(&cp, &cpaint, FillRule::Winding, Transform::identity(), None);

            // Акцентная светящаяся рамка
            let mut border_paint = Paint::default();
            border_paint.set_color_rgba8(theme.accent_color[0], theme.accent_color[1], theme.accent_color[2], 230);
            border_paint.anti_alias = true;
            let stroke = Stroke { width: 1.8, ..Stroke::default() };
            pixmap.stroke_path(&cp, &border_paint, &stroke, Transform::identity(), None);
        }

        // 3. Векторная иконка лупы
        draw_magnifier_icon(pixmap, cx, cy, 7.5, [255, 255, 255, 250]);
        return;
    }

    // --- ВЫЧИСЛЕНИЕ ГЕОМЕТРИИ ОКНА С УЧЕТОМ АНИМАЦИИ ---
    let (wx, wy, ww, wh, radius, content_alpha) = match window.anim {
        WindowAnim::Opening => {
            let p = window.anim_progress;
            let y_offset = (1.0 - p) * 14.0;
            (window.x, window.y + y_offset, window.width, window.height, 16.0, p)
        }
        WindowAnim::Closing => {
            let p = window.anim_progress;
            let y_offset = p * 12.0;
            (window.x, window.y + y_offset, window.width, window.height, 16.0, (1.0 - p).max(0.0))
        }
        WindowAnim::Minimizing => {
            let p = window.anim_progress;
            let (bcx, bcy, br) = window.minimized_bubble_rect(screen_w, screen_h);
            let start_x = window.x;
            let start_y = window.y;
            let start_w = window.width;
            let start_h = window.height;
            let end_x = bcx - br;
            let end_y = bcy - br;
            let end_w = br * 2.0;
            let end_h = br * 2.0;

            let cur_x = start_x + (end_x - start_x) * p;
            let cur_y = start_y + (end_y - start_y) * p;
            let cur_w = start_w + (end_w - start_w) * p;
            let cur_h = start_h + (end_h - start_h) * p;
            let cur_rad = 16.0 + (br - 16.0) * p;
            let alpha = if p < 0.25 { 1.0 - p / 0.25 } else { 0.0 };
            (cur_x, cur_y, cur_w, cur_h, cur_rad, alpha)
        }
        WindowAnim::Unminimizing => {
            let p = window.anim_progress;
            let (bcx, bcy, br) = window.minimized_bubble_rect(screen_w, screen_h);
            let start_x = bcx - br;
            let start_y = bcy - br;
            let start_w = br * 2.0;
            let start_h = br * 2.0;
            let end_x = window.x;
            let end_y = window.y;
            let end_w = window.width;
            let end_h = window.height;

            let cur_x = start_x + (end_x - start_x) * p;
            let cur_y = start_y + (end_y - start_y) * p;
            let cur_w = start_w + (end_w - start_w) * p;
            let cur_h = start_h + (end_h - start_h) * p;
            let cur_rad = br + (16.0 - br) * p;
            let alpha = if p > 0.7 { (p - 0.7) / 0.3 } else { 0.0 };
            (cur_x, cur_y, cur_w, cur_h, cur_rad, alpha)
        }
        WindowAnim::None => {
            (window.x, window.y, window.width, window.height, 16.0, 1.0)
        }
    };

    // 1. Мягкая внешняя глубинная тень окна
    if let Some(shadow_p) = build_rounded_rect(wx, wy + 4.0, ww, wh, radius) {
        let mut spaint = Paint::default();
        let sa = (100.0 * content_alpha) as u8;
        spaint.set_color_rgba8(0, 0, 0, sa);
        spaint.anti_alias = true;
        pixmap.fill_path(&shadow_p, &spaint, FillRule::Winding, Transform::identity(), None);
    }
    if let Some(shadow_p) = build_rounded_rect(wx, wy + 1.5, ww, wh, radius) {
        let mut spaint = Paint::default();
        let sa = (120.0 * content_alpha) as u8;
        spaint.set_color_rgba8(0, 0, 0, sa);
        spaint.anti_alias = true;
        pixmap.fill_path(&shadow_p, &spaint, FillRule::Winding, Transform::identity(), None);
    }

    // 2. Основное тело окна
    let bg_alpha = match window.anim {
        WindowAnim::Opening => (255.0 * window.anim_progress) as u8,
        WindowAnim::Closing => (255.0 * (1.0 - window.anim_progress)) as u8,
        WindowAnim::Minimizing | WindowAnim::Unminimizing => 255,
        WindowAnim::None => 255,
    };
    if let Some(window_p) = build_rounded_rect(wx, wy, ww, wh, radius) {
        let mut bg_paint = Paint::default();
        bg_paint.set_color_rgba8(theme.window_bg[0], theme.window_bg[1], theme.window_bg[2], bg_alpha);
        bg_paint.anti_alias = true;
        pixmap.fill_path(&window_p, &bg_paint, FillRule::Winding, Transform::identity(), None);

        // Тонкая внешняя окантовка 1px
        let mut border_paint = Paint::default();
        border_paint.set_color_rgba8(theme.border_color[0], theme.border_color[1], theme.border_color[2], bg_alpha);
        border_paint.anti_alias = true;
        let stroke = Stroke { width: 1.0, ..Stroke::default() };
        pixmap.stroke_path(&window_p, &border_paint, &stroke, Transform::identity(), None);
    }

    // Если идет сворачивание и окно уже близко к кружку — рисуем лупу
    if (window.anim == WindowAnim::Minimizing && window.anim_progress > 0.6)
        || (window.anim == WindowAnim::Unminimizing && window.anim_progress < 0.4)
    {
        let t = if window.anim == WindowAnim::Minimizing {
            (window.anim_progress - 0.6) / 0.4
        } else {
            (0.4 - window.anim_progress) / 0.4
        };
        let la = (250.0 * t) as u8;
        draw_magnifier_icon(pixmap, wx + ww / 2.0, wy + wh / 2.0, 7.5, [255, 255, 255, la]);
    }

    // Если контент скрыт из-за анимации сворачивания — не рисуем внутренности
    if content_alpha <= 0.05 {
        return;
    }

    // 3. Шапка окна (Header - высота 44px)
    if let Some(header_p) = build_rounded_rect(wx, wy, ww, 44.0, radius) {
        let mut h_paint = Paint::default();
        let ha = (255.0 * content_alpha) as u8;
        h_paint.set_color_rgba8(theme.header_bg[0], theme.header_bg[1], theme.header_bg[2], ha);
        h_paint.anti_alias = true;
        pixmap.fill_path(&header_p, &h_paint, FillRule::Winding, Transform::identity(), None);

        // Нижняя разделительная линия шапки
        let mut div_b = PathBuilder::new();
        div_b.move_to(wx, wy + 44.0);
        div_b.line_to(wx + ww, wy + 44.0);
        if let Some(div_p) = div_b.finish() {
            let mut div_paint = Paint::default();
            let da = (180.0 * content_alpha) as u8;
            div_paint.set_color_rgba8(theme.border_color[0], theme.border_color[1], theme.border_color[2], da);
            div_paint.anti_alias = true;
            let stroke = Stroke { width: 1.0, ..Stroke::default() };
            pixmap.stroke_path(&div_p, &div_paint, &stroke, Transform::identity(), None);
        }
    }

    // Векторная иконка Gemini + чистый заголовок
    draw_sparkle_icon(pixmap, wx + 20.0, wy + 22.0, 6.5, theme.accent_color);
    window.text_renderer.draw_text(
        pixmap,
        "Gemini Vision",
        wx + 32.0,
        wy + 27.0,
        14.0,
        theme.text_primary,
    );

    // Кнопки управления в шапке:
    // [Настройки ⚙] [Свернуть −] [Закрыть ✕]
    let btn_y = wy + 11.0;

    // Закрыть (✕)
    let close_x = wx + ww - 34.0;
    if let Some(btn_p) = build_rounded_rect(close_x, btn_y, 22.0, 22.0, 11.0) {
        let mut p = Paint::default();
        p.set_color_rgba8(224, 93, 82, 230); // Coral Red
        p.anti_alias = true;
        pixmap.fill_path(&btn_p, &p, FillRule::Winding, Transform::identity(), None);

        let mut xb = PathBuilder::new();
        xb.move_to(close_x + 7.0, btn_y + 7.0);
        xb.line_to(close_x + 15.0, btn_y + 15.0);
        xb.move_to(close_x + 15.0, btn_y + 7.0);
        xb.line_to(close_x + 7.0, btn_y + 15.0);
        if let Some(xp) = xb.finish() {
            let mut xpaint = Paint::default();
            xpaint.set_color_rgba8(255, 255, 255, 240);
            xpaint.anti_alias = true;
            let strk = Stroke { width: 1.5, line_cap: LineCap::Round, ..Stroke::default() };
            pixmap.stroke_path(&xp, &xpaint, &strk, Transform::identity(), None);
        }
    }

    // Свернуть (−)
    let min_x = wx + ww - 62.0;
    if let Some(btn_p) = build_rounded_rect(min_x, btn_y, 22.0, 22.0, 11.0) {
        let mut p = Paint::default();
        p.set_color_rgba8(229, 169, 60, 230); // Amber
        p.anti_alias = true;
        pixmap.fill_path(&btn_p, &p, FillRule::Winding, Transform::identity(), None);

        let mut mb = PathBuilder::new();
        mb.move_to(min_x + 6.0, btn_y + 11.0);
        mb.line_to(min_x + 16.0, btn_y + 11.0);
        if let Some(mp) = mb.finish() {
            let mut mpaint = Paint::default();
            mpaint.set_color_rgba8(255, 255, 255, 240);
            mpaint.anti_alias = true;
            let strk = Stroke { width: 1.5, line_cap: LineCap::Round, ..Stroke::default() };
            pixmap.stroke_path(&mp, &mpaint, &strk, Transform::identity(), None);
        }
    }

    // Настройки (⚙)
    let set_x = wx + ww - 90.0;
    if let Some(btn_p) = build_rounded_rect(set_x, btn_y, 22.0, 22.0, 11.0) {
        let mut p = Paint::default();
        let bg_c = if window.settings_open { [64, 134, 244, 230] } else { [54, 58, 70, 230] };
        p.set_color_rgba8(bg_c[0], bg_c[1], bg_c[2], bg_c[3]);
        p.anti_alias = true;
        pixmap.fill_path(&btn_p, &p, FillRule::Winding, Transform::identity(), None);

        // Векторная шестеренка
        draw_gear_icon(pixmap, set_x + 11.0, btn_y + 11.0, [255, 255, 255, 240]);
    }

    // Сбросить диалог (↺)
    let reset_x = wx + ww - 118.0;
    if let Some(btn_p) = build_rounded_rect(reset_x, btn_y, 22.0, 22.0, 11.0) {
        let mut p = Paint::default();
        p.set_color_rgba8(54, 58, 70, 230);
        p.anti_alias = true;
        pixmap.fill_path(&btn_p, &p, FillRule::Winding, Transform::identity(), None);

        // Векторная иконка круговой стрелки
        draw_reset_icon(pixmap, reset_x + 11.0, btn_y + 11.0, [255, 255, 255, 240]);
    }

    // --- РЕЖИМ НАСТРОЕК ---
    if window.settings_open {
        let sy = wy + 60.0;
        window.text_renderer.draw_text(
            pixmap,
            "Настройки Gemini API",
            wx + 20.0,
            sy + 10.0,
            15.0,
            theme.text_primary,
        );

        window.text_renderer.draw_text(
            pixmap,
            "Введите API ключ из Google AI Studio (aistudio.google.com):",
            wx + 20.0,
            sy + 34.0,
            12.0,
            theme.text_secondary,
        );

        // Поле ввода API ключа
        let in_x = wx + 20.0;
        let in_y = sy + 46.0;
        let in_w = ww - 40.0;
        let in_h = 38.0;
        if let Some(inp) = build_rounded_rect(in_x, in_y, in_w, in_h, 8.0) {
            let mut ipaint = Paint::default();
            ipaint.set_color_rgba8(theme.input_bg[0], theme.input_bg[1], theme.input_bg[2], 255);
            ipaint.anti_alias = true;
            pixmap.fill_path(&inp, &ipaint, FillRule::Winding, Transform::identity(), None);

            let mut bpaint = Paint::default();
            bpaint.set_color_rgba8(theme.accent_color[0], theme.accent_color[1], theme.accent_color[2], 200);
            bpaint.anti_alias = true;
            let strk = Stroke { width: 1.0, ..Stroke::default() };
            pixmap.stroke_path(&inp, &bpaint, &strk, Transform::identity(), None);
        }

        let (key_display, show_caret) = if !window.input_text.is_empty() {
            (window.input_text.as_str(), true)
        } else if !window.api_key.is_empty() {
            ("•••••••••••••••••••••••••••••• (ключ задан)", false)
        } else {
            ("Вставьте сюда ключ (AIzaSy...)", true)
        };
        let txt_color = if window.input_text.is_empty() && window.api_key.is_empty() {
            theme.text_secondary
        } else {
            theme.text_primary
        };

        // Подсветка выделения в поле ввода ключа
        if window.has_selection() && !window.input_text.is_empty() {
            let (s, e) = window.selection_range();
            let s_byte = window.input_text.char_indices().nth(s).map(|(i,_)| i).unwrap_or(window.input_text.len());
            let e_byte = window.input_text.char_indices().nth(e).map(|(i,_)| i).unwrap_or(window.input_text.len());
            let x1 = in_x + 12.0 + window.text_renderer.measure_text(&window.input_text[..s_byte], 12.0);
            let x2 = in_x + 12.0 + window.text_renderer.measure_text(&window.input_text[..e_byte], 12.0);
            if let Some(sp) = build_rounded_rect(x1, in_y + 8.0, (x2 - x1).max(2.0), 22.0, 3.0) {
                let mut spaint = Paint::default();
                spaint.set_color_rgba8(64, 134, 244, 110);
                spaint.anti_alias = true;
                pixmap.fill_path(&sp, &spaint, FillRule::Winding, Transform::identity(), None);
            }
        }

        window.text_renderer.draw_text(
            pixmap,
            key_display,
            in_x + 12.0,
            in_y + 24.0,
            12.0,
            txt_color,
        );

        // Текстовый курсор (каретка)
        if show_caret && window.is_cursor_visible() {
            let prefix_w = if window.input_text.is_empty() {
                0.0
            } else {
                let byte_off = window.cursor_byte_offset();
                window.text_renderer.measure_text(&window.input_text[..byte_off], 12.0)
            };
            let caret_x = in_x + 12.0 + prefix_w;
            let caret_y = in_y + 9.0;
            let caret_h = 20.0;
            if let Some(cp) = build_rounded_rect(caret_x, caret_y, 2.0, caret_h, 1.0) {
                let mut cpaint = Paint::default();
                cpaint.set_color_rgba8(theme.accent_color[0], theme.accent_color[1], theme.accent_color[2], 240);
                cpaint.anti_alias = true;
                pixmap.fill_path(&cp, &cpaint, FillRule::Winding, Transform::identity(), None);
            }
        }

        // --- ВЫБОР МОДЕЛИ GEMINI (Интерактивные плашки-чипы) ---
        window.text_renderer.draw_text(
            pixmap,
            "Выберите модель Gemini:",
            wx + 20.0,
            sy + 104.0,
            13.0,
            theme.text_primary,
        );

        for (m, cx, cy, cw, ch) in window.model_chips_layout() {
            let is_selected = window.model == m.api_id;
            if let Some(chip_p) = build_rounded_rect(cx, cy, cw, ch, 8.0) {
                let mut cp = Paint::default();
                if is_selected {
                    cp.set_color_rgba8(theme.accent_color[0], theme.accent_color[1], theme.accent_color[2], 240);
                } else {
                    cp.set_color_rgba8(36, 39, 46, 230);
                }
                cp.anti_alias = true;
                pixmap.fill_path(&chip_p, &cp, FillRule::Winding, Transform::identity(), None);

                let mut border_p = Paint::default();
                if is_selected {
                    border_p.set_color_rgba8(255, 255, 255, 90);
                } else {
                    border_p.set_color_rgba8(theme.border_color[0], theme.border_color[1], theme.border_color[2], 180);
                }
                border_p.anti_alias = true;
                let stroke = Stroke { width: 1.0, ..Stroke::default() };
                pixmap.stroke_path(&chip_p, &border_p, &stroke, Transform::identity(), None);
            }

            let text_c = if is_selected { [255, 255, 255, 255] } else { theme.text_secondary };
            window.text_renderer.draw_text(
                pixmap,
                m.display_name,
                cx + 10.0,
                cy + 19.0,
                11.5,
                text_c,
            );
        }

        if let Some(msg) = &window.status_msg {
            window.text_renderer.draw_text(
                pixmap,
                msg,
                wx + 20.0,
                sy + 250.0,
                12.0,
                [100, 230, 140, 255],
            );
        }

        window.text_renderer.draw_text(
            pixmap,
            "Нажмите Enter для сохранения ключа, Esc для возврата.",
            wx + 20.0,
            wy + wh - 26.0,
            11.0,
            theme.text_secondary,
        );
        return;
    }

    // --- РЕЖИМ ЧАТА С GEMINI ---

    // 4. Миниатюра вырезанного фрагмента
    let thumb_x = wx + 18.0;
    let thumb_y = wy + 56.0;
    let thumb_w = 110.0;
    let thumb_h = 74.0;

    if let Some(box_p) = build_rounded_rect(thumb_x, thumb_y, thumb_w, thumb_h, 8.0) {
        let mut bp = Paint::default();
        bp.set_color_rgba8(20, 21, 26, (255.0 * content_alpha) as u8);
        bp.anti_alias = true;
        pixmap.fill_path(&box_p, &bp, FillRule::Winding, Transform::identity(), None);

        let mut bstrk = Paint::default();
        bstrk.set_color_rgba8(
            theme.border_color[0],
            theme.border_color[1],
            theme.border_color[2],
            (255.0 * content_alpha) as u8,
        );
        bstrk.anti_alias = true;
        let strk = Stroke { width: 1.0, ..Stroke::default() };
        pixmap.stroke_path(&box_p, &bstrk, &strk, Transform::identity(), None);
    }

    if let Some(crop_pix) = &window.crop_pixmap {
        let px = thumb_x + (thumb_w - crop_pix.width() as f32) / 2.0;
        let py = thumb_y + (thumb_h - crop_pix.height() as f32) / 2.0;

        let mut ppaint = PixmapPaint::default();
        ppaint.opacity = content_alpha;
        pixmap.draw_pixmap(
            px.round() as i32,
            py.round() as i32,
            crop_pix.as_ref(),
            &ppaint,
            Transform::identity(),
            None,
        );
    }

    // Текст рядом с превью
    window.text_renderer.draw_text(
        pixmap,
        "Фрагмент экрана",
        wx + 138.0,
        wy + 76.0,
        13.0,
        theme.text_primary,
    );

    let status_str = if window.is_loading {
        "Gemini думает..."
    } else {
        "Готов к вопросам"
    };
    let status_color = if window.is_loading {
        theme.accent_color
    } else {
        theme.text_secondary
    };
    window.text_renderer.draw_text(
        pixmap,
        status_str,
        wx + 138.0,
        wy + 98.0,
        11.5,
        status_color,
    );

    // 5. Контейнер ответа Gemini
    let chat_x = wx + 18.0;
    let chat_y = wy + 140.0;
    let chat_w = ww - 36.0;
    let chat_h = wh - 140.0 - 64.0;

    if let Some(chat_p) = build_rounded_rect(chat_x, chat_y, chat_w, chat_h, 10.0) {
        let mut cp = Paint::default();
        cp.set_color_rgba8(20, 22, 27, 255);
        cp.anti_alias = true;
        pixmap.fill_path(&chat_p, &cp, FillRule::Winding, Transform::identity(), None);

        let mut cbp = Paint::default();
        cbp.set_color_rgba8(theme.border_color[0], theme.border_color[1], theme.border_color[2], 160);
        cbp.anti_alias = true;
        let strk = Stroke { width: 1.0, ..Stroke::default() };
        pixmap.stroke_path(&chat_p, &cbp, &strk, Transform::identity(), None);
    }

    // Рендеринг Markdown ответа со скроллингом и клиппингом
    let content_x = chat_x + 12.0;
    let content_y = chat_y + 10.0;
    let clip_rect = [chat_x + 2.0, chat_y + 4.0, chat_x + chat_w - 4.0, chat_y + chat_h - 4.0];
    let chat_sel = window.chat_select_start.and_then(|s| window.chat_select_end.map(|e| (s, e)));

    if let Some(layout) = &window.markdown_layout {
        render_markdown(
            &window.text_renderer,
            layout,
            pixmap,
            content_x,
            content_y,
            clip_rect,
            window.scroll_offset,
            chat_sel,
        );
    }

    // Кнопка копирования ответа в верхнем правом углу контейнера чата
    let copy_x = chat_x + chat_w - 28.0;
    let copy_y = chat_y + 8.0;
    let is_recently_copied = window.chat_copied_toast_time.map_or(false, |t| t.elapsed().as_millis() < 1500);

    if let Some(cp_btn) = build_rounded_rect(copy_x, copy_y, 20.0, 20.0, 4.0) {
        let mut p = Paint::default();
        let bg_c = if is_recently_copied { [36, 120, 64, 220] } else { [36, 40, 50, 200] };
        p.set_color_rgba8(bg_c[0], bg_c[1], bg_c[2], bg_c[3]);
        p.anti_alias = true;
        pixmap.fill_path(&cp_btn, &p, FillRule::Winding, Transform::identity(), None);

        if is_recently_copied {
            draw_check_icon(pixmap, copy_x + 10.0, copy_y + 10.0, [255, 255, 255, 240]);
        } else {
            draw_copy_icon(pixmap, copy_x + 10.0, copy_y + 10.0, [220, 225, 235, 220]);
        }
    }

    // Тонкий скроллбар при переполнении контента
    if window.max_scroll > 0.0 {
        let track_x = chat_x + chat_w - 7.0;
        let track_y = chat_y + 6.0;
        let track_h = chat_h - 12.0;
        let thumb_w = 3.5;
        let total_h = window.markdown_layout.as_ref().map_or(1.0, |l| l.total_height);
        let visible_ratio = (chat_h / total_h).clamp(0.08, 1.0);
        let thumb_h = (track_h * visible_ratio).clamp(20.0, track_h);
        let scroll_ratio = (window.scroll_offset / window.max_scroll).clamp(0.0, 1.0);
        let thumb_y = track_y + scroll_ratio * (track_h - thumb_h);

        if let Some(tp) = build_rounded_rect(track_x, thumb_y, thumb_w, thumb_h, 1.75) {
            let mut tpaint = Paint::default();
            let alpha = if window.is_scrollbar_dragging { 180 } else { 90 };
            tpaint.set_color_rgba8(255, 255, 255, alpha);
            tpaint.anti_alias = true;
            pixmap.fill_path(&tp, &tpaint, FillRule::Winding, Transform::identity(), None);
        }
    }

    // 6. Нижнее поле ввода
    let in_x = wx + 18.0;
    let in_y = wy + wh - 54.0;
    let in_w = ww - 36.0 - 48.0;
    let in_h = 38.0;

    if let Some(inp) = build_rounded_rect(in_x, in_y, in_w, in_h, 8.0) {
        let mut ip = Paint::default();
        ip.set_color_rgba8(theme.input_bg[0], theme.input_bg[1], theme.input_bg[2], 255);
        ip.anti_alias = true;
        pixmap.fill_path(&inp, &ip, FillRule::Winding, Transform::identity(), None);

        let mut bp = Paint::default();
        bp.set_color_rgba8(theme.border_color[0], theme.border_color[1], theme.border_color[2], 255);
        bp.anti_alias = true;
        let strk = Stroke { width: 1.0, ..Stroke::default() };
        pixmap.stroke_path(&inp, &bp, &strk, Transform::identity(), None);
    }

    // Подсветка выделенного текста в поле ввода
    if window.has_selection() && !window.input_text.is_empty() {
        let (s, e) = window.selection_range();
        let s_byte = window.input_text.char_indices().nth(s).map(|(i,_)| i).unwrap_or(window.input_text.len());
        let e_byte = window.input_text.char_indices().nth(e).map(|(i,_)| i).unwrap_or(window.input_text.len());
        let x1 = in_x + 12.0 + window.text_renderer.measure_text(&window.input_text[..s_byte], 12.0);
        let x2 = in_x + 12.0 + window.text_renderer.measure_text(&window.input_text[..e_byte], 12.0);
        if let Some(sp) = build_rounded_rect(x1, in_y + 8.0, (x2 - x1).max(2.0), 22.0, 3.0) {
            let mut spaint = Paint::default();
            spaint.set_color_rgba8(64, 134, 244, 110);
            spaint.anti_alias = true;
            pixmap.fill_path(&sp, &spaint, FillRule::Winding, Transform::identity(), None);
        }
    }

    if window.input_text.is_empty() {
        window.text_renderer.draw_text(
            pixmap,
            "Спросить о содержимом (Enter)...",
            in_x + 12.0,
            in_y + 23.0,
            12.0,
            theme.text_secondary,
        );
    } else {
        window.text_renderer.draw_text(
            pixmap,
            &window.input_text,
            in_x + 12.0,
            in_y + 23.0,
            12.0,
            theme.text_primary,
        );
    }

    // Текстовый курсор (каретка)
    if window.is_cursor_visible() {
        let prefix_w = if window.input_text.is_empty() {
            0.0
        } else {
            let byte_off = window.cursor_byte_offset();
            window.text_renderer.measure_text(&window.input_text[..byte_off], 12.0)
        };
        let caret_x = in_x + 12.0 + prefix_w;
        let caret_y = in_y + 10.0;
        let caret_h = 18.0;
        if let Some(cp) = build_rounded_rect(caret_x, caret_y, 2.0, caret_h, 1.0) {
            let mut cpaint = Paint::default();
            cpaint.set_color_rgba8(theme.accent_color[0], theme.accent_color[1], theme.accent_color[2], 240);
            cpaint.anti_alias = true;
            pixmap.fill_path(&cp, &cpaint, FillRule::Winding, Transform::identity(), None);
        }
    }

    // Кнопка отправить (➤)
    let send_x = in_x + in_w + 8.0;
    let send_y = in_y;
    let send_w = 40.0;
    let send_h = 38.0;

    if let Some(sp) = build_rounded_rect(send_x, send_y, send_w, send_h, 8.0) {
        let mut p = Paint::default();
        let bg_c = if window.is_loading { [70, 75, 88, 255] } else { theme.accent_color };
        p.set_color_rgba8(bg_c[0], bg_c[1], bg_c[2], bg_c[3]);
        p.anti_alias = true;
        pixmap.fill_path(&sp, &p, FillRule::Winding, Transform::identity(), None);

        // Стрелка ➤
        let mut ab = PathBuilder::new();
        ab.move_to(send_x + 14.0, send_y + 11.0);
        ab.line_to(send_x + 28.0, send_y + 19.0);
        ab.line_to(send_x + 14.0, send_y + 27.0);
        ab.line_to(send_x + 18.0, send_y + 19.0);
        ab.close();
        if let Some(ap) = ab.finish() {
            let mut apaint = Paint::default();
            apaint.set_color_rgba8(255, 255, 255, 240);
            apaint.anti_alias = true;
            pixmap.fill_path(&ap, &apaint, FillRule::Winding, Transform::identity(), None);
        }
    }
}

