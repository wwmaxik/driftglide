use std::time::Duration;
use calloop::timer::{TimeoutAction, Timer};
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use wayland_client::Connection;
use tracing::{error, info, warn};

mod config;
mod gemini;
mod gestures;
mod ipc;
mod render;
mod search;
mod wayland;

use config::Config;
use wayland::AppState;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "driftglide=info".into()),
        )
        .init();

    info!("Запуск демона DriftGlide...");

    let config = Config::default();

    // 1. Подключение к дисплейному серверу Wayland
    let conn = Connection::connect_to_env().map_err(|e| {
        error!("Не удалось подключиться к Wayland compositor: {}", e);
        e
    })?;

    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    // 2. Инициализация состояния приложения
    let mut state = AppState::new(config);

    // 3. Запрос глобальных объектов из wl_registry
    let display = conn.display();
    let _registry = display.get_registry(&qh, ());

    // Выполняем начальный раундтрип для обнаружения протоколов
    event_queue.roundtrip(&mut state)?;

    // Проверка поддержки необходимых протоколов
    if state.compositor.is_none() {
        error!("Wayland compositor не предоставляет интерфейс wl_compositor!");
        return Err("Отсутствует wl_compositor".into());
    }
    if state.shm.is_none() {
        error!("Wayland compositor не предоставляет интерфейс wl_shm!");
        return Err("Отсутствует wl_shm".into());
    }
    if state.layer_shell.is_none() {
        error!("Wayland compositor не поддерживает протокол wlr-layer-shell-v1!");
        return Err("Отсутствует zwlr_layer_shell_v1".into());
    }
    if state.screencopy_manager.is_none() {
        warn!("Протокол wlr-screencopy-unstable-v1 не обнаружен. Circle to Search будет работать в режиме холста без захвата кадра.");
    }

    // 4. Поднятие наэкранной полоски с пилюлей
    state.setup_pill_surface(&qh);
    state.init_cursor_theme(&conn, &qh);

    // Еще один раундтрип для получения начального configure события
    event_queue.roundtrip(&mut state)?;
    let _ = conn.flush();

    // 5. Настройка Event Loop через calloop
    let mut event_loop: EventLoop<AppState> = EventLoop::try_new()?;
    let loop_handle = event_loop.handle();

    // Источник событий Wayland
    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle.clone())
        .map_err(|e| format!("Не удалось зарегистрировать WaylandSource в EventLoop: {}", e))?;

    // Периодический таймер (каждые 30 мс) для проверки долгого нажатия
    let qh_clone = qh.clone();
    let conn_timer = conn.clone();
    let timer = Timer::from_duration(Duration::from_millis(30));
    loop_handle
        .insert_source(timer, move |_, _, state: &mut AppState| {
            let action = state.gesture_detector.check_timer();
            state.handle_gesture_action(action, &qh_clone);
            state.check_idle_timeout(&qh_clone);
            let _ = conn_timer.flush();
            TimeoutAction::ToDuration(Duration::from_millis(30))
        })
        .map_err(|e| format!("Не удалось зарегистрировать таймер жестов: {}", e))?;

    info!("DriftGlide успешно инициализирован и ожидает событий.");

    // 6. Главный цикл обработки событий с мгновенным flush для устранения задержек
    while state.running {
        let is_animating = state.gesture_detector.pill_animating
            || state.gemini_window.is_animating()
            || state.task_switcher.is_animating();
        let timeout = if is_animating {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(30)
        };

        event_loop.dispatch(timeout, &mut state)?;

        if state.gesture_detector.pill_animating {
            state.step_pill_animation(&qh);
        }

        if state.task_switcher.is_animating() {
            let still_animating = state.task_switcher.step_animation();
            state.redraw_switcher(&qh);
            if !still_animating && state.task_switcher.anim == crate::render::launcher::SwitcherAnim::None && state.task_switcher.anim_progress == 0.0 {
                state.close_task_switcher();
                state.redraw_pill(&qh);
            }
        }

        if state.gemini_window.check_pending_response() {
            state.search_needs_redraw = true;
        }

        if state.gemini_window.visible && !state.gemini_window.minimized {
            if state.gemini_window.tick_cursor() {
                state.search_needs_redraw = true;
            }
        }

        if state.gemini_window.is_animating() {
            let still_animating = state.gemini_window.step_animation();
            if !still_animating {
                state.update_search_input_region(&qh);
                if !state.gemini_window.visible && state.circle_to_search.active {
                    state.circle_to_search.stop();
                    state.close_search_overlay();
                    state.redraw_pill(&qh);
                }
            }
            state.search_needs_redraw = true;
        }

        if state.search_needs_redraw {
            state.redraw_search(&qh);
            state.search_needs_redraw = false;
        }

        let _ = conn.flush();
    }

    info!("Остановка DriftGlide.");
    Ok(())
}
