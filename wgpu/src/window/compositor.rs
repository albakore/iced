//! Connect a window with a renderer.
use crate::core::Color;
use crate::graphics::color;
use crate::graphics::compositor;
use crate::graphics::error;
use crate::graphics::{self, Shell, Viewport};
use crate::settings::{self, Settings};
use crate::{Engine, Renderer};

/// A window graphics backend for iced powered by `wgpu`.
pub struct Compositor {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    format: wgpu::TextureFormat,
    alpha_mode: wgpu::CompositeAlphaMode,
    engine: Engine,
    settings: Settings,
}

/// A compositor error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    /// The surface creation failed.
    #[error("the surface creation failed: {0}")]
    SurfaceCreationFailed(#[from] wgpu::CreateSurfaceError),
    /// The surface is not compatible.
    #[error("the surface is not compatible")]
    IncompatibleSurface,
    /// No adapter was found for the options requested.
    #[error("no adapter was found for the options requested: {0:?}")]
    NoAdapterFound(String),
    /// pyzed: the adapter found runs on the CPU and a hardware one was asked for.
    #[error("the adapter found runs on the CPU: {0}")]
    SoftwareAdapter(String),
    /// No device request succeeded.
    #[error("no device request succeeded: {0:?}")]
    RequestDeviceFailed(Vec<(wgpu::Limits, wgpu::RequestDeviceError)>),
}

impl From<Error> for graphics::Error {
    fn from(error: Error) -> Self {
        Self::GraphicsAdapterNotFound {
            backend: "wgpu",
            reason: error::Reason::RequestFailed(error.to_string()),
        }
    }
}

impl Compositor {
    /// Requests a new [`Compositor`] with the given [`Settings`].
    ///
    /// Returns `None` if no compatible graphics adapter could be found.
    pub async fn request<W: compositor::Window>(
        settings: Settings,
        compatible_window: Option<W>,
        shell: Shell,
    ) -> Result<Self, Error> {
        Self::request_with(settings, compatible_window, shell, false).await
    }

    /// pyzed: [`Compositor::request`] that, with `hardware_only`, turns down an adapter that
    /// runs on the CPU before it creates a device on it.
    async fn request_with<W: compositor::Window>(
        settings: Settings,
        compatible_window: Option<W>,
        shell: Shell,
        hardware_only: bool,
    ) -> Result<Self, Error> {
        let instance = wgpu::util::new_instance_with_webgpu_detection(
            &wgpu::InstanceDescriptor {
                backends: settings.backends,
                flags: if cfg!(feature = "strict-assertions") {
                    wgpu::InstanceFlags::debugging()
                } else {
                    wgpu::InstanceFlags::empty()
                },
                ..Default::default()
            },
        )
        .await;

        log::info!("{settings:#?}");

        #[cfg(not(target_arch = "wasm32"))]
        if log::max_level() >= log::LevelFilter::Info {
            let available_adapters: Vec<_> = instance
                .enumerate_adapters(settings.backends)
                .iter()
                .map(wgpu::Adapter::get_info)
                .collect();
            log::info!("Available adapters: {available_adapters:#?}");
        }

        #[allow(unsafe_code)]
        let compatible_surface = compatible_window
            .and_then(|window| instance.create_surface(window).ok());

        let adapter_options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::from_env()
                .unwrap_or(wgpu::PowerPreference::HighPerformance),
            compatible_surface: compatible_surface.as_ref(),
            force_fallback_adapter: false,
        };

        let adapter =
            instance.request_adapter(&adapter_options).await.map_err(
                |_error| Error::NoAdapterFound(format!("{adapter_options:?}")),
            )?;

        log::info!("Selected: {:#?}", adapter.get_info());

        if hardware_only
            && adapter.get_info().device_type == wgpu::DeviceType::Cpu
        {
            return Err(Error::SoftwareAdapter(adapter.get_info().name));
        }

