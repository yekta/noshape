//! Onshape's environment maps (Libraries ▸ Onshape images ▸ Environment
//! maps), used by Reflection analysis. Equirectangular, Z up, row 0 looks
//! straight up; generated procedurally after the library's previews.

use glam::DVec3;

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

/// Sky gradient above the horizon, sun glow, bright horizon line and a
/// darker blue ground that mirrors it.
fn blue_sky(d: DVec3) -> DVec3 {
    if d.z >= 0.0 {
        let sky = rgb(0xa9c8e6).lerp(rgb(0x2c5a9a), d.z.powf(0.6));
        let sun = DVec3::new(0.25, 0.0, 0.97).normalize();
        let glow = d.dot(sun).max(0.0).powf(400.0) + 0.35 * d.dot(sun).max(0.0).powf(30.0);
        sky + DVec3::splat(glow) + DVec3::splat(0.6 * band(d.z, 0.04))
    } else {
        rgb(0x7f9cbc).lerp(rgb(0x1f3654), (-d.z).powf(0.5)) + DVec3::splat(0.4 * band(d.z, 0.04))
    }
}

/// Where the view ray meets a ceiling at height 1 (None looking down).
fn ceiling(d: DVec3) -> Option<(f64, f64)> {
    (d.z > 0.02).then(|| (d.x / d.z, d.y / d.z))
}

fn room_floor(d: DVec3) -> DVec3 {
    rgb(0x8d8d8d).lerp(rgb(0x3b3b3b), (-d.z).max(0.0).powf(0.5))
}

fn ceiling_hexagonal(d: DVec3) -> DVec3 {
    let Some((x, y)) = ceiling(d) else { return room_floor(d) };
    // Distance to the nearest center of a hexagonal lattice.
    let (sx, sy) = (1.2, 1.2 * 3f64.sqrt());
    let cell = |ox: f64, oy: f64| {
        let (px, py) = (x - ox, y - oy);
        let (cx, cy) = ((px / sx).round() * sx, (py / sy).round() * sy);
        ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
    };
    let r = cell(0.0, 0.0).min(cell(sx * 0.5, sy * 0.5));
    let light = 1.0 - smooth(0.32, 0.38, r);
    rgb(0x303236).lerp(DVec3::ONE, light)
}

fn ceiling_square(d: DVec3) -> DVec3 {
    let Some((x, y)) = ceiling(d) else { return room_floor(d) };
    let f = |v: f64| (v / 1.5 - (v / 1.5).round()).abs();
    let light = (1.0 - smooth(0.22, 0.26, f(x))) * (1.0 - smooth(0.22, 0.26, f(y)));
    rgb(0x2e3034).lerp(DVec3::ONE, light)
}

/// Hue around the horizon, white above, black below.
fn chromatic(d: DVec3) -> DVec3 {
    let h = (d.y.atan2(d.x) / std::f64::consts::TAU + 0.5) * 6.0;
    let hue = DVec3::new((h - 3.0).abs() - 1.0, 2.0 - (h - 2.0).abs(), 2.0 - (h - 4.0).abs()).clamp(DVec3::ZERO, DVec3::ONE);
    if d.z >= 0.0 { hue.lerp(DVec3::ONE, d.z) } else { hue.lerp(DVec3::ZERO, -d.z) }
}

fn daylight_road(d: DVec3) -> DVec3 {
    if d.z > 0.12 {
        rgb(0xc6dcef).lerp(rgb(0x4f86c6), d.z.powf(0.7))
    } else if d.z > 0.0 {
        // Tree line along the road.
        let az = d.y.atan2(d.x);
        let top = 0.06 + 0.05 * (az * 7.0).sin().abs() + 0.02 * (az * 23.0).sin();
        if d.z < top { rgb(0x3e5a32) } else { rgb(0xc6dcef) }
    } else {
        // Asphalt with the road's center line straight ahead and behind.
        let lane = band(d.y / d.z.abs().max(0.05), 0.15);
        rgb(0x6d6f72).lerp(rgb(0x3a3b3d), (-d.z).powf(0.5)).lerp(rgb(0xe8e4d8), lane * 0.8)
    }
}

/// White bands above and below the horizon on black.
fn horizontal_stripe(d: DVec3) -> DVec3 {
    let e = d.z.asin().to_degrees();
    let s = [-60.0, -25.0, 10.0, 45.0].iter().map(|c| band(e - c, 9.0)).fold(0.0, f64::max);
    DVec3::splat(s)
}

/// Rings of light around a tunnel running along X.
fn light_tunnel(d: DVec3) -> DVec3 {
    let r = (d.y * d.y + d.z * d.z).sqrt().max(1e-6);
    let along = d.x / r;
    let ring = band(along - along.round(), 0.12);
    rgb(0x15171c).lerp(rgb(0xfff4e0), ring) + rgb(0x202833) * (1.0 - r)
}

/// Warm light strips along the ceiling, pillars and a dark platform.
fn metro_station(d: DVec3) -> DVec3 {
    if let Some((x, y)) = ceiling(d) {
        let strip = band(y - (y / 1.2).round() * 1.2, 0.12) * (1.0 - smooth(6.0, 12.0, x.abs()));
        return rgb(0x3a352d).lerp(rgb(0xfff1cf), strip);
    }
    let az = d.y.atan2(d.x);
    let pillar = band((az * 8.0 / std::f64::consts::PI).fract() - 0.5, 0.1);
    if d.z > -0.05 { rgb(0x6a6458).lerp(rgb(0xb8b2a2), pillar) } else { rgb(0x2a2724).lerp(rgb(0x4a4540), (-d.z).powf(0.4)) }
}

fn nightfall(d: DVec3) -> DVec3 {
    if d.z >= 0.0 {
        let glow = band(d.z, 0.35) * (0.5 + 0.5 * d.x.max(0.0));
        rgb(0x2a2f5c).lerp(rgb(0x0a0c1e), d.z.powf(0.5)) + rgb(0xff8a3c) * glow * 0.8
    } else {
        rgb(0x15172a).lerp(rgb(0x05060c), (-d.z).powf(0.5))
    }
}

/// Sky through the bridge's roof beams, walkway below.
fn pedestrian_bridge(d: DVec3) -> DVec3 {
    if let Some((x, y)) = ceiling(d) {
        let beam = band(x - x.round(), 0.08).max(band(y, 0.06));
        return rgb(0xd5e4f2).lerp(rgb(0x5b8fc8), d.z).lerp(rgb(0x3c3f44), beam);
    }
    if d.z > -0.02 {
        rgb(0x9fb8cf)
    } else {
        let plank = band(d.x / d.z.abs().max(0.05) * 2.0 - (d.x / d.z.abs().max(0.05) * 2.0).round(), 0.1);
        rgb(0x8a7a64).lerp(rgb(0x4d4337), plank)
    }
}

/// Vertical white stripes on black walls.
fn wall_stripes(d: DVec3) -> DVec3 {
    let az = d.y.atan2(d.x) / std::f64::consts::TAU * 16.0;
    let s = band(az - az.round(), 0.3) * (1.0 - smooth(0.8, 0.95, d.z.abs()));
    DVec3::splat(s)
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
