use noise::{NoiseFn, OpenSimplex};
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::engine::{canvas::Cell, Canvas, Rgb, Theme, ThemeAction, ThemeNotice};

#[path = "space_impl/effects.rs"]
mod effects;
#[path = "space_impl/planets.rs"]
mod planets;
#[path = "space_impl/solar_system.rs"]
mod solar_system;
#[path = "space_impl/stars.rs"]
mod stars;

use effects::{mix_color, new_cosmic_object, scale_color, smoothstep};
use planets::{max_planet_radius, new_planet, next_planet_style, planet_body_coverage, ring_pixel};
use solar_system::{new_solar_system, SolarPlanet, SolarSystem};
use stars::new_star;

const STAR_COUNT: usize = 220;
const NEBULA_CACHE_INTERVAL: f32 = 0.25;

#[derive(Clone, Copy)]
struct Star {
    x: f32,
    y: f32,
    parallax_speed: f32,
    brightness: f32,
    phase: f32,
    twinkle_speed: f32,
    color: Rgb,
    glyph: char,
}

#[derive(Clone, Copy)]
struct Meteor {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    length: u8,
}

#[derive(Clone, Copy)]
struct Ship {
    x: f32,
    y: f32,
    speed: f32,
    length: u8,
    trail_phase: f32,
    hull: Rgb,
    accent: Rgb,
}

#[derive(Clone, Copy)]
struct AsteroidGroup {
    x: f32,
    y: f32,
    drift: f32,
    phase: f32,
}

#[derive(Clone, Copy)]
struct Satellite {
    x: f32,
    y: f32,
    speed: f32,
    phase: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CosmicKind {
    Nebula,
    Galaxy,
    BlackHole,
    Sun,
}

#[derive(Clone, Copy)]
enum ColorTheme {
    DeepVoid,
    SolarFlare,
    CyberNeon,
    NebulaDream,
}

#[derive(Clone, Copy)]
pub struct PaletteInfo {
    pub index: u8,
    pub name: &'static str,
    pub accent: Rgb,
}

#[derive(Clone, Copy)]
struct AmbientLight {
    x: f32,
    y: f32,
    radius_x: f32,
    radius_y: f32,
    color: Rgb,
    strength: f32,
}

#[derive(Clone, Copy)]
struct CosmicObject {
    x: f32,
    y: f32,
    radius_x: f32,
    radius_y: f32,
    depth: f32,
    speed: f32,
    phase: f32,
    color: Rgb,
    kind: CosmicKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlanetStyle {
    Gas,
    Ocean,
    Ember,
    Ice,
    Violet,
    Verdant,
    Cratered,
}

#[derive(Clone, Copy)]
enum MoonStyle {
    Rock,
    Ice,
    Volcanic,
    Ocean,
}

#[derive(Clone, Copy)]
struct Moon {
    size_factor: f32,
    orbit_distance: f32,
    orbit_height: f32,
    phase: f32,
    orbit_speed: f32,
    style: MoonStyle,
}

#[derive(Clone, Copy)]
struct Planet {
    x: f32,
    y: f32,
    radius: f32,
    phase: f32,
    ring_angle: f32,
    drift_speed: f32,
    style: PlanetStyle,
    ringed: bool,
    active: bool,
    moons: [Moon; 2],
    moon_count: u8,
}

pub struct SpaceEngine {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
    pixels: Vec<Rgb>,
    cloud_density: Vec<f32>,
    nebula_pixels: Vec<Rgb>,
    nebula_density: Vec<f32>,
    last_nebula_update: f32,
    stars: Vec<Star>,
    meteors: Vec<Meteor>,
    cosmic_objects: Vec<CosmicObject>,
    planets: [Planet; 2],
    ship: Ship,
    asteroid_groups: Vec<AsteroidGroup>,
    satellites: Vec<Satellite>,
    ambient_lights: Vec<AmbientLight>,
    solar_system: SolarSystem,
    noise: OpenSimplex,
    time: f32,
    burst_active: bool,
    input_burst_active: bool,
    color_theme: ColorTheme,
    rng: StdRng,
}

impl SpaceEngine {
    pub fn new(width: u16, height: u16) -> Self {
        let mut rng = StdRng::from_entropy();
        let stars = (0..STAR_COUNT)
            .map(|_| new_star(&mut rng, width, height))
            .collect();
        let mut first = new_planet(&mut rng, width, height, 0.0, true);
        let mut second = new_planet(&mut rng, width, height, 0.0, false);
        if second.style == first.style {
            second.style = next_planet_style(first.style);
        }
        let minimum_size_gap = max_planet_radius(width, height) * 0.26;
        if (first.radius - second.radius).abs() < minimum_size_gap {
            second.radius = if first.radius > max_planet_radius(width, height) * 0.66 {
                max_planet_radius(width, height) * 0.4
            } else {
                max_planet_radius(width, height) * 0.94
            };
            second.drift_speed = 0.55 + second.radius * 0.065;
        }
        first.x = width as f32 + first.radius + rng.gen_range(0.5..first.radius.max(1.0));
        second.x = width as f32 + second.radius + rng.gen_range(0.5..second.radius.max(1.0));
        let asteroid_groups = (0..5)
            .map(|_| AsteroidGroup {
                x: rng.gen_range(0.0..width.max(1) as f32),
                y: rng.gen_range(0.0..height.max(1) as f32),
                drift: rng.gen_range(0.3..1.1),
                phase: rng.gen_range(0.0..std::f32::consts::TAU),
            })
            .collect();
        let satellites = (0..2)
            .map(|_| Satellite {
                x: rng.gen_range(0.0..width.max(1) as f32),
                y: rng.gen_range(0.0..height.max(1) as f32),
                speed: rng.gen_range(0.6..1.8),
                phase: rng.gen_range(0.0..std::f32::consts::TAU),
            })
            .collect();
        let cosmic_kinds = [
            CosmicKind::Nebula,
            CosmicKind::Nebula,
            CosmicKind::Nebula,
            CosmicKind::Galaxy,
            CosmicKind::Galaxy,
            CosmicKind::Galaxy,
            CosmicKind::BlackHole,
            CosmicKind::BlackHole,
            CosmicKind::Sun,
            CosmicKind::Sun,
        ];
        let cosmic_objects = cosmic_kinds
            .into_iter()
            .map(|kind| new_cosmic_object(&mut rng, width, height, kind))
            .collect();
        let pixel_count = width as usize * height as usize * 2;

        let ship_colors = [
            (Rgb(125, 205, 246), Rgb(255, 190, 110)),
            (Rgb(190, 165, 235), Rgb(255, 132, 118)),
            (Rgb(110, 220, 196), Rgb(230, 245, 155)),
            (Rgb(210, 220, 238), Rgb(112, 220, 255)),
        ];
        let (hull, accent) = ship_colors[rng.gen_range(0..ship_colors.len())];

        Self {
            width,
            height,
            cells: vec![Cell::default(); width as usize * height as usize],
            pixels: vec![Rgb(8, 9, 22); pixel_count],
            cloud_density: vec![0.0; pixel_count],
            nebula_pixels: vec![Rgb(8, 9, 22); pixel_count],
            nebula_density: vec![0.0; pixel_count],
            last_nebula_update: f32::NEG_INFINITY,
            stars,
            meteors: Vec::new(),
            cosmic_objects,
            planets: [first, second],
            ship: Ship {
                x: -(10.0 + width as f32 * 0.06),
                y: height as f32 * 0.68,
                speed: rng.gen_range(1.6..2.8),
                length: rng.gen_range(8..=12),
                trail_phase: 0.0,
                hull,
                accent,
            },
            asteroid_groups,
            satellites,
            ambient_lights: Vec::with_capacity(10),
            solar_system: new_solar_system(&mut rng, width, height),
            noise: OpenSimplex::new(rng.gen()),
            time: 0.0,
            burst_active: false,
            input_burst_active: false,
            color_theme: ColorTheme::DeepVoid,
            rng,
        }
    }

