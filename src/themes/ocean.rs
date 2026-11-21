use std::cell::RefCell;

use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::engine::{canvas::Cell, Canvas, Rgb, Theme};

const MAX_BUBBLES: usize = 32;
const MAX_SHARKS: usize = 2;
const MAX_FOREGROUND_CREATURES: usize = 3;
const MEDIUM_SHARK_A: [&str; 5] = [
    "     ▄██▄       ",
    "  ▄████████▄    ",
    "◀███▓███████●▶  ",
    "  ▀████████▀    ",
    "     ▀██▀       ",
];
const MEDIUM_SHARK_B: [&str; 5] = [
    "      ▄██▄      ",
    "  ▄████████▄    ",
    "◀███▓███████●▶  ",
    "  ▀████████▀    ",
    "      ▀██▀      ",
];
const LARGE_SHARK_A: [&str; 5] = [
    "       ▄████▄        ",
    "    ▄██████████▄     ",
    "◀██████▓█████████●▶  ",
    "    ▀██████████▀     ",
    "       ▀████▀        ",
];
const LARGE_SHARK_B: [&str; 5] = [
    "        ▄████▄       ",
    "    ▄██████████▄     ",
    "◀██████▓█████████●▶  ",
    "    ▀██████████▀     ",
    "        ▀████▀       ",
];
const TROPICAL_A: [&str; 3] = ["  ▄▓█▄  ", "◀█▓██▓●▶", "  ▀▓█▀  "];
const TROPICAL_B: [&str; 3] = ["   ▄▓█▄ ", "◀█▓██▓●▶", "   ▀▓█▀ "];
const RAY_A: [&str; 4] = ["   ▄██▄   ", "▄████████▄", "▀████████▀", "  ▀█▀▀█▀  "];
const RAY_B: [&str; 4] = ["  ▄████▄  ", "▄████████▄", "▀████████▀", "   ▀██▀   "];
const JELLY_A: [&str; 4] = ["  ▄████▄  ", " ▓██████▓ ", "  ▀████▀  ", " ░║░║░║░  "];
const JELLY_B: [&str; 4] = ["  ▄████▄  ", " ▓██████▓ ", "  ▀████▀  ", "  ░║║║░   "];

#[derive(Clone, Copy)]
enum BedKind {
    Coral,
    Kelp,
    Rock,
}

#[derive(Clone, Copy)]
struct BedFeature {
    x: f32,
    height: u8,
    phase: f32,
    color: Rgb,
    kind: BedKind,
}

#[derive(Clone, Copy)]
struct Bubble {
    x: f32,
    y: f32,
    speed: f32,
    phase: f32,
    size: u8,
}

#[derive(Clone, Copy)]
struct SmallFish {
    x: f32,
    y: f32,
    speed: f32,
    direction: f32,
    phase: f32,
    color: Rgb,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SharkSize {
    Medium,
    Large,
}

#[derive(Clone, Copy)]
struct Shark {
    x: f32,
    y: f32,
    speed: f32,
    direction: f32,
    phase: f32,
    size: SharkSize,
}

#[derive(Clone, Copy)]
enum CreatureKind {
    Tropical,
    Ray,
    Jellyfish,
}

#[derive(Clone, Copy)]
struct ForegroundCreature {
    x: f32,
    y: f32,
    speed: f32,
    direction: f32,
    phase: f32,
    color: Rgb,
    kind: CreatureKind,
}

pub struct OceanTheme {
    width: u16,
    height: u16,
    seabed_height: u16,
    time: f32,
    activity: f32,
    bubble_emission: f32,
    shark_spawn_timer: f32,
    creature_spawn_timer: f32,
    features: Vec<BedFeature>,
    bubbles: Vec<Bubble>,
    fish: Vec<SmallFish>,
    sharks: Vec<Shark>,
    creatures: Vec<ForegroundCreature>,
    occupancy: RefCell<Vec<bool>>,
    rng: StdRng,
}

impl OceanTheme {
    pub fn new(width: u16, height: u16) -> Self {
        let mut theme = Self {
            width,
            height,
            seabed_height: seabed_height(height),
            time: 0.0,
            activity: 0.0,
            bubble_emission: 0.0,
            shark_spawn_timer: 4.0,
            creature_spawn_timer: 5.0,
            features: Vec::new(),
            bubbles: Vec::new(),
            fish: Vec::new(),
            sharks: Vec::new(),
            creatures: Vec::new(),
            occupancy: RefCell::new(vec![false; width as usize * height as usize]),
            rng: StdRng::from_entropy(),
        };
        theme.rebuild_seabed();
        theme.bubbles = (0..14).map(|_| theme.new_bubble(false)).collect();
        theme.fish.clear();
        for _ in 0..3 {
            let school = theme.new_fish_school();
            theme.fish.extend(school);
        }
        if theme.can_swim() {
            let shark = theme.new_shark();
            let tropical = theme.new_creature(CreatureKind::Tropical);
            let jellyfish = theme.new_creature(CreatureKind::Jellyfish);
            theme.sharks.push(shark);
            theme.creatures.push(tropical);
            theme.creatures.push(jellyfish);
        }
        theme
    }

