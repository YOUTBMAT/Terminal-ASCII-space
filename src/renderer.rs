use std::io::{self, BufWriter, Stdout, Write};

use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor},
};
use vt100::CursorStyle as TerminalCursorStyle;

use crate::{
    pty_shell::{
        Rgb, TerminalGrid, CELL_ATTR_BOLD, CELL_ATTR_DIM, CELL_ATTR_INVERSE, CELL_ATTR_ITALIC,
        CELL_ATTR_UNDERLINE,
    },
    space_engine::{Cell, SpaceEngine},
};

pub struct Renderer {
    width: u16,
    height: u16,
    previous: Vec<Cell>,
    row_cells: Vec<Cell>,
    text_shadow: Vec<bool>,
    notice_chars: Vec<char>,
    output_attrs: Option<u8>,
    output: BufWriter<Stdout>,
    force: bool,
}

impl Renderer {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            previous: vec![Cell::default(); width as usize * height as usize],
            row_cells: Vec::with_capacity(width as usize),
            text_shadow: vec![false; width as usize * height as usize],
            notice_chars: Vec::with_capacity(32),
            output_attrs: None,
            output: BufWriter::with_capacity(128 * 1024, io::stdout()),
            force: true,
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.previous = vec![Cell::default(); width as usize * height as usize];
        self.row_cells = Vec::with_capacity(width as usize);
        self.text_shadow
            .resize(width as usize * height as usize, false);
        self.force = true;
    }

    pub fn force_redraw(&mut self) {
        self.force = true;
    }

    pub fn draw(
        &mut self,
        engine: &SpaceEngine,
        terminal: &TerminalGrid,
        animate: bool,
        notice: Option<(&str, Rgb)>,
    ) -> io::Result<()> {
        let cursor = terminal.cursor;
        self.update_text_shadow(terminal);
        self.notice_chars.clear();
        if let Some((text, _)) = notice {
            self.notice_chars.extend([' ', ' ', '●', ' ']);
            self.notice_chars.extend(text.chars());
            self.notice_chars.extend([' ', ' ']);
        }
        let notice_chars = &self.notice_chars;
        let text_shadow = &self.text_shadow;
        let notice_len = notice_chars.len();
        let stdout = &mut self.output;
        let row_cells = &mut self.row_cells;
        let notice_position = if notice_len == 0 {
            None
        } else {
            find_notice_position(self.width, self.height, terminal, cursor, notice_len)
        };
        for y in 0..self.height {
            row_cells.clear();
            for x in 0..self.width {
                let term_cell = terminal
                    .cells
                    .get(y as usize * terminal.width as usize + x as usize)
                    .copied()
                    .unwrap_or_default();
                let protect_cursor = x.abs_diff(cursor.0) <= 1 && y.abs_diff(cursor.1) <= 1;
                let mut cell = engine.composed_cell(
                    x,
                    y,
                    &term_cell,
                    protect_cursor,
                    text_shadow[y as usize * self.width as usize + x as usize],
                );
                if let (Some((start_x, notice_y)), Some((_, accent))) = (notice_position, notice) {
                    if y == notice_y && x >= start_x && ((x - start_x) as usize) < notice_len {
                        let offset = (x - start_x) as usize;
                        cell.ch = notice_chars[offset];
                        cell.fg = if offset == 2 {
                            accent
                        } else {
                            Rgb(224, 222, 240)
                        };
                        cell.bg = Rgb(25, 23, 40);
                    }
                }
                row_cells.push(cell);
            }
            if terminal.cursor_visible && cursor.1 == y && cursor.0 < self.width {
                if let Some(cell) = row_cells.get_mut(cursor.0 as usize) {
                    apply_cursor(cell, terminal.cursor_style);
                }
            }

            let start = y as usize * self.width as usize;
            let end = start + row_cells.len();
            let line_changed = self.force || row_cells.as_slice() != &self.previous[start..end];
            if line_changed {
                let mut x = 0;
                while x < row_cells.len() {
                    let cell = row_cells[x];
                    if cell.wide_continuation || (!self.force && self.previous[start + x] == cell) {
                        x += 1;
                        continue;
                    }

                    let run_start = x;
                    x += 1;
                    while x < row_cells.len() {
                        let next = row_cells[x];
                        if next.wide_continuation
                            || (!self.force && self.previous[start + x] == next)
                            || !same_style(cell, next)
                        {
                            break;
                        }
                        x += 1;
                    }

                    queue!(
                        stdout,
                        MoveTo(run_start as u16, y),
                        SetForegroundColor(to_color(cell.fg)),
                        SetBackgroundColor(to_color(cell.bg))
                    )?;
                    set_attributes(stdout, &mut self.output_attrs, cell.attrs)?;
                    for cell in &row_cells[run_start..x] {
                        write!(stdout, "{}", cell.ch)?;
                    }
                }
                self.previous[start..end].copy_from_slice(row_cells);
            }
        }
        if !animate {
            queue!(
                stdout,
                SetForegroundColor(Color::Reset),
                SetBackgroundColor(Color::Reset)
            )?;
            if self.output_attrs.is_some() {
                queue!(stdout, SetAttribute(Attribute::Reset))?;
                self.output_attrs = None;
            }
        }
        stdout.flush()?;
        self.force = false;
        Ok(())
    }

    fn update_text_shadow(&mut self, terminal: &TerminalGrid) {
        self.text_shadow.fill(false);
        let width = self.width as usize;
        let height = self.height as usize;
        for y in 0..height.min(terminal.height as usize) {
            for x in 0..width.min(terminal.width as usize) {
                let index = y * terminal.width as usize + x;
                if !terminal.cells.get(index).is_some_and(|cell| cell.ch != ' ') {
                    continue;
                }
                for shadow_y in y.saturating_sub(1)..=(y + 1).min(height.saturating_sub(1)) {
                    let row = shadow_y * width;
                    for shadow_x in x.saturating_sub(1)..=(x + 1).min(width.saturating_sub(1)) {
                        self.text_shadow[row + shadow_x] = true;
                    }
                }
            }
        }
    }
}