    pub fn next_palette(&mut self) -> PaletteInfo {
        self.color_theme = match self.color_theme {
            ColorTheme::DeepVoid => ColorTheme::SolarFlare,
            ColorTheme::SolarFlare => ColorTheme::CyberNeon,
            ColorTheme::CyberNeon => ColorTheme::NebulaDream,
            ColorTheme::NebulaDream => ColorTheme::DeepVoid,
        };
        let (index, name, accent) = match self.color_theme {
            ColorTheme::DeepVoid => (1, "DEEP VOID", Rgb(145, 130, 255)),
            ColorTheme::SolarFlare => (2, "SOLAR FLARE", Rgb(255, 164, 82)),
            ColorTheme::CyberNeon => (3, "CYBER NEON", Rgb(66, 236, 220)),
            ColorTheme::NebulaDream => (4, "NEBULA DREAM", Rgb(231, 120, 236)),
        };
        PaletteInfo {
            index,
            name,
            accent,
        }
    }

    pub fn toggle_solar_system(&mut self) -> bool {
        if self.solar_system.active {
            self.solar_system.active = false;
        } else {
            self.solar_system = new_solar_system(&mut self.rng, self.width, self.height);
            self.solar_system.active = true;
        }
        self.solar_system.active
    }

    fn colorize(&self, color: Rgb) -> Rgb {
        let channel = |value: f32| value.round().clamp(0.0, 255.0) as u8;
        match self.color_theme {
            ColorTheme::DeepVoid => color,
            ColorTheme::SolarFlare => Rgb(
                channel(color.0 as f32 * 1.08 + color.1 as f32 * 0.08),
                channel(color.1 as f32 * 0.78),
                channel(color.2 as f32 * 0.55),
            ),
            ColorTheme::CyberNeon => Rgb(
                channel(color.0 as f32 * 0.72 + color.2 as f32 * 0.2),
                channel(color.1 as f32 * 1.08 + color.0 as f32 * 0.12),
                channel(color.2 as f32 * 1.1 + color.1 as f32 * 0.16),
            ),
            ColorTheme::NebulaDream => Rgb(
                channel(color.0 as f32 * 0.94 + color.2 as f32 * 0.18),
                channel(color.1 as f32 * 0.82 + color.0 as f32 * 0.12),
                channel(color.2 as f32 * 0.9 + color.1 as f32 * 0.18),
            ),
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        let old_width = self.width as f32;
        let old_pixel_height = self.height as f32 * 2.0;
        self.width = width;
        self.height = height;
        let pixel_count = width as usize * height as usize * 2;
        self.cells
            .resize(width as usize * height as usize, Cell::default());
        self.pixels.resize(pixel_count, Rgb(8, 9, 22));
        self.cloud_density.resize(pixel_count, 0.0);
        self.nebula_pixels.resize(pixel_count, Rgb(8, 9, 22));
        self.nebula_density.resize(pixel_count, 0.0);
        self.last_nebula_update = f32::NEG_INFINITY;
        for star in &mut self.stars {
            if star.x >= width as f32 || star.y >= height as f32 {
                *star = new_star(&mut self.rng, width, height);
            }
        }
        for planet in &mut self.planets {
            if old_width > 0.0 {
                planet.x = planet.x / old_width * width as f32;
            }
            planet.radius = planet.radius.min(max_planet_radius(width, height));
            planet.y = if old_pixel_height > 0.0 {
                planet.y / old_pixel_height * height as f32 * 2.0
            } else {
                height as f32
            };
        }
        for object in &mut self.cosmic_objects {
            if old_width > 0.0 {
                object.x = object.x / old_width * width as f32;
            }
            if old_pixel_height > 0.0 {
                object.y = object.y / old_pixel_height * height as f32 * 2.0;
            }
        }
        self.solar_system.scale =
            (width.min(height.saturating_mul(2)) as f32 * 0.25).clamp(4.0, 24.0);
        if old_width > 0.0 && old_pixel_height > 0.0 {
            self.solar_system.x = self.solar_system.x / old_width * width as f32;
            self.solar_system.y = self.solar_system.y / old_pixel_height * height as f32 * 2.0;
        } else {
            self.solar_system.x = width as f32 * 0.55;
            self.solar_system.y = height as f32;
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        input_rate: f32,
        burst: bool,
        cpu_usage: f32,
        memory_fraction: f32,
        warp_drive: bool,
    ) {
        self.time += dt;
        let warp = if warp_drive {
            5.0
        } else {
            (1.0 + input_rate * 0.15).clamp(1.0, 2.5)
        };

        for star in &mut self.stars {
            star.x -= self.width as f32 * star.parallax_speed * dt * warp;
            if star.x < 0.0 {
                star.x += self.width.max(1) as f32;
                star.y = self.rng.gen_range(0.0..self.height.max(1) as f32);
            }
        }

        for index in 0..self.cosmic_objects.len() {
            let mut object = self.cosmic_objects[index];
            object.x -= self.width as f32 * object.speed * dt;
            if object.x + object.radius_x < 0.0 {
                object.x = self.width as f32
                    + object.radius_x
                    + self.rng.gen_range(0.0..self.width.max(1) as f32 * 0.35);
                object.y = self.rng.gen_range(0.0..self.height.max(1) as f32 * 2.0);
                object.phase = self.rng.gen_range(0.0..std::f32::consts::TAU);
            }
            self.cosmic_objects[index] = object;
        }

        for planet in &mut self.planets {
            if planet.active {
                planet.x -= planet.drift_speed * dt;
                planet.phase += dt * 0.12;
                for moon in planet.moons.iter_mut().take(planet.moon_count as usize) {
                    moon.phase += moon.orbit_speed * dt;
                }
            }
        }
        for planet in &mut self.planets {
            if planet.active && planet.x + planet.radius < 0.0 {
                planet.active = false;
            }
        }
        if !self.planets.iter().any(|planet| planet.active) {
            let mut replacement = new_planet(&mut self.rng, self.width, self.height, 0.0, true);
            replacement.x = self.width as f32 + replacement.radius + self.rng.gen_range(1.0..8.0);
            self.planets[0] = replacement;
        }
        let leading_index = self
            .planets
            .iter()
            .enumerate()
            .filter(|(_, planet)| planet.active)
            .min_by(|(_, a), (_, b)| a.x.total_cmp(&b.x))
            .map(|(index, _)| index);
        if let Some(leading_index) = leading_index {
            let leading = self.planets[leading_index];
            if leading.x <= self.width as f32 * 0.65 {
                if let Some(next_index) = self.planets.iter().position(|planet| !planet.active) {
                    let mut next = new_planet(&mut self.rng, self.width, self.height, 0.0, true);
                    if next.style == leading.style {
                        next.style = next_planet_style(leading.style);
                    }
                    let min_gap = max_planet_radius(self.width, self.height) * 0.26;
                    if (next.radius - leading.radius).abs() < min_gap {
                        next.radius =
                            if leading.radius > max_planet_radius(self.width, self.height) * 0.66 {
                                max_planet_radius(self.width, self.height) * 0.4
                            } else {
                                max_planet_radius(self.width, self.height) * 0.94
                            };
                        next.drift_speed = 0.55 + next.radius * 0.065;
                    }
                    next.x = self.width as f32
                        + next.radius
                        + self.rng.gen_range(0.5..next.radius.max(1.0));
                    self.planets[next_index] = next;
                }
            }
        }

        self.ship.x += dt * (self.ship.speed * warp + input_rate.min(12.0) * 0.18);
        self.ship.trail_phase += dt * (4.0 + input_rate * 1.2) * warp.sqrt();
        if self.ship.x > self.width as f32 + self.ship.length as f32 {
            self.ship.x = -(self.ship.length as f32) - self.rng.gen_range(1.0..8.0);
            self.ship.y = self.rng.gen_range(0.2..0.85) * self.height.max(1) as f32;
        }
        for asteroid in &mut self.asteroid_groups {
            asteroid.x -= asteroid.drift * dt;
            if asteroid.x < -3.0 {
                asteroid.x = self.width as f32 + 2.0;
                asteroid.y = self.rng.gen_range(0.0..self.height.max(1) as f32);
            }
        }
        for satellite in &mut self.satellites {
            satellite.x -= satellite.speed * dt;
            satellite.phase += dt * 0.7;
            if satellite.x < -4.0 {
                satellite.x = self.width as f32 + 3.0;
                satellite.y = self.rng.gen_range(0.0..self.height.max(1) as f32);
            }
        }
        if self.solar_system.active {
            self.solar_system.x -= self.solar_system.drift_speed * dt;
            self.solar_system.phase += dt * 0.12;
            for planet in &mut self.solar_system.planets[..self.solar_system.planet_count] {
                planet.orbit_phase += planet.orbit_speed * dt;
                planet.rotation += dt * 0.12;
            }
            if self.solar_system.x + self.solar_system.scale * 1.2 < 0.0 {
                let mut next = new_solar_system(&mut self.rng, self.width, self.height);
                next.active = true;
                next.x = self.width as f32 + next.scale * 1.2;
                next.y = self.rng.gen_range(0.25..0.75) * self.height.max(1) as f32 * 2.0;
                self.solar_system = next;
            }
        }

        for meteor in &mut self.meteors {
            meteor.x += meteor.vx * dt;
            meteor.y += meteor.vy * dt;
        }
        self.meteors.retain(|meteor| {
            meteor.x > -(meteor.length as f32)
                && meteor.y < self.height as f32 + meteor.length as f32
        });

        let burst_started = burst && !self.burst_active;
        self.burst_active = burst;
        let input_burst = input_rate >= 8.0;
        let input_burst_started = input_burst && !self.input_burst_active;
        self.input_burst_active = input_burst;
        let random_meteor = self.rng.gen_bool((0.025 * dt.max(0.0) as f64).min(1.0));
        if (burst_started || input_burst_started || random_meteor)
            && self.width > 2
            && self.height > 1
        {
            self.spawn_meteor();
        }

        self.render(cpu_usage, memory_fraction, input_rate, warp_drive);
    }

    pub fn refresh(
        &mut self,
        cpu_usage: f32,
        memory_fraction: f32,
        input_rate: f32,
        warp_drive: bool,
    ) {
        self.render(cpu_usage, memory_fraction, input_rate, warp_drive);
    }

    fn spawn_meteor(&mut self) {
        let speed = self.rng.gen_range(16.0..32.0);
        let length = self.rng.gen_range(18..36);
        if self.rng.gen_bool(0.38) {
            self.meteors.push(Meteor {
                x: self.width as f32 + 2.0,
                y: self
                    .rng
                    .gen_range(-2.0..=(self.height as f32 * 0.35).max(-1.0)),
                vx: -speed * 0.82,
                vy: speed * 0.34,
                length,
            });
        } else {
            self.meteors.push(Meteor {
                x: self
                    .rng
                    .gen_range(self.width as f32 * 0.15..=self.width as f32 * 1.15),
                y: -1.0,
                vx: -speed * 0.38,
                vy: speed * 0.88,
                length,
            });
        }
    }

    fn render(&mut self, cpu_usage: f32, memory_fraction: f32, input_rate: f32, warp_drive: bool) {
        let heat = (cpu_usage / 100.0)
            .max(memory_fraction * 0.65)
            .clamp(0.0, 1.0);
        self.render_nebula(heat);
        for index in 0..self.cosmic_objects.len() {
            let object = self.cosmic_objects[index];
            if object.depth < 0.42 {
                self.render_cosmic_object(object);
            }
        }
        self.render_solar_system();
        for planet in self.planets {
            if planet.active {
                self.render_ring(planet, false);
                self.render_planet(planet);
                self.render_moon(planet);
                self.render_ring(planet, true);
            }
        }
        for index in 0..self.cosmic_objects.len() {
            let object = self.cosmic_objects[index];
            if object.depth >= 0.42 {
                self.render_cosmic_object(object);
            }
        }
        self.update_ambient_lights();
        self.encode_pixels();
        self.render_stars(input_rate, warp_drive);
        self.render_asteroids();
        self.render_satellites();
        self.render_ship(input_rate);
        self.render_meteors();
    }

    fn update_ambient_lights(&mut self) {
        self.ambient_lights.clear();
        for planet in self.planets.iter().filter(|planet| planet.active) {
            let color = match planet.style {
                PlanetStyle::Gas => Rgb(118, 102, 188),
                PlanetStyle::Ocean => Rgb(34, 128, 190),
                PlanetStyle::Ember => Rgb(210, 78, 42),
                PlanetStyle::Ice => Rgb(88, 186, 226),
                PlanetStyle::Violet => Rgb(182, 72, 196),
                PlanetStyle::Verdant => Rgb(68, 166, 112),
                PlanetStyle::Cratered => Rgb(164, 132, 112),
            };
            self.ambient_lights.push(AmbientLight {
                x: planet.x,
                y: planet.y,
                radius_x: planet.radius,
                radius_y: planet.radius,
                color,
                strength: 0.16,
            });
        }
        if self.solar_system.active {
            self.ambient_lights.push(AmbientLight {
                x: self.solar_system.x,
                y: self.solar_system.y,
                radius_x: self.solar_system.scale * 0.2,
                radius_y: self.solar_system.scale * 0.24,
                color: self.solar_system.sun_color,
                strength: 0.14,
            });
        }
        for object in &self.cosmic_objects {
            let (color, strength) = match object.kind {
                CosmicKind::Sun => (object.color, 0.3),
                CosmicKind::BlackHole => (object.color, 0.2),
                CosmicKind::Galaxy => (object.color, 0.07),
                CosmicKind::Nebula => continue,
            };
            self.ambient_lights.push(AmbientLight {
                x: object.x,
                y: object.y,
                radius_x: object.radius_x,
                radius_y: object.radius_y,
                color,
                strength,
            });
        }
    }

    fn render_nebula(&mut self, heat: f32) {
        let width = self.width as usize;
        let pixel_height = self.height as usize * 2;
        if self.time - self.last_nebula_update >= NEBULA_CACHE_INTERVAL {
            let t = self.time as f64;
            for py in 0..pixel_height {
                for x in 0..width {
                    let nx = x as f64 / width.max(1) as f64;
                    let ny = py as f64 / pixel_height.max(1) as f64;
                    let warp = self
                        .noise
                        .get([nx * 1.8 + t * 0.006, ny * 2.1 - t * 0.004, 7.3])
                        as f32;
                    let cloud = self.noise.get([
                        nx * 2.6 + warp as f64 * 0.32 + t * 0.008,
                        ny * 3.2 + warp as f64 * 0.24,
                        t * 0.012,
                    ]) as f32;
                    let detail = self.noise.get([nx * 7.4 + 19.0, ny * 8.0, t * 0.02]) as f32;
                    let density = smoothstep(-0.45, 0.6, cloud * 0.82 + detail * 0.18) * 0.78;
                    let hue = self.noise.get([nx * 1.2 + 31.0, ny * 1.7, t * 0.005]) as f32;
                    let palette = if hue < -0.3 {
                        mix_color(Rgb(18, 12, 42), Rgb(32, 10, 38), (hue + 1.0) * 0.7)
                    } else {
                        mix_color(Rgb(32, 10, 38), Rgb(10, 28, 42), (hue + 0.3) * 0.75)
                    };
                    let color = mix_color(Rgb(7, 9, 21), palette, density);
                    let warm = mix_color(color, Rgb(76, 25, 16), heat * density * 0.45);
                    let pixel_index = py * width + x;
                    self.nebula_pixels[pixel_index] = warm;
                    self.nebula_density[pixel_index] = density;
                }
            }
            self.last_nebula_update = self.time;
        }
        self.pixels.copy_from_slice(&self.nebula_pixels);
        self.cloud_density.copy_from_slice(&self.nebula_density);
    }

    fn render_cosmic_object(&mut self, object: CosmicObject) {
        let width = self.width as usize;
        let pixel_height = self.height as usize * 2;
        let extent = match object.kind {
            CosmicKind::Nebula => 1.0,
            CosmicKind::Galaxy => 1.0,
            CosmicKind::BlackHole => 1.15,
            CosmicKind::Sun => 1.3,
        };
        let reach_x = object.radius_x * extent;
        let reach_y = object.radius_y * extent;
        let left = (object.x - reach_x).floor().max(0.0) as usize;
        let right = (object.x + reach_x).ceil().min(self.width as f32) as usize;
        let top = (object.y - reach_y).floor().max(0.0) as usize;
        let bottom = (object.y + reach_y).ceil().min(pixel_height as f32) as usize;
        let distant = 0.4 + object.depth * 0.6;

        for py in top..bottom {
            for x in left..right {
                let dx = x as f32 + 0.5 - object.x;
                let dy = py as f32 + 0.5 - object.y;
                let nx = dx / object.radius_x.max(1.0);
                let ny = dy / object.radius_y.max(1.0);
                let radius = (nx * nx + ny * ny).sqrt();
                if radius > extent {
                    continue;
                }
                let noise = self.noise.get([
                    nx as f64 * 4.7 + object.phase as f64,
                    ny as f64 * 5.2,
                    self.time as f64 * 0.012,
                ]) as f32;
                let (color, opacity) = match object.kind {
                    CosmicKind::Nebula => {
                        let wisps = smoothstep(-0.45, 0.45, noise);
                        let edge = (1.0 - radius).max(0.0).powi(2);
                        let hue = mix_color(
                            object.color,
                            Rgb(28, 38, 74),
                            (noise * 0.5 + 0.35).clamp(0.0, 0.7),
                        );
                        (hue, edge * wisps * 0.58 * distant)
                    }
                    CosmicKind::Galaxy => {
                        if radius > 1.0 {
                            continue;
                        }
                        let angle = ny.atan2(nx);
                        let arms = (angle * 2.4 + radius * 9.0 - object.phase).cos();
                        let arm_noise = smoothstep(0.25, 0.78, arms + noise * 0.22);
                        let core = (1.0 - radius).max(0.0).powi(4);
                        let glow = (arm_noise * (1.0 - radius * 0.55) + core * 0.8).clamp(0.0, 1.0);
                        (
                            mix_color(object.color, Rgb(255, 206, 154), core * 0.8),
                            glow * 0.62 * distant,
                        )
                    }
                    CosmicKind::BlackHole => {
                        if radius < 0.24 {
                            (Rgb(1, 2, 8), 0.98)
                        } else if (0.34..=0.65).contains(&radius) {
                            let disc = if noise > 0.0 {
                                Rgb(255, 132, 58)
                            } else {
                                Rgb(187, 66, 143)
                            };
                            (disc, 0.82 * distant)
                        } else if (0.65..=0.92).contains(&radius) {
                            (Rgb(115, 113, 204), 0.32 * distant)
                        } else {
                            continue;
                        }
                    }
                    CosmicKind::Sun => {
                        if radius < 0.72 {
                            let surface = mix_color(
                                Rgb(255, 106, 35),
                                Rgb(255, 225, 118),
                                (0.85 - radius + noise * 0.12).clamp(0.0, 1.0),
                            );
                            (surface, 0.9)
                        } else {
                            let corona = smoothstep(1.3, 0.72, radius) * (0.5 + noise.abs() * 0.3);
                            (Rgb(255, 147, 54), corona * 0.52)
                        }
                    }
                };
                let index = py * width + x;
                self.pixels[index] = mix_color(self.pixels[index], color, opacity);
                if object.kind != CosmicKind::Nebula {
                    self.cloud_density[index] = 0.0;
                } else {
                    self.cloud_density[index] =
                        self.cloud_density[index].max(opacity.clamp(0.0, 0.78));
                }
            }
        }
    }

    fn render_solar_system(&mut self) {
        let system = self.solar_system;
        if !system.active || self.width < 8 || self.height < 6 {
            return;
        }

        let pixel_height = self.height as usize * 2;
        let width = self.width as usize;
        for body in system.planets.iter().take(system.planet_count) {
            let orbit_radius = system.scale * body.orbit_factor;
            for step in 0..72 {
                let angle = std::f32::consts::TAU * step as f32 / 72.0;
                let x = (system.x + angle.cos() * orbit_radius).round() as i32;
                let y = (system.y + angle.sin() * orbit_radius * system.orbit_tilt * 2.0).round()
                    as i32;
                if x < 0 || y < 0 || x >= self.width as i32 || y >= pixel_height as i32 {
                    continue;
                }
                let index = y as usize * width + x as usize;
                let shimmer = (angle * 2.0 + system.phase).sin().abs();
                let opacity = 0.12 + shimmer * 0.1;
                self.pixels[index] = mix_color(self.pixels[index], system.orbit_color, opacity);
            }
        }

        for body in system
            .planets
            .iter()
            .take(system.planet_count)
            .filter(|body| body.orbit_phase.sin() < 0.0)
        {
            self.render_solar_planet(system, *body);
        }

        let sun_radius = (system.scale * 0.14).clamp(1.5, 4.0);
        self.render_cosmic_object(CosmicObject {
            x: system.x,
            y: system.y,
            radius_x: sun_radius,
            radius_y: sun_radius * 1.8,
            depth: 0.36,
            speed: 0.0,
            phase: system.phase,
            color: system.sun_color,
            kind: CosmicKind::Sun,
        });

        for body in system
            .planets
            .iter()
            .take(system.planet_count)
            .filter(|body| body.orbit_phase.sin() >= 0.0)
        {
            self.render_solar_planet(system, *body);
        }
    }

    fn render_solar_planet(&mut self, system: SolarSystem, body: SolarPlanet) {
        let orbit_radius = system.scale * body.orbit_factor;
        let planet = Planet {
            x: system.x + body.orbit_phase.cos() * orbit_radius,
            y: system.y + body.orbit_phase.sin() * orbit_radius * system.orbit_tilt * 2.0,
            radius: (system.scale * body.radius_factor).clamp(1.0, 3.2),
            phase: body.rotation,
            ring_angle: system.orbit_tilt * 0.35,
            drift_speed: 0.0,
            style: body.style,
            ringed: body.ringed,
            active: true,
            moons: self.planets[0].moons,
            moon_count: 0,
        };
        let ring_opacity = if body.ringed { 0.38 } else { 0.0 };
        self.render_ring_with_opacity(planet, false, ring_opacity);

        let width = self.width as usize;
        let pixel_height = self.height as usize * 2;
        let left = (planet.x - planet.radius - 1.0).floor().max(0.0) as usize;
        let right = (planet.x + planet.radius + 1.0)
            .ceil()
            .min(self.width as f32) as usize;
        let top = (planet.y - planet.radius - 1.0).floor().max(0.0) as usize;
        let bottom = (planet.y + planet.radius + 1.0)
            .ceil()
            .min(pixel_height as f32) as usize;
        for py in top..bottom {
            for x in left..right {
                if let Some(color) = self.planet_pixel(planet, x as f32 + 0.5, py as f32 + 0.5) {
                    let index = py * width + x;
                    self.pixels[index] = mix_color(self.pixels[index], color, 0.78);
                    self.cloud_density[index] = 0.0;
                }
            }
        }

        self.render_ring_with_opacity(planet, true, ring_opacity);
    }

    fn render_ring(&mut self, planet: Planet, front: bool) {
        let body_coverage = planet_body_coverage(planet, self.width, self.height);
        if !planet.ringed || body_coverage <= 0.0 {
            return;
        }
        let opacity = smoothstep(0.03, 0.42, body_coverage) * 0.88;
        self.render_ring_with_opacity(planet, front, opacity);
    }

    fn render_ring_with_opacity(&mut self, planet: Planet, front: bool, opacity: f32) {
        if opacity <= 0.0 {
            return;
        }
        let width = self.width as usize;
        let pixel_height = self.height as usize * 2;
        let reach = planet.radius * 2.05 + 1.0;
        let left = (planet.x - reach).floor().max(0.0) as usize;
        let right = (planet.x + reach).ceil().min(self.width as f32) as usize;
        let top = (planet.y - reach).floor().max(0.0) as usize;
        let bottom = (planet.y + reach).ceil().min(pixel_height as f32) as usize;
        for py in top..bottom {
            for x in left..right {
                if let Some(color) = ring_pixel(planet, x as f32 + 0.5, py as f32 + 0.5, front) {
                    let index = py * width + x;
                    self.pixels[index] = mix_color(self.pixels[index], color, opacity);
                    self.cloud_density[index] = 0.0;
                }
            }
        }
    }

    fn render_planet(&mut self, planet: Planet) {
        let width = self.width as usize;
        let pixel_height = self.height as usize * 2;
        let left = (planet.x - planet.radius - 1.0).floor().max(0.0) as usize;
        let right = (planet.x + planet.radius + 1.0)
            .ceil()
            .min(self.width as f32) as usize;
        let top = (planet.y - planet.radius - 1.0).floor().max(0.0) as usize;
        let bottom = (planet.y + planet.radius + 1.0)
            .ceil()
            .min(pixel_height as f32) as usize;
        for py in top..bottom {
            for x in left..right {
                if let Some(color) = self.planet_pixel(planet, x as f32 + 0.5, py as f32 + 0.5) {
                    let index = py * width + x;
                    self.pixels[index] = color;
                    self.cloud_density[index] = 0.0;
                }
            }
        }
    }

    fn planet_pixel(&self, planet: Planet, x: f32, y: f32) -> Option<Rgb> {
        let dx = x - planet.x;
        let dy = y - planet.y;
        let radius_squared = planet.radius * planet.radius;
        let distance_squared = dx * dx + dy * dy;
        if distance_squared > radius_squared {
            return None;
        }
        let z = ((radius_squared - distance_squared) / radius_squared.max(1.0)).sqrt();
        let nx = dx / planet.radius.max(1.0);
        let ny = dy / planet.radius.max(1.0);
        let texture = self.noise.get([
            nx as f64 * 8.0 + planet.phase as f64,
            ny as f64 * 7.0,
            self.time as f64 * 0.025,
        ]) as f32;
        let illumination = (nx * -0.48 + ny * -0.42 + z * 0.78).clamp(-1.0, 1.0);
        let checker = if ((x.floor() as i32 + y.floor() as i32) & 1) == 0 {
            0.045
        } else {
            -0.045
        };
        let dither = checker + texture * 0.055;
        let light = smoothstep(-0.22, 0.38, illumination + dither);
        let base = match planet.style {
            PlanetStyle::Gas => {
                let bands = ((ny * 12.0 + planet.phase + texture * 0.7).sin() + 1.0) * 0.5;
                mix_color(
                    Rgb(68, 64, 128),
                    Rgb(156, 100, 72),
                    (bands * 0.72 + (texture * 0.16 + 0.12)).clamp(0.0, 1.0),
                )
            }
            PlanetStyle::Ocean => {
                if texture > 0.08 {
                    mix_color(Rgb(12, 67, 132), Rgb(22, 130, 130), (texture + 0.5) * 0.5)
                } else {
                    Rgb(10, 46, 112)
                }
            }
            PlanetStyle::Ember => {
                let bands = ((ny * 9.0 + planet.phase * 0.65).sin() + 1.0) * 0.5;
                mix_color(Rgb(84, 28, 33), Rgb(185, 88, 42), bands * 0.72)
            }
            PlanetStyle::Ice => mix_color(
                Rgb(58, 112, 166),
                Rgb(175, 218, 234),
                (texture * 0.45 + ny.sin() * 0.2 + 0.5).clamp(0.0, 1.0),
            ),
            PlanetStyle::Violet => {
                let bands = ((ny * 10.0 + planet.phase).sin() + 1.0) * 0.5;
                mix_color(Rgb(55, 24, 106), Rgb(164, 64, 158), bands * 0.82)
            }
            PlanetStyle::Verdant => {
                if texture > -0.05 {
                    mix_color(Rgb(18, 83, 72), Rgb(82, 143, 88), texture * 0.7 + 0.35)
                } else {
                    Rgb(17, 47, 72)
                }
            }
            PlanetStyle::Cratered => {
                let crater = self.noise.get([
                    nx as f64 * 24.0 + planet.phase as f64 * 0.2,
                    ny as f64 * 22.0,
                    11.7,
                ]) as f32;
                if crater < -0.48 {
                    Rgb(48, 47, 68)
                } else {
                    mix_color(Rgb(92, 82, 82), Rgb(155, 132, 108), (texture + 1.0) * 0.5)
                }
            }
        };
        let ambient = 0.14 + light * 0.86;
        Some(scale_color(base, ambient))
    }

    fn render_moon(&mut self, planet: Planet) {
        let width = self.width as usize;
        let pixel_height = self.height as usize * 2;
        for moon_index in 0..planet.moon_count as usize {
            let moon = planet.moons[moon_index];
            let angle = moon.phase;
            let radius = (planet.radius * moon.size_factor).clamp(1.6, 6.5);
            let moon_x = planet.x + angle.cos() * planet.radius * moon.orbit_distance;
            let moon_y = planet.y + angle.sin() * planet.radius * moon.orbit_height;
            let left = (moon_x - radius - 1.0).floor().max(0.0) as usize;
            let right = (moon_x + radius + 1.0).ceil().min(self.width as f32) as usize;
            let top = (moon_y - radius - 1.0).floor().max(0.0) as usize;
            let bottom = (moon_y + radius + 1.0).ceil().min(pixel_height as f32) as usize;
            for py in top..bottom {
                for x in left..right {
                    let dx = x as f32 + 0.5 - moon_x;
                    let dy = py as f32 + 0.5 - moon_y;
                    let d2 = dx * dx + dy * dy;
                    if d2 > radius * radius {
                        continue;
                    }
                    let z = ((radius * radius - d2) / radius.max(1.0).powi(2)).sqrt();
                    let crater =
                        self.noise
                            .get([dx as f64 * 1.3 + moon.phase as f64, dy as f64 * 1.5, 5.1])
                            as f32;
                    let lighting =
                        (0.25 + 0.75 * ((-dx * 0.5 - dy * 0.4 + z) / 1.5).max(0.0)).clamp(0.0, 1.0);
                    let base = match moon.style {
                        MoonStyle::Rock => Rgb(145, 145, 158),
                        MoonStyle::Ice => Rgb(126, 201, 228),
                        MoonStyle::Volcanic => Rgb(168, 88, 63),
                        MoonStyle::Ocean => Rgb(67, 151, 176),
                    };
                    let shade = if crater < -0.42 { 0.42 } else { 1.0 };
                    let color = scale_color(base, lighting * shade);
                    let index = py * width + x;
                    self.pixels[index] = color;
                    self.cloud_density[index] = 0.0;
                }
            }
        }
    }

    fn encode_pixels(&mut self) {
        let width = self.width as usize;
        for y in 0..self.height as usize {
            for x in 0..width {
                let top = (y * 2) * width + x;
                let bottom = top + width;
                let density = (self.cloud_density[top] + self.cloud_density[bottom]) * 0.5;
                let base_ch = if density > 0.57 {
                    '▒'
                } else if density > 0.34 {
                    '░'
                } else {
                    '▀'
                };
                let fg = self.colorize(self.pixels[top]);
                let bg = self.colorize(self.pixels[bottom]);
                let index = y * width + x;
                self.cells[index] = Cell {
                    ch: base_ch,
                    fg,
                    bg,
                    attrs: 0,
                    wide_continuation: false,
                    base_ch,
                    base_fg: fg,
                    ambient_fg: fg,
                    ambient_bg: bg,
                    cursor_fg: fg,
                };
            }
        }
    }

    fn render_stars(&mut self, input_rate: f32, warp_drive: bool) {
        for index in 0..self.stars.len() {
            let star = self.stars[index];
            if self.star_hidden_by_planet(star.x, star.y) {
                continue;
            }
            let x = star.x.floor() as i32;
            let y = star.y.floor() as i32;
            if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
                continue;
            }
            let twinkle_speed = star.twinkle_speed * (1.0 + input_rate.min(12.0) * 0.16);
            let twinkle = (self.time * twinkle_speed + star.phase).sin() * 0.5 + 0.5;
            let brightness =
                star.brightness * (0.58 + twinkle * 0.42) * (1.0 + input_rate.min(8.0) * 0.06);
            if warp_drive || input_rate >= 8.0 {
                let center_x = self.width as f32 * 0.5;
                let center_y = self.height as f32 * 0.5;
                let dx = star.x - center_x;
                let dy = star.y - center_y;
                let distance = (dx * dx + dy * dy).sqrt().max(1.0);
                let length = if warp_drive {
                    7 + (input_rate.min(10.0) * 0.35) as i32
                } else {
                    2 + (input_rate.min(14.0) * 0.25) as i32
                };
                let streak_glyph = if dx.abs() > dy.abs() * 1.4 {
                    '━'
                } else if dy.abs() > dx.abs() * 1.4 {
                    '│'
                } else if dx * dy > 0.0 {
                    '╲'
                } else {
                    '╱'
                };
                for tail in 1..=length {
                    let tail_x = (star.x - dx / distance * tail as f32).round() as i32;
                    let tail_y = (star.y - dy / distance * tail as f32).round() as i32;
                    let fade = (1.0 - tail as f32 / (length as f32 + 1.0)) * brightness * 0.65;
                    self.set_glyph(tail_x, tail_y, streak_glyph, scale_color(star.color, fade));
                }
            }
            self.set_glyph(x, y, star.glyph, scale_color(star.color, brightness));
        }
    }

    fn star_hidden_by_planet(&self, x: f32, y: f32) -> bool {
        let py = y * 2.0;
        let hidden_by_planet = self.planets.iter().any(|planet| {
            if !planet.active {
                return false;
            }
            let dx = x - planet.x;
            let dy = py - planet.y;
            dx * dx + dy * dy < planet.radius * planet.radius
        });
        hidden_by_planet
            || self.cosmic_objects.iter().any(|object| {
                if !matches!(object.kind, CosmicKind::Sun | CosmicKind::BlackHole) {
                    return false;
                }
                let dx = (x - object.x) / object.radius_x.max(1.0);
                let dy = (py - object.y) / object.radius_y.max(1.0);
                let radius = (dx * dx + dy * dy).sqrt();
                match object.kind {
                    CosmicKind::Sun => radius < 0.72,
                    CosmicKind::BlackHole => radius < 0.65,
                    _ => false,
                }
            })
    }

    fn render_asteroids(&mut self) {
        const CLUSTER: [(i32, i32, char); 7] = [
            (-1, 0, '▒'),
            (0, 0, '▓'),
            (1, 0, '█'),
            (0, -1, '▒'),
            (1, -1, '▓'),
            (-1, 1, '░'),
            (1, 1, '▪'),
        ];
        for index in 0..self.asteroid_groups.len() {
            let group = self.asteroid_groups[index];
            let x = group.x.round() as i32;
            let y = group.y.round() as i32;
            let shade = (110.0 + (self.time * 1.7 + group.phase).sin().abs() * 85.0) as u8;
            for (dx, dy, glyph) in CLUSTER {
                let color = Rgb(shade, shade.saturating_sub(12), shade.saturating_add(12));
                self.set_glyph(x + dx, y + dy, glyph, color);
            }
        }
    }

    fn render_satellites(&mut self) {
        for index in 0..self.satellites.len() {
            let satellite = self.satellites[index];
            let x = satellite.x.round() as i32;
            let y = satellite.y.round() as i32;
            let pulse = (self.time * 2.0 + satellite.phase).sin().abs();
            let color = Rgb(90, (155.0 + pulse * 70.0) as u8, 205);
            self.set_glyph(x - 1, y, '·', color);
            self.set_glyph(x, y, '⊕', color);
            self.set_glyph(x + 1, y, '·', color);
        }
    }

    fn render_ship(&mut self, input_rate: f32) {
        let x = self.ship.x.floor() as i32;
        let y = (self.ship.y + (self.time * 0.7).sin() * 0.8).round() as i32;
        let body_len = self.ship.length as i32;
        let trail_len =
            (6.0 + self.ship.trail_phase.sin().abs() * 2.0 + input_rate.min(16.0) * 0.65).round()
                as i32;
        let trail_glyphs = ['⣿', '⣷', '⣯', '▒', '░'];
        for tail in 1..=trail_len {
            let phase = (tail as f32 + self.ship.trail_phase) as usize % trail_glyphs.len();
            let fade = (1.0 - tail as f32 / (trail_len as f32 + 2.0)).max(0.08);
            self.set_glyph(
                x - tail,
                y,
                trail_glyphs[phase],
                scale_color(Rgb(48, 190, 245), fade * (1.0 + input_rate.min(8.0) * 0.08)),
            );
            if tail <= trail_len / 2 {
                self.set_glyph(
                    x - tail,
                    y - 1,
                    if phase % 2 == 0 { '⣯' } else { '·' },
                    scale_color(Rgb(105, 150, 255), fade * 0.55),
                );
                self.set_glyph(
                    x - tail,
                    y + 1,
                    if phase % 2 == 0 { '⣷' } else { '·' },
                    scale_color(Rgb(255, 142, 78), fade * 0.48),
                );
            }
        }
        for offset in 0..body_len {
            let glyph = if offset == body_len - 1 {
                '▶'
            } else if offset == body_len / 2 {
                '◈'
            } else if offset == 0 {
                '╱'
            } else if offset % 3 == 0 {
                '▰'
            } else {
                '▓'
            };
            let color = if offset == body_len / 2 {
                self.ship.accent
            } else {
                self.ship.hull
            };
            self.set_glyph(x + offset, y, glyph, color);
        }
        self.set_glyph(x + 1, y - 1, '╱', self.ship.accent);
        for offset in 2..body_len.saturating_sub(2) {
            let glyph = if offset % 2 == 0 { '▰' } else { '▒' };
            self.set_glyph(x + offset, y - 1, glyph, self.ship.hull);
            self.set_glyph(x + offset, y + 1, glyph, self.ship.hull);
        }
        self.set_glyph(x + 1, y + 1, '╲', self.ship.accent);
        self.set_glyph(x + body_len - 3, y - 2, '✧', self.ship.accent);
        self.set_glyph(x + body_len - 3, y + 2, '✧', self.ship.accent);
        self.set_glyph(x + body_len - 2, y, '▷', Rgb(235, 248, 255));
    }

    fn render_meteors(&mut self) {
        for index in 0..self.meteors.len() {
            let meteor = self.meteors[index];
            let speed = (meteor.vx * meteor.vx + meteor.vy * meteor.vy)
                .sqrt()
                .max(1.0);
            let direction_x = meteor.vx / speed;
            let direction_y = meteor.vy / speed;
            for tail in 0..meteor.length as i32 {
                let progress = tail as f32 / meteor.length.max(1) as f32;
                let x = (meteor.x - direction_x * tail as f32).round() as i32;
                let y = (meteor.y - direction_y * tail as f32).round() as i32;
                let glyph = if tail == 0 {
                    '✦'
                } else if progress < 0.18 {
                    '━'
                } else if progress < 0.42 {
                    '─'
                } else if progress < 0.72 {
                    '▒'
                } else {
                    '░'
                };
                let fade = (1.0 - progress * 0.88).max(0.12);
                self.set_glyph(x, y, glyph, scale_color(Rgb(135, 205, 255), fade));
            }
        }
    }

    fn set_glyph(&mut self, x: i32, y: i32, ch: char, fg: Rgb) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let index = y as usize * self.width as usize + x as usize;
        self.cells[index].ch = ch;
        self.cells[index].fg = self.colorize(fg);
    }

