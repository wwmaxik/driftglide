use std::time::{Duration, Instant};
use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PillVisualState {
    Idle,
    Pressed,
    Dragging { offset_x: f32, offset_y: f32 },
    Returning { offset_x: f32, offset_y: f32, velocity_x: f32, velocity_y: f32 },
    Triggered,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GestureAction {
    None,
    FocusNext,
    FocusPrev,
    OpenLauncher,
    CloseLauncher,
    StartCircleToSearch,
    RedrawPill,
}

#[derive(Debug, Clone)]
enum State {
    Idle,
    Touching {
        start_x: f32,
        start_y: f32,
        current_x: f32,
        current_y: f32,
        start_time: Instant,
        last_time: Instant,
        last_x: f32,
        last_y: f32,
        velocity_x: f32,
        velocity_y: f32,
        long_press_fired: bool,
    },
    SwipingHorizontal {
        start_x: f32,
        current_x: f32,
        last_time: Instant,
        velocity_x: f32,
    },
    SwipingVertical {
        start_y: f32,
        current_y: f32,
        progress: f32,
    },
}

pub struct GestureDetector {
    config: Config,
    state: State,
    touch_id: Option<i32>,
    pub visual_state: PillVisualState,
    pub pill_animating: bool,
    last_anim_time: Option<Instant>,
}

impl GestureDetector {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: State::Idle,
            touch_id: None,
            visual_state: PillVisualState::Idle,
            pill_animating: false,
            last_anim_time: None,
        }
    }

    /// Обработка касания экрана / нажатия кнопки мыши
    pub fn on_touch_down(&mut self, id: i32, x: f32, y: f32) -> GestureAction {
        if self.touch_id.is_some() {
            return GestureAction::None;
        }

        let now = Instant::now();
        self.touch_id = Some(id);
        self.pill_animating = false;
        self.last_anim_time = None;
        self.state = State::Touching {
            start_x: x,
            start_y: y,
            current_x: x,
            current_y: y,
            start_time: now,
            last_time: now,
            last_x: x,
            last_y: y,
            velocity_x: 0.0,
            velocity_y: 0.0,
            long_press_fired: false,
        };
        self.visual_state = PillVisualState::Pressed;
        GestureAction::RedrawPill
    }

    /// Обработка движения
    pub fn on_touch_motion(&mut self, id: i32, x: f32, y: f32) -> GestureAction {
        if self.touch_id != Some(id) {
            return GestureAction::None;
        }

        let now = Instant::now();

        match &mut self.state {
            State::Touching {
                start_x,
                start_y,
                current_x,
                current_y,
                last_time,
                last_x,
                last_y,
                velocity_x,
                velocity_y,
                long_press_fired,
                ..
            } => {
                let dt = now.duration_since(*last_time).as_secs_f32() * 1000.0;
                if dt > 1.0 {
                    *velocity_x = (x - *last_x) / dt;
                    *velocity_y = (y - *last_y) / dt;
                    *last_time = now;
                    *last_x = x;
                    *last_y = y;
                }

                *current_x = x;
                *current_y = y;

                let dx = x - *start_x;
                let dy = y - *start_y;

                if *long_press_fired {
                    return GestureAction::None;
                }

                let dist_sq = dx * dx + dy * dy;
                let deadzone_sq = self.config.deadzone_threshold * self.config.deadzone_threshold;

                if dist_sq > deadzone_sq {
                    if dx.abs() > dy.abs() * 1.2 {
                        let s_x = *start_x;
                        let vx = *velocity_x;
                        self.state = State::SwipingHorizontal {
                            start_x: s_x,
                            current_x: x,
                            last_time: now,
                            velocity_x: vx,
                        };
                        self.visual_state = PillVisualState::Dragging {
                            offset_x: dx,
                            offset_y: 0.0,
                        };
                        return GestureAction::RedrawPill;
                    } else if dy < -self.config.deadzone_threshold {
                        let progress = (-dy / self.config.swipe_y_threshold).clamp(0.0, 1.0);
                        let s_y = *start_y;
                        self.state = State::SwipingVertical {
                            start_y: s_y,
                            current_y: y,
                            progress,
                        };
                        self.visual_state = PillVisualState::Dragging {
                            offset_x: 0.0,
                            offset_y: dy.clamp(-18.0, 0.0),
                        };
                        return GestureAction::RedrawPill;
                    }
                }

                self.visual_state = PillVisualState::Dragging {
                    offset_x: dx,
                    offset_y: dy.clamp(-18.0, 10.0),
                };
                GestureAction::RedrawPill
            }

            State::SwipingHorizontal {
                start_x,
                current_x,
                last_time,
                velocity_x,
                ..
            } => {
                let dt = now.duration_since(*last_time).as_secs_f32() * 1000.0;
                if dt > 1.0 {
                    *velocity_x = (x - *current_x) / dt;
                    *last_time = now;
                }
                *current_x = x;
                let dx = x - *start_x;

                self.visual_state = PillVisualState::Dragging {
                    offset_x: dx,
                    offset_y: 0.0,
                };
                GestureAction::RedrawPill
            }

            State::SwipingVertical {
                start_y,
                current_y,
                progress,
            } => {
                *current_y = y;
                let dy = y - *start_y;
                *progress = (-dy / self.config.swipe_y_threshold).clamp(0.0, 1.0);

                self.visual_state = PillVisualState::Dragging {
                    offset_x: 0.0,
                    offset_y: dy.clamp(-18.0, 0.0),
                };
                GestureAction::RedrawPill
            }

            State::Idle => GestureAction::None,
        }
    }

    /// Проверка таймера долгого нажатия
    pub fn check_timer(&mut self) -> GestureAction {
        if let State::Touching {
            start_x,
            start_y,
            current_x,
            current_y,
            start_time,
            long_press_fired,
            ..
        } = &mut self.state
        {
            if !*long_press_fired {
                let elapsed = start_time.elapsed();
                let dx = *current_x - *start_x;
                let dy = *current_y - *start_y;
                let dist_sq = dx * dx + dy * dy;

                if elapsed >= Duration::from_millis(self.config.long_press_duration_ms)
                    && dist_sq <= self.config.deadzone_threshold * self.config.deadzone_threshold
                {
                    *long_press_fired = true;
                    self.visual_state = PillVisualState::Triggered;
                    return GestureAction::StartCircleToSearch;
                }
            }
        }
        GestureAction::None
    }

    /// Обработка отпускания
    pub fn on_touch_up(&mut self, id: i32) -> GestureAction {
        if self.touch_id != Some(id) {
            return GestureAction::None;
        }

        self.touch_id = None;
        let mut action = GestureAction::None;

        match self.state {
            State::Touching {
                start_x,
                start_y,
                current_x,
                current_y,
                velocity_x,
                long_press_fired,
                ..
            } => {
                if !long_press_fired {
                    let dx = current_x - start_x;
                    let dy = current_y - start_y;
                    if dy < -self.config.swipe_y_threshold * 0.4 {
                        action = GestureAction::OpenLauncher;
                    } else if dx > self.config.swipe_x_threshold || velocity_x > self.config.flick_velocity_threshold {
                        action = GestureAction::FocusNext;
                    } else if dx < -self.config.swipe_x_threshold || velocity_x < -self.config.flick_velocity_threshold {
                        action = GestureAction::FocusPrev;
                    }
                }
            }

            State::SwipingHorizontal {
                start_x,
                current_x,
                velocity_x,
                ..
            } => {
                let dx = current_x - start_x;
                if dx > self.config.swipe_x_threshold || velocity_x > self.config.flick_velocity_threshold {
                    action = GestureAction::FocusNext;
                } else if dx < -self.config.swipe_x_threshold || velocity_x < -self.config.flick_velocity_threshold {
                    action = GestureAction::FocusPrev;
                }
            }

            State::SwipingVertical { progress, .. } => {
                if progress >= 0.3 {
                    action = GestureAction::OpenLauncher;
                } else {
                    action = GestureAction::CloseLauncher;
                }
            }

            State::Idle => {}
        }

        let (vx, vy) = match &self.state {
            State::Touching { velocity_x, velocity_y, .. } => (*velocity_x, *velocity_y),
            State::SwipingHorizontal { velocity_x, .. } => (*velocity_x, 0.0),
            _ => (0.0, 0.0),
        };

        self.state = State::Idle;

        if let PillVisualState::Dragging { offset_x, offset_y } = self.visual_state {
            if offset_x.abs() > 0.5 || offset_y.abs() > 0.5 || vx.abs() > 10.0 || vy.abs() > 10.0 {
                self.start_spring_return(offset_x, offset_y, vx, vy);
            } else {
                self.visual_state = PillVisualState::Idle;
            }
        } else {
            self.visual_state = PillVisualState::Idle;
        }

        if action == GestureAction::None {
            GestureAction::RedrawPill
        } else {
            action
        }
    }

    /// Отмена жеста
    pub fn on_touch_cancel(&mut self, id: i32) -> GestureAction {
        if self.touch_id == Some(id) {
            self.touch_id = None;
            self.state = State::Idle;
            if let PillVisualState::Dragging { offset_x, offset_y } = self.visual_state {
                if offset_x.abs() > 0.5 || offset_y.abs() > 0.5 {
                    self.start_spring_return(offset_x, offset_y, 0.0, 0.0);
                } else {
                    self.visual_state = PillVisualState::Idle;
                }
            } else {
                self.visual_state = PillVisualState::Idle;
            }
            GestureAction::RedrawPill
        } else {
            GestureAction::None
        }
    }

    /// Запуск пружинной анимации возвращения пилюли (spring return)
    pub fn start_spring_return(&mut self, offset_x: f32, offset_y: f32, velocity_x: f32, velocity_y: f32) {
        self.visual_state = PillVisualState::Returning {
            offset_x,
            offset_y,
            velocity_x,
            velocity_y,
        };
        self.pill_animating = true;
        self.last_anim_time = Some(Instant::now());
    }

    /// Шаг физики пружины (Damped Harmonic Oscillator)
    pub fn step_animation(&mut self) -> bool {
        if let PillVisualState::Returning {
            ref mut offset_x,
            ref mut offset_y,
            ref mut velocity_x,
            ref mut velocity_y,
        } = self.visual_state
        {
            let now = Instant::now();
            let dt = self
                .last_anim_time
                .map(|t| now.duration_since(t).as_secs_f32())
                .unwrap_or(0.016)
                .clamp(0.001, 0.05);
            self.last_anim_time = Some(now);

            // Физика пружины: жесткость 320.0, демпфирование 26.0 (damping ratio ~0.73)
            let stiffness = 320.0;
            let damping = 26.0;

            let ax = -stiffness * *offset_x - damping * *velocity_x;
            let ay = -stiffness * *offset_y - damping * *velocity_y;

            *velocity_x += ax * dt;
            *velocity_y += ay * dt;
            *offset_x += *velocity_x * dt;
            *offset_y += *velocity_y * dt;

            // Порог стабилизации в точке покоя (0.0, 0.0)
            if offset_x.abs() < 0.25 && offset_y.abs() < 0.25 && velocity_x.abs() < 3.0 && velocity_y.abs() < 3.0 {
                self.visual_state = PillVisualState::Idle;
                self.pill_animating = false;
                self.last_anim_time = None;
            }
            true
        } else {
            self.pill_animating = false;
            self.last_anim_time = None;
            false
        }
    }
}
