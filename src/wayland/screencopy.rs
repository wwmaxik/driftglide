use memmap2::MmapMut;
use std::os::unix::io::AsFd;
use tempfile::tempfile;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_shm::{Format, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Dispatch, QueueHandle};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

#[allow(dead_code)]
pub struct ScreencopyFrameState {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: Format,
    pub ready: bool,
    pub failed: bool,
    pub buffer: Option<WlBuffer>,
    pub pool: Option<WlShmPool>,
    pub mmap: Option<MmapMut>,
}

impl Default for ScreencopyFrameState {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            stride: 0,
            format: Format::Argb8888,
            ready: false,
            failed: false,
            buffer: None,
            pool: None,
            mmap: None,
        }
    }
}

pub struct ScreencopyCapture {
    pub manager: ZwlrScreencopyManagerV1,
    pub state: ScreencopyFrameState,
}

impl ScreencopyCapture {
    pub fn new(manager: ZwlrScreencopyManagerV1) -> Self {
        Self {
            manager,
            state: ScreencopyFrameState::default(),
        }
    }

    /// Запускает захват экрана с заданного WlOutput
    pub fn capture_output<D>(
        &mut self,
        output: &WlOutput,
        qh: &QueueHandle<D>,
    ) -> ZwlrScreencopyFrameV1
    where
        D: Dispatch<ZwlrScreencopyFrameV1, ()> + 'static,
    {
        self.state = ScreencopyFrameState::default();
        // 0 = не захватывать аппаратный курсор
        self.manager.capture_output(0, output, qh, ())
    }

    /// Обработка формата буфера от screencopy frame
    pub fn init_buffer<D>(
        &mut self,
        shm: &WlShm,
        qh: &QueueHandle<D>,
        frame: &ZwlrScreencopyFrameV1,
        format: Format,
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<(), std::io::Error>
    where
        D: Dispatch<WlBuffer, ()> + Dispatch<WlShmPool, ()> + 'static,
    {
        self.state.format = format;
        self.state.width = width;
        self.state.height = height;
        self.state.stride = stride;

        let size = (stride * height) as usize;
        let file = tempfile()?;
        file.set_len(size as u64)?;

        let mmap = unsafe { MmapMut::map_mut(&file)? };
        let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            format,
            qh,
            (),
        );

        frame.copy(&buffer);

        self.state.mmap = Some(mmap);
        self.state.pool = Some(pool);
        self.state.buffer = Some(buffer);

        Ok(())
    }

    /// Получение захваченного изображения в формате RGBA (для tiny-skia и сохранения)
    pub fn get_rgba_data(&self) -> Option<Vec<u8>> {
        if !self.state.ready {
            return None;
        }

        let mmap = self.state.mmap.as_ref()?;
        let width = self.state.width as usize;
        let height = self.state.height as usize;
        let stride = self.state.stride as usize;

        let mut rgba = vec![0u8; width * height * 4];

        for y in 0..height {
            let row_start = y * stride;
            let dest_start = y * width * 4;

            for x in 0..width {
                let src_idx = row_start + x * 4;
                let dest_idx = dest_start + x * 4;

                if src_idx + 4 <= mmap.len() && dest_idx + 4 <= rgba.len() {
                    let b = mmap[src_idx];
                    let g = mmap[src_idx + 1];
                    let r = mmap[src_idx + 2];
                    let _a = mmap[src_idx + 3];

                    rgba[dest_idx] = r;
                    rgba[dest_idx + 1] = g;
                    rgba[dest_idx + 2] = b;
                    rgba[dest_idx + 3] = 255; // Захват экрана всегда непрозрачен
                }
            }
        }

        Some(rgba)
    }
}