    fn rebuild_seabed(&mut self) {
        self.seabed_height = seabed_height(self.height);
        self.features.clear();
        if self.width == 0 || self.height == 0 {
            return;
        }
        let count = (self.width as usize / 8).clamp(1, 14);
        let spacing = self.width as f32 / count as f32;
        let coral_colors = [
            Rgb(204, 91, 128),
            Rgb(226, 116, 72),
            Rgb(158, 107, 197),
            Rgb(223, 157, 83),
        ];
        for index in 0..count {
            let jitter = self.rng.gen_range(-spacing * 0.14..spacing * 0.14);
            let x = (spacing * (index as f32 + 0.5) + jitter)
                .clamp(0.0, self.width.saturating_sub(1) as f32);
            let kind = match index % 3 {
                0 => BedKind::Coral,
                1 => BedKind::Kelp,
                _ => BedKind::Rock,
            };
            let color = match kind {
                BedKind::Coral => coral_colors[self.rng.gen_range(0..coral_colors.len())],
                BedKind::Kelp => Rgb(47, 130, 103),
                BedKind::Rock => Rgb(104, 100, 91),
            };
            self.features.push(BedFeature {
                x,
                height: self.rng.gen_range(3..=6),
                phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
                color,
                kind,
            });
        }
    }

    fn new_bubble(&mut self, from_seabed: bool) -> Bubble {
        let x = if from_seabed && !self.features.is_empty() {
            let feature = self.features[self.rng.gen_range(0..self.features.len())];
            feature.x + self.rng.gen_range(-1.0..1.0)
        } else {
            self.rng.gen_range(0.0..self.width.max(1) as f32)
        };
        let surface = self.height.saturating_sub(self.seabed_height) as f32;
        let y = if from_seabed {
            (surface - self.rng.gen_range(0.0..1.8)).max(0.0)
        } else {
            self.rng.gen_range(0.0..surface.max(1.0))
        };
        let size = self.rng.gen_range(1..=4);
        Bubble {
            x,
            y,
            speed: 0.65 + size as f32 * 0.32,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
            size,
        }
    }

    fn new_small_fish(&mut self) -> SmallFish {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let upper_water = self.height.saturating_sub(self.seabed_height).max(1);
        SmallFish {
            x: if direction > 0.0 {
                -(self.rng.gen_range(3.0..8.0))
            } else {
                self.width as f32 + self.rng.gen_range(0.0..8.0)
            },
            y: self.rng.gen_range(0.0..upper_water as f32),
            speed: self.rng.gen_range(1.0..2.8),
            direction,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
            color: [Rgb(48, 113, 142), Rgb(60, 127, 149), Rgb(77, 116, 151)]
                [self.rng.gen_range(0..3)],
        }
    }

    fn new_fish_school(&mut self) -> Vec<SmallFish> {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let start_x = self.rng.gen_range(0.0..self.width.max(1) as f32);
        let upper_water = self.height.saturating_sub(self.seabed_height).max(1);
        let center_y = self.rng.gen_range(0.0..upper_water as f32);
        let speed = self.rng.gen_range(1.0..2.0);
        let phase = self.rng.gen_range(0.0..std::f32::consts::TAU);
        let color =
            [Rgb(48, 113, 142), Rgb(60, 127, 149), Rgb(77, 116, 151)][self.rng.gen_range(0..3)];
        (0..4)
            .map(|index| SmallFish {
                x: start_x - direction * index as f32 * 5.0,
                y: (center_y + (index as f32 - 1.5) * 0.6)
                    .clamp(0.0, upper_water.saturating_sub(1) as f32),
                speed,
                direction,
                phase: phase + index as f32 * 0.22,
                color,
            })
            .collect()
    }

