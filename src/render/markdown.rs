use tiny_skia::*;
use crate::config::ThemeConfig;
use crate::render::text::{TextRenderer, FontStyle};

#[derive(Debug, Clone)]
pub struct VisualSpan {
    pub text: String,
    pub style: FontStyle,
    pub size: f32,
    pub color: [u8; 4],
    pub bg_color: Option<[u8; 4]>,
    pub x: f32,
    pub y: f32, // baseline y
    pub width: f32,
}

#[derive(Debug, Clone)]
pub struct VisualBullet {
    pub cx: f32,
    pub cy: f32,
    pub radius: f32,
    pub color: [u8; 4],
}

#[derive(Debug, Clone)]
pub struct VisualRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [u8; 4],
    pub border_color: Option<[u8; 4]>,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct VisualLine {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub color: [u8; 4],
    pub width: f32,
}

#[derive(Debug, Clone, Default)]
pub struct MarkdownLayout {
    pub spans: Vec<VisualSpan>,
    pub bullets: Vec<VisualBullet>,
    pub rects: Vec<VisualRect>,
    pub lines: Vec<VisualLine>,
    pub total_height: f32,
}

#[derive(Debug, Clone)]
struct InlineSpan {
    text: String,
    bold: bool,
    italic: bool,
    code: bool,
}

/// Построение пути скругленного прямоугольника
pub fn build_rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
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

/// Парсинг инлайн-разметки: **жирный**, *курсив*, `код`, ***жирный курсив***
fn parse_inline_spans(input: &str) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut bold = false;
    let mut italic = false;
    let mut code = false;

    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    let flush = |spans: &mut Vec<InlineSpan>, cur: &mut String, b: bool, it: bool, c: bool| {
        if !cur.is_empty() {
            spans.push(InlineSpan {
                text: std::mem::take(cur),
                bold: b,
                italic: it,
                code: c,
            });
        }
    };

    while i < len {
        let ch = chars[i];

        // Инлайн код: `...`
        if ch == '`' {
            flush(&mut spans, &mut current, bold, italic, code);
            code = !code;
            i += 1;
            continue;
        }

        if !code {
            // ***жирный курсив***
            if i + 2 < len && chars[i] == '*' && chars[i + 1] == '*' && chars[i + 2] == '*' {
                if bold && italic {
                    if i > 0 && !chars[i - 1].is_whitespace() {
                        flush(&mut spans, &mut current, bold, italic, code);
                        bold = false;
                        italic = false;
                        i += 3;
                        continue;
                    }
                } else if !bold && !italic {
                    if i + 3 < len && !chars[i + 3].is_whitespace() {
                        flush(&mut spans, &mut current, bold, italic, code);
                        bold = true;
                        italic = true;
                        i += 3;
                        continue;
                    }
                }
            }

            // **жирный** или __жирный__
            let is_double_star = i + 1 < len && chars[i] == '*' && chars[i + 1] == '*';
            let is_double_under = i + 1 < len && chars[i] == '_' && chars[i + 1] == '_';
            if is_double_star || is_double_under {
                if bold {
                    if i > 0 && !chars[i - 1].is_whitespace() {
                        flush(&mut spans, &mut current, bold, italic, code);
                        bold = false;
                        i += 2;
                        continue;
                    }
                } else if i + 2 < len && !chars[i + 2].is_whitespace() {
                    flush(&mut spans, &mut current, bold, italic, code);
                    bold = true;
                    i += 2;
                    continue;
                }
            }

            // *курсив* или _курсив_
            let is_single_star = chars[i] == '*';
            let is_single_under = chars[i] == '_' && !(i > 0 && chars[i - 1].is_alphanumeric() && i + 1 < len && chars[i + 1].is_alphanumeric());
            if is_single_star || is_single_under {
                if italic {
                    if i > 0 && !chars[i - 1].is_whitespace() {
                        flush(&mut spans, &mut current, bold, italic, code);
                        italic = false;
                        i += 1;
                        continue;
                    }
                } else if i + 1 < len && !chars[i + 1].is_whitespace() {
                    flush(&mut spans, &mut current, bold, italic, code);
                    italic = true;
                    i += 1;
                    continue;
                }
            }
        }

        current.push(ch);
        i += 1;
    }

    flush(&mut spans, &mut current, bold, italic, code);
    spans
}

