//! pyzed: effect layers.
//!
//! A widget draws part of the tree between [`Renderer::start_effect`](crate::Renderer::start_effect)
//! and [`Renderer::end_effect`](crate::Renderer::end_effect). Those layers are rendered into an
//! off-screen texture and composited back with a rotation, an opacity, a blur and a bloom, over a
//! blurred copy of what is behind them (backdrop blur).
use crate::core::{Color, Rectangle};

use wgpu::util::DeviceExt;

/// How a group of layers is composited.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effect {
    /// The area the effect is about, in logical pixels: the center of the rotation and the shape
    /// of the backdrop.
    pub bounds: Rectangle,
    /// Corner radii of the backdrop (top left, top right, bottom right, bottom left).
    pub radius: [f32; 4],
    /// Rotation around the center of `bounds`, in radians (clockwise).
    pub rotation: f32,
    /// Opacity of the whole group.
    pub opacity: f32,
    /// Blur of the content, in logical pixels.
    pub blur: f32,
    /// Light that spills out of the bright parts of the content.
    pub bloom: Option<Bloom>,
    /// Blur of what is behind `bounds`, in logical pixels.
    pub backdrop_blur: f32,
}

/// A bloom: the bright parts of the content, blurred and added on top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bloom {
    /// How strong the light is (0 to 1 is subtle to strong; more is allowed).
    pub intensity: f32,
    /// How far it spreads, in logical pixels.
    pub radius: f32,
    /// Luminance (0 to 1) above which pixels glow.
    pub threshold: f32,
    /// Tints the light, or `None` to keep the content's colors.
    pub color: Option<Color>,
}

impl Effect {
    /// An effect that changes nothing.
    pub fn new(bounds: Rectangle) -> Self {
        Self {
            bounds,
            radius: [0.0; 4],
            rotation: 0.0,
            opacity: 1.0,
            blur: 0.0,
            bloom: None,
            backdrop_blur: 0.0,
        }
    }

    /// Whether drawing with this effect looks the same as drawing without it.
    pub fn is_noop(&self) -> bool {
        self.rotation == 0.0
            && self.opacity >= 1.0
            && self.blur <= 0.0
            && self.bloom.is_none_or(|b| b.intensity <= 0.0)
            && self.backdrop_blur <= 0.0
    }
}

/// A recorded group: the layers `start..end` with their effect.
#[derive(Debug, Clone)]
pub(crate) struct Group {
    pub start: usize,
    pub end: usize,
    pub effect: Effect,
    /// Where the composited group may draw (the clip of the layer it was started in), logical.
    pub clip: Rectangle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    size: [f32; 4],
    center_rot: [f32; 4],
    params: [f32; 4],
    dir: [f32; 4],
    rect: [f32; 4],
    radii: [f32; 4],
    tint: [f32; 4],
}

impl Uniforms {
    fn new(width: u32, height: u32) -> Self {
        Self {
            size: [
                width as f32,
                height as f32,
                1.0 / width as f32,
                1.0 / height as f32,
            ],
            center_rot: [0.0, 0.0, 1.0, 0.0],
            params: [1.0, 0.0, 0.0, 0.0],
            dir: [0.0; 4],
            rect: [0.0; 4],
            radii: [0.0; 4],
            tint: [0.0; 4],
        }
    }
}

/// Which way a pipeline blends into its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Blend {
    Replace,
    Over,
    Add,
}

pub(crate) struct Pipeline {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    blur: wgpu::RenderPipeline,
    bright: wgpu::RenderPipeline,
    composite_over: wgpu::RenderPipeline,
    composite_replace: wgpu::RenderPipeline,
    composite_add: wgpu::RenderPipeline,
    backdrop: wgpu::RenderPipeline,
    pool: Vec<Target>,
}

/// An off-screen texture the size of the viewport.
pub(crate) struct Target {
    _texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    width: u32,
    height: u32,
    in_use: bool,
}

/// A target taken from the pool, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Slot(usize);

impl std::fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("effect::Pipeline").finish_non_exhaustive()
    }
}