        let (format, alpha_mode) = compatible_surface
            .as_ref()
            .and_then(|surface| {
                let capabilities = surface.get_capabilities(&adapter);

                let formats = capabilities.formats.iter().copied();

                log::info!("Available formats: {formats:#?}");

                let mut formats = formats.filter(|format| {
                    format.required_features() == wgpu::Features::empty()
                });

                let format = if color::GAMMA_CORRECTION {
                    formats.find(wgpu::TextureFormat::is_srgb)
                } else {
                    formats.find(|format| !wgpu::TextureFormat::is_srgb(format))
                };

                let format = format.or_else(|| {
                    log::warn!("No format found!");

                    capabilities.formats.first().copied()
                });

                let alpha_modes = capabilities.alpha_modes;

                log::info!("Available alpha modes: {alpha_modes:#?}");

                let preferred_alpha = if alpha_modes
                    .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
                {
                    wgpu::CompositeAlphaMode::PostMultiplied
                } else if alpha_modes
                    .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
                {
                    wgpu::CompositeAlphaMode::PreMultiplied
                } else {
                    wgpu::CompositeAlphaMode::Auto
                };

                format.zip(Some(preferred_alpha))
            })
            .ok_or(Error::IncompatibleSurface)?;

        log::info!(
            "Selected format: {format:?} with alpha mode: {alpha_mode:?}"
        );

        #[cfg(target_arch = "wasm32")]
        let limits = [wgpu::Limits::downlevel_webgl2_defaults()
            .using_resolution(adapter.limits())];

        #[cfg(not(target_arch = "wasm32"))]
        let limits =
            [wgpu::Limits::default(), wgpu::Limits::downlevel_defaults()];

        let limits = limits.into_iter().map(|limits| wgpu::Limits {
            max_bind_groups: settings.max_bind_groups,
            max_non_sampler_bindings: 2048,
            ..limits
        });

        let mut errors = Vec::new();

        for required_limits in limits {
            let result = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some(
                        "iced_wgpu::window::compositor device descriptor",
                    ),
                    required_features: wgpu::Features::empty(),
                    required_limits: required_limits.clone(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    trace: wgpu::Trace::Off,
                    experimental_features: wgpu::ExperimentalFeatures::disabled(
                    ),
                })
                .await;

            match result {
                Ok((device, queue)) => {
                    let engine = Engine::new(
                        &adapter,
                        device,
                        queue,
                        format,
                        settings.antialiasing,
                        shell,
                    );

                    return Ok(Compositor {
                        instance,
                        adapter,
                        format,
                        alpha_mode,
                        engine,
                        settings,
                    });
                }
                Err(error) => {
                    errors.push((required_limits, error));
                }
            }
        }

        Err(Error::RequestDeviceFailed(errors))
    }
}

/// Creates a [`Compositor`] with the given [`Settings`] and window.
pub async fn new<W: compositor::Window>(
    settings: Settings,
    compatible_window: W,
    shell: Shell,
) -> Result<Compositor, Error> {
    Compositor::request(settings, Some(compatible_window), shell).await
}

/// Presents the given primitives with the given [`Compositor`].
pub fn present(
    renderer: &mut Renderer,
    surface: &mut wgpu::Surface<'static>,
    viewport: &Viewport,
    background_color: Color,
    on_pre_present: impl FnOnce(),
) -> Result<(), compositor::SurfaceError> {
    match surface.get_current_texture() {
        Ok(frame) => {
            let view = &frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let _submission = renderer.present(
                Some(background_color),
                frame.texture.format(),
                view,
                viewport,
            );

            // Present the frame
            on_pre_present();
            frame.present();

            Ok(())
        }
        Err(error) => match error {
            wgpu::SurfaceError::Timeout => {
                Err(compositor::SurfaceError::Timeout)
            }
            wgpu::SurfaceError::Outdated => {
                Err(compositor::SurfaceError::Outdated)
            }
            wgpu::SurfaceError::Lost => Err(compositor::SurfaceError::Lost),
            wgpu::SurfaceError::OutOfMemory => {
                Err(compositor::SurfaceError::OutOfMemory)
            }
            wgpu::SurfaceError::Other => Err(compositor::SurfaceError::Other),
        },
    }
}

impl graphics::Compositor for Compositor {
    type Renderer = Renderer;
    type Surface = wgpu::Surface<'static>;

