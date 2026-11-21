use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::engine::{canvas::Cell, Canvas, Rgb, Theme};

const MAX_BUBBLES: usize = 18;
const MAX_IDLE_BUBBLES: usize = 5;
const MAX_LARGE_BUBBLES: usize = 4;
const MAX_SHARKS: usize = 7;
const MAX_FOREGROUND_CREATURES: usize = 5;
const MAX_DISTANT_FISH: usize = 40;
const SCHOOL_MIN_SIZE: usize = 12;
const SCHOOL_MAX_SIZE: usize = 20;

type Subpixel = Option<(Rgb, char)>;
type SubpixelCell = [Subpixel; 2];
type SubpixelBuffer = HashMap<(i32, i32), SubpixelCell>;

pub const RAY_A: &[&str] = &[
    "   ▄██▄    ",
    " ▄▀████▀▄  ",
    "▀████████▀ ",
    "  ▀████▀   ",
    "   ▀██▀    ",
];
pub const RAY_B: &[&str] = &[
    " ▄▀████▀▄  ",
    "▀████████▀ ",
    "  ▀████▀   ",
    "   ▀██▀    ",
    "    ▀▀     ",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SharkSize {
    Abysm,
    Deep,
    Mid,
    Foreground,
    Ultra,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SharkSpecies {
    Classic,
    Mako,
    Hammerhead,
}

#[derive(Clone, Copy)]
pub struct Shark {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub direction: f32,
    pub phase: f32,
    pub size: SharkSize,
    pub species: SharkSpecies,
    #[allow(dead_code)]
    pub depth_tint: f32,
}

#[derive(Clone, Copy)]
pub enum CreatureKind {
    Tropical,
    Ray,
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BubbleKind {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Copy)]
pub struct Bubble {
    pub kind: BubbleKind,
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub phase: f32,
    pub radius: f32,
    pub wobble_speed: f32,
    pub wobble_amp: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BedKind {
    Kelp,
    Rock,
}

#[derive(Clone, Copy)]
pub struct BedFeature {
    pub x: f32,
    pub height: u8,
    pub phase: f32,
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
    pub ultra_spawn_timer: f32,
    pub creature_spawn_timer: f32,
    pub features: Vec<BedFeature>,
    pub bubbles: Vec<Bubble>,
    pub distant_fish: Vec<DistantFish>,
    pub mid_fish: Vec<MidFish>,
    pub sharks: Vec<Shark>,
    pub creatures: Vec<ForegroundCreature>,
    occupancy: RefCell<Vec<bool>>,
    bubble_snapshot: RefCell<Vec<Cell>>,
    shark_subpixels: RefCell<SubpixelBuffer>,
    plankton: Vec<(i32, i32, char)>,
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
            ultra_spawn_timer: 24.0,
            creature_spawn_timer: 2.0,
            features: Vec::new(),
            bubbles: Vec::new(),
            distant_fish: Vec::with_capacity(MAX_DISTANT_FISH),
            mid_fish: Vec::new(),
            sharks: Vec::new(),
            creatures: Vec::new(),
            occupancy: RefCell::new(vec![false; width as usize * height as usize]),
            bubble_snapshot: RefCell::new(vec![Cell::default(); width as usize * height as usize]),
            shark_subpixels: RefCell::new(HashMap::with_capacity(2_048)),
            plankton: Vec::new(),
            rng: StdRng::from_entropy(),
        };
        theme.ultra_spawn_timer = theme.rng.gen_range(18.0..30.0);
        theme.rebuild_seabed();
        theme.rebuild_plankton();
        for _ in 0..4 {
            if let Some(bubble) = theme.new_bubble(false) {
                theme.bubbles.push(bubble);
            }
        }
        for _ in 0..2 {
            let school = theme.new_distant_fish_school();
            theme.distant_fish.extend(school);
        }
        for _ in 0..2 {
            let school = theme.new_fish_school();
            theme.mid_fish.extend(school);
        }
        if theme.can_swim() {
            let starters = [
                (SharkSize::Foreground, 0.0, 0.0),
                (SharkSize::Mid, 0.98, 1.0),
                (SharkSize::Deep, 0.0, 1.0),
                (SharkSize::Abysm, 0.28, 1.0),
                (SharkSize::Abysm, 0.45, 0.9),
            ];
            for (size, x_slot, y_slot) in starters {
                let species = theme.random_shark_species();
                let mut shark = theme.new_shark_with_profile(size, species);
                let max_x = width.saturating_sub(shark_width(size)) as f32;
                let water_height = height.saturating_sub(theme.seabed_height);
                let max_y = water_height.saturating_sub(shark_height(size)) as f32;
                shark.x = max_x * x_slot;
                shark.y = max_y * y_slot;
                theme.sharks.push(shark);
            }

            let tropical = theme.new_creature(CreatureKind::Tropical);
            let ray = theme.new_creature(CreatureKind::Ray);
            theme.creatures.push(tropical);
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
        let cluster_count = (self.width as usize / 36).clamp(1, 5);
        let spacing = self.width as f32 / cluster_count as f32;
        let water_height = self.height.saturating_sub(self.seabed_height).max(1) as u8;

        for cluster in 0..cluster_count {
            let center = (spacing * (cluster as f32 + 0.5)
                + self.rng.gen_range(-spacing * 0.16..spacing * 0.16))
            .clamp(0.0, self.width.saturating_sub(1) as f32);
            let stems = self.rng.gen_range(3..=5);
            for _ in 0..stems {
                let x = (center + self.rng.gen_range(-3.0..=3.0))
                    .clamp(0.0, self.width.saturating_sub(1) as f32);
                self.features.push(BedFeature {
                    x,
                    height: self.rng.gen_range(10..=24).min(water_height),
                    phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
                    kind: BedKind::Kelp,
                });
            }
        }

        let rocks = self.rng.gen_range(2..=3);
        for _ in 0..rocks {
            self.features.push(BedFeature {
                x: self.rng.gen_range(0.0..self.width as f32),
                height: self.rng.gen_range(2..=4),
                phase: 0.0,
                kind: BedKind::Rock,
            });
        }
    }

    fn rebuild_plankton(&mut self) {
        self.plankton.clear();
        for y in 1..self.height.saturating_sub(self.seabed_height).max(1) as i32 {
            for x in 0..self.width as i32 {
                let value = hash(x as u32, y as u32);
                if value % 113 == 0 {
                    self.plankton
                        .push((x, y, if value % 2 == 0 { '·' } else { '˙' }));
                }
            }
        }
    }

    fn new_bubble(&mut self, from_seabed: bool) -> Option<Bubble> {
        let roll = self.rng.gen_range(0..100);
        let mut kind = if self.activity < 1.0 {
            if roll < 75 {
                BubbleKind::Small
            } else {
                BubbleKind::Medium
            }
        } else if roll < 48 {
            BubbleKind::Small
        } else if roll < 91 {
            BubbleKind::Medium
        } else {
            BubbleKind::Large
        };
        let large_count = self
            .bubbles
            .iter()
            .filter(|bubble| bubble.kind == BubbleKind::Large)
            .count();
        if kind == BubbleKind::Large && large_count >= MAX_LARGE_BUBBLES {
            kind = BubbleKind::Medium;
        }

        let surface = self.height.saturating_sub(self.seabed_height) as f32;
        for attempt in 0..32 {
            let x = if from_seabed && !self.features.is_empty() && attempt < 24 {
                let feature = self.features[self.rng.gen_range(0..self.features.len())];
                feature.x + self.rng.gen_range(-1.0..1.0)
            } else {
                self.rng.gen_range(0.0..self.width.max(1) as f32)
            };
            let y = if from_seabed {
                (surface - self.rng.gen_range(0.0..1.5)).max(0.0)
            } else {
                self.rng.gen_range(0.0..surface.max(1.0))
            };
            if let Some(bubble) = self.bubble_at(kind, x, y) {
                return Some(bubble);
            }
        }
        None
    }

    fn bubble_at(&mut self, kind: BubbleKind, x: f32, y: f32) -> Option<Bubble> {
        let large_count = self
            .bubbles
            .iter()
            .filter(|bubble| bubble.kind == BubbleKind::Large)
            .count();
        if kind == BubbleKind::Large && large_count >= MAX_LARGE_BUBBLES {
            return None;
        }
        let radius = match kind {
            BubbleKind::Small => self.rng.gen_range(1.5..=2.2),
            BubbleKind::Medium => self.rng.gen_range(2.7..=3.6),
            BubbleKind::Large => self.rng.gen_range(3.7..=4.5),
        };
        let wobble_amp = self.rng.gen_range(0.3..0.8);
        let spaced = self.bubbles.iter().all(|other| {
            let dx = (x - other.x) * 0.5;
            let dy = y - other.y;
            let clearance = radius + other.radius + 0.4 + (wobble_amp + other.wobble_amp) * 0.5;
            dx * dx + dy * dy >= clearance * clearance
        });
        if !spaced {
            return None;
        }
        Some(Bubble {
            kind,
            x,
            y,
            speed: match kind {
                BubbleKind::Small => self.rng.gen_range(1.2..1.8),
                BubbleKind::Medium => self.rng.gen_range(0.8..1.3),
                BubbleKind::Large => self.rng.gen_range(0.5..0.9),
            },
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
            radius,
            wobble_speed: self.rng.gen_range(2.0..4.5),
            wobble_amp,
        })
    }

    fn bubble_limit(&self) -> usize {
        if self.burst_timer > 0.0 || self.activity >= 8.0 {
            MAX_BUBBLES
        } else if self.activity >= 1.5 {
            10
        } else {
            MAX_IDLE_BUBBLES
        }
    }

    fn emit_tail_bubble(&mut self, dt: f32, bubble_limit: usize) {
        if self.activity < 2.0
            || self.bubbles.len() >= bubble_limit
            || self.sharks.is_empty()
            || !self
                .rng
                .gen_bool((dt * self.activity * 0.12).clamp(0.0, 1.0) as f64)
        {
            return;
        }
        let shark = self.sharks[self.rng.gen_range(0..self.sharks.len())];
        let shark_width = shark_width(shark.size) as f32;
        let x = if shark.direction > 0.0 {
            shark.x
        } else {
            shark.x + shark_width - 1.0
        };
        let water_height = self.height.saturating_sub(self.seabed_height);
        let y = (shark.y + shark_height(shark.size) as f32 * 0.55)
            .clamp(0.0, water_height.saturating_sub(1) as f32);
        let x = x.clamp(0.0, self.width.saturating_sub(1) as f32);
        let kind = if self.activity >= 8.0 && self.rng.gen_bool(0.4) {
            BubbleKind::Medium
        } else {
            BubbleKind::Small
        };
        if let Some(bubble) = self.bubble_at(kind, x, y) {
            self.bubbles.push(bubble);
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
        DistantFish {
            x,
            y: self.rng.gen_range(1.0..swim_ceiling as f32),
            speed: self.rng.gen_range(1.5..3.0),
            direction,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
        }
    }

    pub fn new_distant_fish_school(&mut self) -> Vec<DistantFish> {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let start_x = self.rng.gen_range(0.0..self.width.max(1) as f32);
        let upper_water = self.height.saturating_sub(self.seabed_height).max(2);
        let center_y = self.rng.gen_range(1.0..upper_water as f32);
        let speed = self.rng.gen_range(1.5..3.0);
        let phase = self.rng.gen_range(0.0..std::f32::consts::TAU);
        let count = self.rng.gen_range(SCHOOL_MIN_SIZE..=SCHOOL_MAX_SIZE);

        (0..count)
            .map(|index| DistantFish {
                x: start_x - direction * index as f32 * 1.8,
                y: (center_y + (index as f32 - count as f32 / 2.0) * 0.12)
                    .clamp(1.0, upper_water.saturating_sub(1) as f32),
                speed,
                direction,
                phase: phase + index as f32 * 0.12,
            })
            .collect()
    }

    pub fn new_fish_school(&mut self) -> Vec<MidFish> {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let start_x = self.rng.gen_range(0.0..self.width.max(1) as f32);
        let upper_water = self.height.saturating_sub(self.seabed_height).max(2);
        let center_y = self.rng.gen_range(1.0..upper_water as f32);
        let speed = self.rng.gen_range(4.0..6.0);
        let phase = self.rng.gen_range(0.0..std::f32::consts::TAU);
        let count = self.rng.gen_range(SCHOOL_MIN_SIZE..=SCHOOL_MAX_SIZE);
        let colors = [
            Rgb(126, 171, 194),
            Rgb(146, 190, 208),
            Rgb(111, 157, 186),
            Rgb(158, 194, 210),
        ];
        let color = colors[self.rng.gen_range(0..colors.len())];
        (0..count)
            .map(|index| MidFish {
                x: start_x - direction * index as f32 * 4.5,
                y: (center_y
                    + (index as f32 - count as f32 / 2.0) * 0.32
                    + (index as f32 * 0.55 + phase).sin() * 0.25)
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
            0..=2 => SharkSize::Abysm,
            3..=4 => SharkSize::Deep,
            5..=7 => SharkSize::Mid,
            _ => SharkSize::Foreground,
        };
        let species = self.random_shark_species();
        self.new_shark_with_profile(size, species)
    }

    fn random_shark_species(&mut self) -> SharkSpecies {
        match self.rng.gen_range(0..10) {
            0..=5 => SharkSpecies::Classic,
            6..=8 => SharkSpecies::Mako,
            _ => SharkSpecies::Hammerhead,
        }
    }

    fn new_shark_with_profile(&mut self, size: SharkSize, species: SharkSpecies) -> Shark {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let width = shark_width(size) as f32;
        let swim_height = self
            .height
            .saturating_sub(self.seabed_height)
            .saturating_sub(shark_height(size));
        let speed = match size {
            SharkSize::Abysm => self.rng.gen_range(1.5..3.0),
            SharkSize::Deep => self.rng.gen_range(4.0..6.0),
            SharkSize::Mid => self.rng.gen_range(7.0..10.0),
            SharkSize::Foreground => self.rng.gen_range(11.0..15.0),
            SharkSize::Ultra => unreachable!("ultra sharks use new_ultra_shark"),
        };
        let speed = if species == SharkSpecies::Mako {
            speed * 1.3
        } else {
            speed
        };
        let depth_tint = match size {
            SharkSize::Abysm => 0.82,
            SharkSize::Deep => 0.58,
            SharkSize::Mid => 0.30,
            SharkSize::Foreground => 0.04,
            SharkSize::Ultra => 0.0,
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
            species,
            depth_tint,
        }
    }

    pub fn new_ultra_shark(&mut self) -> Shark {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let width = self.rng.gen_range(85..=110) as f32;
        let upper_water = self.height.saturating_sub(self.seabed_height).max(2);
        let y = if self.rng.gen_bool(0.5) {
            self.rng
                .gen_range(1.0..(upper_water as f32 * 0.22).max(2.0))
        } else {
            self.rng
                .gen_range((upper_water as f32 * 0.72)..upper_water as f32)
        };
        Shark {
            x: if direction > 0.0 {
                -width - self.rng.gen_range(2.0..8.0)
            } else {
                self.width as f32 + self.rng.gen_range(2.0..8.0)
            },
            y,
            speed: self.rng.gen_range(22.0..28.0),
            direction,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
            size: SharkSize::Ultra,
            species: SharkSpecies::Classic,
            depth_tint: 0.0,
        }
    }

    fn place_shark_without_overlap(&self, shark: &mut Shark) -> bool {
        let candidate_width = shark_width(shark.size) as i32;
        let candidate_height = shark_height(shark.size) as i32;
        let max_x = self.width.saturating_sub(candidate_width as u16) as i32;
        let water_height = self.height.saturating_sub(self.seabed_height);
        let max_y = water_height.saturating_sub(candidate_height as u16) as i32;

        for y in (0..=max_y).step_by(2) {
            for x in (0..=max_x).step_by(4) {
                let overlaps = self.sharks.iter().any(|other| {
                    let other_x = other.x.round() as i32;
                    let other_y = other.y.round() as i32;
                    x < other_x + shark_width(other.size) as i32
                        && x + candidate_width > other_x
                        && y < other_y + shark_height(other.size) as i32
                        && y + candidate_height > other_y
                });
                if !overlaps {
                    shark.x = x as f32;
                    shark.y = y as f32;
                    return true;
                }
            }
        }
        false
    }

    pub fn new_creature(&mut self, kind: CreatureKind) -> ForegroundCreature {
        let direction = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let upper_water = self
            .height
            .saturating_sub(self.seabed_height)
            .saturating_sub(4);
        let x = self.rng.gen_range(0.0..self.width.max(1) as f32);
        let y = self.rng.gen_range(1.0..upper_water.max(2) as f32);
        let speed = match kind {
            CreatureKind::Tropical => self.rng.gen_range(1.2..2.0),
            CreatureKind::Ray => self.rng.gen_range(0.8..1.5),
        };
        let color = match kind {
            CreatureKind::Tropical => Rgb(245, 175, 85),
            CreatureKind::Ray => Rgb(140, 185, 210),
        };
        ForegroundCreature {
            x,
            y,
            speed,
            direction,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
            color,
            kind,
        }
    }

    pub fn can_swim(&self) -> bool {
        self.width >= 30 && self.height >= 12
    }

    pub fn update_entities(&mut self, dt: f32, activity: f32) {
        let dt = dt.clamp(0.0, 0.25);
        let target_activity = activity.clamp(0.0, 12.0);
        let activity_blend = 1.0 - (-dt * 3.5).exp();
        self.activity += (target_activity - self.activity) * activity_blend;
        let warp = activity_warp(self.activity);

        // Bubbles update
        for index in 0..self.bubbles.len() {
            let mut bubble = self.bubbles[index];
            bubble.y -= bubble.speed * warp * dt;
            bubble.phase += dt * (bubble.wobble_speed + self.activity * 0.08);
            if bubble.y < 0.0 {
                if let Some(replacement) = self.new_bubble(true) {
                    self.bubbles[index] = replacement;
                } else {
                    bubble.y = 0.0;
                    self.bubbles[index] = bubble;
                }
            } else {
                self.bubbles[index] = bubble;
            }
        }

        // Bubble emissions
        let bubble_limit = self.bubble_limit();
        if self.bubbles.len() < bubble_limit {
            let burst_emission = if self.burst_timer > 0.0 { 2.0 } else { 0.0 };
            self.bubble_emission += dt * (0.025 + self.activity * 0.28 + burst_emission);
            while self.bubble_emission >= 1.0 && self.bubbles.len() < bubble_limit {
                if let Some(bubble) = self.new_bubble(true) {
                    self.bubble_emission -= 1.0;
                    self.bubbles.push(bubble);
                } else {
                    self.bubble_emission = 0.0;
                    break;
                }
            }
        } else {
            self.bubble_emission = 0.0;
        }
        self.emit_tail_bubble(dt, bubble_limit);

        if self.burst_timer > 0.0 {
            self.burst_timer -= dt;
            if self.burst_timer <= 0.0 {
                self.burst_active = false;
            }
        }

        // Distant background fish
        for index in 0..self.distant_fish.len() {
            let vx = self.distant_fish[index].speed * self.distant_fish[index].direction;
            self.distant_fish[index].x += vx * dt;
            let x = self.distant_fish[index].x;
            if (vx > 0.0 && x > self.width as f32 + 3.0) || (vx < 0.0 && x < -3.0) {
                let fish = self.new_distant_fish(false);
                self.distant_fish[index] = fish;
            }
        }

        // Midground schooling fish
        for fish in &mut self.mid_fish {
            let vx = fish.speed * fish.direction;
            fish.x += vx * dt;
            fish.phase += dt * 3.0;
            if (vx > 0.0 && fish.x > self.width as f32 + 6.0) || (vx < 0.0 && fish.x < -8.0) {
                fish.x = if vx > 0.0 {
                    -6.0
                } else {
                    self.width as f32 + 6.0
                };
                let upper = self.height.saturating_sub(self.seabed_height).max(2);
                fish.y = self.rng.gen_range(1.0..upper as f32);
            }
        }

        // Sharks keep a stable phase; tail motion is derived from scene time.
        for shark in &mut self.sharks {
            shark.x += shark.speed * shark.direction * warp * dt;
        }
        self.sharks.retain(|shark| {
            if shark.speed * shark.direction > 0.0 {
                shark.x < self.width as f32 + 8.0
            } else {
                shark.x + shark_width(shark.size) as f32 > -8.0
            }
        });
        if self.can_swim() {
            let water_height = self.height.saturating_sub(self.seabed_height);
            let visible = self
                .sharks
                .iter()
                .filter(|shark| shark_intersects_view(shark, self.width, water_height))
                .count();
            if visible < 4 {
                self.sharks
                    .retain(|shark| shark_intersects_view(shark, self.width, water_height));
                while self.sharks.len() < 4 && self.sharks.len() < MAX_SHARKS {
                    let size = match self.sharks.len() % 3 {
                        0 => SharkSize::Abysm,
                        1 => SharkSize::Deep,
                        _ => SharkSize::Mid,
                    };
                    let species = self.random_shark_species();
                    let mut shark = self.new_shark_with_profile(size, species);
                    if !self.place_shark_without_overlap(&mut shark) {
                        let max_x = self.width.saturating_sub(shark_width(size)) as f32;
                        let max_y = water_height.saturating_sub(shark_height(size)) as f32;
                        shark.x = self.rng.gen_range(0.0..=max_x);
                        shark.y = self.rng.gen_range(0.0..=max_y);
                    }
                    self.sharks.push(shark);
                }
            }
        }
        self.ultra_spawn_timer -= dt;
        if self.ultra_spawn_timer <= 0.0
            && self.sharks.len() < MAX_SHARKS
            && !self
                .sharks
                .iter()
                .any(|shark| shark.size == SharkSize::Ultra)
        {
            let ultra = self.new_ultra_shark();
            self.sharks.push(ultra);
            self.ultra_spawn_timer = self.rng.gen_range(18.0..30.0);
        }
        if self.can_swim() {
            self.shark_spawn_timer -= dt;
            let foreground_count = self
                .sharks
                .iter()
                .filter(|shark| shark.size == SharkSize::Foreground)
                .count();
            if (self.shark_spawn_timer <= 0.0 || self.sharks.len() < 4)
                && self.sharks.len() < MAX_SHARKS
            {
                let shark = self.new_shark();
                if shark.size != SharkSize::Foreground || foreground_count < 2 {
                    self.sharks.push(shark);
                }
                self.shark_spawn_timer = self.rng.gen_range(3.5..7.0);
            }
        }

        // Foreground creatures
        for creature in &mut self.creatures {
            creature.phase += dt * 2.5;
            creature.x += creature.speed * creature.direction * dt;
        }
        self.creatures
            .retain(|creature| creature.x > -14.0 && creature.x < self.width as f32 + 10.0);
        if self.can_swim() {
            self.creature_spawn_timer -= dt;
            if self.creature_spawn_timer <= 0.0 && self.creatures.len() < MAX_FOREGROUND_CREATURES {
                let kind = match self.rng.gen_range(0..2) {
                    0 => CreatureKind::Tropical,
                    _ => CreatureKind::Ray,
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
        let current_speed = 0.2 + self.cpu_usage * 0.005 + self.activity * 0.03;
        let activity_light = 1.0 + self.activity * 0.015;
        let beam_w = (self.width as f32 * 0.042).clamp(2.0, 6.0);

        for y in 0..height {
            let depth = y as f32 / height.saturating_sub(1).max(1) as f32;
            let ambient = mix(surface_water, deep_water, depth);
            for x in 0..width {
                let mut color = ambient;
                let x_f = x as f32;

                for beam in 0..caustic_origins.len() {
                    let center = self.width as f32 * caustic_origins[beam]
                        + y as f32 * caustic_slopes[beam]
                        + (self.time * current_speed + beam as f32 * 2.3).sin() * 1.8;
                    let dist = (x_f - center).abs();
                    let intensity = caustic_weight(dist / beam_w)
                        * (1.0 - depth * 0.75)
                        * 0.23
                        * activity_light;
                    color = mix(color, Rgb(75, 160, 202), intensity);
                }

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

    fn render_currents(&self, scene: &mut SceneBuffer<'_>) {
        let intensity = ((self.activity - 5.0) / 7.0).clamp(0.0, 1.0);
        if intensity <= 0.0 || scene.width == 0 || scene.height == 0 {
            return;
        }
        let water_height = scene.height.saturating_sub(self.seabed_height).max(1);
        let stream_count = ((scene.width as f32 / 7.0) * intensity).ceil() as u32;
        let drift_speed = 2.0 + self.activity * 1.2;
        let streak_length = 1 + (intensity * 2.0).round() as i32;

        for index in 0..stream_count {
            let seed = hash(index.wrapping_add(1), 0x00C0_117E);
            let start_x = (seed % scene.width as u32) as f32;
            let y = ((seed.rotate_left(11)) % water_height as u32) as i32;
            let direction = if seed & 1 == 0 { 1.0 } else { -1.0 };
            let x = (start_x + direction * self.time * drift_speed)
                .rem_euclid(scene.width as f32)
                .round() as i32;
            let color = mix(Rgb(38, 107, 145), Rgb(116, 196, 218), intensity * 0.6);
            for step in 0..streak_length {
                let trail_x = (x - direction as i32 * step).rem_euclid(scene.width as i32);
                scene.put_feature(
                    trail_x,
                    y,
                    if streak_length > 1 { '─' } else { '·' },
                    mix(color, Rgb(0, 0, 0), step as f32 * 0.24),
                );
            }
        }
    }

    pub fn render_seabed(&self, scene: &mut SceneBuffer<'_>) {
        if scene.width == 0 || scene.height == 0 {
            return;
        }
        let width = scene.width as usize;
        for x in 0..scene.width as i32 {
            let ground_y =
                (scene.height as i32 - dune_depth_at(x as f32)).clamp(0, scene.height as i32 - 1);
            for y in ground_y..scene.height as i32 {
                let upper_depth =
                    (y - ground_y) as f32 / (scene.height as i32 - ground_y).max(1) as f32;
                let lower_depth =
                    ((y - ground_y) as f32 + 0.5) / (scene.height as i32 - ground_y).max(1) as f32;
                let caustic_wave = ((x as f32 * 0.075 - self.time * 0.20).sin() * 0.5 + 0.5)
                    * (1.0 - upper_depth).powi(3)
                    * 0.68;
                let upper = mix(sand_color(upper_depth), Rgb(173, 148, 98), caustic_wave);
                let lower = sand_color(lower_depth);
                let index = y as usize * width + x as usize;
                scene.cells[index] = make_cell('▀', upper, lower);
                scene.occupied[index] = true;
            }
        }

        for rock in self
            .features
            .iter()
            .filter(|feature| feature.kind == BedKind::Rock)
        {
            self.render_rock(scene, *rock);
        }
        for kelp in self
            .features
            .iter()
            .filter(|feature| feature.kind == BedKind::Kelp)
        {
            self.render_kelp(scene, *kelp);
        }
    }

    fn render_kelp(&self, scene: &mut SceneBuffer<'_>, feature: BedFeature) {
        let height = (feature.height as i32)
            .min(scene.height.saturating_sub(3) as i32)
            .max(1);
        let root_x = feature.x.round() as i32;
        let root_y = scene.height as i32 - dune_depth_at(feature.x);
        let activity_sway = 1.0 + self.activity.min(8.0) * 0.025;

        for segment in 0..height {
            let t = segment as f32 / height.max(1) as f32;
            let sway = (self.time * 0.72 * activity_sway + feature.phase - t * 1.25).sin()
                * 3.0
                * t.powf(1.45);
            let x = root_x + sway.round() as i32;
            let y = root_y - segment;
            let color = kelp_color(t);
            let stem = match (segment + root_x) % 4 {
                0 => '│',
                1 => '┃',
                2 => '╽',
                _ => '╿',
            };
            scene.put_feature(x, y, stem, color);

            if segment == height - 1 {
                let tip_color = mix(color, Rgb(42, 140, 122), 0.45);
                scene.put_feature(x - 1, y, '~', tip_color);
                scene.put_feature(x + 1, y, '⚶', tip_color);
            }
        }
    }

    fn render_rock(&self, scene: &mut SceneBuffer<'_>, feature: BedFeature) {
        let center_x = feature.x.round() as i32;
        let bottom =
            (scene.height as i32 - dune_depth_at(feature.x) + 2).min(scene.height as i32 - 1);
        let height = feature.height.min(4) as i32;
        let half_width = 2 + height / 2;
        for row in 0..height {
            let y = bottom - height + row;
            let half_span = (half_width * (row + 1) / height.max(1)).max(1);
            for dx in -half_span..=half_span {
                let edge = dx.abs() == half_span;
                let color = if edge {
                    Rgb(20, 36, 51)
                } else {
                    Rgb(29, 51, 71)
                };
                let glyph = if row == 0 || edge { '▆' } else { '█' };
                scene.put_feature(center_x + dx, y, glyph, color);
            }
        }
        scene.put_feature(center_x, bottom, '▀', Rgb(20, 36, 51));
    }

    fn render_plankton(&self, scene: &mut SceneBuffer<'_>) {
        for &(x, y, glyph) in &self.plankton {
            let drift = ((self.time * 0.08 + y as f32 * 0.31).sin() * 1.4).round() as i32;
            let px = x + drift;
            let py = y + ((self.time * 0.12 + x as f32 * 0.21).sin() * 0.25).round() as i32;
            if px >= 0 && py >= 0 && px < scene.width as i32 && py < scene.height as i32 {
                scene.put_object(px, py, glyph, Rgb(126, 188, 194));
            }
        }
    }

    fn render_bubbles(&self, scene: &mut SceneBuffer<'_>) {
        if scene.width == 0 || scene.height == 0 {
            return;
        }
        let mut base = self.bubble_snapshot.borrow_mut();
        if base.len() != scene.cells.len() {
            base.resize(scene.cells.len(), Cell::default());
        }
        base.copy_from_slice(scene.cells);
        for bubble in &self.bubbles {
            let wobble = (self.time * bubble.wobble_speed + bubble.phase).sin() * bubble.wobble_amp;
            let bx = bubble.x + wobble;
            let by = bubble.y;
            let radius = bubble.radius;
            let min_x = (bx - (radius + 0.8) * 2.0).floor() as i32;
            let max_x = (bx + (radius + 0.8) * 2.0).ceil() as i32;
            let min_y = (by - radius - 0.8).floor() as i32;
            let max_y = (by + radius + 0.8).ceil() as i32;

            for cy in min_y..=max_y {
                for cx in min_x..=max_x {
                    if cx < 0 || cy < 0 || cx >= scene.width as i32 || cy >= scene.height as i32 {
                        continue;
                    }

                    let dx = (cx as f32 - bx) * 0.5;
                    let dy = cy as f32 - by;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let index = cy as usize * scene.width as usize + cx as usize;

                    if dist < radius - 0.32 {
                        let factor = (dist / radius).powi(2);
                        let sample_scale = 0.45 + 0.55 * factor;
                        let sample_x = bx + (cx as f32 - bx) * sample_scale;
                        let sample_y = by + (cy as f32 - by) * sample_scale;
                        let sample_x = sample_x
                            .round()
                            .clamp(0.0, scene.width.saturating_sub(1) as f32)
                            as usize;
                        let sample_y = sample_y
                            .round()
                            .clamp(0.0, scene.height.saturating_sub(1) as f32)
                            as usize;
                        let source = base[sample_y * scene.width as usize + sample_x];
                        let mut refracted = source;
                        refracted.fg = cyan_lift(source.fg);
                        refracted.bg = cyan_lift(source.bg);
                        refracted.base_fg = cyan_lift(source.base_fg);
                        refracted.ambient_fg = cyan_lift(source.ambient_fg);
                        refracted.ambient_bg = cyan_lift(source.ambient_bg);
                        refracted.cursor_fg = cyan_lift(source.cursor_fg);

                        if (dx + radius * 0.45).abs() <= 0.4 && (dy + radius * 0.45).abs() <= 0.4 {
                            refracted.ch = '⠁';
                            refracted.fg = Rgb(255, 255, 255);
                        }
                        scene.cells[index] = refracted;
                        continue;
                    }

                    if dist < radius - 0.32 || dist > radius + 0.08 {
                        continue;
                    }
                    let (mask, highlight) = braille_circle_rim(cx, cy, bx, by, radius);
                    if mask == 0 {
                        continue;
                    }
                    let source = base[index];
                    let mut rim = source;
                    rim.ch = char::from_u32(0x2800 + mask as u32).unwrap_or('\u{2800}');
                    rim.fg = if highlight {
                        Rgb(176, 218, 230)
                    } else {
                        Rgb(92, 167, 196)
                    };
                    rim.base_fg = rim.fg;
                    rim.ambient_fg = rim.fg;
                    scene.cells[index] = rim;
                }
            }
        }
    }

    fn render_distant_fish(&self, scene: &mut SceneBuffer<'_>) {
        let deep_water = Rgb(18, 48, 76);
        for (index, fish) in self.distant_fish.iter().enumerate() {
            let y = fish.y + (self.time * 1.2 + fish.phase).sin() * 0.3;
            let ix = fish.x.round() as i32;
            let iy = y.round() as i32;
            let vx = fish.speed * fish.direction;
            let synchronized = (self.time * 1.1 + index as f32 * 0.22).sin();
            if vx > 0.0 {
                scene.put_object(
                    ix - 1,
                    iy,
                    if synchronized > 0.0 { '⠒' } else { '⠤' },
                    deep_water,
                );
                scene.put_object(ix, iy, '⠔', deep_water);
            } else {
                scene.put_object(ix, iy, '⠢', deep_water);
                scene.put_object(
                    ix + 1,
                    iy,
                    if synchronized > 0.0 { '⠒' } else { '⠤' },
                    deep_water,
                );
            }
        }
    }

    fn render_mid_fish(&self, scene: &mut SceneBuffer<'_>) {
        for fish in &self.mid_fish {
            let y = fish.y + (self.time * 1.4 + fish.phase).sin() * 0.4;
            let x = fish.x.round() as i32;
            let iy = y.round() as i32;
            let vx = fish.speed * fish.direction;
            if vx > 0.0 {
                scene.put_object(x - 2, iy, '⠒', fish.color);
                scene.put_object(x - 1, iy, '▄', mix(fish.color, Rgb(220, 238, 240), 0.12));
                scene.put_object(x, iy, '▀', mix(fish.color, Rgb(220, 238, 240), 0.20));
                scene.put_object(x + 1, iy, '▷', mix(fish.color, Rgb(220, 238, 240), 0.25));
            } else {
                scene.put_object(x - 1, iy, '◁', mix(fish.color, Rgb(220, 238, 240), 0.20));
                scene.put_object(x, iy, '▀', mix(fish.color, Rgb(220, 238, 240), 0.20));
                scene.put_object(x + 1, iy, '▄', mix(fish.color, Rgb(220, 238, 240), 0.12));
                scene.put_object(x + 2, iy, '⠒', fish.color);
            }
        }
    }

    fn render_sharks_by_layer(&self, scene: &mut SceneBuffer<'_>, subpixels: &mut SubpixelBuffer) {
        let ocean_blue = Rgb(18, 48, 76);

        // 0. Abysm: 6-9 columns, 82% fog.
        for shark in self.sharks.iter().filter(|s| s.size == SharkSize::Abysm) {
            scene.draw_parametric_shark_with_scratch(
                ParametricSharkSpec {
                    x: shark.x.round() as i32,
                    y: shark.y.round() as i32,
                    width: 8,
                    terminal_height: 2,
                    direction: shark.speed * shark.direction,
                    phase: shark.phase,
                    time: self.time,
                    speed: shark.speed * activity_warp(self.activity),
                    max_swing: 2.5 * (1.0 + self.activity * 0.025),
                    species: shark.species,
                    style: shark_style(shark.species, ocean_blue, 0.82),
                },
                subpixels,
            );
        }

        // 1. Deep: 18 columns, 58% fog.
        for shark in self.sharks.iter().filter(|s| s.size == SharkSize::Deep) {
            scene.draw_parametric_shark_with_scratch(
                ParametricSharkSpec {
                    x: shark.x.round() as i32,
                    y: shark.y.round() as i32,
                    width: 18,
                    terminal_height: 6,
                    direction: shark.speed * shark.direction,
                    phase: shark.phase,
                    time: self.time,
                    speed: shark.speed * activity_warp(self.activity),
                    max_swing: 2.8 * (1.0 + self.activity * 0.025),
                    species: shark.species,
                    style: shark_style(shark.species, ocean_blue, 0.58),
                },
                subpixels,
            );
        }

        // 2. Mid: medium 30-column sharks with 30% fog.
        for shark in self.sharks.iter().filter(|s| s.size == SharkSize::Mid) {
            scene.draw_parametric_shark_with_scratch(
                ParametricSharkSpec {
                    x: shark.x.round() as i32,
                    y: shark.y.round() as i32,
                    width: 30,
                    terminal_height: 8,
                    direction: shark.speed * shark.direction,
                    phase: shark.phase,
                    time: self.time,
                    speed: shark.speed * activity_warp(self.activity),
                    max_swing: 3.0 * (1.0 + self.activity * 0.025),
                    species: shark.species,
                    style: shark_style(shark.species, ocean_blue, 0.30),
                },
                subpixels,
            );
        }

        // 3. Foreground: 48-62 columns, 0-8% fog, maximum two.
        for shark in self
            .sharks
            .iter()
            .filter(|s| s.size == SharkSize::Foreground)
        {
            scene.draw_parametric_shark_with_scratch(
                ParametricSharkSpec {
                    x: shark.x.round() as i32,
                    y: shark.y.round() as i32,
                    width: 56,
                    terminal_height: 13,
                    direction: shark.speed * shark.direction,
                    phase: shark.phase,
                    time: self.time,
                    speed: shark.speed * activity_warp(self.activity),
                    max_swing: 3.5 * (1.0 + self.activity * 0.025),
                    species: shark.species,
                    style: shark_style(shark.species, ocean_blue, 0.04),
                },
                subpixels,
            );
        }

        // 4. Ultra Close-Up: 85-110 columns, dark silhouette, 22-28 columns/second.
        for shark in self.sharks.iter().filter(|s| s.size == SharkSize::Ultra) {
            scene.draw_parametric_shark_with_scratch(
                ParametricSharkSpec {
                    x: shark.x.round() as i32,
                    y: shark.y.round() as i32,
                    width: 98,
                    terminal_height: 24,
                    direction: shark.speed * shark.direction,
                    phase: shark.phase * 1.5,
                    time: self.time,
                    speed: shark.speed * activity_warp(self.activity),
                    max_swing: 4.0 * (1.0 + self.activity * 0.025),
                    species: shark.species,
                    style: SubPixelStyle {
                        dorsal_color: Rgb(4, 16, 30),
                        side_color: Rgb(8, 28, 45),
                        belly_color: Rgb(24, 46, 62),
                        eye_color: Rgb(255, 255, 255),
                        gill_color: Rgb(2, 8, 16),
                        fin_color: Rgb(12, 38, 58),
                        fog_color: Rgb(4, 16, 30),
                        fog_amount: 0.0,
                    },
                },
                subpixels,
            );
        }
    }

    fn render_creatures(&self, scene: &mut SceneBuffer<'_>) {
        for creature in &self.creatures {
            let frame = creature.phase.sin() > 0.0;
            let y = creature.y + (self.time * 0.9 + creature.phase).sin() * 0.45;
            let x = creature.x.round() as i32;
            let iy = y.round() as i32;
            match creature.kind {
                CreatureKind::Tropical => {
                    let vx = creature.speed * creature.direction;
                    let head_color = mix(creature.color, Rgb(255, 210, 130), 0.35);
                    let tail_color = mix(creature.color, Rgb(95, 190, 210), 0.30);
                    if vx > 0.0 {
                        scene.put_object(x - 1, iy, '≋', tail_color);
                        scene.put_object(x, iy, if frame { '●' } else { '◉' }, creature.color);
                        scene.put_object(x + 1, iy, '▷', head_color);
                    } else {
                        scene.put_object(x - 1, iy, '◁', head_color);
                        scene.put_object(x, iy, if frame { '●' } else { '◉' }, creature.color);
                        scene.put_object(x + 1, iy, '≋', tail_color);
                    }
                }
                CreatureKind::Ray => {
                    let vx = creature.speed * creature.direction;
                    let sprite = if frame { RAY_B } else { RAY_A };
                    let direction = vx;
                    let (front_x, marker) = if vx > 0.0 {
                        (x + 9, '›')
                    } else {
                        (x + 2, '‹')
                    };
                    scene.put_object(
                        front_x,
                        iy + 1,
                        marker,
                        mix(creature.color, Rgb(248, 235, 190), 0.55),
                    );
                    scene.draw_sprite(
                        x,
                        iy,
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
        self.bubble_snapshot
            .get_mut()
            .resize(width as usize * height as usize, Cell::default());
        self.rebuild_seabed();
        self.rebuild_plankton();

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
        self.time += dt;
        self.update_entities(dt, activity_intensity);
    }

    fn set_output_burst(&mut self, active: bool) {
        if active && !self.burst_active {
            self.burst_timer = 1.5;
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

        // 2. Layer 1: Far / Deep distant background fish & silhouettes
        self.render_distant_fish(&mut scene);

        // 3. Seabed layer: Dune contours, textured ripples, kelp forest & corals
        self.render_seabed(&mut scene);
        self.render_plankton(&mut scene);
        self.render_currents(&mut scene);

        // 4. Layers 1-4: Sharks across 4 depth layers with atmospheric depth fog
        let mut subpixels = self.shark_subpixels.borrow_mut();
        self.render_sharks_by_layer(&mut scene, &mut subpixels);
        drop(subpixels);

        // 5. Midground schooling fish
        self.render_mid_fish(&mut scene);

        // 6. Foreground creatures (rays and tropical fish)
        self.render_creatures(&mut scene);

        // 7. Refractive circular bubbles
        self.render_bubbles(&mut scene);
    }
}

pub fn seabed_height(height: u16) -> u16 {
    if height == 0 {
        return 0;
    }
    (4 + (height / 16).min(3)).min(7).min(height)
}

fn dune_depth_at(x: f32) -> i32 {
    (5.0 + (x * 0.035).sin() * 1.35 + (x * 0.081 + 1.4).sin() * 0.65)
        .round()
        .clamp(3.0, 7.0) as i32
}

fn sand_color(depth: f32) -> Rgb {
    let depth = depth.clamp(0.0, 1.0);
    if depth < 0.58 {
        mix(Rgb(138, 115, 72), Rgb(89, 72, 43), depth / 0.58)
    } else {
        mix(Rgb(89, 72, 43), Rgb(43, 33, 18), (depth - 0.58) / 0.42)
    }
}

fn kelp_color(height_fraction: f32) -> Rgb {
    if height_fraction < 0.55 {
        mix(Rgb(19, 78, 74), Rgb(29, 111, 99), height_fraction / 0.55)
    } else {
        mix(
            Rgb(29, 111, 99),
            Rgb(42, 140, 122),
            (height_fraction - 0.55) / 0.45,
        )
    }
}

pub fn shark_width(size: SharkSize) -> u16 {
    match size {
        SharkSize::Abysm => 8,
        SharkSize::Deep => 18,
        SharkSize::Mid => 30,
        SharkSize::Foreground => 56,
        SharkSize::Ultra => 98,
    }
}

pub fn shark_height(size: SharkSize) -> u16 {
    match size {
        SharkSize::Abysm => 2,
        SharkSize::Deep => 5,
        SharkSize::Mid => 8,
        SharkSize::Foreground => 13,
        SharkSize::Ultra => 24,
    }
}

fn shark_intersects_view(shark: &Shark, width: u16, water_height: u16) -> bool {
    shark.x < width as f32
        && shark.x + shark_width(shark.size) as f32 > 0.0
        && shark.y < water_height as f32
        && shark.y + shark_height(shark.size) as f32 > 0.0
}

fn activity_warp(activity: f32) -> f32 {
    (1.0 + activity * 0.15).clamp(1.0, 2.5)
}

fn shark_style(species: SharkSpecies, fog_color: Rgb, fog_amount: f32) -> SubPixelStyle {
    let (dorsal_color, side_color, belly_color, fin_color, gill_color) = match species {
        SharkSpecies::Classic => (
            Rgb(30, 58, 82),
            Rgb(75, 126, 158),
            Rgb(184, 212, 227),
            Rgb(95, 145, 177),
            Rgb(15, 30, 45),
        ),
        SharkSpecies::Mako => (
            Rgb(27, 79, 122),
            Rgb(68, 137, 184),
            Rgb(190, 218, 234),
            Rgb(95, 159, 200),
            Rgb(18, 45, 70),
        ),
        SharkSpecies::Hammerhead => (
            Rgb(56, 83, 102),
            Rgb(107, 133, 144),
            Rgb(190, 207, 211),
            Rgb(112, 143, 151),
            Rgb(35, 54, 65),
        ),
    };

    SubPixelStyle {
        dorsal_color,
        side_color,
        belly_color,
        eye_color: Rgb(238, 248, 250),
        gill_color,
        fin_color,
        fog_color,
        fog_amount,
    }
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

fn cyan_lift(color: Rgb) -> Rgb {
    Rgb(
        color.0,
        color.1.saturating_add(10),
        color.2.saturating_add(18),
    )
}

fn braille_circle_rim(cx: i32, cy: i32, bx: f32, by: f32, radius: f32) -> (u8, bool) {
    const BRAILLE_BITS: [[u8; 4]; 2] = [[0, 1, 2, 6], [3, 4, 5, 7]];
    let mut mask = 0_u8;
    let mut highlight = false;
    for (dot_x, column) in BRAILLE_BITS.iter().enumerate() {
        for (dot_y, bit) in column.iter().enumerate() {
            let dx = (cx as f32 - bx) * 0.5 + (dot_x as f32 - 0.5) * 0.25;
            let dy = cy as f32 - by + (dot_y as f32 - 1.5) * 0.25;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist >= radius - 0.32 && dist <= radius + 0.08 {
                mask |= 1_u8 << *bit;
                highlight |= dx < -0.15 && dy < -0.15;
            }
        }
    }
    (mask, highlight)
}

fn add_subpixel(
    buffer: &mut SubpixelBuffer,
    world_x: i32,
    origin_y: i32,
    sub_y: i32,
    height_sub: i32,
    color: Rgb,
) {
    let absolute_sub_y = origin_y * 2 + sub_y;
    if world_x < 0 || absolute_sub_y < 0 {
        return;
    }
    let target_y = absolute_sub_y.div_euclid(2);
    let half = absolute_sub_y.rem_euclid(2);
    let entry = buffer.entry((world_x, target_y)).or_insert([None, None]);
    entry[half as usize] = Some((color, if half == 0 { '▀' } else { '▄' }));
    let _ = height_sub;
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

#[derive(Clone, Copy)]
pub struct SubPixelStyle {
    pub dorsal_color: Rgb,
    pub side_color: Rgb,
    pub belly_color: Rgb,
    pub eye_color: Rgb,
    pub gill_color: Rgb,
    pub fin_color: Rgb,
    pub fog_color: Rgb,
    pub fog_amount: f32,
}

#[derive(Clone, Copy)]
pub struct ParametricSharkSpec {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub terminal_height: i32,
    pub direction: f32,
    pub phase: f32,
    pub time: f32,
    pub speed: f32,
    pub max_swing: f32,
    pub species: SharkSpecies,
    pub style: SubPixelStyle,
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

    #[cfg(test)]
    pub fn draw_parametric_shark(&mut self, spec: ParametricSharkSpec) -> bool {
        let mut subpixels = HashMap::with_capacity(2_048);
        self.draw_parametric_shark_with_scratch(spec, &mut subpixels)
    }

    fn draw_parametric_shark_with_scratch(
        &mut self,
        spec: ParametricSharkSpec,
        subpixels: &mut SubpixelBuffer,
    ) -> bool {
        let width = spec.width.max(8);
        let height_sub = (spec.terminal_height.max(3) * 2).max(6);
        subpixels.clear();
        let mut visible = false;
        let global_sway = (spec.time * 2.0).sin() * 0.4;
        let tail_cycle = (spec.time * spec.speed * 0.65 + spec.phase).sin();
        let pectoral_cycle =
            (spec.time * spec.speed * 0.65 + spec.phase + std::f32::consts::PI).sin();

        for world_sx in 0..width {
            let horizontal_t = world_sx as f32 / (width - 1).max(1) as f32;
            let source_t = if spec.direction > 0.0 {
                1.0 - horizontal_t
            } else {
                horizontal_t
            };
            let source_t = source_t.clamp(0.0, 1.0);

            let tail_flex = if source_t >= 0.58 {
                ((source_t - 0.58) / 0.42).clamp(0.0, 1.0).powf(1.6) * tail_cycle * spec.max_swing
            } else {
                0.0
            };
            let axis = height_sub as f32 * 0.48 + global_sway + tail_flex;

            let mut body_radius = if source_t < 0.22 {
                let p = source_t / 0.22;
                0.025 + p * 0.165
            } else if source_t < 0.78 {
                let p = (source_t - 0.22) / 0.56;
                0.190 - p * 0.105
            } else {
                let p = (source_t - 0.78) / 0.22;
                0.085 - p * 0.050
            };
            if spec.species == SharkSpecies::Mako {
                body_radius *= 0.74;
            }
            let radius_sub = body_radius * height_sub as f32;
            let top = axis - radius_sub;
            let bottom = axis + radius_sub;

            if source_t <= 0.78 {
                let from_top = top.round() as i32;
                let to_bottom = bottom.round() as i32;
                for sub_y in from_top..=to_bottom {
                    let lateral = (sub_y as f32 - top) / (bottom - top).max(1.0);
                    let base_color = if lateral < 0.58 {
                        mix(
                            spec.style.dorsal_color,
                            spec.style.side_color,
                            lateral / 0.58,
                        )
                    } else {
                        mix(
                            spec.style.side_color,
                            spec.style.belly_color,
                            (lateral - 0.58) / 0.42,
                        )
                    };
                    let color = mix(base_color, spec.style.fog_color, spec.style.fog_amount);
                    add_subpixel(
                        subpixels,
                        spec.x + world_sx,
                        spec.y,
                        sub_y,
                        height_sub,
                        color,
                    );
                }
            }

            if spec.species == SharkSpecies::Hammerhead && (0.04..=0.14).contains(&source_t) {
                let p = (source_t - 0.04) / 0.10;
                let half_width =
                    (0.05 + (p * std::f32::consts::PI).sin() * 0.11) * height_sub as f32;
                for sub_y in (axis - half_width).round() as i32..=(axis + half_width).round() as i32
                {
                    add_subpixel(
                        subpixels,
                        spec.x + world_sx,
                        spec.y,
                        sub_y,
                        height_sub,
                        mix(
                            spec.style.side_color,
                            spec.style.fog_color,
                            spec.style.fog_amount,
                        ),
                    );
                }
            }

            // Curved dorsal fin rising behind the head and toward the tail.
            if (0.34..=0.50).contains(&source_t) {
                let center = (source_t - 0.34) / 0.16;
                let fin_height = (1.0 - (center - 0.54).abs() * 1.45).clamp(0.0, 1.0);
                let dorsal_scale = if spec.species == SharkSpecies::Hammerhead {
                    0.36
                } else {
                    0.27
                };
                let fin_top = top - fin_height * height_sub as f32 * dorsal_scale;
                for sub_y in fin_top.round() as i32..=top.round() as i32 {
                    let color = mix(spec.style.dorsal_color, spec.style.fin_color, 0.30);
                    add_subpixel(
                        subpixels,
                        spec.x + world_sx,
                        spec.y,
                        sub_y,
                        height_sub,
                        mix(color, spec.style.fog_color, spec.style.fog_amount),
                    );
                }
            }

            // Long pectoral fin sweeps down and back.
            let (pectoral_start, pectoral_end, pectoral_length) =
                if spec.species == SharkSpecies::Mako {
                    (0.20, 0.47, 0.31)
                } else {
                    (0.25, 0.44, 0.10)
                };
            if (pectoral_start..=pectoral_end).contains(&source_t) {
                let p = (source_t - pectoral_start) / (pectoral_end - pectoral_start);
                let fin_bottom = (bottom
                    + (0.12 + p * pectoral_length) * height_sub as f32
                    + pectoral_cycle * 0.55)
                    .max(bottom);
                for sub_y in bottom.round() as i32..=fin_bottom.round() as i32 {
                    let color = mix(spec.style.fin_color, spec.style.belly_color, 0.25);
                    add_subpixel(
                        subpixels,
                        spec.x + world_sx,
                        spec.y,
                        sub_y,
                        height_sub,
                        mix(color, spec.style.fog_color, spec.style.fog_amount),
                    );
                }
            }

            // Heterocercal tail: a taller upper lobe and shorter lower lobe.
            if source_t >= 0.78 {
                let p = ((source_t - 0.78) / 0.22).clamp(0.0, 1.0);
                let flare = (p * std::f32::consts::PI).sin().max(0.0);
                let upper_center = axis - p * height_sub as f32 * 0.18;
                let lower_center = axis + p * height_sub as f32 * 0.10;
                let upper_half = (0.015 + flare * 0.095) * height_sub as f32;
                let lower_half = (0.012 + flare * 0.050) * height_sub as f32;
                let min_y = (upper_center - upper_half)
                    .min(lower_center - lower_half)
                    .round() as i32;
                let max_y = (upper_center + upper_half)
                    .max(lower_center + lower_half)
                    .round() as i32;
                for sub_y in min_y..=max_y {
                    let upper_lobe = (sub_y as f32 - upper_center).abs() <= upper_half;
                    let lower_lobe = (sub_y as f32 - lower_center).abs() <= lower_half;
                    if !upper_lobe && !lower_lobe {
                        continue;
                    }
                    let color = if upper_lobe {
                        spec.style.dorsal_color
                    } else {
                        spec.style.side_color
                    };
                    add_subpixel(
                        subpixels,
                        spec.x + world_sx,
                        spec.y,
                        sub_y,
                        height_sub,
                        mix(color, spec.style.fog_color, spec.style.fog_amount),
                    );
                }
            }

            // Three dark gill slits behind the conical snout.
            if (0.205..=0.265).contains(&source_t) {
                let slit = ((source_t - 0.205) / 0.060 * 3.0).floor() as i32;
                if (0..3).contains(&slit) {
                    let slit_top = axis - radius_sub * 0.55;
                    let slit_bottom = axis + radius_sub * 0.35;
                    for sub_y in slit_top.round() as i32..=slit_bottom.round() as i32 {
                        add_subpixel(
                            subpixels,
                            spec.x + world_sx,
                            spec.y,
                            sub_y,
                            height_sub,
                            mix(
                                spec.style.gill_color,
                                spec.style.fog_color,
                                spec.style.fog_amount,
                            ),
                        );
                    }
                }
            }

            // A single discrete eye on the face.
            if (0.105..=0.125).contains(&source_t) {
                let eye_y = (axis - radius_sub * 0.42).round() as i32;
                add_subpixel(
                    subpixels,
                    spec.x + world_sx,
                    spec.y,
                    eye_y,
                    height_sub,
                    spec.style.eye_color,
                );
            }
        }

        for (&(cx, cy), &[upper, lower]) in subpixels.iter() {
            if cx < 0 || cy < 0 || cx >= self.width as i32 || cy >= self.height as i32 {
                continue;
            }
            let index = cy as usize * self.width as usize + cx as usize;
            if index >= self.cells.len() || self.occupied[index] {
                continue;
            }
            let bg_color = self.cells[index].bg;
            match (upper, lower) {
                (Some((upper, _)), Some((lower, _))) => {
                    self.cells[index] = make_cell('▀', upper, lower);
                    self.occupied[index] = true;
                    visible = true;
                }
                (Some((upper, _)), None) => {
                    self.cells[index] = make_cell('▀', upper, bg_color);
                    self.occupied[index] = true;
                    visible = true;
                }
                (None, Some((lower, _))) => {
                    self.cells[index] = make_cell('▄', lower, bg_color);
                    self.occupied[index] = true;
                    visible = true;
                }
                (None, None) => {}
            }
        }

        visible
    }
}

fn caustic_weight(normalized_distance: f32) -> f32 {
    const STEPS_PER_UNIT: f32 = 64.0;
    const MAX_DISTANCE: f32 = 4.0;
    static GAUSSIAN_LUT: OnceLock<[f32; 257]> = OnceLock::new();
    if normalized_distance >= MAX_DISTANCE {
        return 0.0;
    }
    let lookup = GAUSSIAN_LUT.get_or_init(|| {
        std::array::from_fn(|index| {
            let distance = index as f32 / STEPS_PER_UNIT;
            (-0.5 * distance * distance).exp()
        })
    });
    let index = (normalized_distance * STEPS_PER_UNIT).round() as usize;
    lookup[index.min(lookup.len() - 1)]
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
        for _ in 0..12 {
            theme.update(0.25, 12.0);
        }
        assert!(theme.bubbles.len() > before);
        for _ in 0..40 {
            theme.update(0.25, 12.0);
        }
        assert!(theme.bubbles.len() <= MAX_BUBBLES);
        assert!(theme.sharks.len() <= MAX_SHARKS);
        let water_height = theme.height.saturating_sub(theme.seabed_height);
        let visible_sharks = theme
            .sharks
            .iter()
            .filter(|shark| shark_intersects_view(shark, theme.width, water_height))
            .count();
        assert!((4..=7).contains(&visible_sharks));
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
    fn bubble_sizes_respect_the_idle_and_active_radius_limits() {
        let mut theme = OceanTheme::new(100, 40);
        assert!(theme.bubbles.len() <= MAX_IDLE_BUBBLES);
        assert!(theme.bubbles.iter().all(|bubble| bubble.radius <= 4.5));

        theme.bubbles.clear();
        for (kind, range) in [
            (BubbleKind::Small, 1.5..=2.2),
            (BubbleKind::Medium, 2.7..=3.6),
            (BubbleKind::Large, 3.7..=4.5),
        ] {
            for _ in 0..30 {
                let bubble = theme.bubble_at(kind, 50.0, 20.0).unwrap();
                assert!(range.contains(&bubble.radius));
            }
        }
    }

    #[test]
    fn bubble_spawns_keep_clearance_and_limit_large_bubbles() {
        let mut theme = OceanTheme::new(120, 50);
        theme.bubbles.clear();
        theme.activity = 12.0;
        for _ in 0..20 {
            let Some(bubble) = theme.new_bubble(true) else {
                break;
            };
            for other in &theme.bubbles {
                let dx = (bubble.x - other.x) * 0.5;
                let dy = bubble.y - other.y;
                let clearance = bubble.radius
                    + other.radius
                    + 0.4
                    + (bubble.wobble_amp + other.wobble_amp) * 0.5;
                assert!(dx * dx + dy * dy >= clearance * clearance);
            }
            theme.bubbles.push(bubble);
        }
        assert!(
            theme
                .bubbles
                .iter()
                .filter(|bubble| bubble.kind == BubbleKind::Large)
                .count()
                <= MAX_LARGE_BUBBLES
        );
        assert!(theme.bubbles.iter().all(|bubble| bubble.radius <= 4.5));
    }

    #[test]
    fn activity_uses_space_warp_curve_and_fades_back_to_idle() {
        assert_eq!(activity_warp(0.0), 1.0);
        assert_eq!(activity_warp(8.0), 2.2);
        assert_eq!(activity_warp(12.0), 2.5);

        let mut theme = OceanTheme::new(80, 28);
        theme.update_entities(0.25, 12.0);
        assert!(theme.activity > 0.0);
        let peak = theme.activity;
        for _ in 0..20 {
            theme.update_entities(0.25, 0.0);
        }
        assert!(theme.activity < peak * 0.01);
    }

    #[test]
    fn bubble_lens_samples_and_tints_the_existing_canvas_without_solid_interior() {
        let mut theme = OceanTheme::new(24, 14);
        theme.bubbles = vec![Bubble {
            kind: BubbleKind::Medium,
            x: 8.0,
            y: 6.0,
            speed: 0.0,
            phase: 0.0,
            radius: 3.5,
            wobble_speed: 0.0,
            wobble_amp: 0.0,
        }];
        let mut cells = (0..24 * 14)
            .map(|index| {
                let x = (index % 24) as u8;
                make_cell(char::from(b'A' + x % 26), Rgb(20, 30, 40), Rgb(50, 60, 70))
            })
            .collect::<Vec<_>>();
        let mut occupied = vec![false; cells.len()];
        let mut scene = SceneBuffer {
            cells: &mut cells,
            occupied: &mut occupied,
            width: 24,
            height: 14,
        };
        theme.render_bubbles(&mut scene);

        let refracted = scene.cells[6 * 24 + 11];
        assert_eq!(refracted.ch, 'K');
        assert_eq!(refracted.fg, Rgb(20, 40, 58));
        assert_eq!(refracted.bg, Rgb(50, 70, 88));

        for cy in 0..14_i32 {
            for cx in 0..24_i32 {
                let dx = (cx as f32 - 8.0) * 0.5;
                let dy = cy as f32 - 6.0;
                let dist = (dx * dx + dy * dy).sqrt();
                if (3.5 - 0.32..=3.5 + 0.08).contains(&dist) {
                    let cell = scene.cells[cy as usize * 24 + cx as usize];
                    if cell.ch != char::from(b'A' + cx as u8 % 26) {
                        assert!((0x2800..=0x28ff).contains(&(cell.ch as u32)));
                    }
                    assert!(!matches!(cell.ch, '(' | ')' | 'L' | 'o'));
                }
            }
        }
    }

    #[test]
    fn five_depth_shark_scales_and_ultra_timer_are_bounded() {
        assert_eq!(shark_width(SharkSize::Abysm), 8);
        assert_eq!(shark_width(SharkSize::Deep), 18);
        assert_eq!(shark_width(SharkSize::Mid), 30);
        assert_eq!(shark_width(SharkSize::Foreground), 56);
        assert_eq!(shark_width(SharkSize::Ultra), 98);

        let mut theme = OceanTheme::new(120, 40);
        assert_eq!(theme.sharks.len(), 5);
        let water_height = theme.height.saturating_sub(theme.seabed_height);
        assert_eq!(
            theme
                .sharks
                .iter()
                .filter(|shark| shark_intersects_view(shark, theme.width, water_height))
                .count(),
            5
        );
        assert!((18.0..30.0).contains(&theme.ultra_spawn_timer));
        theme.update_entities(0.25, 12.0);
        assert!(theme.sharks.len() <= MAX_SHARKS);
        assert!(
            theme
                .sharks
                .iter()
                .filter(|shark| shark.size == SharkSize::Foreground)
                .count()
                <= 2
        );
    }

    #[test]
    fn shark_species_have_distinct_shapes_and_mako_speed() {
        let mut theme = OceanTheme::new(80, 24);
        let mako = theme.new_shark_with_profile(SharkSize::Foreground, SharkSpecies::Mako);
        assert!((14.3..19.5).contains(&mako.speed));

        let render = |species| {
            let mut cells = vec![Cell::default(); 64 * 24];
            let mut occupied = vec![false; cells.len()];
            SceneBuffer {
                cells: &mut cells,
                occupied: &mut occupied,
                width: 64,
                height: 24,
            }
            .draw_parametric_shark(ParametricSharkSpec {
                x: 8,
                y: 3,
                width: 42,
                terminal_height: 11,
                direction: 1.0,
                phase: 0.4,
                time: 1.0,
                speed: 13.0,
                max_swing: 3.5,
                species,
                style: shark_style(species, Rgb(18, 48, 76), 0.04),
            });
            cells
        };
        let classic = render(SharkSpecies::Classic);
        assert_ne!(classic, render(SharkSpecies::Mako));
        assert_ne!(classic, render(SharkSpecies::Hammerhead));
    }

    #[test]
    fn fish_schools_have_requested_population_and_stay_bounded() {
        let mut theme = OceanTheme::new(100, 32);
        assert!((24..=40).contains(&theme.distant_fish.len()));
        assert!((24..=40).contains(&theme.mid_fish.len()));
        assert!(
            (SCHOOL_MIN_SIZE..=SCHOOL_MAX_SIZE).contains(&theme.new_distant_fish_school().len())
        );
        assert!((SCHOOL_MIN_SIZE..=SCHOOL_MAX_SIZE).contains(&theme.new_fish_school().len()));
        for _ in 0..50 {
            theme.update(0.1, 0.0);
        }
        assert!(theme.distant_fish.len() <= MAX_DISTANT_FISH);
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
    fn fish_and_tropical_creatures_face_their_velocity() {
        let mut theme = OceanTheme::new(40, 20);
        theme.mid_fish = vec![MidFish {
            x: 20.0,
            y: 8.0,
            speed: 5.0,
            direction: 1.0,
            phase: 0.0,
            color: Rgb(100, 160, 190),
        }];
        let mut cells = vec![Cell::default(); 40 * 20];
        let mut occupied = vec![false; cells.len()];
        let mut scene = SceneBuffer {
            cells: &mut cells,
            occupied: &mut occupied,
            width: 40,
            height: 20,
        };
        theme.render_mid_fish(&mut scene);
        assert_eq!(scene.cells[8 * 40 + 21].ch, '▷');

        theme.mid_fish[0].direction = -1.0;
        scene.cells.fill(Cell::default());
        scene.occupied.fill(false);
        theme.render_mid_fish(&mut scene);
        assert_eq!(scene.cells[8 * 40 + 19].ch, '◁');

        theme.distant_fish = vec![DistantFish {
            x: 20.0,
            y: 6.0,
            speed: 3.0,
            direction: 1.0,
            phase: 0.0,
        }];
        scene.cells.fill(Cell::default());
        scene.occupied.fill(false);
        theme.render_distant_fish(&mut scene);
        assert_eq!(scene.cells[6 * 40 + 20].ch, '⠔');
        theme.distant_fish[0].direction = -1.0;
        scene.cells.fill(Cell::default());
        scene.occupied.fill(false);
        theme.render_distant_fish(&mut scene);
        assert_eq!(scene.cells[6 * 40 + 20].ch, '⠢');

        theme.creatures = vec![ForegroundCreature {
            x: 20.0,
            y: 12.0,
            speed: 2.0,
            direction: 1.0,
            phase: 0.0,
            color: Rgb(220, 150, 70),
            kind: CreatureKind::Tropical,
        }];
        scene.cells.fill(Cell::default());
        scene.occupied.fill(false);
        theme.render_creatures(&mut scene);
        assert_eq!(scene.cells[12 * 40 + 21].ch, '▷');

        theme.creatures[0].direction = -1.0;
        scene.cells.fill(Cell::default());
        scene.occupied.fill(false);
        theme.render_creatures(&mut scene);
        assert_eq!(scene.cells[12 * 40 + 19].ch, '◁');

        theme.creatures[0].kind = CreatureKind::Ray;
        theme.creatures[0].direction = 1.0;
        theme.creatures[0].y = 5.0;
        scene.cells.fill(Cell::default());
        scene.occupied.fill(false);
        theme.render_creatures(&mut scene);
        assert_eq!(scene.cells[6 * 40 + 29].ch, '›');
        theme.creatures[0].direction = -1.0;
        scene.cells.fill(Cell::default());
        scene.occupied.fill(false);
        theme.render_creatures(&mut scene);
        assert_eq!(scene.cells[6 * 40 + 22].ch, '‹');
    }

    #[test]
    fn parametric_shark_is_solid_and_mirrors_cleanly() {
        let mut right_cells = vec![Cell::default(); 60 * 20];
        let mut right_occupied = vec![false; 60 * 20];
        let mut right_scene = SceneBuffer {
            cells: &mut right_cells,
            occupied: &mut right_occupied,
            width: 60,
            height: 20,
        };
        let style = SubPixelStyle {
            dorsal_color: Rgb(30, 52, 78),
            side_color: Rgb(72, 122, 162),
            belly_color: Rgb(205, 225, 238),
            eye_color: Rgb(245, 252, 255),
            gill_color: Rgb(15, 28, 45),
            fin_color: Rgb(90, 142, 182),
            fog_color: Rgb(18, 48, 76),
            fog_amount: 0.05,
        };
        assert!(right_scene.draw_parametric_shark(ParametricSharkSpec {
            x: 8,
            y: 5,
            width: 42,
            terminal_height: 11,
            direction: 1.0,
            phase: 1.4,
            time: 3.0,
            speed: 13.0,
            max_swing: 3.5,
            species: SharkSpecies::Classic,
            style,
        }));

        let mut left_cells = vec![Cell::default(); 60 * 20];
        let mut left_occupied = vec![false; 60 * 20];
        let mut left_scene = SceneBuffer {
            cells: &mut left_cells,
            occupied: &mut left_occupied,
            width: 60,
            height: 20,
        };
        assert!(left_scene.draw_parametric_shark(ParametricSharkSpec {
            x: 8,
            y: 5,
            width: 42,
            terminal_height: 11,
            direction: -1.0,
            phase: 1.4,
            time: 3.0,
            speed: 13.0,
            max_swing: 3.5,
            species: SharkSpecies::Classic,
            style,
        }));

        assert!(right_occupied.iter().filter(|occupied| **occupied).count() > 100);
        assert!(left_occupied.iter().filter(|occupied| **occupied).count() > 100);
        let right_eye_x = right_cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.fg == style.eye_color || cell.bg == style.eye_color)
            .map(|(index, _)| index % 60)
            .max()
            .unwrap();
        let left_eye_x = left_cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.fg == style.eye_color || cell.bg == style.eye_color)
            .map(|(index, _)| index % 60)
            .min()
            .unwrap();
        assert!(right_eye_x > 8 + 42 / 2);
        assert!(left_eye_x < 8 + 42 / 2);
    }

    #[test]
    fn shark_tail_moves_while_head_and_torso_remain_stable() {
        let style = SubPixelStyle {
            dorsal_color: Rgb(30, 52, 78),
            side_color: Rgb(72, 122, 162),
            belly_color: Rgb(205, 225, 238),
            eye_color: Rgb(245, 252, 255),
            gill_color: Rgb(15, 28, 45),
            fin_color: Rgb(90, 142, 182),
            fog_color: Rgb(18, 48, 76),
            fog_amount: 0.05,
        };
        let render = |time| {
            let mut cells = vec![Cell::default(); 60 * 20];
            let mut occupied = vec![false; cells.len()];
            SceneBuffer {
                cells: &mut cells,
                occupied: &mut occupied,
                width: 60,
                height: 20,
            }
            .draw_parametric_shark(ParametricSharkSpec {
                x: 8,
                y: 5,
                width: 42,
                terminal_height: 11,
                direction: 1.0,
                phase: 0.0,
                time,
                speed: 13.0,
                max_swing: 3.5,
                species: SharkSpecies::Classic,
                style,
            });
            cells
        };
        let still_tail = render(0.0);
        let swung_tail = render(std::f32::consts::PI);
        for y in 0..20 {
            for x in (8 + 18)..=(8 + 22) {
                let index = y * 60 + x;
                assert_eq!(still_tail[index], swung_tail[index]);
            }
            for x in (8 + 32)..=(8 + 41) {
                let index = y * 60 + x;
                assert_eq!(still_tail[index], swung_tail[index]);
            }
        }
        assert!((8..=8 + 9)
            .any(|x| { (0..20).any(|y| still_tail[y * 60 + x] != swung_tail[y * 60 + x]) }));
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
    fn clean_sand_uses_only_subpixel_fill_and_dunes_stay_three_to_seven_rows() {
        let mut theme = OceanTheme::new(120, 40);
        theme.features.clear();
        let mut cells = vec![Cell::default(); 120 * 40];
        let mut occupied = vec![false; cells.len()];
        let mut scene = SceneBuffer {
            cells: &mut cells,
            occupied: &mut occupied,
            width: 120,
            height: 40,
        };
        theme.render_seabed(&mut scene);

        for x in 0..120 {
            let depth = dune_depth_at(x as f32);
            assert!((3..=7).contains(&depth));
            let ground_y = 40 - depth;
            for y in ground_y..40 {
                let cell = scene.cells[y as usize * 120 + x as usize];
                assert_eq!(cell.ch, '▀');
            }
        }
        assert_eq!(sand_color(0.0), Rgb(138, 115, 72));
        assert_eq!(sand_color(1.0), Rgb(43, 33, 18));
    }

    #[test]
    fn seabed_features_form_kelp_clusters_and_include_rocks() {
        let theme = OceanTheme::new(160, 40);
        let kelp = theme
            .features
            .iter()
            .filter(|feature| feature.kind == BedKind::Kelp)
            .collect::<Vec<_>>();
        let rocks = theme
            .features
            .iter()
            .filter(|feature| feature.kind == BedKind::Rock)
            .count();

        assert!(kelp.len() >= 12);
        assert!(kelp
            .iter()
            .all(|feature| (10..=24).contains(&feature.height)));
        assert!((2..=3).contains(&rocks));
    }

    #[test]
    fn kelp_roots_stay_fixed_while_the_tips_sway() {
        let mut theme = OceanTheme::new(80, 40);
        let feature = BedFeature {
            x: 40.0,
            height: 20,
            phase: 0.0,
            kind: BedKind::Kelp,
        };
        let root_y = 40 - dune_depth_at(feature.x);

        let mut first_cells = vec![Cell::default(); 80 * 40];
        let mut first_occupied = vec![false; first_cells.len()];
        let mut first_scene = SceneBuffer {
            cells: &mut first_cells,
            occupied: &mut first_occupied,
            width: 80,
            height: 40,
        };
        theme.time = 0.0;
        theme.render_kelp(&mut first_scene, feature);

        let mut later_cells = vec![Cell::default(); 80 * 40];
        let mut later_occupied = vec![false; later_cells.len()];
        let mut later_scene = SceneBuffer {
            cells: &mut later_cells,
            occupied: &mut later_occupied,
            width: 80,
            height: 40,
        };
        theme.time = 2.0;
        theme.render_kelp(&mut later_scene, feature);

        let root_index = root_y as usize * 80 + 40;
        assert!(first_occupied[root_index]);
        assert!(later_occupied[root_index]);
        assert_eq!(first_cells[root_index].fg, Rgb(19, 78, 74));
        assert_ne!(
            first_cells.iter().map(|cell| cell.ch).collect::<Vec<_>>(),
            later_cells.iter().map(|cell| cell.ch).collect::<Vec<_>>()
        );
    }

    #[test]
    fn creature_spawns_use_rays_and_tropical_fish_only() {
        let mut theme = OceanTheme::new(80, 24);
        let ray = theme.new_creature(CreatureKind::Ray);
        let tropical = theme.new_creature(CreatureKind::Tropical);
        assert!((0.0..80.0).contains(&ray.x));
        assert!((0.0..80.0).contains(&tropical.x));
        assert!(matches!(ray.kind, CreatureKind::Ray));
        assert!(matches!(tropical.kind, CreatureKind::Tropical));
    }

    #[test]
    fn caustic_lookup_stays_close_to_the_gaussian_curve() {
        for step in 0..=256 {
            let distance = step as f32 / 64.0;
            let expected = (-0.5 * distance * distance).exp();
            assert!((caustic_weight(distance) - expected).abs() < 0.006);
        }
    }
}