/// Размещение инлайн спанов с автоматическим переносом по словам
fn layout_spans(
    renderer: &TextRenderer,
    spans: &[InlineSpan],
    layout: &mut MarkdownLayout,
    indent_x: f32,
    max_w: f32,
    cur_y: &mut f32,
    base_size: f32,
    line_h: f32,
    theme: &ThemeConfig,
) {
    let mut cur_x = indent_x;
    let mut has_content = false;

    for span in spans {
        let style = if span.code {
            FontStyle::Mono
        } else if span.bold {
            FontStyle::Bold
        } else {
            FontStyle::Regular
        };

        let size = if span.code { base_size - 0.5 } else { base_size };

        let color = if span.code {
            [235, 240, 252, 255]
        } else if span.bold {
            [255, 255, 255, 255]
        } else if span.italic {
            [210, 220, 235, 240]
        } else {
            theme.text_primary
        };

        let bg_color = if span.code {
            Some([38, 42, 52, 220])
        } else {
            None
        };

        // Токенизация на слова и пробелы
        let mut tokens = Vec::new();
        let mut cur_token = String::new();
        let mut is_ws = false;

        for c in span.text.chars() {
            if c.is_whitespace() {
                if !is_ws && !cur_token.is_empty() {
                    tokens.push((false, std::mem::take(&mut cur_token)));
                }
                is_ws = true;
                cur_token.push(c);
            } else {
                if is_ws && !cur_token.is_empty() {
                    tokens.push((true, std::mem::take(&mut cur_token)));
                }
                is_ws = false;
                cur_token.push(c);
            }
        }
        if !cur_token.is_empty() {
            tokens.push((is_ws, cur_token));
        }

        for (ws, text) in tokens {
            if ws {
                if cur_x > indent_x {
                    let ws_w = renderer.measure_text_style(&text, size, style);
                    cur_x += ws_w;
                }
            } else {
                let word_w = renderer.measure_text_style(&text, size, style);
                if cur_x + word_w > max_w && cur_x > indent_x {
                    cur_x = indent_x;
                    *cur_y += line_h;
                }

                layout.spans.push(VisualSpan {
                    text,
                    style,
                    size,
                    color,
                    bg_color,
                    x: cur_x,
                    y: *cur_y + size,
                    width: word_w,
                });

                cur_x += word_w;
                has_content = true;
            }
        }
    }

    if has_content {
        *cur_y += line_h;
    }
}

