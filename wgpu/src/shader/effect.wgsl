// pyzed: effect layers (see effect.rs). Every pass draws one full-screen triangle; the
// scissor rectangle limits the work to the area that changes.

struct Uniforms {
    size: vec4<f32>,       // width, height, 1 / width, 1 / height
    center_rot: vec4<f32>, // rotation center (physical pixels), cos, sin
    params: vec4<f32>,     // opacity, blur sigma, bright threshold, unused
    dir: vec4<f32>,        // blur direction
    rect: vec4<f32>,       // backdrop rectangle: x, y, width, height
    radii: vec4<f32>,      // backdrop corner radii: top left, top right, bottom right, bottom left
    tint: vec4<f32>,       // bloom tint (a = 1 to use it)
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(index & 1u) * 4 - 1);
    let y = f32(i32(index >> 1u) * 4 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn fetch(p: vec2<f32>) -> vec4<f32> {
    if (p.x < 0.0 || p.y < 0.0 || p.x > u.size.x || p.y > u.size.y) {
        return vec4<f32>(0.0);
    }
    return textureSampleLevel(source, source_sampler, p * u.size.zw, 0.0);
}

// Where the pixel `p` of a target comes from in a source rotated around the center.
fn unrotate(p: vec2<f32>) -> vec2<f32> {
    let c = u.center_rot.xy;
    let d = p - c;
    let cs = u.center_rot.z;
    let sn = u.center_rot.w;
    return c + vec2<f32>(cs * d.x + sn * d.y, -sn * d.x + cs * d.y);
}

@fragment
fn fs_blur(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let sigma = u.params.y;
    let reach = ceil(sigma * 3.0);
    let stride = max(1.0, ceil(reach / 32.0));
    let steps = i32(ceil(reach / stride));
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var i = -steps; i <= steps; i = i + 1) {
        let offset = f32(i) * stride;
        let weight = exp(-(offset * offset) / (2.0 * sigma * sigma));
        sum = sum + fetch(position.xy + u.dir.xy * offset) * weight;
        total = total + weight;
    }
    return sum / total;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_bright(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let c = fetch(position.xy);
    if (c.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let l = luma(c.rgb / c.a);
    let t = u.params.z;
    return c * smoothstep(t, t + 0.25, l);
}

@fragment
fn fs_composite(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let c = fetch(unrotate(position.xy)) * u.params.x;
    if (u.tint.a > 0.0) {
        return vec4<f32>(u.tint.rgb * max(c.r, max(c.g, c.b)), c.a);
    }
    return c;
}

fn rounded_rect(p: vec2<f32>) -> f32 {
    let half = u.rect.zw * 0.5;
    let center = u.rect.xy + half;
    let d = p - center;
    var r = u.radii.x;
    if (d.x > 0.0 && d.y < 0.0) {
        r = u.radii.y;
    } else if (d.x > 0.0 && d.y > 0.0) {
        r = u.radii.z;
    } else if (d.x < 0.0 && d.y > 0.0) {
        r = u.radii.w;
    }
    r = min(r, min(half.x, half.y));
    let q = abs(d) - half + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_backdrop(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let coverage = clamp(0.5 - rounded_rect(unrotate(position.xy)), 0.0, 1.0);
    return fetch(position.xy) * coverage * u.params.x;
}
