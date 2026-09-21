use memmap2::MmapMut;
use std::os::unix::io::AsFd;
use tempfile::tempfile;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_shm::{Format, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Dispatch, QueueHandle};

/// Управляемый SHM буфер для отрисовки через tiny-skia
#[allow(dead_code)]
pub struct ShmBuffer {
    pub buffer: WlBuffer,
    pub pool: WlShmPool,
    pub mmap: MmapMut,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub size: usize,
    pub busy: bool,
}

impl ShmBuffer {
    pub fn new<D>(
        shm: &WlShm,
        qh: &QueueHandle<D>,
        width: u32,
        height: u32,
    ) -> Result<Self, std::io::Error>
    where
        D: Dispatch<WlShmPool, ()> + Dispatch<WlBuffer, ()> + 'static,
    {
        let stride = width * 4;
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
            Format::Argb8888,
            qh,
            (),
        );

        Ok(Self {
            buffer,
            pool,
            mmap,
            width,
            height,
            stride,
            size,
            busy: false,
        })
    }

    /// Предоставляет доступ к срезу памяти для отрисовки
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.mmap[..self.size]
    }

    /// Конвертирует RGBA из tiny-skia в Wayland ARGB8888 (little endian BGRA)
    pub fn rgba_to_argb8888(&mut self) {
        let data = self.as_mut_slice();
        for chunk in data.chunks_exact_mut(4) {
            // chunk: [R, G, B, A] -> [B, G, R, A]
            chunk.swap(0, 2);
        }
    }
}

/// Пул буферизации для избежания мерцаний и артефактов при анимациях
pub struct DoubleBufferedShm {
    buffers: [Option<ShmBuffer>; 3],
    current: usize,
    width: u32,
    height: u32,
}

impl DoubleBufferedShm {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            buffers: [None, None, None],
            current: 0,
            width,
            height,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.buffers = [None, None, None];
            self.current = 0;
        }
    }

    /// Получает следующий свободный буфер для отрисовки
    pub fn next_buffer<D>(
        &mut self,
        shm: &WlShm,
        qh: &QueueHandle<D>,
    ) -> Result<&mut ShmBuffer, std::io::Error>
    where
        D: Dispatch<WlShmPool, ()> + Dispatch<WlBuffer, ()> + 'static,
    {
        // 1. Ищем буфер, который не занят композитором (не busy)
        let mut target_idx = None;
        for i in 0..3 {
            let idx = (self.current + 1 + i) % 3;
            if let Some(buf) = &self.buffers[idx] {
                if !buf.busy {
                    target_idx = Some(idx);
                    break;
                }
            } else {
                target_idx = Some(idx);
                break;
            }
        }

        let idx = target_idx.unwrap_or((self.current + 1) % 3);
        if self.buffers[idx].is_none() {
            let buf = ShmBuffer::new(shm, qh, self.width, self.height)?;
            self.buffers[idx] = Some(buf);
        }

        self.current = idx;
        let buf = self.buffers[self.current].as_mut().unwrap();
        buf.busy = true;
        Ok(buf)
    }

    pub fn mark_released(&mut self, buffer: &WlBuffer) {
        for b in self.buffers.iter_mut().flatten() {
            if &b.buffer == buffer {
                b.busy = false;
            }
        }
    }
}