/// Вычисление визуальной разметки Markdown для заданной ширины
pub fn layout_markdown(
    renderer: &TextRenderer,
    text: &str,
    max_w: f32,
    theme: &ThemeConfig,
) -> MarkdownLayout {
    let mut layout = MarkdownLayout::default();
    let mut cur_y: f32 = 6.0;

    let mut in_code_block = false;
    let mut code_start_y: f32 = 0.0;

    for line in text.lines() {
        let trimmed = line.trim();

        // 1. Блок кода ```
        if trimmed.starts_with("```") {
            if in_code_block {
                // Завершение блока кода
                let code_h = ((cur_y - code_start_y + 4.0) as f32).max(20.0);
                layout.rects.push(VisualRect {
                    x: 0.0,
                    y: code_start_y - 2.0,
                    w: max_w,
                    h: code_h,
                    color: [16, 18, 23, 255],
                    border_color: Some([42, 46, 58, 220]),
                    radius: 6.0,
                });
                cur_y += 6.0;
                in_code_block = false;
            } else {
                // Начало блока кода
                in_code_block = true;
                cur_y += 4.0;
                code_start_y = cur_y;
            }
            continue;
        }

        if in_code_block {
            let code_size = 11.0;
            let code_line_h = 16.0;
            let code_w = renderer.measure_text_style(line, code_size, FontStyle::Mono);
            layout.spans.push(VisualSpan {
                text: line.to_string(),
                style: FontStyle::Mono,
                size: code_size,
                color: [220, 226, 238, 245],
                bg_color: None,
                x: 10.0,
                y: cur_y + code_size,
                width: code_w,
            });
            cur_y += code_line_h;
            continue;
        }

        // 2. Пустая строка
        if trimmed.is_empty() {
            cur_y += 6.0;
            continue;
        }

        // 3. Горизонтальный разделитель (---, ***, ___)
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            cur_y += 4.0;
            layout.lines.push(VisualLine {
                x1: 0.0,
                y1: cur_y,
                x2: max_w,
                y2: cur_y,
                color: [theme.border_color[0], theme.border_color[1], theme.border_color[2], 160],
                width: 1.0,
            });
            cur_y += 8.0;
            continue;
        }

        // 4. Заголовки (#, ##, ###, ####)
        if trimmed.starts_with('#') {
            let mut level = 0;
            for c in trimmed.chars() {
                if c == '#' {
                    level += 1;
                } else {
                    break;
                }
            }
            if level <= 4 && trimmed.chars().nth(level) == Some(' ') {
                let h_text = trimmed[level + 1..].trim();
                let (h_size, h_line_h, top_gap, bot_gap, h_color) = match level {
                    1 => (15.0, 20.0, 8.0, 4.0, theme.accent_color),
                    2 => (14.0, 19.0, 6.0, 3.0, [255, 255, 255, 255]),
                    3 => (13.0, 18.0, 5.0, 2.0, [240, 245, 255, 255]),
                    _ => (12.0, 17.0, 4.0, 2.0, [230, 235, 245, 255]),
                };

                cur_y += top_gap;
                let spans = vec![InlineSpan {
                    text: h_text.to_string(),
                    bold: true,
                    italic: false,
                    code: false,
                }];
                layout_spans(renderer, &spans, &mut layout, 0.0, max_w, &mut cur_y, h_size, h_line_h, theme);
                // Подкрашиваем спаны заголовка
                if let Some(last_span) = layout.spans.last_mut() {
                    last_span.color = h_color;
                }
                cur_y += bot_gap;
                continue;
            }
        }

        // 5. Цитата (> ...)
        if trimmed.starts_with("> ") || trimmed == ">" {
            let q_text = if trimmed.len() > 2 { &trimmed[2..] } else { "" };
            let q_start_y = cur_y;
            let spans = parse_inline_spans(q_text);
            layout_spans(renderer, &spans, &mut layout, 14.0, max_w, &mut cur_y, 12.0, 18.0, theme);
            let bar_h = (cur_y - q_start_y).max(16.0);
            layout.rects.push(VisualRect {
                x: 2.0,
                y: q_start_y + 1.0,
                w: 3.0,
                h: bar_h,
                color: theme.accent_color,
                border_color: None,
                radius: 1.5,
            });
            cur_y += 3.0;
            continue;
        }

        // 6. Маркированный список (* ..., - ..., • ...)
        let leading_spaces = line.chars().take_while(|c| *c == ' ').count();
        let line_no_indent = &line[leading_spaces..];
        let is_bullet = line_no_indent.starts_with("* ")
            || line_no_indent.starts_with("- ")
            || line_no_indent.starts_with("• ");

        if is_bullet {
            let depth = (leading_spaces / 2).min(3) as f32;
            let bullet_x = depth * 14.0 + 4.0;
            let text_indent = depth * 14.0 + 16.0;

            let item_text = if line_no_indent.starts_with("• ") {
                &line_no_indent[4..] // "•" is 3 bytes in UTF-8 + space
            } else {
                &line_no_indent[2..]
            };

            layout.bullets.push(VisualBullet {
                cx: bullet_x + 3.0,
                cy: cur_y + 6.0,
                radius: 2.2,
                color: theme.accent_color,
            });

            let spans = parse_inline_spans(item_text);
            layout_spans(renderer, &spans, &mut layout, text_indent, max_w, &mut cur_y, 12.0, 18.0, theme);
            cur_y += 2.0;
            continue;
        }

        // 7. Нумерованный список (1. ..., 2. ...)
        let mut is_numbered = false;
        let mut num_end = 0;
        for (idx, c) in line_no_indent.char_indices() {
            if c.is_ascii_digit() {
                continue;
            }
            if c == '.' && idx > 0 && line_no_indent[idx + 1..].starts_with(' ') {
                is_numbered = true;
                num_end = idx + 2;
            }
            break;
        }

        if is_numbered {
            let depth = (leading_spaces / 2).min(3) as f32;
            let num_x = depth * 14.0 + 2.0;
            let text_indent = depth * 14.0 + 22.0;
            let num_str = &line_no_indent[..num_end - 1]; // e.g. "1."
            let item_text = &line_no_indent[num_end..];

            let num_w = renderer.measure_text_style(num_str, 12.0, FontStyle::Bold);
            layout.spans.push(VisualSpan {
                text: num_str.to_string(),
                style: FontStyle::Bold,
                size: 12.0,
                color: theme.accent_color,
                bg_color: None,
                x: num_x,
                y: cur_y + 12.0,
                width: num_w,
            });

            let spans = parse_inline_spans(item_text);
            layout_spans(renderer, &spans, &mut layout, text_indent, max_w, &mut cur_y, 12.0, 18.0, theme);
            cur_y += 2.0;
            continue;
        }

        // 8. Обычный абзац
        let spans = parse_inline_spans(trimmed);
        layout_spans(renderer, &spans, &mut layout, 0.0, max_w, &mut cur_y, 12.0, 18.0, theme);
        cur_y += 3.0;
    }

    layout.total_height = cur_y + 6.0;
    layout
}