    fn new_shark(&mut self) -> Shark {
        let size = if self.rng.gen_bool(0.55) {
            SharkSize::Medium
        } else {
            SharkSize::Large
        };
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let width = shark_width(size) as f32;
        let swim_height = self
            .height
            .saturating_sub(self.seabed_height)
            .saturating_sub(shark_height(size));
        Shark {
            x: if direction > 0.0 {
                -width - self.rng.gen_range(1.0..8.0)
            } else {
                self.width as f32 + self.rng.gen_range(1.0..8.0)
            },
            y: self.rng.gen_range(1.0..swim_height.max(2) as f32),
            speed: if size == SharkSize::Large { 1.8 } else { 2.7 },
            direction,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
            size,
        }
    }

    fn new_creature(&mut self, kind: CreatureKind) -> ForegroundCreature {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let upper_water = self
            .height
            .saturating_sub(self.seabed_height)
            .saturating_sub(4)
            .max(2);
        let x = if matches!(kind, CreatureKind::Jellyfish) {
            self.rng.gen_range(0.0..self.width.max(1) as f32)
        } else if direction > 0.0 {
            -8.0
        } else {
            self.width as f32 + 2.0
        };
        ForegroundCreature {
            x,
            y: self.rng.gen_range(1.0..upper_water as f32),
            speed: match kind {
                CreatureKind::Tropical => self.rng.gen_range(2.0..3.5),
                CreatureKind::Ray => self.rng.gen_range(1.2..2.2),
                CreatureKind::Jellyfish => self.rng.gen_range(0.5..1.0),
            },
            direction,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
            color: match kind {
                CreatureKind::Tropical => Rgb(236, 153, 71),
                CreatureKind::Ray => Rgb(107, 163, 194),
                CreatureKind::Jellyfish => Rgb(191, 129, 220),
            },
            kind,
        }
    }

    fn can_swim(&self) -> bool {
        self.height >= self.seabed_height.saturating_add(7) && self.width >= 12
    }

    fn update_entities(&mut self, dt: f32, activity: f32) {
        for index in 0..self.bubbles.len() {
            self.bubbles[index].y -= self.bubbles[index].speed * dt;
            if self.bubbles[index].y < -1.0 {
                let bubble = self.new_bubble(true);
                self.bubbles[index] = bubble;
            }
        }
        if self.bubbles.len() < MAX_BUBBLES {
            self.bubble_emission += dt * (0.12 + activity * 0.34);
            while self.bubble_emission >= 1.0 && self.bubbles.len() < MAX_BUBBLES {
                self.bubble_emission -= 1.0;
                let bubble = self.new_bubble(true);
                self.bubbles.push(bubble);
            }
        } else {
            self.bubble_emission = 0.0;
        }

        for index in 0..self.fish.len() {
            let fish = &mut self.fish[index];
            fish.x += fish.speed * fish.direction * dt;
            if fish.direction > 0.0 && fish.x > self.width as f32 + 2.0
                || fish.direction < 0.0 && fish.x < -4.0
            {
                self.fish[index] = self.new_small_fish();
            }
        }

        for shark in &mut self.sharks {
            shark.x += shark.speed * shark.direction * dt;
            shark.phase += dt * 2.2;
        }
        self.sharks.retain(|shark| {
            if shark.direction > 0.0 {
                shark.x < self.width as f32 + 2.0
            } else {
                shark.x + shark_width(shark.size) as f32 > -2.0
            }
        });
        if self.can_swim() {
            self.shark_spawn_timer -= dt;
            if self.shark_spawn_timer <= 0.0 && self.sharks.len() < MAX_SHARKS {
                let shark = self.new_shark();
                self.sharks.push(shark);
                self.shark_spawn_timer = self.rng.gen_range(8.0..14.0);
            }
        }

        for creature in &mut self.creatures {
            creature.phase += dt * 2.0;
            match creature.kind {
                CreatureKind::Jellyfish => creature.y -= creature.speed * dt,
                CreatureKind::Tropical | CreatureKind::Ray => {
                    creature.x += creature.speed * creature.direction * dt;
                }
            }
        }
        self.creatures.retain(|creature| match creature.kind {
            CreatureKind::Jellyfish => creature.y > -5.0,
            CreatureKind::Tropical | CreatureKind::Ray => {
                creature.x > -12.0 && creature.x < self.width as f32 + 4.0
            }
        });
        if self.can_swim() {
            self.creature_spawn_timer -= dt;
            if self.creature_spawn_timer <= 0.0 && self.creatures.len() < MAX_FOREGROUND_CREATURES {
                let kind = match self.rng.gen_range(0..3) {
                    0 => CreatureKind::Tropical,
                    1 => CreatureKind::Ray,
                    _ => CreatureKind::Jellyfish,
                };
                let creature = self.new_creature(kind);
                self.creatures.push(creature);
                self.creature_spawn_timer = self.rng.gen_range(5.0..10.0);
            }
        }
    }

