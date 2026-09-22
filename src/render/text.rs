use std::path::{Path, PathBuf};
use std::sync::Arc;
use fontdue::{Font, FontSettings};
use tiny_skia::PixmapMut;
use tracing::{info, warn};

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
    fallback_fonts: Vec<Arc<Font>>,
}

impl TextRenderer {
    pub fn new() -> Self {
        let regular_candidates = [
            // Liberation Sans (Fedora, Arch, RHEL, openSUSE, Debian/Ubuntu)
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation-fonts/LiberationSans-Regular.ttf",
            // Noto Sans
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/google-noto-vf/NotoSans[wght].ttf",
            // DejaVu Sans
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            // Adwaita Sans (GNOME 45+)
            "/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf",
            // GNU FreeSans
            "/usr/share/fonts/gnu-free/FreeSans.otf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
            // Cantarell
            "/usr/share/fonts/cantarell/Cantarell-Regular.otf",
            "/usr/share/fonts/cantarell/Cantarell-VF.otf",
            // Ubuntu font
            "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
        ];

        let bold_candidates = [
            // Liberation Sans Bold
            "/usr/share/fonts/liberation/LiberationSans-Bold.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
            "/usr/share/fonts/truetype/LiberationSans-Bold.ttf",
            "/usr/share/fonts/liberation-fonts/LiberationSans-Bold.ttf",
            // Noto Sans Bold
            "/usr/share/fonts/noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/google-noto-vf/NotoSans[wght].ttf",
            // DejaVu Sans Bold
            "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
            // GNU FreeSans Bold
            "/usr/share/fonts/gnu-free/FreeSansBold.otf",
            "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf",
            // Cantarell Bold
            "/usr/share/fonts/cantarell/Cantarell-Bold.otf",
            "/usr/share/fonts/cantarell/Cantarell-VF.otf",
            // Ubuntu font
            "/usr/share/fonts/truetype/ubuntu/Ubuntu-B.ttf",
        ];

        let mono_candidates = [
            // Liberation Mono
            "/usr/share/fonts/liberation/LiberationMono-Regular.ttf",
            "/usr/share/fonts/liberation-mono/LiberationMono-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
            "/usr/share/fonts/truetype/LiberationMono-Regular.ttf",
            "/usr/share/fonts/liberation-fonts/LiberationMono-Regular.ttf",
            // Noto Sans Mono
            "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoMono-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSansMono-Regular.ttf",
            "/usr/share/fonts/google-noto-vf/NotoSansMono[wght].ttf",
            // DejaVu Sans Mono
            "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
            // Adwaita Mono
            "/usr/share/fonts/Adwaita/AdwaitaMono-Regular.ttf",
            // GNU FreeMono
            "/usr/share/fonts/gnu-free/FreeMono.otf",
            "/usr/share/fonts/truetype/freefont/FreeMono.ttf",
            // Ubuntu Mono
            "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
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

        let find_font_fc = |pattern: &str| -> Option<Arc<Font>> {
            let output = std::process::Command::new("fc-match")
                .args(["-f", "%{file}", pattern])
                .output()
                .ok()?;
            if output.status.success() {
                let path_str = String::from_utf8(output.stdout).ok()?;
                let path = Path::new(path_str.trim());
                if path.is_file() {
                    if let Ok(data) = std::fs::read(path) {
                        if let Ok(f) = Font::from_bytes(data, FontSettings::default()) {
                            return Some(Arc::new(f));
                        }
                    }
                }
            }
            None
        };

        let scan_fonts_dir = |patterns: &[&str]| -> Option<Arc<Font>> {
            let mut search_dirs = vec![
                PathBuf::from("/usr/share/fonts"),
                PathBuf::from("/usr/local/share/fonts"),
            ];
            if let Some(home) = std::env::var_os("HOME") {
                let home_p = PathBuf::from(home);
                search_dirs.push(home_p.join(".local/share/fonts"));
                search_dirs.push(home_p.join(".fonts"));
            }

            fn walk_dir<F>(dir: &Path, depth: usize, cb: &mut F)
            where
                F: FnMut(&Path) -> bool,
            {
                if depth > 5 {
                    return;
                }
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            walk_dir(&path, depth + 1, cb);
                        } else if path.is_file() {
                            if cb(&path) {
                                return;
                            }
                        }
                    }
                }
            }