fn same_style(left: Cell, right: Cell) -> bool {
    left.fg == right.fg && left.bg == right.bg && left.attrs == right.attrs
}

fn set_attributes(
    stdout: &mut BufWriter<Stdout>,
    active: &mut Option<u8>,
    attrs: u8,
) -> io::Result<()> {
    let current = match *active {
        Some(current) => current,
        None => {
            queue!(stdout, SetAttribute(Attribute::Reset))?;
            0
        }
    };

    if current & (CELL_ATTR_BOLD | CELL_ATTR_DIM) != attrs & (CELL_ATTR_BOLD | CELL_ATTR_DIM) {
        queue!(stdout, SetAttribute(Attribute::NormalIntensity))?;
        if attrs & CELL_ATTR_BOLD != 0 {
            queue!(stdout, SetAttribute(Attribute::Bold))?;
        }
        if attrs & CELL_ATTR_DIM != 0 {
            queue!(stdout, SetAttribute(Attribute::Dim))?;
        }
    }
    if current & CELL_ATTR_ITALIC != attrs & CELL_ATTR_ITALIC {
        queue!(
            stdout,
            SetAttribute(if attrs & CELL_ATTR_ITALIC != 0 {
                Attribute::Italic
            } else {
                Attribute::NoItalic
            })
        )?;
    }
    if current & CELL_ATTR_UNDERLINE != attrs & CELL_ATTR_UNDERLINE {
        queue!(
            stdout,
            SetAttribute(if attrs & CELL_ATTR_UNDERLINE != 0 {
                Attribute::Underlined
            } else {
                Attribute::NoUnderline
            })
        )?;
    }
    if current & CELL_ATTR_INVERSE != attrs & CELL_ATTR_INVERSE {
        queue!(
            stdout,
            SetAttribute(if attrs & CELL_ATTR_INVERSE != 0 {
                Attribute::Reverse
            } else {
                Attribute::NoReverse
            })
        )?;
    }
    *active = Some(attrs);
    Ok(())
}

