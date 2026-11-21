use std::cell::RefCell;

use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::engine::{canvas::Cell, Canvas, Rgb, Theme};

const MAX_BUBBLES: usize = 56;
const MAX_SHARKS: usize = 4;
const MAX_FOREGROUND_CREATURES: usize = 5;
const MAX_DISTANT_FISH: usize = 16;

// Large Great White Shark sprites (4-frame swimming cycle with realistic tail undulation)
pub const LARGE_SHARK_0: [&str; 5] = [
    "         ▄▄█▄▄        ",
    "      ▄█████████▄     ",
    "◄◄◄████████████████●► ",
    "   ▀█████████████▀▀   ",
    "       ▀▀███▀▀        ",
];
pub const LARGE_SHARK_1: [&str; 5] = [
    "  ▲      ▄▄█▄▄        ",
    "  ◄██ ▄█████████▄     ",
    "   ◄███████████████●► ",
    "   ▀█████████████▀▀   ",
    "       ▀▀███▀▀        ",
];
pub const LARGE_SHARK_2: [&str; 5] = [
    "         ▄▄█▄▄        ",
    "      ▄█████████▄     ",
    "◄◄◄████████████████●► ",
    "   ▀█████████████▀▀   ",
    "       ▀▀███▀▀        ",
];
pub const LARGE_SHARK_3: [&str; 5] = [
    "         ▄▄█▄▄        ",
    "      ▄█████████▄     ",
    "   ◄███████████████●► ",
    "  ◄██ ▀██████████▀▀   ",
    "  ▼    ▀▀███▀▀        ",
];

// Medium Cruiser Shark sprites (4-frame swimming cycle)
pub const MEDIUM_SHARK_0: [&str; 4] = [
    "      ▄█▄       ",
    "   ▄████████▄   ",
    "◄◄███▓███████●► ",
    "   ▀████████▀   ",
];
pub const MEDIUM_SHARK_1: [&str; 4] = [
    " ▲    ▄█▄       ",
    " ◄██████████▄   ",
    "  ◄██▓███████●► ",
    "   ▀████████▀   ",
];
pub const MEDIUM_SHARK_2: [&str; 4] = [
    "      ▄█▄       ",
    "   ▄████████▄   ",
    "◄◄███▓███████●► ",
    "   ▀████████▀   ",
];
pub const MEDIUM_SHARK_3: [&str; 4] = [
    "      ▄█▄       ",
    "   ▄████████▄   ",
    "  ◄██▓███████●► ",
    " ▼ ▀████████▀   ",
];

// Distant silhouette shark cruising in deep water
pub const SHADOW_SHARK_0: [&str; 2] = ["    ▲▄▄▄    ", "◄◄████████●►"];
pub const SHADOW_SHARK_1: [&str; 2] = [" ▲  ▲▄▄▄    ", " ◄████████●►"];

pub const TROPICAL_A: [&str; 3] = ["  ▲  ", "◄███●►", "  ▼  "];
pub const TROPICAL_B: [&str; 3] = [" ◢█  ", "◄███●►", " ◥█  "];

pub const RAY_A: [&str; 5] = [
    "  ▄▄████▄▄  ",
    "▄██████████▄",
    "▀██████████▀",
    "    ▀██▀    ",
    "     ││     ",
];
pub const RAY_B: [&str; 5] = [
    " ▄████████▄ ",
    "▀██████████▀",
    "  ▀██████▀  ",
    "    ▀██▀    ",
    "     ││     ",
];

pub const JELLY_A: [&str; 4] = [" ▄████▄ ", "████████", " ▀▀██▀▀ ", " ⌠│⌡⌠│⌡ "];
pub const JELLY_B: [&str; 4] = [" ▄████▄ ", " ▓████▓ ", "  ▀██▀  ", "  ││││  "];

