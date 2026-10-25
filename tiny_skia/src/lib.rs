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
        // pyzed: what each group draws, physical (a rotated key in a timeline is a few pixels
        // of a group as large as the panel).
        let extents =
            group_extents(&groups, self.layers.as_slice(), scale_factor);

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
                for (group, extent) in groups
                    .iter()
                    .zip(&extents)
                    .filter(|(g, _)| g.start == index)
                {
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
                        effect_pixmap(
                            width,
                            height,
                            group,
                            *extent,
                            damage_bounds,
                            scale_factor,
                        ),
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
                for (group, extent) in groups
                    .iter()
                    .zip(&extents)
                    .rev()
                    .filter(|(g, _)| g.end == index + 1)
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
                            *extent,
                            damage_bounds,
                            scale_factor,
                        ),
                        None => composite(
                            pixels,
                            clip_mask,
                            &mut content,
                            backdrop.as_ref(),
                            group,
                            *extent,
                            damage_bounds,
                            scale_factor,
                        ),
                    }
                    EFFECT_PIXMAPS.with_borrow_mut(|pool| {
                        if pool.len() < 2 {
                            pool.push(content);
                        }
                    });
                }
            }
        }

        self.engine.trim();
    }

    /// pyzed: each effect group of the frame with the part of the window it can change,
    /// logical: what its layers draw, turned, or its whole clip when it blurs or glows.
    pub(crate) fn effect_regions(&self) -> Vec<(effect::Group, Rectangle)> {
        let extents = group_extents(&self.effects, self.layers.as_slice(), 1.0);
        self.effects
            .iter()
            .zip(extents)
            .map(|(group, extent)| {
                let region = match extent {
                    Some(extent) if is_tight(group) => group
                        .clip
                        .intersection(
                            &extent
                                .union(&turned(
                                    extent,
                                    effect_transform(group, 1.0),
                                ))
                                .expand(2.0),
                        )
                        .unwrap_or(Rectangle::new(
                            group.clip.position(),
                            Size::ZERO,
                        )),
                    _ => group.clip,
                };
                (group.clone(), region)
            })
            .collect()
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

    // pyzed: the clip mask is adjusted by whatever needs it (most of what is drawn is inside
    // its clip bounds and draws without one).
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
    extent: Option<Rectangle>,
    damage_bounds: Rectangle,
    scale_factor: f32,
) {
    let Some(clip) = (group.clip * scale_factor).intersection(&damage_bounds)
    else {
        return;
    };

    let fx = &group.effect;
    let bounds = fx.bounds * scale_factor;
    let transform = effect_transform(group, scale_factor);
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

    if fx.blur > 0.0
        && let Some(region) = region
    {
        effect::blur(content, region, fx.blur * scale_factor);
    }

    if let Some(backdrop) = backdrop {
        engine::adjust_clip_mask(clip_mask, clip);
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
            draw_clipped(
                target,
                light.as_ref(),
                tiny_skia::PixmapPaint {
                    opacity: strength.min(1.0),
                    blend_mode: tiny_skia::BlendMode::Plus,
                    quality,
                },
                transform,
                clip,
            );
            strength -= 1.0;
        }
    }

    // Without blur or bloom nothing is drawn beyond what the group drew, turned.
    let drawn = match extent {
        Some(extent) if is_tight(group) => {
            match clip.intersection(&turned(extent, transform).expand(1.0)) {
                Some(drawn) => drawn,
                None => return,
            }
        }
        _ => clip,
    };

    draw_clipped(
        target,
        content.as_ref(),
        tiny_skia::PixmapPaint {
            opacity,
            blend_mode: tiny_skia::BlendMode::SourceOver,
            quality,
        },
        transform,
        drawn,
    );
}

/// pyzed: how the content of `group` is turned onto its target.
fn effect_transform(
    group: &effect::Group,
    scale_factor: f32,
) -> tiny_skia::Transform {
    let center = (group.effect.bounds * scale_factor).center();
    tiny_skia::Transform::from_rotate_at(
        group.effect.rotation.to_degrees(),
        center.x,
        center.y,
    )
}

/// pyzed: what each of `groups` draws (see `content_extent`), scaled; `None` when a group
/// holds another one, which may turn its content beyond the layers.
fn group_extents(
    groups: &[effect::Group],
    layers: &[Layer],
    scale_factor: f32,
) -> Vec<Option<Rectangle>> {
    groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let nested = groups.iter().enumerate().any(|(j, other)| {
                j != i && group.start <= other.start && other.end <= group.end
            });
            if nested {
                return None;
            }
            layers
                .get(group.start..group.end.min(layers.len()))?
                .iter()
                .filter_map(|layer| content_extent(layer, scale_factor))
                .reduce(|a, b| a.union(&b))
        })
        .collect()
}

/// pyzed: whether a group reads around what it draws (turned, blurred or glowing): it is
/// drawn whole whenever part of it is.
pub(crate) fn reads_around(group: &effect::Group) -> bool {
    group.effect.rotation != 0.0 || !is_tight(group)
}

/// pyzed: whether a group draws only over what its layers drew (no blur, bloom or backdrop).
fn is_tight(group: &effect::Group) -> bool {
    let fx = &group.effect;
    fx.blur <= 0.0
        && fx.backdrop_blur <= 0.0
        && fx.bloom.is_none_or(|bloom| bloom.intensity <= 0.0)
}