fn apply_cursor(cell: &mut Cell, style: TerminalCursorStyle) {
    cell.ch = match style {
        TerminalCursorStyle::BlinkBlock | TerminalCursorStyle::SteadyBlock => '█',
        TerminalCursorStyle::BlinkUnderline | TerminalCursorStyle::SteadyUnderline => '▁',
        TerminalCursorStyle::Default
        | TerminalCursorStyle::BlinkVerticalBar
        | TerminalCursorStyle::SteadyVerticalBar => '▏',
    };
    cell.fg = Rgb(240, 245, 255);
    cell.attrs = 0;
    cell.wide_continuation = false;
}

fn find_notice_position(
    width: u16,
    height: u16,
    terminal: &TerminalGrid,
    cursor: (u16, u16),
    text_len: usize,
) -> Option<(u16, u16)> {
    if text_len == 0 || text_len > width as usize {
        return None;
    }
    let last_start = width - text_len as u16;
    (0..height).find_map(|y| {
        (0..=last_start).rev().find_map(|x| {
            let available = (0..text_len).all(|offset| {
                let cell_x = x + offset as u16;
                if cell_x >= terminal.width || y >= terminal.height {
                    return false;
                }
                let index = y as usize * terminal.width as usize + cell_x as usize;
                let term_cell = terminal.cells.get(index).copied().unwrap_or_default();
                term_cell.ch == ' '
                    && term_cell.bg.is_none()
                    && !(cell_x.abs_diff(cursor.0) <= 1 && y.abs_diff(cursor.1) <= 1)
            });
            available.then_some((x, y))
        })
    })
}

fn to_color(rgb: Rgb) -> Color {
    Color::Rgb {
        r: rgb.0,
        g: rgb.1,
        b: rgb.2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_shadow_is_limited_to_a_single_cell_neighborhood() {
        let mut terminal = TerminalGrid::new(5, 3);
        terminal.cells[7].ch = 'x';
        let mut renderer = Renderer::new(5, 3);
        renderer.update_text_shadow(&terminal);

        assert!(renderer.text_shadow[6]);
        assert!(renderer.text_shadow[8]);
        assert!(!renderer.text_shadow[0]);
    }

    #[test]
    fn cursor_is_rendered_as_a_static_marker() {
        let mut cell = Cell::default();
        apply_cursor(&mut cell, TerminalCursorStyle::BlinkVerticalBar);
        assert_eq!(cell.ch, '▏');
        assert_eq!(cell.fg, Rgb(240, 245, 255));

        apply_cursor(&mut cell, TerminalCursorStyle::BlinkBlock);
        assert_eq!(cell.ch, '█');

        apply_cursor(&mut cell, TerminalCursorStyle::BlinkUnderline);
        assert_eq!(cell.ch, '▁');
    }

    #[test]
    fn adjacent_cells_coalesce_only_when_their_terminal_style_matches() {
        let first = Cell::default();
        let mut matching_cell = first;
        matching_cell.ch = 'A';
        let mut different_style = matching_cell;
        different_style.fg = Rgb(1, 2, 3);

        assert!(same_style(first, matching_cell));
        assert!(!same_style(first, different_style));
    }

    #[test]
    fn palette_notice_uses_free_cells_and_stays_away_from_cursor() {
        let width = 20;
        let height = 2;
        let terminal = TerminalGrid::new(width, height);
        assert_eq!(
            find_notice_position(width, height, &terminal, (10, 0), 8),
            Some((12, 0))
        );
        assert_eq!(
            find_notice_position(8, 1, &TerminalGrid::new(8, 1), (4, 0), 8),
            None
        );
    }
}
