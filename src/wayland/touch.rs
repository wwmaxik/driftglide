#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TouchEvent {
    Down { id: i32, x: f64, y: f64, time: u32 },
    Motion { id: i32, x: f64, y: f64, time: u32 },
    Up { id: i32, time: u32 },
    Cancel,
    Frame,
}

#[allow(dead_code)]
pub struct TouchState {
    pub active_touch_id: Option<i32>,
    pub last_x: f64,
    pub last_y: f64,
}

#[allow(dead_code)]
impl TouchState {
    pub fn new() -> Self {
        Self {
            active_touch_id: None,
            last_x: 0.0,
            last_y: 0.0,
        }
    }

    pub fn handle_event(&mut self, event: TouchEvent) -> Option<TouchEvent> {
        match event {
            TouchEvent::Down { id, x, y, time } => {
                if self.active_touch_id.is_none() {
                    self.active_touch_id = Some(id);
                    self.last_x = x;
                    self.last_y = y;
                    Some(TouchEvent::Down { id, x, y, time })
                } else {
                    None
                }
            }
            TouchEvent::Motion { id, x, y, time } => {
                if self.active_touch_id == Some(id) {
                    self.last_x = x;
                    self.last_y = y;
                    Some(TouchEvent::Motion { id, x, y, time })
                } else {
                    None
                }
            }
            TouchEvent::Up { id, time } => {
                if self.active_touch_id == Some(id) {
                    self.active_touch_id = None;
                    Some(TouchEvent::Up { id, time })
                } else {
                    None
                }
            }
            TouchEvent::Cancel => {
                self.active_touch_id = None;
                Some(TouchEvent::Cancel)
            }
            TouchEvent::Frame => Some(TouchEvent::Frame),
        }
    }
}