impl Pipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("pyzed.effect.shader"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                    include_str!("shader/effect.wgsl"),
                )),
            });

        let layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("pyzed.effect.bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float {
                                filterable: true,
                            },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering,
                        ),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pyzed.effect.pipeline_layout"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            });

        let pipeline = |entry: &str, blend: Blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("pyzed.effect.pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options:
                        wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(match blend {
                            Blend::Replace => wgpu::BlendState::REPLACE,
                            Blend::Over => {
                                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING
                            }
                            Blend::Add => wgpu::BlendState {
                                color: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::One,
                                    dst_factor: wgpu::BlendFactor::One,
                                    operation: wgpu::BlendOperation::Add,
                                },
                                alpha: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::Zero,
                                    dst_factor: wgpu::BlendFactor::One,
                                    operation: wgpu::BlendOperation::Add,
                                },
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options:
                        wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };

        Self {
            blur: pipeline("fs_blur", Blend::Replace),
            bright: pipeline("fs_bright", Blend::Replace),
            composite_over: pipeline("fs_composite", Blend::Over),
            composite_replace: pipeline("fs_composite", Blend::Replace),
            composite_add: pipeline("fs_composite", Blend::Add),
            backdrop: pipeline("fs_backdrop", Blend::Over),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("pyzed.effect.sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            layout,
            pool: Vec::new(),
        }
    }

    /// Takes a free off-screen target of `width` x `height`.
    pub fn acquire(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Slot {
        if let Some(index) = self
            .pool
            .iter()
            .position(|t| !t.in_use && t.width == width && t.height == height)
        {
            self.pool[index].in_use = true;
            return Slot(index);
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("pyzed.effect.target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.pool.push(Target {
            _texture: texture,
            view,
            width,
            height,
            in_use: true,
        });

        Slot(self.pool.len() - 1)
    }

    pub fn view(&self, slot: Slot) -> &wgpu::TextureView {
        &self.pool[slot.0].view
    }

    /// Frees every target and drops the ones of another size.
    pub fn trim(&mut self, width: u32, height: u32) {
        self.pool.retain(|t| t.width == width && t.height == height);
        for target in &mut self.pool {
            target.in_use = false;
        }
    }

    fn draw(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
        clear: bool,
        scissor: Option<Rectangle<u32>>,
        uniforms: Uniforms,
    ) {
        let buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("pyzed.effect.uniforms"),
                contents: bytemuck::bytes_of(&uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pyzed.effect.bind_group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("pyzed.effect.pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if clear {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if let Some(scissor) = scissor {
            if scissor.width == 0 || scissor.height == 0 {
                return;
            }
            pass.set_scissor_rect(
                scissor.x,
                scissor.y,
                scissor.width,
                scissor.height,
            );
        }

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Blurs `source` into `target` (through `scratch`) inside `region` (physical pixels).
    pub fn blur(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        scratch: &wgpu::TextureView,
        target: &wgpu::TextureView,
        region: Rectangle<u32>,
        size: (u32, u32),
        radius: f32,
    ) {
        let sigma = (radius / 2.0).max(0.5);

        for (from, to, dir) in
            [(source, scratch, [1.0, 0.0]), (scratch, target, [0.0, 1.0])]
        {
            let mut uniforms = Uniforms::new(size.0, size.1);
            uniforms.params = [1.0, sigma, 0.0, 0.0];
            uniforms.dir = [dir[0], dir[1], 0.0, 0.0];
            self.draw(
                device,
                encoder,
                &self.blur,
                from,
                to,
                true,
                Some(region),
                uniforms,
            );
        }
    }

    /// Keeps the parts of `source` brighter than `threshold`.
    pub fn bright(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
        region: Rectangle<u32>,
        size: (u32, u32),
        threshold: f32,
    ) {
        let mut uniforms = Uniforms::new(size.0, size.1);
        uniforms.params = [1.0, 0.0, threshold, 0.0];
        self.draw(
            device,
            encoder,
            &self.bright,
            source,
            target,
            true,
            Some(region),
            uniforms,
        );
    }

    /// Draws `source` over `target`, rotated around `center` and faded by `opacity`.
    #[allow(clippy::too_many_arguments)]
    pub fn composite(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
        scissor: Option<Rectangle<u32>>,
        size: (u32, u32),
        center: [f32; 2],
        rotation: f32,
        opacity: f32,
        mode: Composite,
    ) {
        let mut uniforms = Uniforms::new(size.0, size.1);
        uniforms.center_rot =
            [center[0], center[1], rotation.cos(), rotation.sin()];
        uniforms.params = [opacity, 0.0, 0.0, 0.0];
        let pipeline = match mode {
            Composite::Over => &self.composite_over,
            Composite::Replace => &self.composite_replace,
            Composite::Add(tint) => {
                if let Some(tint) = tint {
                    uniforms.tint = [tint.r, tint.g, tint.b, 1.0];
                }
                &self.composite_add
            }
        };
        self.draw(
            device, encoder, pipeline, source, target, false, scissor, uniforms,
        );
    }

    /// Draws `source` (a blurred backdrop) inside the rounded rectangle `rect`, rotated with it.
    #[allow(clippy::too_many_arguments)]
    pub fn backdrop(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
        scissor: Option<Rectangle<u32>>,
        size: (u32, u32),
        rect: Rectangle,
        radii: [f32; 4],
        rotation: f32,
        opacity: f32,
    ) {
        let mut uniforms = Uniforms::new(size.0, size.1);
        let center = rect.center();
        uniforms.center_rot =
            [center.x, center.y, rotation.cos(), rotation.sin()];
        uniforms.params = [opacity, 0.0, 0.0, 0.0];
        uniforms.rect = [rect.x, rect.y, rect.width, rect.height];
        uniforms.radii = radii;
        self.draw(
            device,
            encoder,
            &self.backdrop,
            source,
            target,
            false,
            scissor,
            uniforms,
        );
    }
}

/// How [`Pipeline::composite`] blends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Composite {
    Over,
    Replace,
    /// Adds light, optionally tinted.
    Add(Option<Color>),
}

/// `rect` (physical pixels) grown by `margin` and clipped to `size`.
pub(crate) fn region(
    rect: Rectangle,
    margin: f32,
    size: (u32, u32),
) -> Option<Rectangle<u32>> {
    let grown = Rectangle {
        x: rect.x - margin,
        y: rect.y - margin,
        width: rect.width + margin * 2.0,
        height: rect.height + margin * 2.0,
    };
    let screen = Rectangle::new(
        crate::core::Point::ORIGIN,
        crate::core::Size::new(size.0 as f32, size.1 as f32),
    );
    grown.intersection(&screen).and_then(Rectangle::snap)
}