pub const SCHOOL_FISH_A: [&str; 3] = [" ▄▄█▄ ", "◄███●►", " ▀▀█▀ "];
pub const SCHOOL_FISH_B: [&str; 3] = ["  ▄▄█▄", "◄███●►", "  ▀▀█▀"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SharkSize {
    Shadow,
    Medium,
    Large,
}

#[derive(Clone, Copy)]
pub struct Shark {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub direction: f32,
    pub phase: f32,
    pub size: SharkSize,
    pub depth_tint: f32,
}

#[derive(Clone, Copy)]
pub enum CreatureKind {
    Tropical,
    Ray,
    Jellyfish,
}

#[derive(Clone, Copy)]
pub struct ForegroundCreature {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub direction: f32,
    pub phase: f32,
    pub color: Rgb,
    pub kind: CreatureKind,
}

#[derive(Clone, Copy)]
pub struct DistantFish {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub direction: f32,
    pub glyph: char,
    pub phase: f32,
}

#[derive(Clone, Copy)]
pub struct MidFish {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub direction: f32,
    pub phase: f32,
    pub color: Rgb,
}

#[derive(Clone, Copy)]
pub enum BubbleKind {
    Tiny,    // ·, °
    Small,   // ⚬, o
    Medium,  // ○, ◌
    Cluster, // ( ), (°)
}

#[derive(Clone, Copy)]
pub struct Bubble {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub phase: f32,
    pub kind: BubbleKind,
    pub wobble_speed: f32,
    pub wobble_amp: f32,
    pub color: Rgb,
}

#[derive(Clone, Copy)]
pub enum BedKind {
    Coral,
    Kelp,
    RockVent,
    SeaAnemone,
}

#[derive(Clone, Copy)]
pub struct BedFeature {
    pub x: f32,
    pub height: u8,
    pub phase: f32,
    pub color: Rgb,
    pub kind: BedKind,
}

pub struct OceanTheme {
    pub width: u16,
    pub height: u16,
    pub seabed_height: u16,
    pub time: f32,
    pub activity: f32,
    pub bubble_emission: f32,
    pub burst_active: bool,
    pub burst_timer: f32,
    pub cpu_usage: f32,
    pub memory_fraction: f32,
    pub shark_spawn_timer: f32,
    pub creature_spawn_timer: f32,
    pub features: Vec<BedFeature>,
    pub bubbles: Vec<Bubble>,
    pub distant_fish: Vec<DistantFish>,
    pub mid_fish: Vec<MidFish>,
    pub sharks: Vec<Shark>,
    pub creatures: Vec<ForegroundCreature>,
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
            burst_active: false,
            burst_timer: 0.0,
            cpu_usage: 0.0,
            memory_fraction: 0.0,
            shark_spawn_timer: 1.5,
            creature_spawn_timer: 2.0,
            features: Vec::new(),
            bubbles: Vec::new(),
            distant_fish: Vec::new(),
            mid_fish: Vec::new(),
            sharks: Vec::new(),
            creatures: Vec::new(),
            occupancy: RefCell::new(vec![false; width as usize * height as usize]),
            rng: StdRng::from_entropy(),
        };
        theme.rebuild_seabed();
        theme.bubbles = (0..20).map(|_| theme.new_bubble(false)).collect();
        theme.distant_fish = (0..MAX_DISTANT_FISH)
            .map(|_| theme.new_distant_fish(true))
            .collect();
        for _ in 0..2 {
            let school = theme.new_fish_school();
            theme.mid_fish.extend(school);
        }
        if theme.can_swim() {
            // Immediate shark presence across depths
            let mut shark1 = theme.new_shark();
            shark1.x = width as f32 * 0.25;
            let mut shark2 = theme.new_shark();
            shark2.x = width as f32 * 0.7;
            shark2.direction = -1.0;
            theme.sharks.push(shark1);
            theme.sharks.push(shark2);

            let tropical = theme.new_creature(CreatureKind::Tropical);
            let jellyfish = theme.new_creature(CreatureKind::Jellyfish);
            let ray = theme.new_creature(CreatureKind::Ray);
            theme.creatures.push(tropical);
            theme.creatures.push(jellyfish);
            theme.creatures.push(ray);
        }
        theme
    }

    pub fn rebuild_seabed(&mut self) {
        self.seabed_height = seabed_height(self.height);
        self.features.clear();
        if self.width == 0 || self.height == 0 {
            return;
        }
        let count = (self.width as usize / 7).clamp(2, 22);
        let spacing = self.width as f32 / count as f32;
        let coral_colors = [
            Rgb(242, 108, 142),
            Rgb(245, 148, 86),
            Rgb(185, 122, 225),
            Rgb(248, 192, 92),
            Rgb(95, 218, 195),
        ];
        let kelp_colors = [
            Rgb(32, 145, 105),
            Rgb(46, 172, 122),
            Rgb(24, 118, 84),
            Rgb(62, 198, 142),
        ];
        for index in 0..count {
            let jitter = self.rng.gen_range(-spacing * 0.2..spacing * 0.2);
            let x = (spacing * (index as f32 + 0.5) + jitter)
                .clamp(0.0, self.width.saturating_sub(1) as f32);
            let kind = match index % 4 {
                0 => BedKind::Kelp,
                1 => BedKind::Coral,
                2 => BedKind::Kelp,
                _ => {
                    if index % 2 == 0 {
                        BedKind::RockVent
                    } else {
                        BedKind::SeaAnemone
                    }
                }
            };
            let color = match kind {
                BedKind::Coral => coral_colors[self.rng.gen_range(0..coral_colors.len())],
                BedKind::Kelp => kelp_colors[self.rng.gen_range(0..kelp_colors.len())],
                BedKind::RockVent => Rgb(95, 90, 85),
                BedKind::SeaAnemone => coral_colors[self.rng.gen_range(0..coral_colors.len())],
            };
            let height = match kind {
                BedKind::Kelp => self.rng.gen_range(4..=8),
                BedKind::Coral => self.rng.gen_range(3..=5),
                BedKind::RockVent => self.rng.gen_range(2..=3),
                BedKind::SeaAnemone => 2,
            };
            self.features.push(BedFeature {
                x,
                height,
                phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
                color,
                kind,
            });
        }
    }

    pub fn new_bubble(&mut self, from_seabed: bool) -> Bubble {
        let x = if from_seabed && !self.features.is_empty() {
            let feature = self.features[self.rng.gen_range(0..self.features.len())];
            feature.x + self.rng.gen_range(-0.8..0.8)
        } else {
            self.rng.gen_range(0.0..self.width.max(1) as f32)
        };
        let surface = self.height.saturating_sub(self.seabed_height) as f32;
        let y = if from_seabed {
            (surface - self.rng.gen_range(0.0..1.5)).max(0.0)
        } else {
            self.rng.gen_range(0.0..surface.max(1.0))
        };
        let kind = match self.rng.gen_range(0..10) {
            0..=3 => BubbleKind::Tiny,
            4..=6 => BubbleKind::Small,
            7..=8 => BubbleKind::Medium,
            _ => BubbleKind::Cluster,
        };
        let speed = match kind {
            BubbleKind::Tiny => self.rng.gen_range(0.5..0.9),
            BubbleKind::Small => self.rng.gen_range(0.8..1.3),
            BubbleKind::Medium => self.rng.gen_range(1.1..1.7),
            BubbleKind::Cluster => self.rng.gen_range(1.3..2.0),
        };
        Bubble {
            x,
            y,
            speed,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
            kind,
            wobble_speed: self.rng.gen_range(2.0..4.5),
            wobble_amp: self.rng.gen_range(0.4..1.1),
            color: Rgb(190, 238, 250),
        }
    }

    pub fn new_distant_fish(&mut self, anywhere: bool) -> DistantFish {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let swim_ceiling = self.height.saturating_sub(self.seabed_height).max(2);
        let x = if anywhere {
            self.rng.gen_range(0.0..self.width.max(1) as f32)
        } else if direction > 0.0 {
            -4.0
        } else {
            self.width as f32 + 4.0
        };
        let glyphs = ['›', '‹', '≺', '≻', '∾', '·'];
        DistantFish {
            x,
            y: self.rng.gen_range(1.0..swim_ceiling as f32),
            speed: self.rng.gen_range(0.4..1.0),
            direction,
            glyph: glyphs[self.rng.gen_range(0..glyphs.len())],
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
        }
    }

    pub fn new_fish_school(&mut self) -> Vec<MidFish> {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let start_x = self.rng.gen_range(0.0..self.width.max(1) as f32);
        let upper_water = self.height.saturating_sub(self.seabed_height).max(2);
        let center_y = self.rng.gen_range(1.0..upper_water as f32);
        let speed = self.rng.gen_range(1.4..2.4);
        let phase = self.rng.gen_range(0.0..std::f32::consts::TAU);
        let colors = [
            Rgb(58, 142, 176),
            Rgb(72, 165, 192),
            Rgb(86, 178, 168),
            Rgb(102, 148, 195),
        ];
        let color = colors[self.rng.gen_range(0..colors.len())];
        (0..4)
            .map(|index| MidFish {
                x: start_x - direction * index as f32 * 4.5,
                y: (center_y + (index as f32 - 1.5) * 0.6)
                    .clamp(1.0, upper_water.saturating_sub(1) as f32),
                speed,
                direction,
                phase: phase + index as f32 * 0.25,
                color,
            })
            .collect()
    }

    pub fn new_shark(&mut self) -> Shark {
        let size = match self.rng.gen_range(0..10) {
            0..=2 => SharkSize::Shadow,
            3..=6 => SharkSize::Medium,
            _ => SharkSize::Large,
        };
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let width = shark_width(size) as f32;
        let swim_height = self
            .height
            .saturating_sub(self.seabed_height)
            .saturating_sub(shark_height(size));
        let speed = match size {
            SharkSize::Shadow => self.rng.gen_range(0.9..1.6),
            SharkSize::Medium => self.rng.gen_range(2.0..3.0),
            SharkSize::Large => self.rng.gen_range(1.6..2.5),
        };
        let depth_tint = match size {
            SharkSize::Shadow => 0.55,
            SharkSize::Medium => 0.22,
            SharkSize::Large => 0.05,
        };
        Shark {
            x: if direction > 0.0 {
                -width - self.rng.gen_range(1.0..6.0)
            } else {
                self.width as f32 + self.rng.gen_range(1.0..6.0)
            },
            y: self.rng.gen_range(1.0..swim_height.max(2) as f32),
            speed,
            direction,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
            size,
            depth_tint,
        }
    }

    pub fn new_creature(&mut self, kind: CreatureKind) -> ForegroundCreature {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let upper_water = self
            .height
            .saturating_sub(self.seabed_height)
            .saturating_sub(4)
            .max(2);
        let x = if matches!(kind, CreatureKind::Jellyfish) {
            self.rng.gen_range(0.0..self.width.max(1) as f32)
        } else if direction > 0.0 {
            -10.0
        } else {
            self.width as f32 + 3.0
        };
        ForegroundCreature {
            x,
            y: self.rng.gen_range(1.0..upper_water as f32),
            speed: match kind {
                CreatureKind::Tropical => self.rng.gen_range(2.2..3.8),
                CreatureKind::Ray => self.rng.gen_range(1.3..2.2),
                CreatureKind::Jellyfish => self.rng.gen_range(0.5..1.0),
            },
            direction,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
            color: match kind {
                CreatureKind::Tropical => Rgb(248, 168, 64),
                CreatureKind::Ray => Rgb(118, 178, 210),
                CreatureKind::Jellyfish => Rgb(208, 142, 235),
            },
            kind,
        }
    }

    pub fn can_swim(&self) -> bool {
        self.height >= self.seabed_height.saturating_add(6) && self.width >= 10
    }

    pub fn trigger_bubble_burst(&mut self) {
        self.burst_active = true;
        self.burst_timer = 2.5;
        let surge = 8.min(MAX_BUBBLES.saturating_sub(self.bubbles.len()));
        for _ in 0..surge {
            let bubble = self.new_bubble(true);
            self.bubbles.push(bubble);
        }
    }

    fn update_entities(&mut self, dt: f32, activity: f32) {
        // Bubbles update
        let active_wobble_boost = if self.burst_timer > 0.0 { 1.5 } else { 1.0 };
        for index in 0..self.bubbles.len() {
            self.bubbles[index].y -= self.bubbles[index].speed * active_wobble_boost * dt;
            if self.bubbles[index].y < -1.0 {
                let bubble = self.new_bubble(true);
                self.bubbles[index] = bubble;
            }
        }

        // Bubble emissions
        if self.bubbles.len() < MAX_BUBBLES {
            let burst_emission = if self.burst_timer > 0.0 { 4.0 } else { 0.0 };
            self.bubble_emission += dt * (0.18 + activity * 0.45 + burst_emission);
            while self.bubble_emission >= 1.0 && self.bubbles.len() < MAX_BUBBLES {
                self.bubble_emission -= 1.0;
                let bubble = self.new_bubble(true);
                self.bubbles.push(bubble);
            }
        } else {
            self.bubble_emission = 0.0;
        }

        if self.burst_timer > 0.0 {
            self.burst_timer -= dt;
            if self.burst_timer <= 0.0 {
                self.burst_active = false;
            }
        }

        // Distant background fish
        for index in 0..self.distant_fish.len() {
            let (speed, direction) = (
                self.distant_fish[index].speed,
                self.distant_fish[index].direction,
            );
            self.distant_fish[index].x += speed * direction * dt;
            let x = self.distant_fish[index].x;
            if (direction > 0.0 && x > self.width as f32 + 3.0) || (direction < 0.0 && x < -3.0) {
                let fish = self.new_distant_fish(false);
                self.distant_fish[index] = fish;
            }
        }

        // Midground schooling fish
        for fish in &mut self.mid_fish {
            fish.x += fish.speed * fish.direction * dt;
            fish.phase += dt * 3.0;
            if (fish.direction > 0.0 && fish.x > self.width as f32 + 6.0)
                || (fish.direction < 0.0 && fish.x < -8.0)
            {
                fish.x = if fish.direction > 0.0 {
                    -6.0
                } else {
                    self.width as f32 + 6.0
                };
                let upper = self.height.saturating_sub(self.seabed_height).max(2);
                fish.y = self.rng.gen_range(1.0..upper as f32);
            }
        }

        // Sharks (predators with fluid tail undulation)
        for shark in &mut self.sharks {
            shark.x += shark.speed * shark.direction * dt;
            shark.phase += dt * 2.8;
        }
        self.sharks.retain(|shark| {
            if shark.direction > 0.0 {
                shark.x < self.width as f32 + 6.0
            } else {
                shark.x + shark_width(shark.size) as f32 > -6.0
            }
        });
        if self.can_swim() {
            self.shark_spawn_timer -= dt;
            if (self.shark_spawn_timer <= 0.0 || self.sharks.len() < 2)
                && self.sharks.len() < MAX_SHARKS
            {
                let shark = self.new_shark();
                self.sharks.push(shark);
                self.shark_spawn_timer = self.rng.gen_range(3.5..7.0);
            }
        }

        // Foreground creatures (rays, tropical, jellyfish)
        for creature in &mut self.creatures {
            creature.phase += dt * 2.5;
            match creature.kind {
                CreatureKind::Jellyfish => {
                    creature.y -= creature.speed * dt;
                    creature.x += (creature.phase * 0.8).sin() * 0.25 * dt;
                }
                CreatureKind::Tropical | CreatureKind::Ray => {
                    creature.x += creature.speed * creature.direction * dt;
                }
            }
        }
        self.creatures.retain(|creature| match creature.kind {
            CreatureKind::Jellyfish => creature.y > -5.0,
            CreatureKind::Tropical | CreatureKind::Ray => {
                creature.x > -14.0 && creature.x < self.width as f32 + 10.0
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
                self.creature_spawn_timer = self.rng.gen_range(4.0..8.0);
            }
        }
    }

    fn render_background(&self, cells: &mut [Cell]) {
        let width = self.width as usize;
        let height = self.height as usize;
        let surface_water = Rgb(16, 58, 96);
        let deep_water = Rgb(3, 14, 30);
        let caustic_origins = [0.12, 0.38, 0.65, 0.88];
        let caustic_slopes = [0.15, -0.09, 0.12, -0.14];
        let current_speed = 0.2 + self.cpu_usage * 0.005;

        for y in 0..height {
            let depth = y as f32 / height.saturating_sub(1).max(1) as f32;
            let ambient = mix(surface_water, deep_water, depth);
            for x in 0..width {
                let mut color = ambient;
                let x_f = x as f32;

                // Subtle animated light shafts (caustics) piercing down from surface
                for beam in 0..caustic_origins.len() {
                    let center = self.width as f32 * caustic_origins[beam]
                        + y as f32 * caustic_slopes[beam]
                        + (self.time * current_speed + beam as f32 * 2.3).sin() * 1.8;
                    let dist = (x_f - center).abs();
                    let beam_w = (self.width as f32 * 0.042).clamp(2.0, 6.0);
                    let intensity = (-(dist * dist) / (2.0 * beam_w * beam_w)).exp()
                        * (1.0 - depth * 0.75)
                        * 0.38;
                    color = mix(color, Rgb(75, 160, 202), intensity);
                }

                // Shimmering water ripples
                let shimmer =
                    ((x_f * 0.04 + y as f32 * 0.02 + self.time * 0.2).sin() * 1.8).round();
                let fg = Rgb(
                    (color.0 as f32 + shimmer).clamp(0.0, 255.0) as u8,
                    (color.1 as f32 + shimmer).clamp(0.0, 255.0) as u8,
                    (color.2 as f32 + shimmer).clamp(0.0, 255.0) as u8,
                );
                cells[y * width + x] = make_cell('▀', fg, color);
            }
        }
    }

    pub fn render_seabed(&self, scene: &mut SceneBuffer<'_>) {
        if scene.width == 0 || scene.height == 0 {
            return;
        }
        let width = scene.width as usize;
        let base_y = scene.height.saturating_sub(self.seabed_height) as i32;
        let sand_sunlit = Rgb(162, 132, 78);
        let sand_dark = Rgb(48, 42, 34);

        for x in 0..scene.width as i32 {
            // Naturally curving undulating sand dunes
            let x_f = x as f32;
            let dune = ((x_f * 0.08).sin() * 1.2 + (x_f * 0.18 + 1.2).cos() * 0.6).round() as i32;
            let ground_y = (base_y + dune).clamp(0, scene.height as i32 - 1);

            for y in ground_y..scene.height as i32 {
                let depth = (y - ground_y) as f32 / (scene.height as i32 - ground_y).max(1) as f32;
                let mut bg = mix(sand_sunlit, sand_dark, depth);
                let n = hash(x as u32, y as u32);

                if n % 7 == 0 {
                    bg = mix(bg, Rgb(185, 155, 95), 0.25);
                }

                // Varied, rich sand texture: ripples, shells, pebbles, starfish
                let glyph = if y == ground_y {
                    match n % 6 {
                        0 => '~',
                        1 => '≈',
                        2 => '∿',
                        3 => '▄',
                        4 => '░',
                        _ => '▀',
                    }
                } else if n % 29 == 0 {
                    '★' // Small starfish
                } else if n % 17 == 0 {
                    '⚬' // Small shell / sea pebble
                } else if n % 9 == 0 {
                    '▒'
                } else if n % 5 == 0 {
                    '░'
                } else {
                    '·'
                };

                let fg = match glyph {
                    '★' => Rgb(225, 120, 95),
                    '⚬' => Rgb(235, 230, 210),
                    '~' | '≈' | '∿' => mix(bg, Rgb(205, 175, 115), 0.6),
                    _ => mix(bg, Rgb(190, 155, 90), 0.3),
                };

                let index = y as usize * width + x as usize;
                scene.cells[index] = make_cell(glyph, fg, bg);
                scene.occupied[index] = true;
            }
        }

        // Render seabed features: Kelp forests, corals, rock vents, sea anemones
        for feature in &self.features {
            match feature.kind {
                BedKind::Kelp => self.render_kelp(scene, *feature),
                BedKind::Coral => self.render_coral(scene, *feature),
                BedKind::RockVent => self.render_vent(scene, *feature),
                BedKind::SeaAnemone => self.render_anemone(scene, *feature),
            }
        }
    }

    fn render_kelp(&self, scene: &mut SceneBuffer<'_>, feature: BedFeature) {
        let max_h = feature
            .height
            .min(self.seabed_height.saturating_add(3) as u8) as i32;
        let active_sway_mult = 1.0 + self.activity * 0.08;

        for segment in 0..max_h {
            let y = scene.height as i32 - 1 - segment;
            // Graceful multi-joint physics: tips sway wider with harmonic wave lag
            let sway = (self.time * 1.8 * active_sway_mult + feature.phase + segment as f32 * 0.45)
                .sin()
                * (0.35 + segment as f32 * 0.28);
            let center = feature.x.round() as i32 + sway.round() as i32;

            let glyph = if segment == max_h - 1 {
                '◈' // Budding top leaf
            } else if segment % 2 == 0 {
                '▌'
            } else {
                '▐'
            };

            // Color gradient from deep sea kelp base to light-kissed emerald tip
            let shade = segment as f32 / max_h.max(1) as f32;
            let kelp_color = mix(feature.color, Rgb(85, 225, 155), shade * 0.7);

            scene.put_feature(center, y, glyph, kelp_color);

            // Side leaves reaching out into the water
            if segment > 1 && segment < max_h - 1 && segment % 2 == 1 {
                let side_glyph = if (segment / 2) % 2 == 0 { '⌠' } else { '⌡' };
                scene.put_feature(
                    center - 1,
                    y,
                    side_glyph,
                    mix(kelp_color, Rgb(40, 160, 110), 0.3),
                );
                scene.put_feature(
                    center + 1,
                    y,
                    side_glyph,
                    mix(kelp_color, Rgb(40, 160, 110), 0.3),
                );
            }
        }
    }

    fn render_coral(&self, scene: &mut SceneBuffer<'_>, feature: BedFeature) {
        let bottom = scene.height as i32 - 1;
        let branches = feature
            .height
            .min(self.seabed_height.saturating_add(1) as u8) as i32;

        for segment in 0..branches {
            let y = bottom - segment;
            let sway = (self.time * 0.6 + feature.phase + segment as f32 * 0.3).sin() * 0.3;
            let center = feature.x.round() as i32 + sway.round() as i32;

            let stem = match segment % 3 {
                0 => '█',
                1 => '▓',
                _ => '▒',
            };
            let shaded = mix(feature.color, Rgb(255, 205, 175), segment as f32 * 0.1);
            scene.put_feature(center, y, stem, shaded);

            if segment > 0 && segment % 2 == 0 {
                scene.put_feature(
                    center - 1,
                    y,
                    '◢',
                    mix(feature.color, Rgb(255, 185, 160), 0.25),
                );
                scene.put_feature(
                    center + 1,
                    y,
                    '◣',
                    mix(feature.color, Rgb(255, 185, 160), 0.15),
                );
            }
            if segment == branches - 1 {
                scene.put_feature(
                    center - 1,
                    y - 1,
                    '●',
                    mix(feature.color, Rgb(255, 220, 180), 0.4),
                );
                scene.put_feature(
                    center + 1,
                    y - 1,
                    '●',
                    mix(feature.color, Rgb(255, 220, 180), 0.3),
                );
            }
        }
    }

    fn render_vent(&self, scene: &mut SceneBuffer<'_>, feature: BedFeature) {
        let bottom = scene.height as i32 - 1;
        let x = feature.x.round() as i32;
        // Volcanic rock cone with venting orifice
        scene.put_feature(x - 1, bottom, '◢', Rgb(75, 70, 65));
        scene.put_feature(x, bottom, '█', Rgb(55, 50, 48));
        scene.put_feature(x + 1, bottom, '◣', Rgb(75, 70, 65));
        scene.put_feature(x, bottom - 1, '▲', Rgb(95, 88, 80));
    }

    fn render_anemone(&self, scene: &mut SceneBuffer<'_>, feature: BedFeature) {
        let bottom = scene.height as i32 - 1;
        let x = feature.x.round() as i32;
        let sway = (self.time * 2.2 + feature.phase).sin() * 0.4;
        let sx = x + sway.round() as i32;
        scene.put_feature(sx - 1, bottom - 1, '⌠', feature.color);
        scene.put_feature(
            sx,
            bottom - 1,
            '✿',
            mix(feature.color, Rgb(255, 220, 200), 0.3),
        );
        scene.put_feature(sx + 1, bottom - 1, '⌡', feature.color);
    }

    fn render_bubbles(&self, scene: &mut SceneBuffer<'_>) {
        for bubble in &self.bubbles {
            let wobble = (self.time * bubble.wobble_speed + bubble.phase).sin() * bubble.wobble_amp;
            let ix = (bubble.x + wobble).round() as i32;
            let iy = bubble.y.round() as i32;

            match bubble.kind {
                BubbleKind::Tiny => {
                    scene.put_object(ix, iy, '·', Rgb(180, 230, 245));
                }
                BubbleKind::Small => {
                    scene.put_object(ix, iy, '°', Rgb(200, 240, 252));
                }
                BubbleKind::Medium => {
                    scene.put_object(ix, iy, '○', bubble.color);
                    scene.put_object(ix - 1, iy, '·', Rgb(150, 215, 235));
                }
                BubbleKind::Cluster => {
                    scene.put_object(ix, iy, 'o', Rgb(220, 248, 255));
                    scene.put_object(ix - 1, iy + 1, '°', Rgb(175, 228, 245));
                    scene.put_object(ix + 1, iy - 1, '·', Rgb(190, 235, 250));
                }
            }
        }
    }

    fn render_distant_fish(&self, scene: &mut SceneBuffer<'_>) {
        let deep_water = Rgb(18, 48, 76);
        for fish in &self.distant_fish {
            let y = fish.y + (self.time * 1.2 + fish.phase).sin() * 0.3;
            let ix = fish.x.round() as i32;
            let iy = y.round() as i32;
            scene.put_object(ix, iy, fish.glyph, deep_water);
        }
    }

    fn render_mid_fish(&self, scene: &mut SceneBuffer<'_>) {
        for fish in &self.mid_fish {
            let y = fish.y + (self.time * 1.4 + fish.phase).sin() * 0.4;
            let frame = fish.phase.sin() > 0.0;
            let sprite = if frame {
                &SCHOOL_FISH_A
            } else {
                &SCHOOL_FISH_B
            };
            scene.draw_sprite(
                fish.x.round() as i32,
                y.round() as i32,
                sprite,
                fish.direction,
                SpriteStyle {
                    color: fish.color,
                    eye_color: Rgb(225, 245, 250),
                    eye_row: 1,
                },
            );
        }
    }

    fn render_sharks(&self, scene: &mut SceneBuffer<'_>) {
        for shark in &self.sharks {
            let cycle = (shark.phase.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU
                * 4.0) as usize;
            let (sprite, color, eye_row): (&[&str], Rgb, usize) = match (shark.size, cycle % 4) {
                (SharkSize::Large, 0) => (&LARGE_SHARK_0, Rgb(128, 185, 208), 2),
                (SharkSize::Large, 1) => (&LARGE_SHARK_1, Rgb(128, 185, 208), 2),
                (SharkSize::Large, 2) => (&LARGE_SHARK_2, Rgb(128, 185, 208), 2),
                (SharkSize::Large, _) => (&LARGE_SHARK_3, Rgb(128, 185, 208), 2),

                (SharkSize::Medium, 0) => (&MEDIUM_SHARK_0, Rgb(102, 162, 188), 2),
                (SharkSize::Medium, 1) => (&MEDIUM_SHARK_1, Rgb(102, 162, 188), 2),
                (SharkSize::Medium, 2) => (&MEDIUM_SHARK_2, Rgb(102, 162, 188), 2),
                (SharkSize::Medium, _) => (&MEDIUM_SHARK_3, Rgb(102, 162, 188), 2),

                (SharkSize::Shadow, 0) | (SharkSize::Shadow, 2) => {
                    (&SHADOW_SHARK_0, Rgb(32, 68, 98), 1)
                }
                (SharkSize::Shadow, _) => (&SHADOW_SHARK_1, Rgb(32, 68, 98), 1),
            };

            // Depth tint shading: distant sharks blend into deep ocean blues
            let final_color = mix(color, Rgb(12, 38, 64), shark.depth_tint);
            scene.draw_sprite(
                shark.x.round() as i32,
                shark.y.round() as i32,
                sprite,
                shark.direction,
                SpriteStyle {
                    color: final_color,
                    eye_color: if shark.size == SharkSize::Shadow {
                        Rgb(65, 115, 150)
                    } else {
                        Rgb(240, 252, 255)
                    },
                    eye_row,
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
                creature.y + (self.time * 0.9 + creature.phase).sin() * 0.45
            };
            scene.draw_sprite(
                creature.x.round() as i32,
                y.round() as i32,
                sprite,
                direction,
                SpriteStyle {
                    color: creature.color,
                    eye_color: Rgb(248, 235, 190),
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
        "A living ocean with apex sharks, multi-depth fish, fizzy bubble streams, and lush kelp."
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

        for fish in &mut self.mid_fish {
            fish.x = fish.x / old_width * width as f32;
            fish.y = (fish.y / old_height * height as f32)
                .min(height.saturating_sub(self.seabed_height).saturating_sub(1) as f32);
        }
        for shark in &mut self.sharks {
            shark.x = shark.x / old_width * width as f32;
            shark.y = (shark.y / old_height * height as f32)
                .min(height.saturating_sub(self.seabed_height).saturating_sub(4) as f32);
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

    fn set_output_burst(&mut self, active: bool) {
        if active && !self.burst_active {
            self.trigger_bubble_burst();
        }
        self.burst_active = active;
    }

    fn set_system_metrics(&mut self, cpu_usage: f32, memory_fraction: f32) {
        self.cpu_usage = cpu_usage;
        self.memory_fraction = memory_fraction;
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

        // 1. Water background + animated light shafts
        self.render_background(cells);

        let mut scene = SceneBuffer {
            cells,
            occupied: &mut occupancy,
            width: self.width,
            height: self.height,
        };

        // 2. Far / Deep layer: Distant background fish silhouettes
        self.render_distant_fish(&mut scene);

        // 3. Seabed layer: Dune contours, textured ripples, kelp forest & corals
        self.render_seabed(&mut scene);

        // 4. Midground layer: Cruising sharks (predators)
        self.render_sharks(&mut scene);

        // 5. Midground schooling fish
        self.render_mid_fish(&mut scene);

        // 6. Foreground creatures (rays, tropical angelfish, pulsating jellyfish)
        self.render_creatures(&mut scene);

        // 7. Dynamic multi-glyph bubbling streams
        self.render_bubbles(&mut scene);
    }
}

pub fn seabed_height(height: u16) -> u16 {
    if height == 0 {
        return 0;
    }
    (4 + (height / 16).min(3)).min(7).min(height)
}

pub fn shark_width(size: SharkSize) -> u16 {
    match size {
        SharkSize::Shadow => 12,
        SharkSize::Medium => 16,
        SharkSize::Large => 22,
    }
}

pub fn shark_height(_size: SharkSize) -> u16 {
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

pub struct SceneBuffer<'a> {
    pub cells: &'a mut [Cell],
    pub occupied: &'a mut [bool],
    pub width: u16,
    pub height: u16,
}

pub struct SpriteStyle {
    pub color: Rgb,
    pub eye_color: Rgb,
    pub eye_row: usize,
}

impl SceneBuffer<'_> {
    pub fn put_feature(&mut self, x: i32, y: i32, ch: char, fg: Rgb) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let index = y as usize * self.width as usize + x as usize;
        let bg = self.cells[index].bg;
        self.cells[index] = make_cell(ch, fg, bg);
        self.occupied[index] = true;
    }

    pub fn put_object(&mut self, x: i32, y: i32, ch: char, fg: Rgb) -> bool {
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

    pub fn draw_sprite(
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
                } else if matches!(glyph, '▄' | '▀' | '◢' | '◣' | '▲' | '▼') {
                    mix(style.color, Rgb(228, 245, 250), 0.28)
                } else if matches!(glyph, '▓' | '▒' | '░') {
                    mix(style.color, Rgb(14, 42, 68), 0.35)
                } else if matches!(glyph, '►' | '◄' | '▶' | '◀') {
                    mix(style.color, Rgb(190, 225, 238), 0.3)
                } else {
                    style.color
                };
                visible |= self.put_object(x + column as i32, y + row_index as i32, glyph, fg);
            });
        }
        visible
    }
}

pub fn for_each_sprite_glyph(row: &str, direction: f32, mut visit: impl FnMut(usize, char)) {
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

pub fn mirror(glyph: char) -> char {
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
        '◄' => '►',
        '►' => '◄',
        '◢' => '◣',
        '◣' => '◢',
        '◤' => '◥',
        '◥' => '◤',
        '⌠' => '⌡',
        '⌡' => '⌠',
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
        assert!((4..=7).contains(&theme.seabed_height));

        let mut canvas = Canvas::new(72, 24);
        theme.update(0.1, 0.0);
        theme.render(&mut canvas);
        assert_eq!((canvas.width, canvas.height), (72, 24));
        assert_ne!(canvas.cell(0, 0).bg, canvas.cell(0, 23).bg);
        assert!(canvas
            .cells_mut()
            .iter()
            .any(|cell| { matches!(cell.ch, '◈' | '█' | '▐' | '▓' | '▌') }));

        theme.resize(45, 12);
        theme.render(&mut canvas);
        assert_eq!((canvas.width, canvas.height), (45, 12));
        assert_eq!(theme.seabed_height, 4);
        assert!(canvas
            .cells_mut()
            .iter()
            .skip(45 * 8)
            .any(|cell| matches!(cell.ch, '~' | '≈' | '∿' | '░' | '▄' | '▀')));
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
    fn output_burst_triggers_bubble_surge() {
        let mut theme = OceanTheme::new(80, 28);
        let count_before = theme.bubbles.len();
        theme.set_output_burst(true);
        assert!(theme.bubbles.len() >= count_before);
        assert!(theme.burst_active);
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
    fn shark_sprites_have_distinct_frames_and_mirror_directional_glyphs() {
        assert_ne!(LARGE_SHARK_0, LARGE_SHARK_1);
        assert_ne!(MEDIUM_SHARK_0, MEDIUM_SHARK_1);
        assert_ne!(SHADOW_SHARK_0, SHADOW_SHARK_1);
        assert!(LARGE_SHARK_0
            .iter()
            .all(|row| row.chars().count() <= shark_width(SharkSize::Large) as usize));
        assert!(MEDIUM_SHARK_0
            .iter()
            .all(|row| row.chars().count() <= shark_width(SharkSize::Medium) as usize));
        assert_eq!(mirror('/'), '\\');
        assert_eq!(mirror('<'), '>');
        assert_eq!(mirror('◀'), '▶');
        assert_eq!(mirror('◄'), '►');
        assert!(
            LARGE_SHARK_0
                .iter()
                .chain(MEDIUM_SHARK_0.iter())
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
