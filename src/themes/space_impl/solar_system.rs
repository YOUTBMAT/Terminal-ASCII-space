use rand::{rngs::StdRng, Rng};

use super::PlanetStyle;
use crate::engine::Rgb;

#[derive(Clone, Copy)]
pub(super) struct SolarPlanet {
    pub orbit_factor: f32,
    pub orbit_phase: f32,
    pub orbit_speed: f32,
    pub rotation: f32,
    pub radius_factor: f32,
    pub style: PlanetStyle,
    pub ringed: bool,
}

#[derive(Clone, Copy)]
pub(super) struct SolarSystem {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub drift_speed: f32,
    pub orbit_tilt: f32,
    pub phase: f32,
    pub sun_color: Rgb,
    pub orbit_color: Rgb,
    pub planet_count: usize,
    pub planets: [SolarPlanet; 5],
}

pub(super) fn new_solar_system(rng: &mut StdRng, width: u16, height: u16) -> SolarSystem {
    let variant = rng.gen_range(0..4);
    let (sun_color, orbit_color, styles) = match variant {
        0 => (
            Rgb(255, 218, 132),
            Rgb(86, 154, 224),
            [
                PlanetStyle::Ocean,
                PlanetStyle::Ice,
                PlanetStyle::Violet,
                PlanetStyle::Gas,
                PlanetStyle::Cratered,
            ],
        ),
        1 => (
            Rgb(255, 150, 86),
            Rgb(221, 130, 91),
            [
                PlanetStyle::Ember,
                PlanetStyle::Gas,
                PlanetStyle::Cratered,
                PlanetStyle::Ocean,
                PlanetStyle::Ice,
            ],
        ),
        2 => (
            Rgb(224, 172, 255),
            Rgb(175, 117, 224),
            [
                PlanetStyle::Violet,
                PlanetStyle::Ice,
                PlanetStyle::Gas,
                PlanetStyle::Ocean,
                PlanetStyle::Verdant,
            ],
        ),
        _ => (
            Rgb(158, 255, 210),
            Rgb(87, 190, 160),
            [
                PlanetStyle::Verdant,
                PlanetStyle::Ocean,
                PlanetStyle::Ice,
                PlanetStyle::Cratered,
                PlanetStyle::Gas,
            ],
        ),
    };
    let planets = std::array::from_fn(|index| SolarPlanet {
        orbit_factor: 0.30 + index as f32 * 0.19 + rng.gen_range(0.0..0.035),
        orbit_phase: rng.gen_range(0.0..std::f32::consts::TAU),
        orbit_speed: rng.gen_range(0.07..0.15) / (index as f32 + 1.0).sqrt(),
        rotation: rng.gen_range(0.0..std::f32::consts::TAU),
        radius_factor: rng.gen_range(0.055..0.105),
        style: styles[index],
        ringed: rng.gen_bool(if index == 1 { 0.55 } else { 0.2 }),
    });

    SolarSystem {
        active: false,
        x: width as f32 * 0.55,
        y: height as f32,
        scale: (width.min(height.saturating_mul(2)) as f32 * 0.25).clamp(4.0, 24.0),
        drift_speed: rng.gen_range(0.12..0.3),
        orbit_tilt: rng.gen_range(0.42..0.72),
        phase: rng.gen_range(0.0..std::f32::consts::TAU),
        sun_color,
        orbit_color,
        planet_count: rng.gen_range(3..=5),
        planets,
    }
}