    fn render_background(&self, cells: &mut [Cell]) {
        let width = self.width as usize;
        let height = self.height as usize;
        let top = Rgb(10, 37, 64);
        let bottom = Rgb(2, 12, 27);
        let beam_origins = [0.14, 0.49, 0.83];
        let beam_slopes = [0.16, -0.11, 0.12];
        for y in 0..height {
            let depth = y as f32 / height.saturating_sub(1).max(1) as f32;
            let vertical = mix(top, bottom, depth);
            for x in 0..width {
                let mut color = vertical;
                let x_float = x as f32;
                for beam in 0..beam_origins.len() {
                    let center = self.width as f32 * beam_origins[beam]
                        + y as f32 * beam_slopes[beam]
                        + (self.time * 0.15 + beam as f32 * 2.1).sin() * 1.3;
                    let distance = (x_float - center).abs();
                    let width = (self.width as f32 * 0.038).clamp(1.8, 5.2);
                    let intensity = (-(distance * distance) / (2.0 * width * width)).exp()
                        * (1.0 - depth * 0.82)
                        * 0.34;
                    color = mix(color, Rgb(65, 137, 174), intensity);
                }
                let shimmer = (x_float * 0.035 + self.time * 0.12).sin() * 1.5;
                let fg = Rgb(
                    (color.0 as f32 + shimmer).clamp(0.0, 255.0) as u8,
                    (color.1 as f32 + shimmer).clamp(0.0, 255.0) as u8,
                    (color.2 as f32 + shimmer).clamp(0.0, 255.0) as u8,
                );
                cells[y * width + x] = make_cell('▀', fg, color);
            }
        }
    }

    fn render_seabed(&self, scene: &mut SceneBuffer<'_>) {
        if scene.width == 0 || scene.height == 0 {
            return;
        }
        let width = scene.width as usize;
        let base_y = scene.height.saturating_sub(self.seabed_height) as i32;
        let sand_top = Rgb(116, 89, 49);
        let sand_bottom = Rgb(43, 37, 31);
        for x in 0..scene.width as i32 {
            let surface_wave = (hash(x as u32, 0) % 3) as i32 - 1;
            let ground_y = (base_y + surface_wave).clamp(0, scene.height as i32 - 1);
            for y in ground_y..scene.height as i32 {
                let depth = (y - ground_y) as f32 / (scene.height as i32 - ground_y).max(1) as f32;
                let mut bg = mix(sand_top, sand_bottom, depth);
                let noise = hash(x as u32, y as u32);
                if noise % 9 == 0 {
                    bg = mix(bg, Rgb(135, 105, 57), 0.22);
                }
                let glyph = if y == ground_y {
                    if noise % 3 == 0 {
                        '▀'
                    } else {
                        '~'
                    }
                } else if noise % 19 == 0 {
                    '▓'
                } else if noise % 7 == 0 {
                    '░'
                } else {
                    '·'
                };
                let fg = mix(
                    bg,
                    Rgb(159, 126, 69),
                    if glyph == '~' { 0.55 } else { 0.25 },
                );
                let index = y as usize * width + x as usize;
                scene.cells[index] = make_cell(glyph, fg, bg);
                scene.occupied[index] = true;
            }
        }
        for feature in &self.features {
            match feature.kind {
                BedKind::Coral => self.render_coral(scene, *feature),
                BedKind::Kelp => self.render_kelp(scene, *feature),
                BedKind::Rock => self.render_rock(scene, *feature),
            }
        }
    }