    fn ambient_colors(&self, x: u16, y: u16, fg: Rgb, cursor_fg: Rgb, bg: Rgb) -> (Rgb, Rgb, Rgb) {
        let mut fg = fg;
        let mut cursor_fg = cursor_fg;
        let mut bg = bg;
        for light in &self.ambient_lights {
            let dx = (x as f32 + 0.5 - light.x) / light.radius_x.max(1.0);
            let dy = (y as f32 * 2.0 + 1.0 - light.y) / light.radius_y.max(1.0);
            let distance_squared = dx * dx + dy * dy;
            if distance_squared > 3.0625 {
                continue;
            }
            let radial = distance_squared.sqrt();
            let falloff = (-((radial - 1.0) / 0.52).powi(2)).exp();
            let intensity = (falloff * light.strength).clamp(0.0, 0.34);
            let color = self.colorize(light.color);
            fg = mix_color(fg, color, intensity);
            cursor_fg = mix_color(cursor_fg, color, intensity);
            bg = mix_color(bg, color, intensity);
        }
        (fg, cursor_fg, bg)
    }
}

pub struct SpaceTheme {
    scene: SpaceEngine,
    cpu_usage: f32,
    memory_fraction: f32,
    warp_drive: bool,
    output_burst: bool,
}

impl SpaceTheme {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            scene: SpaceEngine::new(width, height),
            cpu_usage: 0.0,
            memory_fraction: 0.0,
            warp_drive: false,
            output_burst: false,
        }
    }
}

