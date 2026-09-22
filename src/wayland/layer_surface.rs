use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::QueueHandle;
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

pub struct LayerSurface {
    pub surface: WlSurface,
    pub layer_surface: ZwlrLayerSurfaceV1,
    pub configured_width: u32,
    pub configured_height: u32,
    pub configured: bool,
}

impl LayerSurface {
    /// Создает нижнюю полоску для пилюли
    pub fn new_pill_bar<D: 'static>(
        compositor: &WlCompositor,
        layer_shell: &ZwlrLayerShellV1,
        output: Option<&WlOutput>,
        bar_height: u32,
        qh: &QueueHandle<D>,
    ) -> Self
    where
        D: wayland_client::Dispatch<WlSurface, ()>
            + wayland_client::Dispatch<ZwlrLayerSurfaceV1, ()>,
    {
        let surface = compositor.create_surface(qh, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            output,
            Layer::Top,
            "driftglide_pill".into(),
            qh,
            (),
        );

        // Якоря: низ, лево, право (на всю ширину внизу экрана)
        layer_surface.set_anchor(Anchor::Bottom | Anchor::Left | Anchor::Right);
        layer_surface.set_size(0, bar_height);
        // Не резервируем эксклюзивную зону, чтобы не сдвигать рабочее пространство
        layer_surface.set_exclusive_zone(0);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);

        surface.commit();

        Self {
            surface,
            layer_surface,
            configured_width: 0,
            configured_height: bar_height,
            configured: false,
        }
    }

    /// Создает Dock / Task Switcher в виде полноэкранного оверлея
    /// (позволяет отслеживать клики вне дока для закрытия)
    pub fn new_task_switcher_dock<D: 'static>(
        compositor: &WlCompositor,
        layer_shell: &ZwlrLayerShellV1,
        output: Option<&WlOutput>,
        qh: &QueueHandle<D>,
    ) -> Self
    where
        D: wayland_client::Dispatch<WlSurface, ()>
            + wayland_client::Dispatch<ZwlrLayerSurfaceV1, ()>,
    {
        let surface = compositor.create_surface(qh, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            output,
            Layer::Overlay,
            "driftglide_switcher".into(),
            qh,
            (),
        );

        // Полноэкранный прозрачный оверлей для перехвата кликов мимо дока (click outside to close)
        layer_surface.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
        layer_surface.set_size(0, 0);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);

        surface.commit();

        Self {
            surface,
            layer_surface,
            configured_width: 0,
            configured_height: 0,
            configured: false,
        }
    }

    /// Создает полноэкранный оверлей (для Circle to Search)
    pub fn new_fullscreen_overlay<D: 'static>(
        compositor: &WlCompositor,
        layer_shell: &ZwlrLayerShellV1,
        output: Option<&WlOutput>,
        namespace: &str,
        qh: &QueueHandle<D>,
    ) -> Self
    where
        D: wayland_client::Dispatch<WlSurface, ()>
            + wayland_client::Dispatch<ZwlrLayerSurfaceV1, ()>,
    {
        let surface = compositor.create_surface(qh, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            output,
            Layer::Overlay,
            namespace.into(),
            qh,
            (),
        );

        // На весь экран
        layer_surface.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
        layer_surface.set_size(0, 0);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);

        surface.commit();

        Self {
            surface,
            layer_surface,
            configured_width: 0,
            configured_height: 0,
            configured: false,
        }
    }
}