    fn render_coral(&self, scene: &mut SceneBuffer<'_>, feature: BedFeature) {
        let bottom = scene.height as i32 - 1;
        let branches = feature
            .height
            .min(self.seabed_height.saturating_add(2) as u8) as i32;
        for segment in 0..branches {
            let y = bottom - segment;
            let sway = (self.time * 0.16 + feature.phase + segment as f32 * 0.55).sin() * 0.35;
            let center = feature.x.round() as i32 + sway.round() as i32;
            let stem = match segment % 3 {
                0 => '█',
                1 => '▓',
                _ => '▒',
            };
            let shaded = mix(feature.color, Rgb(255, 190, 160), segment as f32 * 0.08);
            scene.put_feature(center, y, stem, shaded);
            if segment > 0 && segment % 2 == 0 {
                scene.put_feature(
                    center - 1,
                    y + 1,
                    if segment % 3 == 0 { '◢' } else { '▌' },
                    mix(feature.color, Rgb(255, 182, 151), 0.2),
                );
                scene.put_feature(
                    center + 1,
                    y + 1,
                    if segment % 3 == 0 { '◣' } else { '▐' },
                    mix(feature.color, Rgb(255, 182, 151), 0.12),
                );
            }
            if segment == branches - 1 {
                scene.put_feature(
                    center - 1,
                    y,
                    '●',
                    mix(feature.color, Rgb(255, 214, 174), 0.3),
                );
                scene.put_feature(
                    center + 1,
                    y,
                    '●',
                    mix(feature.color, Rgb(255, 214, 174), 0.22),
                );
            }
        }
    }

    fn render_kelp(&self, scene: &mut SceneBuffer<'_>, feature: BedFeature) {
        let height = feature
            .height
            .min(self.seabed_height.saturating_add(2) as u8) as i32;
        for segment in 0..height {
            let y = scene.height as i32 - 1 - segment;
            let sway = (self.time * 0.24 + feature.phase + segment as f32 * 0.44).sin() * 1.0;
            let center = feature.x.round() as i32 + sway.round() as i32;
            let glyph = if segment == height - 1 {
                '◈'
            } else if segment % 2 == 0 {
                '▌'
            } else {
                '▐'
            };
            let color = if segment == height - 1 {
                Rgb(126, 201, 140)
            } else {
                feature.color
            };
            scene.put_feature(center, y, glyph, color);
            if segment > 0 && segment % 2 == 1 {
                scene.put_feature(
                    center - 1,
                    y,
                    if segment % 3 == 0 { '◢' } else { '◣' },
                    color,
                );
            }
        }
    }

    fn render_rock(&self, scene: &mut SceneBuffer<'_>, feature: BedFeature) {
        let x = feature.x.round() as i32;
        let bottom = scene.height as i32 - 1;
        for (dy, row) in [" ░▓▒ ", "▒████▒", "▓█████▓"].iter().enumerate()
        {
            for (dx, glyph) in row.chars().enumerate() {
                if glyph == ' ' {
                    continue;
                }
                let color = if glyph == '█' {
                    mix(feature.color, Rgb(42, 48, 52), 0.38)
                } else {
                    mix(feature.color, Rgb(180, 161, 128), 0.18)
                };
                scene.put_feature(x + dx as i32 - 2, bottom - 2 + dy as i32, glyph, color);
            }
        }
    }

    fn render_bubbles(&self, scene: &mut SceneBuffer<'_>) {
        for bubble in &self.bubbles {
            let glyph = match bubble.size {
                1 => '●',
                2 => '◉',
                3 => '◎',
                _ => '⦿',
            };
            let x = bubble.x + (self.time * 1.15 + bubble.phase).sin() * 0.9;
            let ix = x.round() as i32;
            let iy = bubble.y.round() as i32;
            let color = match bubble.size {
                1 => Rgb(117, 200, 221),
                2 => Rgb(135, 215, 235),
                3 => Rgb(164, 225, 238),
                _ => Rgb(186, 233, 241),
            };
            if bubble.size >= 3 {
                scene.put_object(ix - 1, iy, '░', Rgb(48, 112, 145));
                scene.put_object(ix + 1, iy, '░', Rgb(48, 112, 145));
            }
            scene.put_object(ix, iy, glyph, color);
            scene.put_object(ix - 1, iy - 1, '·', Rgb(205, 241, 245));
        }
    }

    fn render_fish(&self, scene: &mut SceneBuffer<'_>) {
        for fish in &self.fish {
            let y = fish.y + (self.time * 0.9 + fish.phase).sin() * 0.35;
            let sprite = ["  ▄▓▄  ", "◀████●▶", "  ▀▓▀  "];
            scene.draw_sprite(
                fish.x.round() as i32,
                y.round() as i32,
                &sprite,
                fish.direction,
                SpriteStyle {
                    color: fish.color,
                    eye_color: Rgb(222, 243, 231),
                    eye_row: 1,
                },
            );
        }
    }

