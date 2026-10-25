//! pyzed: effect layers, in software.
//!
//! A widget draws part of the tree between [`Renderer::start_effect`](crate::Renderer::start_effect)
//! and [`Renderer::end_effect`](crate::Renderer::end_effect). Those layers are drawn into their
//! own pixmap and composited back with a rotation, an opacity, a blur and a bloom, over a blurred
//! copy of what is behind them (backdrop blur). Same API as the `iced_wgpu` version.
use crate::core::{Color, Rectangle};

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
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Group {
    pub start: usize,
    pub end: usize,
    pub effect: Effect,
    /// Where the composited group may draw (the clip of the layer it was started in), logical.
    pub clip: Rectangle,
}

/// Blurs the pixels of `pixmap` inside `region` (physical pixels), like a gaussian of `radius`.
pub(crate) fn blur(
    pixmap: &mut tiny_skia::Pixmap,
    region: tiny_skia::IntRect,
    radius: f32,
) {
    let sigma = (radius / 2.0).max(0.5);
    // Three box blurs approximate a gaussian (http://blog.ivank.net/fastest-gaussian-blur.html).
    let n = 3.0;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut lower = ideal.floor() as i32;
    if lower % 2 == 0 {
        lower -= 1;
    }
    let upper = lower + 2;
    let m = ((12.0 * sigma * sigma
        - n * (lower * lower) as f32
        - 4.0 * n * lower as f32
        - 3.0 * n)
        / (-4.0 * lower as f32 - 4.0))
        .round() as i32;

    let width = pixmap.width() as usize;
    let x0 = region.x().max(0) as usize;
    let y0 = region.y().max(0) as usize;
    let x1 = (region.x() + region.width() as i32).min(pixmap.width() as i32)
        as usize;
    let y1 = (region.y() + region.height() as i32).min(pixmap.height() as i32)
        as usize;
    if x1 <= x0 || y1 <= y0 {
        return;
    }

    let data = pixmap.data_mut();
    let mut line: Vec<[u32; 4]> = Vec::new();

    for pass in 0..3 {
        let size = if pass < m { lower } else { upper };
        let r = ((size - 1) / 2).max(0) as usize;
        if r == 0 {
            continue;
        }
        // horizontal
        for y in y0..y1 {
            box_line(data, (y * width + x0) * 4, 4, x1 - x0, r, &mut line);
        }
        // vertical
        for x in x0..x1 {
            box_line(
                data,
                (y0 * width + x) * 4,
                width * 4,
                y1 - y0,
                r,
                &mut line,
            );
        }
    }
}

/// One box blur of `len` pixels starting at byte `start`, `step` bytes apart.
fn box_line(
    data: &mut [u8],
    start: usize,
    step: usize,
    len: usize,
    r: usize,
    line: &mut Vec<[u32; 4]>,
) {
    line.clear();
    line.extend((0..len).map(|i| {
        let o = start + i * step;
        [
            data[o] as u32,
            data[o + 1] as u32,
            data[o + 2] as u32,
            data[o + 3] as u32,
        ]
    }));
    let window = (2 * r + 1) as u32;
    let mut sum = [0u32; 4];
    // edges extend with transparent pixels
    for px in line.iter().take(r + 1) {
        for c in 0..4 {
            sum[c] += px[c];
        }
    }
    for i in 0..len {
        let o = start + i * step;
        for c in 0..4 {
            data[o + c] = (sum[c] / window) as u8;
        }
        if i + r + 1 < len {
            let add = line[i + r + 1];
            for c in 0..4 {
                sum[c] += add[c];
            }
        }
        if i >= r {
            let sub = line[i - r];
            for c in 0..4 {
                sum[c] -= sub[c];
            }
        }
    }
}

/// The parts of `source` brighter than `threshold` (luminance, 0 to 1), optionally tinted.
pub(crate) fn bright(
    source: &tiny_skia::Pixmap,
    threshold: f32,
    tint: Option<Color>,
) -> tiny_skia::Pixmap {
    let mut out = source.clone();
    for px in out.data_mut().chunks_exact_mut(4) {
        let a = px[3] as f32 / 255.0;
        if a <= 0.0 {
            continue;
        }
        // tiny-skia keeps iced's pixels as BGRA
        let (b, g, r) = (
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
        );
        let l = (0.2126 * r + 0.7152 * g + 0.0722 * b) / a;
        let t = ((l - threshold) / 0.25).clamp(0.0, 1.0);
        let k = t * t * (3.0 - 2.0 * t);
        match tint {
            Some(tint) => {
                let m = r.max(g).max(b) * k;
                px[0] = (tint.b * m * 255.0) as u8;
                px[1] = (tint.g * m * 255.0) as u8;
                px[2] = (tint.r * m * 255.0) as u8;
                px[3] = (a * k * 255.0) as u8;
            }
            None => {
                for c in px.iter_mut() {
                    *c = (*c as f32 * k) as u8;
                }
            }
        }
    }
    out
}

/// `rect` (physical pixels) grown by `margin` and clipped to `width` x `height`.
pub(crate) fn region(
    rect: Rectangle,
    margin: f32,
    width: u32,
    height: u32,
) -> Option<tiny_skia::IntRect> {
    let screen = Rectangle::new(
        crate::core::Point::ORIGIN,
        crate::core::Size::new(width as f32, height as f32),
    );
    let grown = Rectangle {
        x: rect.x - margin,
        y: rect.y - margin,
        width: rect.width + margin * 2.0,
        height: rect.height + margin * 2.0,
    }
    .intersection(&screen)?
    .snap()?;
    tiny_skia::IntRect::from_xywh(
        grown.x as i32,
        grown.y as i32,
        grown.width,
        grown.height,
    )
}
