use tiny_skia::*;

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl BoundingBox {
    pub fn from_points(points: &[(f32, f32)]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }

        let mut min_x = points[0].0;
        let mut max_x = points[0].0;
        let mut min_y = points[0].1;
        let mut max_y = points[0].1;

        for &(x, y) in points.iter().skip(1) {
            if x < min_x { min_x = x; }
            if x > max_x { max_x = x; }
            if y < min_y { min_y = y; }
            if y > max_y { max_y = y; }
        }

        if (max_x - min_x) < 5.0 {
            min_x -= 2.5;
            max_x += 2.5;
        }
        if (max_y - min_y) < 5.0 {
            min_y -= 2.5;
            max_y += 2.5;
        }

        Some(Self { min_x, min_y, max_x, max_y })
    }

    pub fn width(&self) -> f32 {
        (self.max_x - self.min_x).max(1.0)
    }

    pub fn height(&self) -> f32 {
        (self.max_y - self.min_y).max(1.0)
    }
}

pub struct LassoRenderer;

impl LassoRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Быстрая отрисовка интерфейса Circle to Search
    pub fn render(
        &self,
        target_buffer: &mut [u8],
        width: u32,
        height: u32,
        src_width: u32,
        src_height: u32,
        base_scrim_argb: Option<&[u8]>,
        screenshot_argb: Option<&[u8]>,
        lasso_points: &[(f32, f32)],
        bbox: Option<&BoundingBox>,
    ) {
        let eff_src_w = if src_width > 0 { src_width } else { width };
        let eff_src_h = if src_height > 0 { src_height } else { height };
        let copy_w = width.min(eff_src_w);
        let copy_h = height.min(eff_src_h);
        let row_bytes = (copy_w * 4) as usize;
        let dst_stride = (width * 4) as usize;
        let src_stride = (eff_src_w * 4) as usize;

        // 1. Быстрое и безопасное построчное копирование замороженного экрана
        if let Some(scrim) = base_scrim_argb.or(screenshot_argb) {
            for y in 0..copy_h {
                let dst_offset = (y as usize) * dst_stride;
                let src_offset = (y as usize) * src_stride;
                if dst_offset + row_bytes <= target_buffer.len() && src_offset + row_bytes <= scrim.len() {
                    target_buffer[dst_offset..dst_offset + row_bytes]
                        .copy_from_slice(&scrim[src_offset..src_offset + row_bytes]);
                }
            }
        } else {
            // Элегантная темная подложка, если скриншот еще не готов
            target_buffer.fill(40);
        }

        // 2. Восстановление исходной 100% яркости внутри выделенного Bounding Box
        if let (Some(bb), Some(src_argb)) = (bbox, screenshot_argb) {
            let x1 = (bb.min_x.max(0.0) as u32).min(copy_w);
            let x2 = (bb.max_x.max(0.0) as u32).min(copy_w);
            let y1 = (bb.min_y.max(0.0) as u32).min(copy_h);
            let y2 = (bb.max_y.max(0.0) as u32).min(copy_h);

            let sel_row_bytes = (x2.saturating_sub(x1) * 4) as usize;
            if sel_row_bytes > 0 {
                for y in y1..y2 {
                    let dst_offset = ((y * width + x1) * 4) as usize;
                    let src_offset = ((y * eff_src_w + x1) * 4) as usize;
                    if dst_offset + sel_row_bytes <= target_buffer.len()
                        && src_offset + sel_row_bytes <= src_argb.len()
                    {
                        target_buffer[dst_offset..dst_offset + sel_row_bytes]
                            .copy_from_slice(&src_argb[src_offset..src_offset + sel_row_bytes]);
                    }
                }
            }
        }

        // 3. Отрисовка векторных элементов через tiny-skia
        let mut pixmap = match PixmapMut::from_bytes(target_buffer, width, height) {
            Some(p) => p,
            None => return,
        };

        // Тонкая элегантная рамка по периметру экрана (индикатор активного режима Circle to Search)
        if let Some(screen_rect) = Rect::from_xywh(1.0, 1.0, width as f32 - 2.0, height as f32 - 2.0) {
            let mut perimeter_paint = Paint::default();
            perimeter_paint.set_color_rgba8(100, 175, 255, 120);
            perimeter_paint.anti_alias = true;
            let stroke = Stroke { width: 2.0, ..Stroke::default() };
            let mut pb = PathBuilder::new();
            pb.push_rect(screen_rect);
            if let Some(p) = pb.finish() {
                pixmap.stroke_path(&p, &perimeter_paint, &stroke, Transform::identity(), None);
            }
        }

        // Рамка Bounding Box с мягким свечением
        if let Some(bb) = bbox {
            if let Some(rect) = Rect::from_xywh(bb.min_x, bb.min_y, bb.width(), bb.height()) {
                let mut border_paint = Paint::default();
                border_paint.set_color_rgba8(120, 195, 255, 230);
                border_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 2.0,
                    dash: StrokeDash::new(vec![6.0, 4.0], 0.0),
                    ..Stroke::default()
                };

                let mut pb = PathBuilder::new();
                pb.push_rect(rect);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &border_paint, &stroke, Transform::identity(), None);
                }
            }
        }

        // Светящаяся траектория лассо
        if lasso_points.len() >= 2 {
            let mut pb = PathBuilder::new();
            pb.move_to(lasso_points[0].0, lasso_points[0].1);
            for &(x, y) in lasso_points.iter().skip(1) {
                pb.line_to(x, y);
            }

            if let Some(path) = pb.finish() {
                // Внешнее свечение
                let mut glow_paint = Paint::default();
                glow_paint.set_color_rgba8(80, 160, 255, 120);
                glow_paint.anti_alias = true;
                let glow_stroke = Stroke {
                    width: 7.0,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Stroke::default()
                };
                pixmap.stroke_path(&path, &glow_paint, &glow_stroke, Transform::identity(), None);

                // Центральная белая линия
                let mut line_paint = Paint::default();
                line_paint.set_color_rgba8(255, 255, 255, 255);
                line_paint.anti_alias = true;
                let main_stroke = Stroke {
                    width: 3.0,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Stroke::default()
                };
                pixmap.stroke_path(&path, &line_paint, &main_stroke, Transform::identity(), None);
            }
        }
    }
}
