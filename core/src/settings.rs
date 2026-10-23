//! Configure your application.
use crate::{Font, Pixels};

use std::borrow::Cow;

/// The settings of an iced program.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The identifier of the application.
    ///
    /// If provided, this identifier may be used to identify the application or
    /// communicate with it through the windowing system.
    pub id: Option<String>,

    /// The fonts to load on boot.
    pub fonts: Vec<Cow<'static, [u8]>>,

    /// The default [`Font`] to be used.
    ///
    /// By default, it uses [`Family::SansSerif`](crate::font::Family::SansSerif).
    pub default_font: Font,

    /// The text size that will be used by default.
    ///
    /// The default value is `16.0`.
    pub default_text_size: Pixels,

    /// If set to true, the renderer will try to perform antialiasing for some
    /// primitives.
    ///
    /// Enabling it can produce a smoother result in some widgets, like the
    /// `canvas` widget, at a performance cost.
    ///
    /// By default, it is enabled.
    pub antialiasing: bool,

    /// Whether or not to attempt to synchronize rendering when possible.
    ///
    /// Disabling it can improve rendering performance on some platforms.
    ///
    /// By default, it is enabled.
    pub vsync: bool,

    /// pyzed: how the windows are drawn and paced: the samples of the antialiasing, the
    /// presentation mode, the GPU and the frame rate limits (see [`Rendering`]).
    pub rendering: Rendering,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            id: None,
            fonts: Vec::new(),
            default_font: Font::default(),
            default_text_size: Pixels(16.0),
            antialiasing: true,
            vsync: true,
            rendering: Rendering::default(),
        }
    }
}

/// pyzed: how the windows are drawn and paced.
///
/// Each field can be overridden by an environment variable, so a deployed app can be tuned
/// without changing its code (see [`Rendering::with_env`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rendering {
    /// The samples of the multisample antialiasing when [`Settings::antialiasing`] is on:
    /// 2, 4, 8 or 16 (`ICED_MSAA`, where `0` or `off` turns it off).
    ///
    /// The default is 4.
    pub samples: u8,

    /// How finished frames reach the screen (`ICED_PRESENT_MODE`).
    pub present_mode: PresentMode,

    /// Which GPU to prefer when there are several (`WGPU_POWER_PREF`).
    pub power: PowerPreference,

    /// Whether the GPU renderer favors speed or a small memory footprint (`ICED_MEMORY`).
    pub memory: Memory,

    /// The frames the GPU may queue ahead of the screen, from 1 to 3 (`ICED_FRAME_LATENCY`).
    ///
    /// 1, the default, keeps the input latency lowest; 2 or 3 smooths heavy animations on slow
    /// GPUs at the cost of one or two frames of delay.
    pub frame_latency: u8,

    /// A part of the name of the GPU to use, like `"nvidia"` or `"intel"`, matched without
    /// case (`WGPU_ADAPTER_NAME`). `None` lets [`Rendering::power`] decide.
    pub adapter: Option<&'static str>,

    /// The most frames per second that animations draw in a focused window (`ICED_MAX_FPS`).
    ///
    /// Input and new content are drawn at once; only the frames that animations ask for wait.
    /// `None`, the default, follows the display.
    pub max_fps: Option<u16>,

    /// The most frames per second that animations draw in a window without the focus
    /// (`ICED_BACKGROUND_FPS`); `Some(0)` pauses them until the window gets the focus back.
    ///
    /// `None`, the default, uses [`Rendering::max_fps`].
    pub background_fps: Option<u16>,
}

impl Default for Rendering {
    fn default() -> Self {
        Self {
            samples: 4,
            present_mode: PresentMode::default(),
            power: PowerPreference::default(),
            memory: Memory::default(),
            frame_latency: 1,
            adapter: None,
            max_fps: None,
            background_fps: None,
        }
    }
}

