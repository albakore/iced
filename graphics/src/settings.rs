use crate::Antialiasing;
use crate::core::{self, Font, Pixels};

/// The settings of a renderer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
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

    /// Whether or not to synchronize frames.
    ///
    /// By default, it is `true`.
    pub vsync: bool,

    /// The maximum number of bind groups that can be used simultaneously
    /// in a shader pipeline.
    ///
    /// The [WebGPU specification] guarantees a minimum of `4`, which is also
    /// the default value used by `wgpu`. Applications embedding custom render
    /// pipelines (e.g. via [`Shader`] primitives) may need more bind groups
    /// than iced's own pipelines require.
    ///
    /// By default, it is set to `4`.
    ///
    /// [WebGPU specification]: https://www.w3.org/TR/webgpu/#limits
    /// [`Shader`]: https://docs.rs/iced/latest/iced/widget/shader/index.html
    pub max_bind_groups: u32,

    /// pyzed: the presentation, GPU and frame rate settings, with the environment applied.
    pub rendering: core::settings::Rendering,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            default_font: Font::default(),
            default_text_size: Pixels(16.0),
            antialiasing: None,
            vsync: true,
            max_bind_groups: 4,
            rendering: core::settings::Rendering::default(),
        }
    }
}

impl From<core::Settings> for Settings {
    fn from(settings: core::Settings) -> Self {
        // pyzed: the environment can retune a deployed app (`ICED_MSAA`, `ICED_MAX_FPS`...).
        let (rendering, antialiasing) =
            settings.rendering.with_env(settings.antialiasing);

        Self {
            default_font: if cfg!(all(
                target_arch = "wasm32",
                feature = "fira-sans"
            )) && settings.default_font == Font::default()
            {
                Font::with_name("Fira Sans")
            } else {
                settings.default_font
            },
            default_text_size: settings.default_text_size,
            antialiasing: antialiasing
                .then_some(Antialiasing::from_samples(rendering.samples))
                .flatten(),
            vsync: settings.vsync,
            max_bind_groups: 4,
            rendering,
        }
    }
}
