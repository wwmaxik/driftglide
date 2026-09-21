use std::path::PathBuf;
use std::process::Command;
use tracing::{error, info, warn};
use crate::config::Config;
use crate::render::BoundingBox;

pub struct CircleToSearch {
    config: Config,
    pub active: bool,
    pub is_drawing: bool,
    /// Исходный скриншот в формате RGBA (для сохранения кропа в PNG)
    pub screenshot_rgba: Option<Vec<u8>>,
    /// Исходный скриншот в формате Wayland ARGB8888
    pub screenshot_argb: Option<Vec<u8>>,
    /// Предварительно затемненный скриншот в формате Wayland ARGB8888 (готов к отрисовке)
    pub base_scrim_argb: Option<Vec<u8>>,
    pub screen_width: u32,
    pub screen_height: u32,
    pub lasso_points: Vec<(f32, f32)>,
    pub current_bbox: Option<BoundingBox>,
}

impl CircleToSearch {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            active: false,
            is_drawing: false,
            screenshot_rgba: None,
            screenshot_argb: None,
            base_scrim_argb: None,
            screen_width: 0,
            screen_height: 0,
            lasso_points: Vec::new(),
            current_bbox: None,
        }
    }

    /// Активация режима поиска с оптимизированной однократной подготовкой буферов
    pub fn start(&mut self, mut screenshot_rgba: Vec<u8>, width: u32, height: u32) {
        self.active = true;
        self.is_drawing = false;
        self.screen_width = width;
        self.screen_height = height;
        self.lasso_points.clear();
        self.current_bbox = None;

        let total_pixels = (width * height) as usize;
        let mut argb = vec![0u8; total_pixels * 4];
        let mut scrim = vec![0u8; total_pixels * 4];

        // Однократная быстрая конвертация RGBA -> ARGB8888 и создание деликатно затемненного фона
        for (src, (dest_argb, dest_scrim)) in screenshot_rgba
            .chunks_exact_mut(4)
            .zip(argb.chunks_exact_mut(4).zip(scrim.chunks_exact_mut(4)))
        {
            src[3] = 255;
            let r = src[0];
            let g = src[1];
            let b = src[2];

            // Little-endian ARGB8888: [B, G, R, A]
            dest_argb[0] = b;
            dest_argb[1] = g;
            dest_argb[2] = r;
            dest_argb[3] = 255;

            // Деликатное затемнение: 85% исходной яркости для четкой читаемости экрана
            dest_scrim[0] = ((b as u16 * 215) / 255) as u8;
            dest_scrim[1] = ((g as u16 * 215) / 255) as u8;
            dest_scrim[2] = ((r as u16 * 215) / 255) as u8;
            dest_scrim[3] = 255;
        }

        self.screenshot_rgba = Some(screenshot_rgba);
        self.screenshot_argb = Some(argb);
        self.base_scrim_argb = Some(scrim);

        info!("Активирован режим Circle to Search ({}x{}, буферы подготовлены)", width, height);
    }

    /// Завершение и освобождение тяжелых буферов
    pub fn stop(&mut self) {
        self.active = false;
        self.is_drawing = false;
        self.screenshot_rgba = None;
        self.screenshot_argb = None;
        self.base_scrim_argb = None;
        self.lasso_points.clear();
        self.current_bbox = None;
        info!("Деактивирован режим Circle to Search");
    }

    /// Добавление точки траектории пальца с прореживанием (decimation) для устранения лагов
    pub fn add_point(&mut self, x: f32, y: f32) -> bool {
        if let Some(&(last_x, last_y)) = self.lasso_points.last() {
            let dx = x - last_x;
            let dy = y - last_y;
            // Пропускаем микродвижения меньше 4 пикселей
            if dx * dx + dy * dy < 16.0 {
                return false;
            }
        }

        self.lasso_points.push((x, y));
        self.current_bbox = BoundingBox::from_points(&self.lasso_points);
        true
    }

    /// Завершение обводки: вырезание области и сохранение
    pub fn finish_selection(&mut self) -> Option<PathBuf> {
        let bbox = self.current_bbox?;
        let screenshot = self.screenshot_rgba.as_ref()?;

        let x1 = (bbox.min_x.max(0.0) as u32).min(self.screen_width);
        let x2 = (bbox.max_x.max(0.0) as u32).min(self.screen_width);
        let y1 = (bbox.min_y.max(0.0) as u32).min(self.screen_height);
        let y2 = (bbox.max_y.max(0.0) as u32).min(self.screen_height);

        let crop_w = x2.saturating_sub(x1);
        let crop_h = y2.saturating_sub(y1);

        if crop_w < 12 || crop_h < 12 {
            warn!("Выделенная область слишком мала: {}x{}", crop_w, crop_h);
            return None;
        }

        // Вырезаем пиксели RGBA
        let mut crop_data = vec![0u8; (crop_w * crop_h * 4) as usize];
        for y in 0..crop_h {
            let src_y = y1 + y;
            let src_start = ((src_y * self.screen_width + x1) * 4) as usize;
            let src_end = src_start + (crop_w * 4) as usize;

            let dest_start = ((y * crop_w) * 4) as usize;
            let dest_end = dest_start + (crop_w * 4) as usize;

            if src_end <= screenshot.len() && dest_end <= crop_data.len() {
                crop_data[dest_start..dest_end].copy_from_slice(&screenshot[src_start..src_end]);
            }
        }

        // Гарантируем, что альфа-канал кропа всегда непрозрачен (255)
        for chunk in crop_data.chunks_exact_mut(4) {
            chunk[3] = 255;
        }

        // Сохраняем кроп во временный PNG-файл
        let cache_dir = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(|h| PathBuf::from(h).join(".cache"))
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
            })
            .join("driftglide");

        let _ = std::fs::create_dir_all(&cache_dir);
        let crop_path = cache_dir.join("last_crop.png");

        if let Some(img) = image::RgbaImage::from_raw(crop_w, crop_h, crop_data) {
            if let Err(e) = img.save(&crop_path) {
                error!("Не удалось сохранить кроп изображения: {}", e);
                return None;
            }
            info!("Кроп успешно сохранен в {:?}", crop_path);

            // Копируем изображение в буфер обмена (wl-copy)
            if let Ok(file) = std::fs::File::open(&crop_path) {
                let _ = Command::new("wl-copy")
                    .arg("-t")
                    .arg("image/png")
                    .stdin(file)
                    .spawn();
            }

            // Запускаем пользовательский хук, если он существует
            if let Some(script) = &self.config.search_hook_script {
                if script.exists() {
                    let img_arg = crop_path.to_string_lossy().to_string();
                    let _ = Command::new(script).arg(img_arg).spawn();
                }
            }

            Some(crop_path)
        } else {
            error!("Ошибка формирования RgbaImage из буфера кропа");
            None
        }
    }
}
