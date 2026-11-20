use rand::{rngs::StdRng, Rng};

use super::{Rgb, Star};

pub(super) fn new_star(rng: &mut StdRng, width: u16, height: u16) -> Star {
    let parallax_speed = match rng.gen_range(0..10) {
        0..=5 => 0.015,
        6..=8 => 0.04,
        _ => 0.08,
    };
    let colors = [
        Rgb(178, 205, 255),
        Rgb(224, 232, 255),
        Rgb(255, 222, 164),
        Rgb(112, 220, 244),
    ];
    let glyphs = ['·', '∘', '⋆', '✦'];
    Star {
        x: rng.gen_range(0.0..width.max(1) as f32),
        y: rng.gen_range(0.0..height.max(1) as f32),
        parallax_speed,
        brightness: rng.gen_range(0.58..1.0),
        phase: rng.gen_range(0.0..std::f32::consts::TAU),
        twinkle_speed: rng.gen_range(0.8..3.2),
        color: colors[rng.gen_range(0..colors.len())],
        glyph: glyphs[rng.gen_range(0..glyphs.len())],
    }
}