    fn render_sharks(&self, scene: &mut SceneBuffer<'_>) {
        for shark in &self.sharks {
            let frame = shark.phase.sin() > 0.0;
            let sprite = match (shark.size, frame) {
                (SharkSize::Medium, false) => &MEDIUM_SHARK_A,
                (SharkSize::Medium, true) => &MEDIUM_SHARK_B,
                (SharkSize::Large, false) => &LARGE_SHARK_A,
                (SharkSize::Large, true) => &LARGE_SHARK_B,
            };
            scene.draw_sprite(
                shark.x.round() as i32,
                shark.y.round() as i32,
                sprite,
                shark.direction,
                SpriteStyle {
                    color: if shark.size == SharkSize::Large {
                        Rgb(112, 174, 196)
                    } else {
                        Rgb(96, 155, 181)
                    },
                    eye_color: Rgb(222, 239, 226),
                    eye_row: 2,
                },
            );
        }
    }

    fn render_creatures(&self, scene: &mut SceneBuffer<'_>) {
        for creature in &self.creatures {
            let frame = creature.phase.sin() > 0.0;
            let (sprite, direction): (&[&str], f32) = match (creature.kind, frame) {
                (CreatureKind::Tropical, false) => (&TROPICAL_A, creature.direction),
                (CreatureKind::Tropical, true) => (&TROPICAL_B, creature.direction),
                (CreatureKind::Ray, false) => (&RAY_A, creature.direction),
                (CreatureKind::Ray, true) => (&RAY_B, creature.direction),
                (CreatureKind::Jellyfish, false) => (&JELLY_A, 1.0),
                (CreatureKind::Jellyfish, true) => (&JELLY_B, 1.0),
            };
            let y = if matches!(creature.kind, CreatureKind::Jellyfish) {
                creature.y
            } else {
                creature.y + (self.time * 0.7 + creature.phase).sin() * 0.45
            };
            scene.draw_sprite(
                creature.x.round() as i32,
                y.round() as i32,
                sprite,
                direction,
                SpriteStyle {
                    color: creature.color,
                    eye_color: Rgb(242, 219, 171),
                    eye_row: 1,
                },
            );
        }
    }
}

impl Theme for OceanTheme {
    fn id(&self) -> &'static str {
        "ocean"
    }

    fn name(&self) -> &'static str {
        "Ocean"
    }

    fn description(&self) -> &'static str {
        "A layered seabed with drifting schools, bubbles, sharks, and tropical life."
    }

    fn resize(&mut self, width: u16, height: u16) {
        let old_width = self.width.max(1) as f32;
        let old_height = self.height.max(1) as f32;
        self.width = width;
        self.height = height;
        self.seabed_height = seabed_height(height);
        self.occupancy
            .get_mut()
            .resize(width as usize * height as usize, false);
        self.rebuild_seabed();
        for fish in &mut self.fish {
            fish.x = fish.x / old_width * width as f32;
            fish.y = (fish.y / old_height * height as f32)
                .min(height.saturating_sub(self.seabed_height).saturating_sub(1) as f32);
        }
        for shark in &mut self.sharks {
            shark.x = shark.x / old_width * width as f32;
            shark.y = (shark.y / old_height * height as f32)
                .min(height.saturating_sub(self.seabed_height).saturating_sub(5) as f32);
        }
        for creature in &mut self.creatures {
            creature.x = creature.x / old_width * width as f32;
            creature.y = (creature.y / old_height * height as f32)
                .min(height.saturating_sub(self.seabed_height).saturating_sub(4) as f32);
        }
    }

    fn update(&mut self, dt: f32, activity_intensity: f32) {
        let dt = dt.clamp(0.0, 0.25);
        self.activity = activity_intensity.clamp(0.0, 12.0);
        self.time += dt;
        self.update_entities(dt, self.activity);
    }

    fn render(&self, canvas: &mut Canvas) {
        canvas.resize(self.width, self.height);
        canvas.clear();
        if self.width == 0 || self.height == 0 {
            return;
        }
        let cells = canvas.cells_mut();
        let mut occupancy = self.occupancy.borrow_mut();
        occupancy.resize(cells.len(), false);
        occupancy.fill(false);
        self.render_background(cells);
        let mut scene = SceneBuffer {
            cells,
            occupied: &mut occupancy,
            width: self.width,
            height: self.height,
        };
        self.render_seabed(&mut scene);
        self.render_sharks(&mut scene);
        self.render_creatures(&mut scene);
        self.render_bubbles(&mut scene);
        self.render_fish(&mut scene);
    }
}