impl Rendering {
    /// Applies the environment variables that override these settings, and returns the
    /// antialiasing switch they leave (`ICED_MSAA=0` turns it off).
    pub fn with_env(mut self, antialiasing: bool) -> (Self, bool) {
        let mut antialiasing = antialiasing;

        if let Some(value) = env("ICED_MSAA") {
            match value.as_str() {
                "0" | "off" | "none" | "false" => antialiasing = false,
                value => {
                    if let Ok(samples) = value.trim_end_matches('x').parse() {
                        self.samples = samples;
                        antialiasing = true;
                    }
                }
            }
        }

        if let Some(mode) =
            env("ICED_PRESENT_MODE").and_then(|v| PresentMode::parse(&v))
        {
            self.present_mode = mode;
        }

        if let Some(power) =
            env("WGPU_POWER_PREF").and_then(|v| PowerPreference::parse(&v))
        {
            self.power = power;
        }

        if let Some(memory) = env("ICED_MEMORY").and_then(|v| Memory::parse(&v))
        {
            self.memory = memory;
        }

        if let Some(latency) =
            env("ICED_FRAME_LATENCY").and_then(|v| v.parse().ok())
        {
            self.frame_latency = latency;
        }

        if let Some(name) =
            env("WGPU_ADAPTER_NAME").filter(|name| !name.is_empty())
        {
            self.adapter = Some(Box::leak(name.into_boxed_str()));
        }

        if let Some(fps) = env("ICED_MAX_FPS") {
            self.max_fps = fps.parse().ok().filter(|fps| *fps > 0);
        }

        if let Some(fps) = env("ICED_BACKGROUND_FPS") {
            self.background_fps = fps.parse().ok();
        }

        self.frame_latency = self.frame_latency.clamp(1, 3);

        (self, antialiasing)
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_lowercase())
}

/// pyzed: how finished frames reach the screen.
///
/// A mode the platform does not offer falls back to [`PresentMode::Vsync`] or
/// [`PresentMode::NoVsync`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PresentMode {
    /// [`PresentMode::Vsync`] when [`Settings::vsync`] is on, [`PresentMode::NoVsync`] when off.
    #[default]
    Auto,
    /// Waits for the display, showing a late frame at once where the platform can.
    Vsync,
    /// Does not wait for the display, without tearing where the platform can.
    NoVsync,
    /// Always waits for the display: no tearing, the lowest power use.
    Fifo,
    /// Waits for the display, but shows a late frame at once (it may tear when late).
    FifoRelaxed,
    /// Replaces the queued frame with the newest one: no tearing, low latency, more power.
    Mailbox,
    /// Shows each frame at once: the lowest latency, it may tear.
    Immediate,
}

impl PresentMode {
    /// Reads a mode by name: `auto`, `vsync`, `no_vsync`, `fifo`, `fifo_relaxed`, `mailbox` or
    /// `immediate`.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "auto" => Self::Auto,
            "vsync" | "on" => Self::Vsync,
            "no_vsync" | "off" => Self::NoVsync,
            "fifo" => Self::Fifo,
            "fifo_relaxed" | "adaptive" => Self::FifoRelaxed,
            "mailbox" => Self::Mailbox,
            "immediate" => Self::Immediate,
            _ => return None,
        })
    }
}

/// pyzed: which GPU to prefer when there are several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerPreference {
    /// The fastest one, usually a discrete GPU.
    #[default]
    HighPerformance,
    /// The one that uses less power, usually the integrated GPU.
    LowPower,
}

impl PowerPreference {
    /// Reads a preference by name: `high` or `low` (also `high_performance`, `low_power`).
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "high" | "high_performance" | "performance" => {
                Self::HighPerformance
            }
            "low" | "low_power" | "battery" => Self::LowPower,
            _ => return None,
        })
    }
}

/// pyzed: whether the GPU renderer favors speed or a small memory footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Memory {
    /// Allocates GPU memory in small blocks and frees it soon: the smallest footprint.
    #[default]
    Low,
    /// Allocates GPU memory in large blocks and keeps it: fewer allocations while drawing.
    Performance,
}

impl Memory {
    /// Reads a preference by name: `low` or `performance`.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "low" | "memory" | "low_memory" => Self::Low,
            "performance" | "high" | "fast" => Self::Performance,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_parse() {
        assert_eq!(PresentMode::parse("mailbox"), Some(PresentMode::Mailbox));
        assert_eq!(PresentMode::parse("no_vsync"), Some(PresentMode::NoVsync));
        assert_eq!(PresentMode::parse("tearing"), None);
        assert_eq!(
            PowerPreference::parse("battery"),
            Some(PowerPreference::LowPower)
        );
        assert_eq!(Memory::parse("performance"), Some(Memory::Performance));
    }

    #[test]
    fn defaults_keep_iced_behavior() {
        let rendering = Rendering::default();

        assert_eq!(rendering.samples, 4);
        assert_eq!(rendering.present_mode, PresentMode::Auto);
        assert_eq!(rendering.frame_latency, 1);
        assert_eq!(rendering.max_fps, None);
        assert_eq!(rendering.background_fps, None);
    }
}
