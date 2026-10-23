#![allow(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
pub mod window;

pub mod effect;
mod engine;
mod layer;
mod primitive;
mod settings;
mod text;

#[cfg(feature = "image")]
mod raster;

#[cfg(feature = "svg")]
mod vector;

#[cfg(feature = "geometry")]
pub mod geometry;

use iced_debug as debug;
pub use iced_graphics as graphics;
pub use iced_graphics::core;

pub use layer::Layer;
pub use primitive::Primitive;
pub use settings::Settings;

#[cfg(feature = "geometry")]
pub use geometry::Geometry;

use crate::core::renderer;
use crate::core::{
    Background, Color, Font, Pixels, Point, Rectangle, Size, Transformation,
};
use crate::engine::Engine;
use crate::graphics::Viewport;
use crate::graphics::compositor;
use crate::graphics::text::{Editor, Paragraph};

/// A [`tiny-skia`] graphics renderer for [`iced`].
///
/// [`tiny-skia`]: https://github.com/RazrFalcon/tiny-skia
/// [`iced`]: https://github.com/iced-rs/iced
#[derive(Debug)]
pub struct Renderer {
    default_font: Font,
    default_text_size: Pixels,
    layers: layer::Stack,
    engine: Engine, // TODO: Shared engine

    // pyzed: effect layers (see `effect`).
    effects: Vec<effect::Group>,
    open_effects: Vec<usize>,
}

impl Renderer {
    pub fn new(default_font: Font, default_text_size: Pixels) -> Self {
        Self {
            default_font,
            default_text_size,
            layers: layer::Stack::new(),
            engine: Engine::new(),
            effects: Vec::new(),
            open_effects: Vec::new(),
        }
    }

    pub fn layers(&mut self) -> &[Layer] {
        self.layers.flush();
        self.layers.as_slice()
    }

    pub fn draw(
        &mut self,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        viewport: &Viewport,
        damage: &[Rectangle],
        background_color: Color,
    ) {
        let scale_factor = viewport.scale_factor();
        let (width, height) = (pixels.width(), pixels.height());

        self.layers.flush();

        // pyzed: effect groups, outermost first.
        let mut groups = self.effects.clone();
        groups.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));

        for &damage_bounds in damage {
            let damage_bounds = damage_bounds * scale_factor;

            let path = tiny_skia::PathBuilder::from_rect(
                tiny_skia::Rect::from_xywh(
                    damage_bounds.x,
                    damage_bounds.y,
                    damage_bounds.width,
                    damage_bounds.height,
                )
                .expect("Create damage rectangle"),
            );

            pixels.fill_path(
                &path,
                &tiny_skia::Paint {
                    shader: tiny_skia::Shader::SolidColor(engine::into_color(
                        background_color,
                    )),
                    anti_alias: false,
                    blend_mode: tiny_skia::BlendMode::Source,
                    ..Default::default()
                },
                tiny_skia::FillRule::default(),
                tiny_skia::Transform::identity(),
                None,
            );

            // pyzed: the pixmaps of the open effect groups, with their blurred backdrops.
            let mut stack: Vec<(tiny_skia::Pixmap, Option<tiny_skia::Pixmap>)> =
                Vec::new();

            for (index, layer) in self.layers.iter().enumerate() {
                for group in groups.iter().filter(|g| g.start == index) {
                    let backdrop =
                        (group.effect.backdrop_blur > 0.0).then(|| {
                            let mut source = match stack.last() {
                                Some((pixmap, _)) => pixmap.clone(),
                                None => pixels.as_ref().to_owned(),
                            };
                            let radius =
                                group.effect.backdrop_blur * scale_factor;
                            if let Some(region) = effect::region(
                                group.effect.bounds * scale_factor,
                                radius * 1.5,
                                width,
                                height,
                            ) {
                                effect::blur(&mut source, region, radius);
                            }
                            source
                        });
                    stack.push((
                        tiny_skia::Pixmap::new(width, height)
                            .expect("Create effect pixmap"),
                        backdrop,
                    ));
                }

                match stack.last_mut() {
                    Some((pixmap, _)) => draw_layer(
                        &mut self.engine,
                        layer,
                        &mut pixmap.as_mut(),
                        clip_mask,
                        damage_bounds,
                        scale_factor,
                    ),
                    None => draw_layer(
                        &mut self.engine,
                        layer,
                        pixels,
                        clip_mask,
                        damage_bounds,
                        scale_factor,
                    ),
                }

                // pyzed: composite the groups that end after this layer, innermost first.
                for group in groups.iter().rev().filter(|g| g.end == index + 1)
                {
                    let Some((mut content, backdrop)) = stack.pop() else {
                        continue;
                    };
                    match stack.last_mut() {
                        Some((pixmap, _)) => composite(
                            &mut pixmap.as_mut(),
                            clip_mask,
                            &mut content,
                            backdrop.as_ref(),
                            group,
                            damage_bounds,
                            scale_factor,
                        ),
                        None => composite(
                            pixels,
                            clip_mask,
                            &mut content,
                            backdrop.as_ref(),
                            group,
                            damage_bounds,
                            scale_factor,
                        ),
                    }
                }
            }
        }

        self.engine.trim();
    }

    /// pyzed: whether the last frame drew effect layers (they need a full redraw).
    pub fn has_effects(&self) -> bool {
        !self.effects.is_empty()
    }

    /// pyzed: starts drawing a group of layers that is composited with `effect`.
    pub fn start_effect(&mut self, effect: effect::Effect) {
        let (layer, transformation) = self.layers.current_mut();
        let clip = layer.bounds;
        let mut effect = effect;
        effect.bounds = effect.bounds * transformation;
        let scale = transformation.scale_factor();
        effect.radius = effect.radius.map(|r| r * scale);
        effect.blur *= scale;
        effect.backdrop_blur *= scale;
        if let Some(bloom) = &mut effect.bloom {
            bloom.radius *= scale;
        }

        // `push_clip` transforms its bounds; the clip of the current layer already is.
        self.layers.push_clip(clip * transformation.inverse());
        let start = self.layers.as_slice().len() - 1;
        self.open_effects.push(self.effects.len());
        self.effects.push(effect::Group {
            start,
            end: start + 1,
            effect,
            clip,
        });
    }

    /// pyzed: ends the group started by the last [`Renderer::start_effect`].
    pub fn end_effect(&mut self) {
        self.layers.pop_clip();
        if let Some(open) = self.open_effects.pop() {
            self.effects[open].end = self.layers.as_slice().len();
        }
    }
}