fn seabed_height(height: u16) -> u16 {
    if height == 0 {
        return 0;
    }
    (4 + (height / 18).min(2)).min(6).min(height)
}

fn shark_width(size: SharkSize) -> u16 {
    match size {
        SharkSize::Medium => 16,
        SharkSize::Large => 21,
    }
}

fn shark_height(_size: SharkSize) -> u16 {
    5
}

fn make_cell(ch: char, fg: Rgb, bg: Rgb) -> Cell {
    Cell {
        ch,
        fg,
        bg,
        base_ch: '▀',
        base_fg: fg,
        ambient_fg: fg,
        ambient_bg: bg,
        cursor_fg: fg,
        ..Cell::default()
    }
}

fn mix(a: Rgb, b: Rgb, amount: f32) -> Rgb {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |left: u8, right: u8| {
        (left as f32 + (right as f32 - left as f32) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgb(channel(a.0, b.0), channel(a.1, b.1), channel(a.2, b.2))
}

fn hash(x: u32, y: u32) -> u32 {
    let mut value = x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value
}

struct SceneBuffer<'a> {
    cells: &'a mut [Cell],
    occupied: &'a mut [bool],
    width: u16,
    height: u16,
}

struct SpriteStyle {
    color: Rgb,
    eye_color: Rgb,
    eye_row: usize,
}

impl SceneBuffer<'_> {
    fn put_feature(&mut self, x: i32, y: i32, ch: char, fg: Rgb) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let index = y as usize * self.width as usize + x as usize;
        let bg = self.cells[index].bg;
        self.cells[index] = make_cell(ch, fg, bg);
        self.occupied[index] = true;
    }

    fn put_object(&mut self, x: i32, y: i32, ch: char, fg: Rgb) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return false;
        }
        let index = y as usize * self.width as usize + x as usize;
        if self.occupied[index] {
            return false;
        }
        let bg = self.cells[index].bg;
        self.cells[index] = make_cell(ch, fg, bg);
        self.occupied[index] = true;
        true
    }

    fn draw_sprite(
        &mut self,
        x: i32,
        y: i32,
        sprite: &[&str],
        direction: f32,
        style: SpriteStyle,
    ) -> bool {
        let mut visible = false;
        for (row_index, row) in sprite.iter().enumerate() {
            for_each_sprite_glyph(row, direction, |column, glyph| {
                if glyph == ' ' {
                    return;
                }
                let fg = if row_index == style.eye_row && glyph == '●' {
                    style.eye_color
                } else if matches!(glyph, '▄' | '▀' | '◢' | '◣') {
                    mix(style.color, Rgb(224, 242, 235), 0.25)
                } else if matches!(glyph, '▓' | '▒' | '░') {
                    mix(style.color, Rgb(12, 39, 61), 0.32)
                } else if glyph == '▶' || glyph == '◀' {
                    mix(style.color, Rgb(180, 218, 228), 0.28)
                } else {
                    style.color
                };
                visible |= self.put_object(x + column as i32, y + row_index as i32, glyph, fg);
            });
        }
        visible
    }
}

fn for_each_sprite_glyph(row: &str, direction: f32, mut visit: impl FnMut(usize, char)) {
    if direction < 0.0 {
        for (column, glyph) in row.chars().rev().enumerate() {
            visit(column, mirror(glyph));
        }
    } else {
        for (column, glyph) in row.chars().enumerate() {
            visit(column, glyph);
        }
    }
}

