use std::sync::Arc;
use fontdue::{Font, FontSettings};
use tiny_skia::PixmapMut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    Regular,
    Bold,
    Mono,
}

/// Высокопроизводительный рендерер типографики на базе fontdue
pub struct TextRenderer {
    font_regular: Option<Arc<Font>>,
    font_bold: Option<Arc<Font>>,
    font_mono: Option<Arc<Font>>,
}

impl TextRenderer {
    pub fn new() -> Self {
        let regular_candidates = [
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ];

        let bold_candidates = [
            "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        ];

        let mono_candidates = [
            "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoMono-Regular.ttf",
        ];

        let load_first = |candidates: &[&str]| -> Option<Arc<Font>> {
            for path in candidates {
                if let Ok(data) = std::fs::read(path) {
                    if let Ok(f) = Font::from_bytes(data, FontSettings::default()) {
                        return Some(Arc::new(f));
                    }
                }
            }
            None
        };

        let font_regular = load_first(&regular_candidates);
        let font_bold = load_first(&bold_candidates).or_else(|| font_regular.clone());
        let font_mono = load_first(&mono_candidates).or_else(|| font_regular.clone());

        Self {
            font_regular,
            font_bold,
            font_mono,
        }
    }

    pub fn get_font(&self, style: FontStyle) -> Option<&Arc<Font>> {
        match style {
            FontStyle::Bold => self.font_bold.as_ref().or(self.font_regular.as_ref()),
            FontStyle::Mono => self.font_mono.as_ref().or(self.font_regular.as_ref()),
            FontStyle::Regular => self.font_regular.as_ref().or(self.font_bold.as_ref()),
        }
    }

    /// Отрисовка строки с заданным стилем и пиксельным клиппингом
    pub fn draw_text_styled_clipped(
        &self,
        pixmap: &mut PixmapMut,
        text: &str,
        start_x: f32,
        baseline_y: f32,
        size: f32,
        color: [u8; 4],
        style: FontStyle,
        clip: Option<[f32; 4]>, // [min_x, min_y, max_x, max_y]
    ) -> f32 {
        let font = match self.get_font(style) {
            Some(f) => f,
            None => return start_x,
        };

        let (clip_min_x, clip_min_y, clip_max_x, clip_max_y) = match clip {
            Some(c) => (c[0] as i32, c[1] as i32, c[2] as i32, c[3] as i32),
            None => (0, 0, pixmap.width() as i32, pixmap.height() as i32),
        };

        // Быстрая отсечка, если строка целиком вне вертикального диапазона
        if (baseline_y as i32) + 12 < clip_min_y || (baseline_y as i32) - (size as i32) - 8 > clip_max_y {
            return start_x + self.measure_text_style(text, size, style);
        }

        let mut pen_x = start_x;
        let w = pixmap.width() as i32;
        let h = pixmap.height() as i32;
        let data = pixmap.data_mut();

        for ch in text.chars() {
            let (metrics, bitmap) = font.rasterize(ch, size);
            let gx = pen_x as i32 + metrics.xmin;
            let gy = baseline_y as i32 - metrics.height as i32 - metrics.ymin;

            for row in 0..metrics.height {
                let py = gy + row as i32;
                if py < clip_min_y || py >= clip_max_y || py < 0 || py >= h {
                    continue;
                }
                for col in 0..metrics.width {
                    let px = gx + col as i32;
                    if px < clip_min_x || px >= clip_max_x || px < 0 || px >= w {
                        continue;
                    }

                    let coverage = bitmap[row * metrics.width + col];
                    if coverage == 0 {
                        continue;
                    }

                    let idx = ((py * w + px) * 4) as usize;
                    if idx + 4 <= data.len() {
                        let alpha = (color[3] as u32 * coverage as u32) / 255;
                        let inv_alpha = 255 - alpha;

                        let r = (color[0] as u32 * alpha) / 255;
                        let g = (color[1] as u32 * alpha) / 255;
                        let b = (color[2] as u32 * alpha) / 255;

                        data[idx] = (r + (data[idx] as u32 * inv_alpha) / 255) as u8;
                        data[idx + 1] = (g + (data[idx + 1] as u32 * inv_alpha) / 255) as u8;
                        data[idx + 2] = (b + (data[idx + 2] as u32 * inv_alpha) / 255) as u8;
                        data[idx + 3] = (alpha + (data[idx + 3] as u32 * inv_alpha) / 255) as u8;
                    }
                }
            }

            pen_x += metrics.advance_width;
        }

        pen_x
    }

    /// Отрисовка одной строки обычного текста
    pub fn draw_text(
        &self,
        pixmap: &mut PixmapMut,
        text: &str,
        start_x: f32,
        baseline_y: f32,
        size: f32,
        color: [u8; 4],
    ) -> f32 {
        self.draw_text_styled_clipped(
            pixmap,
            text,
            start_x,
            baseline_y,
            size,
            color,
            FontStyle::Regular,
            None,
        )
    }

    /// Измерение ширины текста с заданным стилем
    pub fn measure_text_style(&self, text: &str, size: f32, style: FontStyle) -> f32 {
        let font = match self.get_font(style) {
            Some(f) => f,
            None => return 0.0,
        };

        text.chars().map(|c| font.metrics(c, size).advance_width).sum()
    }

    /// Измерение ширины обычного текста
    #[allow(dead_code)]
    pub fn measure_text(&self, text: &str, size: f32) -> f32 {
        self.measure_text_style(text, size, FontStyle::Regular)
    }

    /// Отрисовка абзаца текста с автоматическим переносом слов по ширине
    #[allow(dead_code)]
    pub fn draw_wrapped_text(
        &self,
        pixmap: &mut PixmapMut,
        text: &str,
        x: f32,
        y: f32,
        max_w: f32,
        size: f32,
        line_height: f32,
        color: [u8; 4],
        max_lines: usize,
    ) -> f32 {
        let font = match self.get_font(FontStyle::Regular) {
            Some(f) => f,
            None => return y,
        };

        let mut cur_y = y + size;
        let mut cur_line = String::new();
        let mut cur_w = 0.0;
        let mut lines_rendered = 0;

        for line in text.lines() {
            if lines_rendered >= max_lines {
                break;
            }

            if line.is_empty() {
                cur_y += line_height * 0.6;
                continue;
            }

            for word in line.split(' ') {
                let space_w = font.metrics(' ', size).advance_width;
                let word_w: f32 = word.chars().map(|c| font.metrics(c, size).advance_width).sum();

                if cur_w + word_w > max_w && !cur_line.is_empty() {
                    self.draw_text(pixmap, &cur_line, x, cur_y, size, color);
                    cur_line.clear();
                    cur_w = 0.0;
                    cur_y += line_height;
                    lines_rendered += 1;
                    if lines_rendered >= max_lines {
                        break;
                    }
                }

                if !cur_line.is_empty() {
                    cur_line.push(' ');
                    cur_w += space_w;
                }
                cur_line.push_str(word);
                cur_w += word_w;
            }

            if !cur_line.is_empty() && lines_rendered < max_lines {
                self.draw_text(pixmap, &cur_line, x, cur_y, size, color);
                cur_line.clear();
                cur_w = 0.0;
                cur_y += line_height;
                lines_rendered += 1;
            }
        }

        cur_y
    }
}