/// pyzed: draws one layer (the body of iced's draw loop).
fn draw_layer(
    engine: &mut Engine,
    layer: &Layer,
    pixels: &mut tiny_skia::PixmapMut<'_>,
    clip_mask: &mut tiny_skia::Mask,
    damage_bounds: Rectangle,
    scale_factor: f32,
) {
    let Some(layer_bounds) =
        damage_bounds.intersection(&(layer.bounds * scale_factor))
    else {
        return;
    };

    engine::adjust_clip_mask(clip_mask, layer_bounds);

    if !layer.quads.is_empty() {
        let render_span = debug::render(debug::Primitive::Quad);
        for (quad, background) in &layer.quads {
            engine.draw_quad(
                quad,
                background,
                Transformation::scale(scale_factor),
                pixels,
                clip_mask,
                layer_bounds,
            );
        }
        render_span.finish();
    }

    if !layer.primitives.is_empty() {
        let render_span = debug::render(debug::Primitive::Triangle);

        for group in &layer.primitives {
            let Some(group_bounds) = (group.clip_bounds() * scale_factor)
                .intersection(&layer_bounds)
            else {
                continue;
            };

            engine::adjust_clip_mask(clip_mask, group_bounds);

            for primitive in group.as_slice() {
                engine.draw_primitive(
                    primitive,
                    Transformation::scale(scale_factor)
                        * group.transformation(),
                    pixels,
                    clip_mask,
                    group_bounds,
                );
            }

            engine::adjust_clip_mask(clip_mask, layer_bounds);
        }

        render_span.finish();
    }

    if !layer.images.is_empty() {
        let render_span = debug::render(debug::Primitive::Image);

        for image in &layer.images {
            engine.draw_image(
                image,
                Transformation::scale(scale_factor),
                pixels,
                clip_mask,
                layer_bounds,
            );
        }

        render_span.finish();
    }

    if !layer.text.is_empty() {
        let render_span = debug::render(debug::Primitive::Image);

        for group in &layer.text {
            let Some(group_bounds) = (group.clip_bounds() * scale_factor)
                .intersection(&layer_bounds)
            else {
                continue;
            };

            engine::adjust_clip_mask(clip_mask, group_bounds);

            for text in group.as_slice() {
                engine.draw_text(
                    text,
                    Transformation::scale(scale_factor)
                        * group.transformation(),
                    pixels,
                    clip_mask,
                    group_bounds,
                );
            }

            engine::adjust_clip_mask(clip_mask, layer_bounds);
        }

        render_span.finish();
    }
}

