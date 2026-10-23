//! Configure a renderer.
pub use crate::core::settings::Memory;
use crate::core::settings::{PowerPreference, PresentMode};
use crate::core::{Font, Pixels};
use crate::graphics::{self, Antialiasing};

/// The settings of a [`Renderer`].
///
/// [`Renderer`]: crate::Renderer
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// The present mode of the [`Renderer`].
    ///
    /// [`Renderer`]: crate::Renderer
    pub present_mode: wgpu::PresentMode,

    /// The graphics backends to use.
    pub backends: wgpu::Backends,

    /// The default [`Font`] to use.
    pub default_font: Font,

    /// The default size of text.
    ///
    /// By default, it will be set to `16.0`.
    pub default_text_size: Pixels,

    /// The antialiasing strategy that will be used for triangle primitives.
    ///
    /// By default, it is `None`.
    pub antialiasing: Option<Antialiasing>,

    /// The maximum number of bind groups that can be used simultaneously
    /// in a shader pipeline.
    ///
    /// By default, it is set to `4`.
    pub max_bind_groups: u32,

    /// pyzed: which GPU to prefer when there are several.
    pub power_preference: wgpu::PowerPreference,

    /// pyzed: whether GPU memory is allocated for speed or for a small footprint.
    pub memory: Memory,

    /// pyzed: the frames the GPU may queue ahead of the screen (1 to 3).
    pub frame_latency: u32,

    /// pyzed: a part of the name of the GPU to use, matched without case.
    pub adapter: Option<&'static str>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            present_mode: wgpu::PresentMode::AutoVsync,
            backends: wgpu::Backends::all(),
            default_font: Font::default(),
            default_text_size: Pixels(16.0),
            antialiasing: None,
            max_bind_groups: 4,
            power_preference: wgpu::PowerPreference::HighPerformance,
            memory: Memory::Low,
            frame_latency: 1,
            adapter: None,
        }
    }
}

impl From<graphics::Settings> for Settings {
    fn from(settings: graphics::Settings) -> Self {
        let rendering = settings.rendering;

        Self {
            present_mode: match rendering.present_mode {
                PresentMode::Auto if settings.vsync => {
                    wgpu::PresentMode::AutoVsync
                }
                PresentMode::Auto | PresentMode::NoVsync => {
                    wgpu::PresentMode::AutoNoVsync
                }
                PresentMode::Vsync => wgpu::PresentMode::AutoVsync,
                PresentMode::Fifo => wgpu::PresentMode::Fifo,
                PresentMode::FifoRelaxed => wgpu::PresentMode::FifoRelaxed,
                PresentMode::Mailbox => wgpu::PresentMode::Mailbox,
                PresentMode::Immediate => wgpu::PresentMode::Immediate,
            },
            default_font: settings.default_font,
            default_text_size: settings.default_text_size,
            antialiasing: settings.antialiasing,
            max_bind_groups: settings.max_bind_groups,
            power_preference: match rendering.power {
                PowerPreference::HighPerformance => {
                    wgpu::PowerPreference::HighPerformance
                }
                PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
            },
            memory: rendering.memory,
            frame_latency: u32::from(rendering.frame_latency.clamp(1, 3)),
            adapter: rendering.adapter,
            ..Settings::default()
        }
    }
}

/// Obtains a [`wgpu::PresentMode`] from the current environment
/// configuration, if set.
///
/// The value returned by this function can be changed by setting
/// the `ICED_PRESENT_MODE` env variable. The possible values are:
///
/// - `vsync` → [`wgpu::PresentMode::AutoVsync`]
/// - `no_vsync` → [`wgpu::PresentMode::AutoNoVsync`]
/// - `immediate` → [`wgpu::PresentMode::Immediate`]
/// - `fifo` → [`wgpu::PresentMode::Fifo`]
/// - `fifo_relaxed` → [`wgpu::PresentMode::FifoRelaxed`]
/// - `mailbox` → [`wgpu::PresentMode::Mailbox`]
pub fn present_mode_from_env() -> Option<wgpu::PresentMode> {
    let present_mode = std::env::var("ICED_PRESENT_MODE").ok()?;

    match present_mode.to_lowercase().as_str() {
        "vsync" => Some(wgpu::PresentMode::AutoVsync),
        "no_vsync" => Some(wgpu::PresentMode::AutoNoVsync),
        "immediate" => Some(wgpu::PresentMode::Immediate),
        "fifo" => Some(wgpu::PresentMode::Fifo),
        "fifo_relaxed" => Some(wgpu::PresentMode::FifoRelaxed),
        "mailbox" => Some(wgpu::PresentMode::Mailbox),
        _ => None,
    }
}