impl Theme for SpaceTheme {
    fn id(&self) -> &'static str {
        "space"
    }

    fn name(&self) -> &'static str {
        "Space"
    }

    fn description(&self) -> &'static str {
        "An animated deep-space scene with stars, nebulae, planets, ships, and meteors."
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.scene.resize(width, height);
    }

    fn update(&mut self, dt: f32, activity_intensity: f32) {
        self.scene.update(
            dt,
            activity_intensity,
            self.output_burst,
            self.cpu_usage,
            self.memory_fraction,
            self.warp_drive,
        );
    }

    fn refresh(&mut self, activity_intensity: f32) {
        self.scene.refresh(
            self.cpu_usage,
            self.memory_fraction,
            activity_intensity,
            self.warp_drive,
        );
    }

    fn render(&self, canvas: &mut Canvas) {
        canvas.resize(self.scene.width, self.scene.height);
        canvas.clear();
        let canvas_cells = canvas.cells_mut();
        for y in 0..self.scene.height {
            for x in 0..self.scene.width {
                let index = y as usize * self.scene.width as usize + x as usize;
                let mut cell = self.scene.cells[index];
                (cell.ambient_fg, cell.cursor_fg, cell.ambient_bg) =
                    self.scene
                        .ambient_colors(x, y, cell.fg, cell.base_fg, cell.bg);
                canvas_cells[index] = cell;
            }
        }
    }

    fn set_system_metrics(&mut self, cpu_usage: f32, memory_fraction: f32) {
        self.cpu_usage = cpu_usage;
        self.memory_fraction = memory_fraction;
    }

    fn set_output_burst(&mut self, active: bool) {
        self.output_burst = active;
    }

    fn action(&mut self, action: ThemeAction) -> Option<ThemeNotice> {
        match action {
            ThemeAction::CyclePalette => {
                let palette = self.scene.next_palette();
                Some(ThemeNotice {
                    text: format!("{}/4 | {}", palette.index, palette.name),
                    accent: palette.accent,
                })
            }
            ThemeAction::ToggleFeature("solar-system") => {
                let enabled = self.scene.toggle_solar_system();
                Some(ThemeNotice {
                    text: if enabled {
                        "SOLAR SYSTEM ON".to_string()
                    } else {
                        "SOLAR SYSTEM OFF".to_string()
                    },
                    accent: if enabled {
                        Rgb(126, 222, 255)
                    } else {
                        Rgb(170, 170, 190)
                    },
                })
            }
            ThemeAction::ToggleFeature("warp-drive") => {
                self.warp_drive = !self.warp_drive;
                None
            }
            ThemeAction::ToggleFeature(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_diff_equality_ignores_hidden_base_metadata() {
        let first = Cell::default();
        let mut second = first;
        second.base_ch = '░';
        second.base_fg = Rgb(100, 100, 100);
        assert_eq!(first, second);
    }

    #[test]
    fn small_and_zero_sized_canvases_render_without_panicking() {
        for (width, height) in [(0, 0), (1, 1), (4, 2), (12, 5), (39, 14)] {
            let mut theme = SpaceTheme::new(width, height);
            let mut canvas = Canvas::new(width, height);
            theme.update(0.033, 0.0);
            theme.render(&mut canvas);
            assert_eq!((canvas.width, canvas.height), (width, height));
            assert_eq!(canvas.cells_mut().len(), width as usize * height as usize);
        }
    }

    #[test]
    fn star_field_uses_continuous_positions_and_three_parallax_speeds() {
        let engine = SpaceEngine::new(80, 24);
        assert!((200..=240).contains(&engine.stars.len()));
        assert!(engine
            .stars
            .iter()
            .all(|star| { [0.015, 0.04, 0.08].contains(&star.parallax_speed) }));
        assert!(
            engine
                .stars
                .iter()
                .filter(|star| { star.x.fract() != 0.0 || star.y.fract() != 0.0 })
                .count()
                >= 120
        );
    }

    #[test]
    fn planets_have_distinct_sizes_and_enter_from_beyond_the_edge() {
        let mut engine = SpaceEngine::new(80, 24);
        assert!(engine.planets[0].active);
        assert!(!engine.planets[1].active);
        assert!(engine.planets[0].x - engine.planets[0].radius > 80.0);
        assert!(
            (engine.planets[0].radius - engine.planets[1].radius).abs()
                >= max_planet_radius(80, 24) * 0.25
        );
        assert_ne!(engine.planets[0].style, engine.planets[1].style);

        let initial_x = engine.planets[0].x;
        engine.update(0.05, 0.0, false, 0.0, 0.0, false);
        assert!(engine.planets[0].x < initial_x);
        assert!(engine.planets[0].x - engine.planets[0].radius > 80.0);
    }

    #[test]
    fn next_planet_is_reseeded_after_the_lead_body_passes_sixty_five_percent() {
        let mut engine = SpaceEngine::new(80, 24);
        engine.planets[0].x = 80.0 * 0.65 + 0.001;
        engine.update(0.01, 0.0, false, 0.0, 0.0, false);
        assert!(engine.planets[1].active);
        assert!(engine.planets[1].x - engine.planets[1].radius > 80.0);
    }

    #[test]
    fn ring_is_not_drawn_until_its_planet_intersects_the_screen() {
        let mut engine = SpaceEngine::new(80, 24);
        let mut planet = engine.planets[0];
        planet.active = true;
        planet.ringed = true;
        planet.ring_angle = 0.0;
        planet.x = 80.0 + planet.radius + 0.1;
        planet.y = 24.0;
        let before = engine.pixels.clone();
        engine.render_ring(planet, false);
        assert_eq!(engine.pixels, before);

        planet.x = 80.0 + planet.radius - 0.1;
        assert!(planet_body_coverage(planet, 80, 24) > 0.0);
    }

    #[test]
    fn distant_objects_move_more_slowly_and_multiple_kinds_coexist() {
        let engine = SpaceEngine::new(80, 24);
        assert_eq!(engine.cosmic_objects.len(), 10);
        for object in &engine.cosmic_objects {
            assert!((object.speed - (0.001 + object.depth * 0.014)).abs() < 0.00001);
        }
        assert!(engine
            .cosmic_objects
            .iter()
            .any(|object| object.kind == CosmicKind::Nebula));
        assert!(engine
            .cosmic_objects
            .iter()
            .any(|object| object.kind == CosmicKind::Galaxy));
        assert!(engine
            .cosmic_objects
            .iter()
            .any(|object| object.kind == CosmicKind::BlackHole));
        assert!(engine
            .cosmic_objects
            .iter()
            .any(|object| object.kind == CosmicKind::Sun));
    }

    #[test]
    fn every_meteor_spawn_begins_off_screen() {
        let mut engine = SpaceEngine::new(80, 24);
        for _ in 0..64 {
            engine.spawn_meteor();
        }
        assert!(engine.meteors.iter().all(|meteor| {
            if meteor.y == -1.0 {
                (80.0 * 0.15..=80.0 * 1.15).contains(&meteor.x)
            } else {
                meteor.x == 82.0 && (-2.0..=24.0 * 0.35).contains(&meteor.y)
            }
        }));
    }

    #[test]
    fn palette_cycle_changes_rendered_colors() {
        let mut engine = SpaceEngine::new(80, 24);
        let original = engine.colorize(Rgb(100, 120, 140));
        engine.next_palette();
        assert_ne!(engine.colorize(Rgb(100, 120, 140)), original);
    }

    #[test]
    fn warp_drive_stretches_stars_and_speeds_their_parallax() {
        let mut engine = SpaceEngine::new(80, 24);
        let star = engine
            .stars
            .iter()
            .position(|star| star.x > 5.0 && star.parallax_speed == 0.015)
            .unwrap();
        let initial_x = engine.stars[star].x;
        engine.update(0.01, 0.0, false, 0.0, 0.0, true);
        assert!((engine.stars[star].x - (initial_x - 80.0 * 0.015 * 0.01 * 5.0)).abs() < 0.001);
        assert!(engine
            .cells
            .iter()
            .any(|cell| matches!(cell.ch, '━' | '│' | '╲' | '╱')));
    }

    #[test]
    fn fast_typing_triggers_a_meteor_burst_only_on_transition() {
        let mut engine = SpaceEngine::new(80, 24);
        engine.update(0.0, 8.0, false, 0.0, 0.0, false);
        assert_eq!(engine.meteors.len(), 1);
        engine.update(0.0, 9.0, false, 0.0, 0.0, false);
        assert_eq!(engine.meteors.len(), 1);
    }

    #[test]
    fn solar_system_is_optional_and_has_multiple_orbiting_bodies() {
        let mut engine = SpaceEngine::new(80, 24);
        assert!(!engine.solar_system.active);
        let unchanged_pixels = engine.pixels.clone();
        engine.render_solar_system();
        assert_eq!(engine.pixels, unchanged_pixels);
        assert!(engine.toggle_solar_system());
        assert!((3..=5).contains(&engine.solar_system.planet_count));
        engine.render_solar_system();
        assert!(engine.pixels.iter().any(|pixel| *pixel != Rgb(8, 9, 22)));
        let first_phase = engine.solar_system.planets[0].orbit_phase;
        engine.update(0.1, 0.0, false, 0.0, 0.0, false);
        assert_ne!(engine.solar_system.planets[0].orbit_phase, first_phase);
        assert!(!engine.toggle_solar_system());
    }

    #[test]
    fn nebula_noise_cache_refreshes_at_a_reduced_rate() {
        let mut engine = SpaceEngine::new(40, 12);
        engine.render_nebula(0.0);
        assert_eq!(engine.last_nebula_update, 0.0);
        engine.time = 0.2;
        engine.render_nebula(0.0);
        assert_eq!(engine.last_nebula_update, 0.0);
        engine.time = 0.26;
        engine.render_nebula(0.0);
        assert_eq!(engine.last_nebula_update, 0.26);
    }

    #[test]
    fn ambient_light_tints_space_without_touching_terminal_text() {
        let mut engine = SpaceEngine::new(80, 24);
        let background = Rgb(20, 20, 30);
        engine.ambient_lights.push(AmbientLight {
            x: 10.0,
            y: 10.0,
            radius_x: 5.0,
            radius_y: 5.0,
            color: Rgb(255, 80, 20),
            strength: 0.3,
        });
        let tinted = engine.ambient_colors(10, 4, background, background, background);
        assert_ne!(tinted.0, background);
        assert_eq!(tinted.0, tinted.1);
        assert_eq!(tinted.1, tinted.2);
    }
}
