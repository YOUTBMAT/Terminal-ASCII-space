use rand::{rngs::StdRng, Rng};

use super::{Moon, MoonStyle, Planet, PlanetStyle, Rgb};

pub(super) fn new_planet(
    rng: &mut StdRng,
    width: u16,
    height: u16,
    x: f32,
    active: bool,
) -> Planet {
    let style = match rng.gen_range(0..7) {
        0 => PlanetStyle::Gas,
        1 => PlanetStyle::Ocean,
        2 => PlanetStyle::Ember,
        3 => PlanetStyle::Ice,
        4 => PlanetStyle::Violet,
        5 => PlanetStyle::Verdant,
        _ => PlanetStyle::Cratered,
    };
    let max_radius = max_planet_radius(width, height);
    let radius = rng.gen_range(max_radius * 0.32..max_radius);
    let moons = [new_moon(rng), new_moon(rng)];
    Planet {
        x,
        y: rng.gen_range(0.18..0.82) * height.max(1) as f32 * 2.0,
        radius,
        phase: rng.gen_range(0.0..std::f32::consts::TAU),
        ring_angle: rng.gen_range(-0.45..0.45),
        drift_speed: 0.45 + radius * 0.07,
        style,
        ringed: rng.gen_bool(0.52),
        active: active && width > 0,
        moons,
        moon_count: rng.gen_range(1..=2),
    }
}

fn new_moon(rng: &mut StdRng) -> Moon {
    let style = match rng.gen_range(0..4) {
        0 => MoonStyle::Rock,
        1 => MoonStyle::Ice,
        2 => MoonStyle::Volcanic,
        _ => MoonStyle::Ocean,
    };
    Moon {
        size_factor: rng.gen_range(0.09..0.25),
        orbit_distance: rng.gen_range(1.45..2.45),
        orbit_height: rng.gen_range(0.28..0.78),
        phase: rng.gen_range(0.0..std::f32::consts::TAU),
        orbit_speed: rng.gen_range(0.12..0.32),
        style,
    }
}

pub(super) fn max_planet_radius(width: u16, height: u16) -> f32 {
    (height as f32 * 0.78)
        .min(width.max(12) as f32 * 0.3)
        .clamp(5.5, 28.0)
}

pub(super) fn next_planet_style(style: PlanetStyle) -> PlanetStyle {
    match style {
        PlanetStyle::Gas => PlanetStyle::Ocean,
        PlanetStyle::Ocean => PlanetStyle::Ember,
        PlanetStyle::Ember => PlanetStyle::Ice,
        PlanetStyle::Ice => PlanetStyle::Violet,
        PlanetStyle::Violet => PlanetStyle::Verdant,
        PlanetStyle::Verdant => PlanetStyle::Cratered,
        PlanetStyle::Cratered => PlanetStyle::Gas,
    }
}

pub(super) fn ring_pixel(planet: Planet, x: f32, y: f32, front: bool) -> Option<Rgb> {
    let dx = x - planet.x;
    let dy = y - planet.y;
    let (sin, cos) = planet.ring_angle.sin_cos();
    let ring_x = dx * cos + dy * sin;
    let ring_y = -dx * sin + dy * cos;
    let radial = (ring_x * ring_x + (ring_y / 0.28).powi(2)).sqrt() / planet.radius.max(1.0);
    if !(1.25..=2.05).contains(&radial) || (ring_y > 0.0) != front {
        return None;
    }
    let band = (radial * 7.0).floor() as i32;
    let color = if band.rem_euclid(3) == 0 {
        Rgb(176, 150, 112)
    } else {
        Rgb(94, 94, 143)
    };
    Some(color)
}

pub(super) fn planet_body_coverage(planet: Planet, width: u16, height: u16) -> f32 {
    if !planet.active || planet.radius <= 0.0 {
        return 0.0;
    }
    let horizontal = ((planet.x + planet.radius).min(width as f32)
        - (planet.x - planet.radius).max(0.0))
    .max(0.0)
        / (planet.radius * 2.0);
    let pixel_height = height as f32 * 2.0;
    let vertical = ((planet.y + planet.radius).min(pixel_height)
        - (planet.y - planet.radius).max(0.0))
    .max(0.0)
        / (planet.radius * 2.0);
    horizontal * vertical
}