/// pyzed: the bounding box of `bounds` moved by `transform`.
fn turned(bounds: Rectangle, transform: tiny_skia::Transform) -> Rectangle {
    let mut points = [
        tiny_skia::Point::from_xy(bounds.x, bounds.y),
        tiny_skia::Point::from_xy(bounds.x + bounds.width, bounds.y),
        tiny_skia::Point::from_xy(bounds.x, bounds.y + bounds.height),
        tiny_skia::Point::from_xy(
            bounds.x + bounds.width,
            bounds.y + bounds.height,
        ),
    ];
    transform.map_points(&mut points);
    let (mut x0, mut y0) = (f32::INFINITY, f32::INFINITY);
    let (mut x1, mut y1) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    for point in points {
        x0 = x0.min(point.x);
        y0 = y0.min(point.y);
        x1 = x1.max(point.x);
        y1 = y1.max(point.y);
    }
    Rectangle {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    }
}

/// pyzed: where `layer` draws, physical: its quads (with their shadows), or its whole bounds
/// when it has anything else.
fn content_extent(layer: &Layer, scale_factor: f32) -> Option<Rectangle> {
    let bounds = layer.bounds * scale_factor;
    if !layer.primitives.is_empty()
        || !layer.images.is_empty()
        || !layer.text.is_empty()
    {
        return Some(bounds);
    }
    layer
        .quads
        .iter()
        .map(|(quad, _)| {
            let shadow = if quad.shadow.color.a > 0.0 {
                quad.shadow.offset.x.abs().max(quad.shadow.offset.y.abs())
                    + quad.shadow.blur_radius
            } else {
                0.0
            };
            quad.bounds.expand(shadow) * scale_factor
        })
        .reduce(|a, b| a.union(&b))?
        .intersection(&bounds)
}

thread_local! {
    /// pyzed: the pixmaps of finished effect groups, drawn again by the next ones.
    static EFFECT_PIXMAPS: std::cell::RefCell<Vec<tiny_skia::Pixmap>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// pyzed: a transparent pixmap for the content of `group`: a used one when there is one of the
/// size, cleared only where the group draws and is read (a new one each time cost a whole
/// window of memory per group, per frame).
fn effect_pixmap(
    width: u32,
    height: u32,
    group: &effect::Group,
    extent: Option<Rectangle>,
    damage_bounds: Rectangle,
    scale_factor: f32,
) -> tiny_skia::Pixmap {
    let used = EFFECT_PIXMAPS.with_borrow_mut(|pool| {
        let index = pool
            .iter()
            .position(|p| p.width() == width && p.height() == height)?;
        Some(pool.swap_remove(index))
    });
    let Some(mut pixmap) = used else {
        return tiny_skia::Pixmap::new(width, height)
            .expect("Create effect pixmap");
    };

    // Outside the damage nothing is drawn into it, nor composited.
    if !(group.clip * scale_factor).intersects(&damage_bounds) {
        return pixmap;
    }

    match extent.filter(|_| is_tight(group)) {
        Some(extent) => {
            // What composite reads: the turned extent, turned back (a turned key reads
            // around its corners).
            let turn = effect_transform(group, scale_factor);
            let read = match turn.invert() {
                Some(back) => {
                    turned(turned(extent, turn).expand(1.0), back).expand(2.0)
                }
                None => Rectangle::new(
                    Point::ORIGIN,
                    Size::new(width as f32, height as f32),
                ),
            };
            clear_rect(&mut pixmap, read);
        }
        None => pixmap.fill(tiny_skia::Color::TRANSPARENT),
    }

    pixmap
}

/// pyzed: makes the pixels of `pixmap` inside `bounds` transparent.
fn clear_rect(pixmap: &mut tiny_skia::Pixmap, bounds: Rectangle) {
    let (width, height) = (pixmap.width() as usize, pixmap.height() as usize);
    let x0 = (bounds.x.floor().max(0.0) as usize).min(width);
    let x1 = ((bounds.x + bounds.width).ceil().max(0.0) as usize).min(width);
    let y0 = (bounds.y.floor().max(0.0) as usize).min(height);
    let y1 = ((bounds.y + bounds.height).ceil().max(0.0) as usize).min(height);
    if x0 >= x1 {
        return;
    }
    let data = pixmap.data_mut();
    for y in y0..y1 {
        data[(y * width + x0) * 4..(y * width + x1) * 4].fill(0);
    }
}

/// pyzed: draws `pixmap` (as large as `target`) through the rectangle `clip` only: drawn whole
/// with a clip mask, every effect group cost every pixel of the window.
fn draw_clipped(
    target: &mut tiny_skia::PixmapMut<'_>,
    pixmap: tiny_skia::PixmapRef<'_>,
    paint: tiny_skia::PixmapPaint,
    transform: tiny_skia::Transform,
    clip: Rectangle,
) {
    let clip = engine::snap_to_pixels(clip);
    let Some(rect) =
        tiny_skia::Rect::from_xywh(clip.x, clip.y, clip.width, clip.height)
    else {
        return;
    };

    target.fill_rect(
        rect,
        &tiny_skia::Paint {
            shader: tiny_skia::Pattern::new(
                pixmap,
                tiny_skia::SpreadMode::Pad,
                paint.quality,
                paint.opacity,
                transform,
            ),
            blend_mode: paint.blend_mode,
            anti_alias: false,
            ..tiny_skia::Paint::default()
        },
        tiny_skia::Transform::identity(),
        None,
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
