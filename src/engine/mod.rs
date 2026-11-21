pub mod canvas;
pub mod pty_shell;
pub mod reactivity;
pub mod renderer;

mod runner;

pub use canvas::{Canvas, Rgb};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeAction {
    CyclePalette,
    ToggleFeature(&'static str),
}

pub struct ThemeNotice {
    pub text: String,
    pub accent: Rgb,
}

pub trait Theme {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn resize(&mut self, width: u16, height: u16);
    fn update(&mut self, dt: f32, activity_intensity: f32);
    fn render(&self, canvas: &mut Canvas);

    fn refresh(&mut self, _activity_intensity: f32) {}

    fn set_system_metrics(&mut self, _cpu_usage: f32, _memory_fraction: f32) {}

    fn set_output_burst(&mut self, _active: bool) {}

    fn action(&mut self, _action: ThemeAction) -> Option<ThemeNotice> {
        None
    }
}

pub fn run(theme: Box<dyn Theme>, fps: u16) -> Result<(), Box<dyn std::error::Error>> {
    runner::run(theme, fps)
}
