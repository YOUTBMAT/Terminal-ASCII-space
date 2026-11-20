use rand::{rngs::StdRng, Rng};

use super::{CosmicKind, CosmicObject, Rgb};

pub(super) fn new_cosmic_object(
    rng: &mut StdRng,
    width: u16,
    height: u16,
    kind: CosmicKind,
) -> CosmicObject {
    let (radius_x, radius_y, depth, colors): (f32, f32, f32, &[Rgb]) = match kind {
        CosmicKind::Nebula => {
            let radius = rng.gen_range(14.0..34.0);
            (
                radius,
                radius * rng.gen_range(0.55..1.35),
                rng.gen_range(0.02..0.15),
                &[
                    Rgb(54, 25, 91),
                    Rgb(79, 24, 67),
                    Rgb(16, 55, 76),
                    Rgb(63, 35, 101),
                ],
            )
        }
        CosmicKind::Galaxy => {
            let radius = rng.gen_range(6.0..14.0);
            (
                radius,
                radius * rng.gen_range(0.48..0.82),
                rng.gen_range(0.12..0.34),
                &[Rgb(100, 92, 182), Rgb(92, 150, 178), Rgb(168, 102, 158)],
            )
        }
        CosmicKind::BlackHole => {
            let radius = rng.gen_range(3.0..7.5);
            (
                radius,
                radius * rng.gen_range(0.7..1.15),
                rng.gen_range(0.38..0.68),
                &[Rgb(174, 76, 158), Rgb(255, 139, 68), Rgb(111, 130, 222)],
            )
        }
        CosmicKind::Sun => {
            let radius = rng.gen_range(3.0..8.0);
            (
                radius,
                radius * rng.gen_range(0.82..1.18),
                rng.gen_range(0.62..0.94),
                &[Rgb(255, 178, 62), Rgb(255, 111, 68), Rgb(255, 225, 133)],
            )
        }
    };
    let depth_speed = 0.001 + depth * 0.014;
    CosmicObject {
        x: rng.gen_range(-radius_x..width as f32 + radius_x),
        y: rng.gen_range(0.0..height.max(1) as f32 * 2.0),
        radius_x,
        radius_y,
        depth,
        speed: depth_speed,
        phase: rng.gen_range(0.0..std::f32::consts::TAU),
        color: colors[rng.gen_range(0..colors.len())],
        kind,
    }
}

pub(super) fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn mix_color(a: Rgb, b: Rgb, amount: f32) -> Rgb {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |left: u8, right: u8| {
        (left as f32 + (right as f32 - left as f32) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

pub(super) fn scale_color(color: Rgb, amount: f32) -> Rgb {
    let scale = |value: u8| (value as f32 * amount).round().clamp(0.0, 255.0) as u8;
    Rgb(scale(color.0), scale(color.1), scale(color.2))
}
