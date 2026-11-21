use crate::engine::pty_shell::TerminalCell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub attrs: u8,
    pub wide_continuation: bool,
    pub(crate) base_ch: char,
    pub(crate) base_fg: Rgb,
    pub(crate) ambient_fg: Rgb,
    pub(crate) ambient_bg: Rgb,
    pub(crate) cursor_fg: Rgb,
}

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.ch == other.ch
            && self.fg == other.fg
            && self.bg == other.bg
            && self.attrs == other.attrs
            && self.wide_continuation == other.wide_continuation
    }
}

impl Eq for Cell {}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: '▀',
            fg: Rgb(18, 12, 42),
            bg: Rgb(8, 9, 22),
            attrs: 0,
            wide_continuation: false,
            base_ch: '▀',
            base_fg: Rgb(18, 12, 42),
            ambient_fg: Rgb(18, 12, 42),
            ambient_bg: Rgb(8, 9, 22),
            cursor_fg: Rgb(18, 12, 42),
        }
    }
}

pub struct Canvas {
    pub width: u16,
    pub height: u16,
    cells: Vec<Cell>,
}

impl Canvas {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width as usize * height as usize],
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.cells
            .resize(width as usize * height as usize, Cell::default());
    }

    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
    }

    pub fn cells_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }

    pub fn cell(&self, x: u16, y: u16) -> Cell {
        self.cells
            .get(y as usize * self.width as usize + x as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn composed_cell(
        &self,
        x: u16,
        y: u16,
        terminal: &TerminalCell,
        protect_cursor: bool,
        near_text: bool,
    ) -> Cell {
        let mut cell = self.cell(x, y);
        if terminal.wide_continuation {
            cell.ch = ' ';
            cell.wide_continuation = true;
            cell.attrs = 0;
        } else if terminal.ch != ' ' || terminal.bg.is_some() {
            cell.ch = terminal.ch;
            cell.attrs = terminal.attrs;
            if let Some(fg) = terminal.fg {
                cell.fg = fg;
            } else if terminal.ch != ' ' {
                cell.fg = Rgb(225, 225, 235);
            }
            if let Some(bg) = terminal.bg {
                cell.bg = bg;
            } else if terminal.ch != ' ' {
                cell.bg = scale_color(cell.bg, 0.76);
            } else if protect_cursor {
                cell.bg = scale_color(cell.bg, 0.6);
            } else if near_text {
                cell.bg = scale_color(cell.bg, 0.88);
                cell.fg = scale_color(cell.fg, 0.88);
            }
        } else if protect_cursor {
            cell.fg = scale_color(cell.cursor_fg, 0.6);
            cell.bg = scale_color(cell.ambient_bg, 0.6);
            cell.ch = cell.base_ch;
        } else {
            cell.fg = cell.ambient_fg;
            cell.bg = cell.ambient_bg;
            if near_text {
                cell.bg = scale_color(cell.bg, 0.88);
                cell.fg = scale_color(cell.fg, 0.88);
            }
        }
        cell
    }
}

fn scale_color(color: Rgb, amount: f32) -> Rgb {
    let scale = |value: u8| (value as f32 * amount).round().clamp(0.0, 255.0) as u8;
    Rgb(scale(color.0), scale(color.1), scale(color.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_terminal_cells_preserve_the_theme_layer() {
        let mut canvas = Canvas::new(1, 1);
        canvas.cells[0].ch = '✦';
        canvas.cells[0].fg = Rgb(240, 240, 255);
        canvas.cells[0].ambient_fg = Rgb(240, 240, 255);

        let composed = canvas.composed_cell(0, 0, &TerminalCell::default(), false, false);
        assert_eq!(composed.ch, '✦');
        assert_eq!(composed.fg, Rgb(240, 240, 255));
    }

    #[test]
    fn terminal_text_and_cursor_protection_compose_over_theme_cells() {
        let mut canvas = Canvas::new(1, 1);
        canvas.cells[0].ch = '✦';
        canvas.cells[0].fg = Rgb(255, 255, 255);
        canvas.cells[0].base_ch = '▀';
        canvas.cells[0].base_fg = Rgb(18, 12, 42);
        canvas.cells[0].cursor_fg = Rgb(18, 12, 42);
        canvas.cells[0].ambient_bg = Rgb(8, 9, 22);

        let text = TerminalCell {
            ch: 'A',
            ..TerminalCell::default()
        };
        assert_eq!(canvas.composed_cell(0, 0, &text, false, false).ch, 'A');

        let cursor = canvas.composed_cell(0, 0, &TerminalCell::default(), true, false);
        assert_eq!(cursor.ch, '▀');
        assert_eq!(cursor.fg, Rgb(11, 7, 25));
    }

    #[test]
    fn terminal_text_uses_unlit_theme_colors_while_empty_cells_keep_ambient_light() {
        let mut canvas = Canvas::new(1, 1);
        canvas.cells[0].bg = Rgb(80, 60, 40);
        canvas.cells[0].ambient_bg = Rgb(120, 90, 60);
        let text = TerminalCell {
            ch: 'A',
            ..TerminalCell::default()
        };
        assert_eq!(
            canvas.composed_cell(0, 0, &text, false, false).bg,
            scale_color(Rgb(80, 60, 40), 0.76)
        );
        assert_eq!(
            canvas
                .composed_cell(0, 0, &TerminalCell::default(), false, false)
                .bg,
            Rgb(120, 90, 60)
        );
    }
}
