use crate::engine::{canvas::Cell, Canvas, Rgb, Theme};

pub struct AuroraTheme {
    width: u16,
    height: u16,
    time: f32,
}

impl AuroraTheme {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            time: 0.0,
        }
    }
}

impl Theme for AuroraTheme {
    fn id(&self) -> &'static str {
        "aurora"
    }

    fn name(&self) -> &'static str {
        "Aurora"
    }

    fn description(&self) -> &'static str {
        "Slow ribbons of teal, emerald, and violet polar light."
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }

    fn update(&mut self, dt: f32, activity_intensity: f32) {
        self.time += dt * (1.0 + activity_intensity.clamp(0.0, 12.0) * 0.025);
    }

    fn render(&self, canvas: &mut Canvas) {
        canvas.resize(self.width, self.height);
        canvas.clear();
        let width = self.width as usize;
        let height = self.height as usize;
        let cells = canvas.cells_mut();
        for y in 0..height {
            let vertical = y as f32 / height.max(1) as f32;
            for x in 0..width {
                let horizontal = x as f32;
                let wave_one = (horizontal * 0.045
                    + self.time * 0.22
                    + (vertical * 8.0 + self.time * 0.12).sin() * 1.4)
                    .sin();
                let wave_two = (horizontal * 0.027 - self.time * 0.17
                    + (vertical * 6.0 - self.time * 0.09).cos() * 1.8)
                    .sin();
                let center_one = height as f32 * (0.32 + wave_one * 0.14);
                let center_two = height as f32 * (0.68 + wave_two * 0.11);
                let ribbon_one = (-((y as f32 - center_one) / 3.4).powi(2)).exp();
                let ribbon_two = (-((y as f32 - center_two) / 4.6).powi(2)).exp();
                let teal = ribbon_one * 0.68;
                let violet = ribbon_two * 0.54;
                let index = y * width + x;
                let mut ch = if teal + violet > 0.72 {
                    '▒'
                } else if teal + violet > 0.22 {
                    '░'
                } else {
                    '▀'
                };
                let bg = Rgb(
                    (4.0 + teal * 8.0 + violet * 11.0) as u8,
                    (8.0 + teal * 38.0 + violet * 12.0) as u8,
                    (24.0 + teal * 34.0 + violet * 42.0) as u8,
                );
                let mut fg = Rgb(
                    (18.0 + teal * 74.0 + violet * 92.0) as u8,
                    (28.0 + teal * 188.0 + violet * 55.0) as u8,
                    (60.0 + teal * 158.0 + violet * 178.0) as u8,
                );
                let star_hash = ((horizontal * 12.9898 + y as f32 * 78.233).sin() * 43_758.547)
                    .fract()
                    .abs();
                if star_hash > 0.997 {
                    ch = '·';
                    fg = Rgb(156, 222, 230);
                }
                let cell = Cell {
                    ch,
                    fg,
                    bg,
                    ambient_fg: fg,
                    ambient_bg: bg,
                    ..Cell::default()
                };
                cells[index] = cell;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aurora_renders_and_resizes_to_the_canvas() {
        let mut theme = AuroraTheme::new(30, 10);
        let mut canvas = Canvas::new(30, 10);
        theme.update(0.033, 2.0);
        theme.render(&mut canvas);
        assert_eq!((canvas.width, canvas.height), (30, 10));
        assert!(canvas.cells_mut().iter().any(|cell| cell.ch == '▒'));

        theme.resize(20, 8);
        theme.render(&mut canvas);
        assert_eq!((canvas.width, canvas.height), (20, 8));
    }
}