            let mut found_font = None;
            for dir in &search_dirs {
                if !dir.exists() {
                    continue;
                }
                walk_dir(dir, 0, &mut |p| {
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf") {
                        let filename = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                        if patterns.iter().any(|pat| filename.contains(pat)) {
                            if let Ok(data) = std::fs::read(p) {
                                if let Ok(f) = Font::from_bytes(data, FontSettings::default()) {
                                    found_font = Some(Arc::new(f));
                                    return true;
                                }
                            }
                        }
                    }
                    false
                });
                if found_font.is_some() {
                    return found_font;
                }
            }

            // Fallback: any valid ttf or otf font
            for dir in &search_dirs {
                if !dir.exists() {
                    continue;
                }
                walk_dir(dir, 0, &mut |p| {
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf") {
                        if let Ok(data) = std::fs::read(p) {
                            if let Ok(f) = Font::from_bytes(data, FontSettings::default()) {
                                found_font = Some(Arc::new(f));
                                return true;
                            }
                        }
                    }
                    false
                });
                if found_font.is_some() {
                    return found_font;
                }
            }

            None
        };

        let font_regular = load_first(&regular_candidates)
            .or_else(|| find_font_fc("sans-serif"))
            .or_else(|| scan_fonts_dir(&["sans", "noto", "dejavu", "liberation", "adwaita", "free"]));

        let font_bold = load_first(&bold_candidates)
            .or_else(|| find_font_fc("sans-serif:bold"))
            .or_else(|| scan_fonts_dir(&["bold"]))
            .or_else(|| font_regular.clone());

        let font_mono = load_first(&mono_candidates)
            .or_else(|| find_font_fc("monospace"))
            .or_else(|| scan_fonts_dir(&["mono"]))
            .or_else(|| font_regular.clone());

        let font_regular = font_regular.or_else(|| font_bold.clone()).or_else(|| font_mono.clone());

        // Загрузка дополнительных шрифтов-фоллбеков (символы, моноширинные, эмодзи)
        let mut fallback_fonts = Vec::new();
        let fallback_candidates = [
            "/usr/share/fonts/Adwaita/AdwaitaMono-Regular.ttf",
            "/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf",
            "/usr/share/fonts/gnu-free/FreeSerif.otf",
            "/usr/share/fonts/gnu-free/FreeSans.otf",
            "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
            "/usr/share/fonts/google-noto-color-emoji-fonts/NotoColorEmoji.ttf",
            "/usr/share/fonts/noto-color-emoji/NotoColorEmoji.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSerif.ttf",
        ];
        for path in &fallback_candidates {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(f) = Font::from_bytes(data, FontSettings::default()) {
                    fallback_fonts.push(Arc::new(f));
                }
            }
        }

        if font_regular.is_none() {
            warn!("TextRenderer: Не удалось обнаружить ни один системный шрифт TrueType/OpenType!");
        } else {
            info!("TextRenderer: Системные шрифты успешно инициализированы.");
        }

        Self {
            font_regular,
            font_bold,
            font_mono,
            fallback_fonts,
        }
    }

    pub fn get_font(&self, style: FontStyle) -> Option<&Arc<Font>> {
        match style {
            FontStyle::Bold => self.font_bold.as_ref().or(self.font_regular.as_ref()),
            FontStyle::Mono => self.font_mono.as_ref().or(self.font_regular.as_ref()),
            FontStyle::Regular => self.font_regular.as_ref().or(self.font_bold.as_ref()),
        }
    }

    /// Поиск подходящего шрифта для конкретного символа с каскадным фоллбеком
    pub fn get_font_for_char(&self, style: FontStyle, ch: char) -> Option<&Arc<Font>> {
        let primary = self.get_font(style);
        if let Some(f) = primary {
            if f.lookup_glyph_index(ch) != 0 {
                return Some(f);
            }
        }
        for fb in &self.fallback_fonts {
            if fb.lookup_glyph_index(ch) != 0 {
                return Some(fb);
            }
        }
        for other in [&self.font_regular, &self.font_bold, &self.font_mono].into_iter().flatten() {
            if other.lookup_glyph_index(ch) != 0 {
                return Some(other);
            }
        }
        primary
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
        if (baseline_y + size) < clip_min_y as f32 || (baseline_y - size * 1.5) > clip_max_y as f32 {
            return start_x + self.measure_text_style(text, size, style);
        }

        let mut pen_x = start_x;
        let w = pixmap.width() as i32;
        let h = pixmap.height() as i32;
        let data = pixmap.data_mut();

        for ch in text.chars() {
            let font_for_ch = self.get_font_for_char(style, ch).unwrap_or(font);
            let (metrics, bitmap) = font_for_ch.rasterize(ch, size);
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

        text.chars()
            .map(|c| {
                let font_for_c = self.get_font_for_char(style, c).unwrap_or(font);
                font_for_c.metrics(c, size).advance_width
            })
            .sum()
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

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Pixmap;

    #[test]
    fn test_text_renderer_loads_fonts() {
        let renderer = TextRenderer::new();
        assert!(renderer.get_font(FontStyle::Regular).is_some(), "Regular font should be loaded");
        assert!(renderer.get_font(FontStyle::Bold).is_some(), "Bold font should be loaded");
        assert!(renderer.get_font(FontStyle::Mono).is_some(), "Mono font should be loaded");
    }

    #[test]
    fn test_text_renderer_draws_text() {
        let renderer = TextRenderer::new();
        let mut pixmap = Pixmap::new(300, 100).expect("Failed to create pixmap");
        let mut pixmap_mut = pixmap.as_mut();

        let w = renderer.draw_text(
            &mut pixmap_mut,
            "Hello World!",
            10.0,
            40.0,
            16.0,
            [255, 255, 255, 255],
        );
        assert!(w > 10.0, "pen_x should advance");

        // Verify non-zero pixels were drawn
        let has_non_zero = pixmap.data().iter().any(|&b| b > 0);
        assert!(has_non_zero, "Pixels should be drawn for ASCII text");

        // Verify Cyrillic text
        let mut cyrillic_pixmap = Pixmap::new(300, 100).expect("Failed to create pixmap");
        let mut cyrillic_mut = cyrillic_pixmap.as_mut();
        let w_cyr = renderer.draw_text(
            &mut cyrillic_mut,
            "Фрагмент экрана",
            10.0,
            40.0,
            16.0,
            [255, 255, 255, 255],
        );
        assert!(w_cyr > 10.0, "pen_x should advance for Cyrillic");
        let cyr_non_zero = cyrillic_pixmap.data().iter().any(|&b| b > 0);
        assert!(cyr_non_zero, "Pixels should be drawn for Cyrillic text");
    }
}