    async fn with_backend(
        settings: graphics::Settings,
        _display: impl compositor::Display,
        compatible_window: impl compositor::Window + Clone,
        shell: Shell,
        backend: Option<&str>,
    ) -> Result<Self, graphics::Error> {
        match backend {
            None | Some("wgpu") => {
                let mut settings = Settings::from(settings);
                let chosen = wgpu::Backends::from_env();

                if let Some(backends) = chosen {
                    settings.backends = backends;
                }

                if let Some(present_mode) = settings::present_mode_from_env() {
                    settings.present_mode = present_mode;
                }

                // pyzed: try the native APIs (Vulkan, Metal, DX12) first. Starting the GL
                // backend too costs time and memory at every launch (and prints driver
                // warnings on Mesa), so it is only started when none of them has an adapter.
                // pyzed: an adapter that runs on the CPU (Mesa's llvmpipe and lavapipe,
                // Windows' WARP, SwiftShader) draws slower than tiny-skia and takes about
                // 100 MB more, so unless a backend was named (`ICED_BACKEND=wgpu` or
                // `WGPU_BACKEND`, e.g. for `pz.shader`) the fallback renderer takes over.
                let hardware_only = backend.is_none() && chosen.is_none();
                if hardware_only && !has_gpu_device() {
                    // Without a GPU device, starting Vulkan or GL would only find the CPU,
                    // and leave their drivers loaded (about 50 MB) for nothing.
                    return Err(Error::SoftwareAdapter(
                        "no GPU device".to_owned(),
                    )
                    .into());
                }

                if chosen.is_none()
                    && settings.backends == wgpu::Backends::all()
                {
                    let native = Settings {
                        backends: wgpu::Backends::PRIMARY,
                        ..settings
                    };
                    match Compositor::request_with(
                        native,
                        Some(compatible_window.clone()),
                        shell.clone(),
                        hardware_only,
                    )
                    .await
                    {
                        Ok(compositor) => return Ok(compositor),
                        // A software Vulkan found first: GL would find the same CPU.
                        Err(error @ Error::SoftwareAdapter(_)) => {
                            return Err(error.into());
                        }
                        Err(_) => {}
                    }
                }

                Ok(Compositor::request_with(
                    settings,
                    Some(compatible_window),
                    shell,
                    hardware_only,
                )
                .await?)
            }
            Some(backend) => Err(graphics::Error::GraphicsAdapterNotFound {
                backend: "wgpu",
                reason: error::Reason::DidNotMatch {
                    preferred_backend: backend.to_owned(),
                },
            }),
        }
    }

    fn create_renderer(&self) -> Self::Renderer {
        Renderer::new(
            self.engine.clone(),
            self.settings.default_font,
            self.settings.default_text_size,
        )
    }

    fn create_surface<W: compositor::Window>(
        &mut self,
        window: W,
        width: u32,
        height: u32,
    ) -> Self::Surface {
        let mut surface = self
            .instance
            .create_surface(window)
            .expect("Create surface");

        if width > 0 && height > 0 {
            self.configure_surface(&mut surface, width, height);
        }

        surface
    }

    fn configure_surface(
        &mut self,
        surface: &mut Self::Surface,
        width: u32,
        height: u32,
    ) {
        let capabilities = surface.get_capabilities(&self.adapter);
        let has_copy_src =
            capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC);
        let usage = if has_copy_src {
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        };
        surface.configure(
            &self.engine.device,
            &wgpu::SurfaceConfiguration {
                usage,
                format: self.format,
                present_mode: self.settings.present_mode,
                width,
                height,
                alpha_mode: self.alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 1,
            },
        );
    }

    fn information(&self) -> compositor::Information {
        let information = self.adapter.get_info();

        compositor::Information {
            adapter: information.name,
            backend: format!("{:?}", information.backend),
        }
    }

    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        present(
            renderer,
            surface,
            viewport,
            background_color,
            on_pre_present,
        )
    }

    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &Viewport,
        background_color: Color,
    ) -> Vec<u8> {
        renderer.screenshot(viewport, background_color)
    }
}

/// pyzed: whether the system has a GPU a hardware adapter could run on. On Linux that is a
/// DRM render node, NVIDIA's device or WSL's `/dev/dxg`; elsewhere it is left to the adapter.
fn has_gpu_device() -> bool {
    #[cfg(target_os = "linux")]
    {
        let render_node = std::fs::read_dir("/dev/dri").is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                entry.file_name().to_string_lossy().starts_with("renderD")
            })
        });
        render_node
            || std::path::Path::new("/dev/nvidia0").exists()
            || std::path::Path::new("/dev/dxg").exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}
