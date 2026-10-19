//! Onshape's environment maps (Libraries ▸ Onshape images ▸ Environment
//! maps), used by Reflection analysis. Equirectangular, Z up, row 0 looks
//! straight up; generated procedurally after the library's previews.

use glam::{DVec2, DVec3};

/// The library's images, in its order.
pub const NAMES: [&str; 11] = ["Blue sky", "Ceiling hexagonal", "Ceiling square", "Chromatic", "Daylight road", "Horizontal stripe", "Light tunnel", "Metro station", "Nightfall", "Pedestrian bridge", "Wall stripes"];

pub const WIDTH: u32 = 512;
pub const HEIGHT: u32 = 256;

/// An environment image: sRGB RGBA8, row 0 on top.
pub struct EnvImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The named library image, `None` for an unknown name.
pub fn generate(name: &str) -> Option<EnvImage> {
    let f: fn(DVec3) -> DVec3 = match name {
        "Blue sky" => blue_sky,
        "Ceiling hexagonal" => ceiling_hexagonal,
        "Ceiling square" => ceiling_square,
        "Chromatic" => chromatic,
        "Daylight road" => daylight_road,
        "Horizontal stripe" => horizontal_stripe,
        "Light tunnel" => light_tunnel,
        "Metro station" => metro_station,
        "Nightfall" => nightfall,
        "Pedestrian bridge" => pedestrian_bridge,
        "Wall stripes" => wall_stripes,
        _ => return None,
    };
    let mut rgba = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        let polar = (y as f64 + 0.5) / HEIGHT as f64 * std::f64::consts::PI;
        for x in 0..WIDTH {
            // u = 0.5 looks along +X (the shader's atan2 convention).
            let az = ((x as f64 + 0.5) / WIDTH as f64 - 0.5) * std::f64::consts::TAU;
            let d = DVec3::new(polar.sin() * az.cos(), polar.sin() * az.sin(), polar.cos());
            let c = f(d).clamp(DVec3::ZERO, DVec3::ONE);
            rgba.extend_from_slice(&[(c.x * 255.0).round() as u8, (c.y * 255.0).round() as u8, (c.z * 255.0).round() as u8, 255]);
        }
    }
    Some(EnvImage { width: WIDTH, height: HEIGHT, rgba })
}

fn rgb(hex: u32) -> DVec3 {
    DVec3::new(((hex >> 16) & 0xff) as f64, ((hex >> 8) & 0xff) as f64, (hex & 0xff) as f64) / 255.0
}