/// pyzed: draws a finished effect group onto `target`.
fn composite(
    target: &mut tiny_skia::PixmapMut<'_>,
    clip_mask: &mut tiny_skia::Mask,
    content: &mut tiny_skia::Pixmap,
    backdrop: Option<&tiny_skia::Pixmap>,
    group: &effect::Group,
    damage_bounds: Rectangle,
    scale_factor: f32,
) {
    let Some(clip) = (group.clip * scale_factor).intersection(&damage_bounds)
    else {
        return;
    };
    engine::adjust_clip_mask(clip_mask, clip);

    let fx = &group.effect;
    let bounds = fx.bounds * scale_factor;
    let center = bounds.center();
    let transform = tiny_skia::Transform::from_rotate_at(
        fx.rotation.to_degrees(),
        center.x,
        center.y,
    );
    let opacity = fx.opacity.clamp(0.0, 1.0);
    let (width, height) = (content.width(), content.height());
    let region = effect::region(group.clip * scale_factor, 0.0, width, height);

    let light = fx.bloom.filter(|b| b.intensity > 0.0).map(|bloom| {
        let mut light = effect::bright(content, bloom.threshold, bloom.color);
        if let Some(region) = region {
            effect::blur(
                &mut light,
                region,
                bloom.radius.max(1.0) * scale_factor,
            );
        }
        (light, bloom.intensity)
    });

    if fx.blur > 0.0 {
        if let Some(region) = region {
            effect::blur(content, region, fx.blur * scale_factor);
        }
    }

    if let Some(backdrop) = backdrop {
        let mut mask = clip_mask.clone();
        mask.intersect_path(
            &engine::rounded_rectangle(
                bounds,
                fx.radius.map(|r| r * scale_factor),
            ),
            tiny_skia::FillRule::Winding,
            true,
            transform,
        );
        target.draw_pixmap(
            0,
            0,
            backdrop.as_ref(),
            &tiny_skia::PixmapPaint {
                opacity,
                blend_mode: tiny_skia::BlendMode::SourceOver,
                quality: tiny_skia::FilterQuality::Nearest,
            },
            tiny_skia::Transform::identity(),
            Some(&mask),
        );
    }

    let quality = if fx.rotation == 0.0 {
        tiny_skia::FilterQuality::Nearest
    } else {
        tiny_skia::FilterQuality::Bilinear
    };

    // The light goes under the content, so the content stays legible.
    if let Some((light, intensity)) = light {
        let mut strength = intensity * opacity;
        while strength > 0.0 {
            target.draw_pixmap(
                0,
                0,
                light.as_ref(),
                &tiny_skia::PixmapPaint {
                    opacity: strength.min(1.0),
                    blend_mode: tiny_skia::BlendMode::Plus,
                    quality,
                },
                transform,
                Some(clip_mask),
            );
            strength -= 1.0;
        }
    }

    target.draw_pixmap(
        0,
        0,
        content.as_ref(),
        &tiny_skia::PixmapPaint {
            opacity,
            blend_mode: tiny_skia::BlendMode::SourceOver,
            quality,
        },
        transform,
        Some(clip_mask),
    );
}

impl core::Renderer for Renderer {
    fn start_layer(&mut self, bounds: Rectangle) {
        self.layers.push_clip(bounds);
    }

    fn end_layer(&mut self) {
        self.layers.pop_clip();
    }

    fn start_transformation(&mut self, transformation: Transformation) {
        self.layers.push_transformation(transformation);
    }

    fn end_transformation(&mut self) {
        self.layers.pop_transformation();
    }

    fn fill_quad(
        &mut self,
        quad: renderer::Quad,
        background: impl Into<Background>,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_quad(quad, background.into(), transformation);
    }

    fn reset(&mut self, new_bounds: Rectangle) {
        self.layers.reset(new_bounds);
        self.effects.clear();
        self.open_effects.clear();
    }

    fn allocate_image(
        &mut self,
        _handle: &core::image::Handle,
        callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>)
        + Send
        + 'static,
    ) {
        #[cfg(feature = "image")]
        #[allow(unsafe_code)]
        // TODO: Concurrency
        callback(self.engine.raster_pipeline.load(_handle));

        #[cfg(not(feature = "image"))]
        callback(Err(core::image::Error::Unsupported));
    }
}

impl core::text::Renderer for Renderer {
    type Font = Font;
    type Paragraph = Paragraph;
    type Editor = Editor;