fn mirror(glyph: char) -> char {
    match glyph {
        '<' => '>',
        '>' => '<',
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '◀' => '▶',
        '▶' => '◀',
        '◢' => '◣',
        '◣' => '◢',
        '/' => '\\',
        '\\' => '/',
        _ => glyph,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocean_register_metadata_and_resize_keep_the_seabed_anchored() {
        let mut theme = OceanTheme::new(72, 24);
        assert_eq!(theme.id(), "ocean");
        assert!((4..=6).contains(&theme.seabed_height));

        let mut canvas = Canvas::new(72, 24);
        theme.update(0.1, 0.0);
        theme.render(&mut canvas);
        assert_eq!((canvas.width, canvas.height), (72, 24));
        assert_ne!(canvas.cell(0, 0).bg, canvas.cell(0, 23).bg);
        assert!(canvas
            .cells_mut()
            .iter()
            .any(|cell| { matches!(cell.ch, '◈' | '█' | '▐' | '▓') }));

        theme.resize(45, 12);
        theme.render(&mut canvas);
        assert_eq!((canvas.width, canvas.height), (45, 12));
        assert_eq!(theme.seabed_height, 4);
        assert!(canvas
            .cells_mut()
            .iter()
            .skip(45 * 8)
            .any(|cell| cell.ch == '~'));
    }

    #[test]
    fn terminal_activity_increases_bubble_emission_and_entities_stay_bounded() {
        let mut theme = OceanTheme::new(80, 28);
        let before = theme.bubbles.len();
        theme.update(0.25, 0.0);
        assert_eq!(theme.bubbles.len(), before);
        theme.update(0.25, 12.0);
        assert!(theme.bubbles.len() > before);
        for _ in 0..40 {
            theme.update(0.25, 12.0);
        }
        assert!(theme.bubbles.len() <= MAX_BUBBLES);
        assert!(theme.sharks.len() <= MAX_SHARKS);
        assert!(theme.creatures.len() <= MAX_FOREGROUND_CREATURES);
    }

    #[test]
    fn tiny_and_zero_sized_terminals_render_without_invalid_indices() {
        for (width, height) in [(0, 0), (1, 1), (4, 2), (12, 5), (39, 14)] {
            let mut theme = OceanTheme::new(width, height);
            let mut canvas = Canvas::new(width, height);
            theme.resize(width, height);
            theme.update(0.033, 1.0);
            theme.render(&mut canvas);
            assert_eq!((canvas.width, canvas.height), (width, height));
            assert_eq!(canvas.cells_mut().len(), width as usize * height as usize);
        }
    }

    #[test]
    fn shark_sprites_have_two_frames_and_mirror_directional_glyphs() {
        assert_ne!(MEDIUM_SHARK_A, MEDIUM_SHARK_B);
        assert_ne!(LARGE_SHARK_A, LARGE_SHARK_B);
        assert!(MEDIUM_SHARK_A
            .iter()
            .all(|row| row.chars().count() <= shark_width(SharkSize::Medium) as usize));
        assert!(LARGE_SHARK_A
            .iter()
            .all(|row| row.chars().count() <= shark_width(SharkSize::Large) as usize));
        assert_eq!(mirror('/'), '\\');
        assert_eq!(mirror('<'), '>');
        assert_eq!(mirror('◀'), '▶');
        assert!(
            MEDIUM_SHARK_A
                .iter()
                .chain(LARGE_SHARK_A.iter())
                .flat_map(|line| line.chars())
                .filter(|glyph| matches!(glyph, '█' | '▓' | '▄' | '▀'))
                .count()
                > 30
        );
        assert!(TROPICAL_A
            .iter()
            .chain(RAY_A.iter())
            .chain(JELLY_A.iter())
            .flat_map(|line| line.chars())
            .any(|glyph| matches!(glyph, '█' | '▓' | '▄' | '▀')));
    }

    #[test]
    fn seabed_sand_color_profile_is_steady_while_plants_sway() {
        let mut theme = OceanTheme::new(80, 24);
        let mut first_cells = vec![Cell::default(); 80 * 24];
        let mut first_occupancy = vec![false; 80 * 24];
        theme.render_seabed(&mut SceneBuffer {
            cells: &mut first_cells,
            occupied: &mut first_occupancy,
            width: 80,
            height: 24,
        });
        let first_profile = first_cells.iter().map(|cell| cell.bg).collect::<Vec<_>>();

        theme.time = 60.0;
        let mut second_cells = vec![Cell::default(); 80 * 24];
        let mut second_occupancy = vec![false; 80 * 24];
        theme.render_seabed(&mut SceneBuffer {
            cells: &mut second_cells,
            occupied: &mut second_occupancy,
            width: 80,
            height: 24,
        });

        assert_eq!(
            first_profile,
            second_cells.iter().map(|cell| cell.bg).collect::<Vec<_>>()
        );
    }

    #[test]
    fn jellyfish_spawn_inside_the_water_column_before_rising() {
        let mut theme = OceanTheme::new(80, 24);
        let jellyfish = theme.new_creature(CreatureKind::Jellyfish);
        assert!((0.0..80.0).contains(&jellyfish.x));
        assert!(matches!(jellyfish.kind, CreatureKind::Jellyfish));
    }
}
