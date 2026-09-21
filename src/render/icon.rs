use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tiny_skia::*;
use tracing::debug;

pub struct IconManager {
    cache: HashMap<String, Option<Pixmap>>,
    search_dirs: Vec<PathBuf>,
}

impl IconManager {
    pub fn new() -> Self {
        let mut search_dirs = Vec::new();

        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            let local_icons = home_path.join(".local/share/icons");
            search_dirs.push(local_icons.join("hicolor/48x48/apps"));
            search_dirs.push(local_icons.join("hicolor/64x64/apps"));
            search_dirs.push(local_icons.join("hicolor/scalable/apps"));
            search_dirs.push(local_icons);

            let dot_icons = home_path.join(".icons");
            for theme in &["Flat-Remix-Blue-Dark", "Flat-Remix-Blue-Light"] {
                search_dirs.push(dot_icons.join(theme).join("apps/scalable"));
                search_dirs.push(dot_icons.join(theme).join("apps/48"));
                search_dirs.push(dot_icons.join(theme).join("apps/64"));
            }
        }

        for theme in &["Papirus", "Papirus-Dark", "Papirus-Light", "breeze", "breeze-dark", "Adwaita", "hicolor"] {
            let base = PathBuf::from(format!("/usr/share/icons/{}", theme));
            search_dirs.push(base.join("48x48/apps"));
            search_dirs.push(base.join("64x64/apps"));
            search_dirs.push(base.join("scalable/apps"));
            search_dirs.push(base.join("apps/48"));
            search_dirs.push(base.join("apps/64"));
            search_dirs.push(base.join("apps/scalable"));
        }
        search_dirs.push(PathBuf::from("/usr/share/pixmaps"));

        Self {
            cache: HashMap::new(),
            search_dirs,
        }
    }

    /// Поиск и загрузка иконки приложения по его app_id
    pub fn get_icon(&mut self, app_id: &str, target_size: u32) -> Option<&Pixmap> {
        let key = format!("{}:{}", app_id.to_lowercase(), target_size);
        if !self.cache.contains_key(&key) {
            let pixmap = self.load_icon(app_id, target_size);
            self.cache.insert(key.clone(), pixmap);
        }
        self.cache.get(&key).and_then(|opt| opt.as_ref())
    }

    fn load_icon(&self, app_id: &str, target_size: u32) -> Option<Pixmap> {
        let icon_path = self.find_icon_path(app_id)?;
        debug!("Загрузка иконки для {}: {:?}", app_id, icon_path);

        if icon_path.extension().and_then(|s| s.to_str()) == Some("svg") {
            return Self::load_svg(&icon_path, target_size);
        }

        let img = image::open(&icon_path).ok()?;
        let rgba_img = img.to_rgba8();

        // Масштабируем до target_size
        let resized = if rgba_img.width() != target_size || rgba_img.height() != target_size {
            image::imageops::resize(
                &rgba_img,
                target_size,
                target_size,
                image::imageops::FilterType::Triangle,
            )
        } else {
            rgba_img
        };

        let mut pixmap = Pixmap::new(target_size, target_size)?;
        let raw_data = resized.into_raw();

        // Копируем с конвертацией в premultiplied RGBA для tiny-skia
        for (i, chunk) in raw_data.chunks_exact(4).enumerate() {
            let r = chunk[0];
            let g = chunk[1];
            let b = chunk[2];
            let a = chunk[3];
            let pm = PremultipliedColorU8::from_rgba(r, g, b, a)
                .unwrap_or(PremultipliedColorU8::TRANSPARENT);
            pixmap.pixels_mut()[i] = pm;
        }

        Some(pixmap)
    }

    fn load_svg(path: &Path, target_size: u32) -> Option<Pixmap> {
        let data = std::fs::read(path).ok()?;
        let opt = resvg::usvg::Options::default();
        let tree = resvg::usvg::Tree::from_data(&data, &opt).ok()?;

        let tw = tree.size().width();
        let th = tree.size().height();
        if tw <= 0.0 || th <= 0.0 {
            return None;
        }

        let mut pixmap = Pixmap::new(target_size, target_size)?;
        let sx = target_size as f32 / tw;
        let sy = target_size as f32 / th;
        let scale = sx.min(sy);
        let tx = (target_size as f32 - tw * scale) / 2.0;
        let ty = (target_size as f32 - th * scale) / 2.0;

        let transform = Transform::from_scale(scale, scale).post_translate(tx, ty);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        Some(pixmap)
    }

    fn find_icon_path(&self, app_id: &str) -> Option<PathBuf> {
        let lower = app_id.to_lowercase();
        let stripped = lower.split('.').last().unwrap_or(&lower);

        let mut base_names = vec![
            lower.clone(),
            stripped.to_string(),
            app_id.to_string(),
        ];

        // Специальные маппинги для популярных приложений
        if lower.contains("foot") {
            base_names.insert(0, "foot".into());
            base_names.insert(1, "utilities-terminal".into());
        }
        if lower.contains("firefox") {
            base_names.insert(0, "firefox".into());
            base_names.insert(1, "firefox-esr".into());
        }
        if lower.contains("code") {
            base_names.insert(0, "code".into());
            base_names.insert(1, "vscode".into());
            base_names.insert(2, "com.visualstudio.code".into());
        }
        if lower.contains("thunar") {
            base_names.insert(0, "thunar".into());
            base_names.insert(1, "org.xfce.thunar".into());
            base_names.insert(2, "system-file-manager".into());
        }
        if lower.contains("term") || lower.contains("alacritty") || lower.contains("kitty") {
            base_names.push("utilities-terminal".into());
            base_names.push("terminal".into());
            base_names.push("alacritty".into());
            base_names.push("kitty".into());
        }

        // Также пробуем прочитать Icon= из .desktop файла
        if let Some(desktop_icon) = self.find_icon_from_desktop(&lower, stripped) {
            if desktop_icon.starts_with('/') {
                let p = PathBuf::from(&desktop_icon);
                if p.is_file() {
                    return Some(p);
                }
            }
            base_names.insert(0, desktop_icon);
        }

        for dir in &self.search_dirs {
            for name in &base_names {
                let svg_p = dir.join(format!("{}.svg", name));
                if svg_p.is_file() {
                    return Some(svg_p);
                }
                let png_p = dir.join(format!("{}.png", name));
                if png_p.is_file() {
                    return Some(png_p);
                }
                let exact_p = dir.join(name);
                if exact_p.is_file() {
                    return Some(exact_p);
                }
            }
        }

        None
    }

    fn find_icon_from_desktop(&self, lower: &str, stripped: &str) -> Option<String> {
        let mut desktop_dirs = vec![
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            desktop_dirs.push(PathBuf::from(home).join(".local/share/applications"));
        }

        for dir in &desktop_dirs {
            for cand in &[format!("{}.desktop", lower), format!("{}.desktop", stripped)] {
                let p = dir.join(cand);
                if let Ok(content) = std::fs::read_to_string(&p) {
                    for line in content.lines() {
                        if let Some(icon) = line.strip_prefix("Icon=") {
                            let icon_name = icon.trim();
                            if !icon_name.is_empty() {
                                return Some(icon_name.to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