    const ICON_FONT: Font = Font::with_name("Iced-Icons");
    const CHECKMARK_ICON: char = '\u{f00c}';
    const ARROW_DOWN_ICON: char = '\u{e800}';
    const ICED_LOGO: char = '\u{e801}';
    const SCROLL_UP_ICON: char = '\u{e802}';
    const SCROLL_DOWN_ICON: char = '\u{e803}';
    const SCROLL_LEFT_ICON: char = '\u{e804}';
    const SCROLL_RIGHT_ICON: char = '\u{e805}';

    fn default_font(&self) -> Self::Font {
        self.default_font
    }

    fn default_size(&self) -> Pixels {
        self.default_text_size
    }

    fn fill_paragraph(
        &mut self,
        text: &Self::Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();

        layer.draw_paragraph(
            text,
            position,
            color,
            clip_bounds,
            transformation,
        );
    }

    fn fill_editor(
        &mut self,
        editor: &Self::Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_editor(editor, position, color, clip_bounds, transformation);
    }

    fn fill_text(
        &mut self,
        text: core::Text,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text(text, position, color, clip_bounds, transformation);
    }
}

impl graphics::text::Renderer for Renderer {
    fn fill_raw(&mut self, raw: graphics::text::Raw) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text_raw(raw, transformation);
    }
}

#[cfg(feature = "geometry")]
impl graphics::geometry::Renderer for Renderer {
    type Geometry = Geometry;
    type Frame = geometry::Frame;

    fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
        geometry::Frame::new(bounds)
    }

    fn draw_geometry(&mut self, geometry: Self::Geometry) {
        let (layer, transformation) = self.layers.current_mut();

        match geometry {
            Geometry::Live {
                primitives,
                images,
                text,
                clip_bounds,
            } => {
                layer.draw_primitive_group(
                    primitives,
                    clip_bounds,
                    transformation,
                );

                for image in images {
                    layer.draw_image(image, transformation);
                }

                layer.draw_text_group(text, clip_bounds, transformation);
            }
            Geometry::Cache(cache) => {
                layer.draw_primitive_cache(
                    cache.primitives,
                    cache.clip_bounds,
                    transformation,
                );

                for image in cache.images.iter() {
                    layer.draw_image(image.clone(), transformation);
                }

                layer.draw_text_cache(
                    cache.text,
                    cache.clip_bounds,
                    transformation,
                );
            }
        }
    }
}

impl graphics::mesh::Renderer for Renderer {
    fn draw_mesh(&mut self, _mesh: graphics::Mesh) {
        log::warn!("iced_tiny_skia does not support drawing meshes");
    }

    fn draw_mesh_cache(&mut self, _cache: iced_graphics::mesh::Cache) {
        log::warn!("iced_tiny_skia does not support drawing meshes");
    }
}

#[cfg(feature = "image")]
impl core::image::Renderer for Renderer {
    type Handle = core::image::Handle;

    fn load_image(
        &self,
        handle: &Self::Handle,
    ) -> Result<core::image::Allocation, core::image::Error> {
        self.engine.raster_pipeline.load(handle)
    }

    fn measure_image(
        &self,
        handle: &Self::Handle,
    ) -> Option<crate::core::Size<u32>> {
        self.engine.raster_pipeline.dimensions(handle)
    }

    fn draw_image(
        &mut self,
        image: core::Image,
        bounds: Rectangle,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_raster(image, bounds, clip_bounds, transformation);
    }
}

#[cfg(feature = "svg")]
impl core::svg::Renderer for Renderer {
    fn measure_svg(
        &self,
        handle: &core::svg::Handle,
    ) -> crate::core::Size<u32> {
        self.engine.vector_pipeline.viewport_dimensions(handle)
    }

    fn draw_svg(
        &mut self,
        svg: core::Svg,
        bounds: Rectangle,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_svg(svg, bounds, clip_bounds, transformation);
    }
}

impl compositor::Default for Renderer {
    type Compositor = window::Compositor;
}

impl renderer::Headless for Renderer {
    async fn new(
        default_font: Font,
        default_text_size: Pixels,
        backend: Option<&str>,
    ) -> Option<Self> {
        if backend.is_some_and(|backend| {
            !["tiny-skia", "tiny_skia"].contains(&backend)
        }) {
            return None;
        }

        Some(Self::new(default_font, default_text_size))
    }

    fn name(&self) -> String {
        "tiny-skia".to_owned()
    }

    fn screenshot(
        &mut self,
        size: Size<u32>,
        scale_factor: f32,
        background_color: Color,
    ) -> Vec<u8> {
        let viewport = Viewport::with_physical_size(size, scale_factor);

        window::compositor::screenshot(self, &viewport, background_color)
    }
}
