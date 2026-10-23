use crate::window::MonitorIndex;

/// pyzed: how a window covers a monitor in fullscreen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fullscreen {
    /// The monitor to cover; `None` keeps the one the window is on.
    pub monitor: Option<MonitorIndex>,

    /// Whether to take the monitor over with its own video mode (the highest refresh rate at
    /// its current resolution) instead of a borderless window that covers it.
    ///
    /// An exclusive window can skip the compositor on Windows and X11, which lowers latency
    /// for games and kiosks. Wayland and the web do not offer it and use a borderless window.
    pub exclusive: bool,
}

impl Fullscreen {
    /// A borderless window that covers the given monitor.
    pub fn on(monitor: MonitorIndex) -> Self {
        Self {
            monitor: Some(monitor),
            exclusive: false,
        }
    }

    /// Takes the monitor over with its own video mode (see [`Fullscreen::exclusive`]).
    pub fn exclusive(self) -> Self {
        Self {
            exclusive: true,
            ..self
        }
    }
}