/// Рендеринг подготовленного Markdown с поддержкой скроллинга и клиппинга
pub fn render_markdown(
    renderer: &TextRenderer,
    layout: &MarkdownLayout,
    pixmap: &mut PixmapMut,
    origin_x: f32,
    origin_y: f32,
    clip_rect: [f32; 4], // [min_x, min_y, max_x, max_y]
    scroll_offset: f32,
    selection: Option<((f32, f32), (f32, f32))>,
) {
    let (c_min_x, c_min_y, c_max_x, c_max_y) = (clip_rect[0], clip_rect[1], clip_rect[2], clip_rect[3]);

    let sel_bounds = selection.and_then(|(s, e)| {
        let min_y = s.1.min(e.1);
        let max_y = s.1.max(e.1);
        let min_x = s.0.min(e.0);
        let max_x = s.0.max(e.0);
        if (max_y - min_y).abs() >= 4.0 || (max_x - min_x).abs() >= 4.0 {
            Some((s, e, min_x, min_y, max_x, max_y))
        } else {
            None
        }
    });

    // 1. Прямоугольники (блоки кода, вертикальные полосы цитат)
    for rect in &layout.rects {
        let sy = origin_y + rect.y - scroll_offset;
        let ey = sy + rect.h;
        if ey < c_min_y || sy > c_max_y {
            continue;
        }

        let draw_x = (origin_x + rect.x).max(c_min_x);
        let draw_w = (rect.w - (draw_x - (origin_x + rect.x))).min(c_max_x - draw_x);
        let draw_y = sy.max(c_min_y);
        let draw_h = (rect.h - (draw_y - sy)).min(c_max_y - draw_y);

        if draw_w > 0.0 && draw_h > 0.0 {
            if let Some(p) = build_rounded_rect(draw_x, draw_y, draw_w, draw_h, rect.radius) {
                let mut paint = Paint::default();
                paint.set_color_rgba8(rect.color[0], rect.color[1], rect.color[2], rect.color[3]);
                paint.anti_alias = true;
                pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);

                if let Some(bc) = rect.border_color {
                    let mut bpaint = Paint::default();
                    bpaint.set_color_rgba8(bc[0], bc[1], bc[2], bc[3]);
                    bpaint.anti_alias = true;
                    let stroke = Stroke { width: 1.0, ..Stroke::default() };
                    pixmap.stroke_path(&p, &bpaint, &stroke, Transform::identity(), None);
                }
            }
        }
    }

    // 2. Линии-разделители
    for line in &layout.lines {
        let sy1 = origin_y + line.y1 - scroll_offset;
        let sy2 = origin_y + line.y2 - scroll_offset;
        if sy1 < c_min_y || sy1 > c_max_y {
            continue;
        }

        let mut pb = PathBuilder::new();
        pb.move_to((origin_x + line.x1).max(c_min_x), sy1);
        pb.line_to((origin_x + line.x2).min(c_max_x), sy2);
        if let Some(p) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(line.color[0], line.color[1], line.color[2], line.color[3]);
            paint.anti_alias = true;
            let stroke = Stroke { width: line.width, ..Stroke::default() };
            pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
        }
    }

    // 3. Маркеры списков (буллеты)
    for bullet in &layout.bullets {
        let scy = origin_y + bullet.cy - scroll_offset;
        if scy + bullet.radius < c_min_y || scy - bullet.radius > c_max_y {
            continue;
        }

        let mut pb = PathBuilder::new();
        pb.push_circle(origin_x + bullet.cx, scy, bullet.radius);
        if let Some(p) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(bullet.color[0], bullet.color[1], bullet.color[2], bullet.color[3]);
            paint.anti_alias = true;
            pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    // 4. Текстовые спаны
    for span in &layout.spans {
        let sx = origin_x + span.x;
        let sy = origin_y + span.y - scroll_offset;

        // Быстрая вертикальная отсечка
        if sy + 8.0 < c_min_y || sy - span.size - 6.0 > c_max_y {
            continue;
        }

        // Инлайн фон (например, для `кода`)
        if let Some(bg) = span.bg_color {
            let bg_y = sy - span.size + 1.0;
            let bg_h = span.size + 3.0;
            let bg_x = sx - 2.0;
            let bg_w = span.width + 4.0;
            if bg_y + bg_h >= c_min_y && bg_y <= c_max_y {
                let draw_x = bg_x.max(c_min_x);
                let draw_w = (bg_w - (draw_x - bg_x)).min(c_max_x - draw_x);
                let draw_y = bg_y.max(c_min_y);
                let draw_h = (bg_h - (draw_y - bg_y)).min(c_max_y - draw_y);
                if draw_w > 0.0 && draw_h > 0.0 {
                    if let Some(p) = build_rounded_rect(draw_x, draw_y, draw_w, draw_h, 3.0) {
                        let mut paint = Paint::default();
                        paint.set_color_rgba8(bg[0], bg[1], bg[2], bg[3]);
                        paint.anti_alias = true;
                        pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
                    }
                }
            }
        }

        // Подсветка выделенного фрагмента текста
        if let Some((s, e, _, _, _, _)) = sel_bounds {
            let line_top = span.y - span.size - 3.0;
            let line_bot = span.y + 5.0;
            let span_left = span.x;
            let span_right = span.x + span.width;

            let forward = s.1 < e.1 || ((s.1 - e.1).abs() <= span.size && s.0 <= e.0);
            let (start_pt, end_pt) = if forward { (s, e) } else { (e, s) };

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
                let sel_y = sy - span.size;
                let sel_h = span.size + 4.0;
                let sel_x = sx - 1.0;
                let sel_w = span.width + 2.0;
                if sel_y + sel_h >= c_min_y && sel_y <= c_max_y {
                    let draw_x = sel_x.max(c_min_x);
                    let draw_w = (sel_w - (draw_x - sel_x)).min(c_max_x - draw_x);
                    let draw_y = sel_y.max(c_min_y);
                    let draw_h = (sel_h - (draw_y - sel_y)).min(c_max_y - draw_y);
                    if draw_w > 0.0 && draw_h > 0.0 {
                        if let Some(p) = build_rounded_rect(draw_x, draw_y, draw_w, draw_h, 2.0) {
                            let mut paint = Paint::default();
                            paint.set_color_rgba8(64, 134, 244, 110);
                            paint.anti_alias = true;
                            pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
                        }
                    }
                }
            }
        }

        renderer.draw_text_styled_clipped(
            pixmap,
            &span.text,
            sx,
            sy,
            span.size,
            span.color,
            span.style,
            Some(clip_rect),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ThemeConfig;
    use crate::render::text::TextRenderer;

    #[test]
    fn test_markdown_layout_and_render() {
        let renderer = TextRenderer::new();
        let theme = ThemeConfig::default();
        let text = "# Заголовок\n\nЭто **жирный** текст и *курсив*, а также `код`.\n- Пункт 1\n- Пункт 2";

        let layout = layout_markdown(&renderer, text, 400.0, &theme);
        assert!(!layout.spans.is_empty(), "Markdown should produce spans");
        assert!(layout.total_height > 0.0, "Total height should be positive");

        let mut pixmap = Pixmap::new(400, 300).expect("Failed to create pixmap");
        let clip_rect = [0.0, 0.0, 400.0, 300.0];
        render_markdown(
            &renderer,
            &layout,
            &mut pixmap.as_mut(),
            10.0,
            10.0,
            clip_rect,
            0.0,
            None,
        );

        let has_non_zero = pixmap.data().iter().any(|&b| b > 0);
        assert!(has_non_zero, "Pixels should be drawn for rendered markdown");
    }
}