fn smooth(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A soft band of `width` around 0.
fn band(x: f64, width: f64) -> f64 {
    1.0 - smooth(width * 0.5, width, x.abs())
}

/// Cheap hash noise in 0..1 (grain for floors and stars).
fn noise(d: DVec3, scale: f64) -> f64 {
    let q = (d * scale).floor();
    let h = (q.x * 127.1 + q.y * 311.7 + q.z * 74.7).sin() * 43758.5453;
    h - h.floor()
}

/// Sky gradient with a high sun and a bright horizon, mirrored below as
/// a calm sea with the sun's reflection.
fn blue_sky(d: DVec3) -> DVec3 {
    let sun = DVec3::new(0.64, 0.0, 0.77).normalize();
    let glow = |d: DVec3| d.dot(sun).max(0.0).powf(600.0) * 1.5 + 0.25 * d.dot(sun).max(0.0).powf(25.0);
    if d.z >= 0.0 {
        rgb(0xb7cbe0).lerp(rgb(0x3d6aa6), d.z.powf(0.55)) + DVec3::splat(glow(d) + 0.45 * band(d.z, 0.06))
    } else {
        let m = DVec3::new(d.x, d.y, -d.z);
        rgb(0x8ea6c2).lerp(rgb(0x22385a), (-d.z).powf(0.5)) + DVec3::splat(0.5 * glow(m).min(0.6) + 0.4 * band(d.z, 0.06))
    }
}

/// Where the view ray meets a ceiling at height 1 (None looking down).
fn ceiling(d: DVec3) -> Option<(f64, f64)> {
    (d.z > 0.02).then(|| (d.x / d.z, d.y / d.z))
}

/// Walls and floor of a soft grey room.
fn room_floor(d: DVec3) -> DVec3 {
    if d.z > 0.0 { rgb(0x6e6c78).lerp(rgb(0x4a4854), d.z) } else { rgb(0x7a7884).lerp(rgb(0x8c8a96), (-d.z).powf(0.5)) }
}

/// White hexagonal light panels in a dark frame overhead.
fn ceiling_hexagonal(d: DVec3) -> DVec3 {
    let Some((x, y)) = ceiling(d).filter(|_| d.z > 0.12) else { return room_floor(d) };
    // Nearest center of a hexagonal lattice (two offset rectangular grids).
    let (sx, sy) = (0.8 * 3f64.sqrt(), 1.2);
    let near = |ox: f64, oy: f64| {
        let (px, py) = (x - ox, y - oy);
        let (cx, cy) = ((px / sx).round() * sx, (py / sy).round() * sy);
        DVec2::new(px - cx, py - cy)
    };
    let (a, b) = (near(0.0, 0.0), near(sx * 0.5, sy * 0.5));
    let q = if a.length() < b.length() { a } else { b };
    // Hexagon "radius" (flat top and bottom).
    let hex = (q.x.abs() * 3f64.sqrt() * 0.5 + q.y.abs() * 0.5).max(q.y.abs());
    let panel = 1.0 - smooth(0.5, 0.54, hex);
    rgb(0x2a2a2e).lerp(DVec3::ONE, panel)
}

/// A grid of white light strips on a dark ceiling.
fn ceiling_square(d: DVec3) -> DVec3 {
    let Some((x, y)) = ceiling(d).filter(|_| d.z > 0.12) else { return room_floor(d) };
    let line = |v: f64| band(v / 0.6 - (v / 0.6).round(), 0.08);
    rgb(0x3a3648).lerp(DVec3::ONE, line(x).max(line(y)))
}

/// Hue around the horizon, white above, black below.
fn chromatic(d: DVec3) -> DVec3 {
    let h = (d.y.atan2(d.x) / std::f64::consts::TAU + 0.5) * 6.0;
    let hue = DVec3::new((h - 3.0).abs() - 1.0, 2.0 - (h - 2.0).abs(), 2.0 - (h - 4.0).abs()).clamp(DVec3::ZERO, DVec3::ONE);
    if d.z >= 0.0 { hue.lerp(DVec3::splat(0.85), d.z * 0.8) } else { hue.lerp(DVec3::splat(0.1), -d.z * 0.8) }
}

/// Sky, a low sun, fields either side of a straight road.
fn daylight_road(d: DVec3) -> DVec3 {
    let sun = DVec3::new(-0.9, 0.4, 0.06).normalize();
    if d.z > 0.04 {
        rgb(0xd9e4ee).lerp(rgb(0x4a7fc0), d.z.powf(0.6)) + DVec3::splat(d.dot(sun).max(0.0).powf(300.0) * 1.5 + 0.3 * d.dot(sun).max(0.0).powf(12.0))
    } else if d.z > -0.04 {
        // Crops along the horizon.
        rgb(0x6f7a3c).lerp(rgb(0x9a9a52), noise(d, 60.0))
    } else {
        // Asphalt straight ahead and behind, grass to the sides.
        let road = 1.0 - smooth(0.25, 0.4, (d.y / d.z.abs().max(0.05)).abs());
        rgb(0x5d6a2c).lerp(rgb(0x3e3c3a), road) * (0.85 + 0.3 * noise(d, 200.0))
    }
}

/// One bright band along the horizon on black.
fn horizontal_stripe(d: DVec3) -> DVec3 {
    DVec3::splat(band(d.z, 0.035) + 0.25 * band(d.z, 0.1))
}

/// Light rings over a tunnel running along X, grey road surface below.
fn light_tunnel(d: DVec3) -> DVec3 {
    if d.z < 0.0 {
        return rgb(0x8a8f9a) * (0.75 + 0.5 * noise(d, 300.0));
    }
    let r = (d.y * d.y + d.z * d.z).sqrt().max(1e-6);
    let along = d.x / r * 1.5;
    let ring = band(along - along.round(), 0.07) * (1.0 - smooth(0.2, 0.05, d.z));
    DVec3::splat(ring)
}

/// A bright station: pale walls and vault, a dark band of platform and
/// tracks at eye level, columns.
fn metro_station(d: DVec3) -> DVec3 {
    let az = d.y.atan2(d.x);
    let column = band((az * 4.0 / std::f64::consts::PI).fract() - 0.5, 0.04) * (1.0 - smooth(0.3, 0.5, d.z.abs()));
    let base = if d.z > 0.12 {
        rgb(0xd8d2c8).lerp(rgb(0xf0ede8), d.z)
    } else if d.z > -0.05 {
        rgb(0x2a2a2c)
    } else {
        rgb(0xc8c2b8).lerp(rgb(0xe2ddd5), (-d.z).powf(0.5))
    };
    base.lerp(rgb(0x707070), column)
}

/// Dusk: a starry sky, an orange glow low on the horizon, dark ground.
fn nightfall(d: DVec3) -> DVec3 {
    if d.z >= 0.0 {
        let stars = if noise(d, 400.0) > 0.997 { 0.8 } else { 0.0 };
        let glow = band(d.z, 0.25) * (0.4 + 0.6 * (-d.x).max(0.0));
        rgb(0x3a4458).lerp(rgb(0x10141e), d.z.powf(0.4)) + rgb(0xff9a48) * glow * 0.7 + DVec3::splat(stars)
    } else {
        rgb(0x3b2a1c).lerp(rgb(0x1e150e), (-d.z).powf(0.5)) * (0.8 + 0.4 * noise(d, 120.0))
    }
}

/// A glazed footbridge: white curved roof, a band of blue glass with black
/// posts at eye level, a pale floor.
fn pedestrian_bridge(d: DVec3) -> DVec3 {
    if d.z > 0.18 {
        return rgb(0xd8dce2).lerp(rgb(0xf6f7f8), d.z);
    }
    if d.z > -0.02 {
        let az = d.y.atan2(d.x) / std::f64::consts::TAU * 36.0;
        let post = band(az - az.round(), 0.08);
        return rgb(0x6d93b8).lerp(rgb(0x9cc0dc), noise(d, 40.0) * 0.5).lerp(rgb(0x101010), post);
    }
    rgb(0xb4bcc8).lerp(rgb(0xdfe3e8), (-d.z).powf(0.5))
}

/// Purple room with white horizontal stripes on two facing walls.
fn wall_stripes(d: DVec3) -> DVec3 {
    let facing = smooth(0.55, 0.8, d.x.abs());
    let h = d.z / d.x.abs().max(0.2) * 3.0;
    let stripe = band(h - h.round(), 0.15) * facing * (1.0 - smooth(1.0, 1.6, h.abs()));
    rgb(0x8a5fb8).lerp(rgb(0x6e4596), d.z.abs()).lerp(DVec3::ONE, stripe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_library_image_generates() {
        for name in NAMES {
            let img = generate(name).unwrap();
            assert_eq!(img.rgba.len(), (img.width * img.height * 4) as usize);
            // Not a flat color: the maps have something to reflect.
            let lum: Vec<u32> = img.rgba.chunks(4).map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32).collect();
            let (lo, hi) = (lum.iter().min().unwrap(), lum.iter().max().unwrap());
            assert!(hi - lo > 150, "{name}: {lo}..{hi}");
        }
        assert!(generate("Nope").is_none());
    }
}
