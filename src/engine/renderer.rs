use std::io::{self, BufWriter, Stdout, Write};

use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor},
};
use unicode_width::UnicodeWidthChar;
use vt100::CursorStyle as TerminalCursorStyle;

use crate::engine::{
    canvas::{Cell, Rgb},
    pty_shell::{
        TerminalGrid, CELL_ATTR_BOLD, CELL_ATTR_DIM, CELL_ATTR_INVERSE, CELL_ATTR_ITALIC,
        CELL_ATTR_UNDERLINE,
    },
    Canvas,
};
use crate::themes::ThemeMetadata;

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
    text_shadow_dirty: bool,
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
            text_shadow_dirty: true,
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
        self.text_shadow_dirty = true;
    }

    pub fn force_redraw(&mut self) {
        self.force = true;
    }

    pub fn terminal_changed(&mut self) {
        self.text_shadow_dirty = true;
    }

    pub fn draw(
        &mut self,
        canvas: &Canvas,
        terminal: &TerminalGrid,
        animate: bool,
        notice: Option<(&str, Rgb)>,
        picker: Option<(&[ThemeMetadata], usize)>,
    ) -> io::Result<()> {
        debug_assert_eq!(self.width, canvas.width);
        debug_assert_eq!(self.height, canvas.height);
        let cursor = terminal.cursor;
        if self.text_shadow_dirty {
            self.update_text_shadow(terminal);
            self.text_shadow_dirty = false;
        }
        self.notice_chars.clear();
        if let Some((text, _)) = notice {
            self.notice_chars.extend(['[', ' ']);
            self.notice_chars.extend(text.chars());
            self.notice_chars.extend([' ', ']']);
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
            let start = y as usize * self.width as usize;
            for x in 0..self.width {
                let term_cell = terminal
                    .cells
                    .get(y as usize * terminal.width as usize + x as usize)
                    .copied()
                    .unwrap_or_default();
                let protect_cursor = terminal.cursor_visible && x == cursor.0 && y == cursor.1;
                let mut cell = canvas.composed_cell(
                    x,
                    y,
                    &term_cell,
                    protect_cursor,
                    text_shadow[y as usize * self.width as usize + x as usize],
                );
                if let (Some((start_x, notice_y)), Some((_, accent))) = (notice_position, notice) {
                    if y == notice_y && x >= start_x && ((x - start_x) as usize) < notice_len {
                        let offset = (x - start_x) as usize;
                        apply_notice(&mut cell, notice_chars[offset], offset, accent);
                    }
                }
                row_cells.push(cell);
            }
            if terminal.cursor_visible && cursor.1 == y && cursor.0 < self.width {
                if let Some(cell) = row_cells.get_mut(cursor.0 as usize) {
                    let has_text = terminal
                        .cells
                        .get(y as usize * terminal.width as usize + cursor.0 as usize)
                        .is_some_and(|c| c.ch != ' ');
                    apply_cursor(cell, terminal.cursor_style, has_text);
                }
            }
            if let Some((items, selected)) = picker {
                draw_picker_row(row_cells, y, self.width, self.height, items, selected);
            }
            normalize_wide_cells(row_cells, terminal, y, cursor);
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

fn normalize_wide_cells(cells: &mut [Cell], terminal: &TerminalGrid, row: u16, cursor: (u16, u16)) {
    for column in 0..cells.len() {
        if cells[column].wide_continuation
            || UnicodeWidthChar::width(cells[column].ch).unwrap_or(1) <= 1
        {
            continue;
        }

        let next_column = column + 1;
        if next_column >= cells.len() {
            cells[column].ch = '·';
            continue;
        }
        let terminal_cell = if next_column < terminal.width as usize && row < terminal.height {
            terminal
                .cells
                .get(row as usize * terminal.width as usize + next_column)
                .copied()
                .unwrap_or_default()
        } else {
            Default::default()
        };
        let next_is_cursor = cursor == (next_column as u16, row) && terminal.cursor_visible;
        if next_is_cursor
            || (!terminal_cell.wide_continuation
                && (terminal_cell.ch != ' ' || terminal_cell.bg.is_some()))
        {
            cells[column].ch = '·';
        } else {
            let (left, right) = cells.split_at_mut(next_column);
            let previous_cell = left[column];
            let next_cell = &mut right[0];
            next_cell.ch = ' ';
            next_cell.fg = previous_cell.fg;
            next_cell.bg = previous_cell.bg;
            next_cell.attrs = previous_cell.attrs;
            next_cell.wide_continuation = true;
        }
    }
}

fn apply_notice(cell: &mut Cell, ch: char, offset: usize, accent: Rgb) {
    cell.ch = ch;
    cell.fg = if offset == 2 {
        accent
    } else {
        Rgb(224, 222, 240)
    };
    cell.bg = Rgb(25, 23, 40);
    cell.attrs = 0;
    cell.wide_continuation = false;
}

#[derive(Clone, Copy)]
struct PickerLayout {
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    first_item: usize,
    visible_items: usize,
}

fn picker_layout(
    width: u16,
    height: u16,
    items: &[ThemeMetadata],
    selected: usize,
) -> Option<PickerLayout> {
    if items.is_empty() || width < 8 || height < 5 {
        return None;
    }
    let visible_items = items.len().min(height.saturating_sub(4) as usize);
    let selected = selected.min(items.len() - 1);
    let first_item = selected.saturating_add(1).saturating_sub(visible_items);
    let content_width = items
        .iter()
        .map(|item| item.name.chars().count() + 4)
        .max()
        .unwrap_or_default()
        .max("Selecionar tema".chars().count() + 4)
        .max("↑/↓ mover  Enter trocar  Esc fechar".chars().count() + 4);
    let popup_width = content_width.min(width.saturating_sub(2) as usize);
    let popup_height = visible_items + 4;
    Some(PickerLayout {
        left: (width as usize - popup_width) / 2,
        top: (height as usize - popup_height) / 2,
        width: popup_width,
        height: popup_height,
        first_item,
        visible_items,
    })
}

fn draw_picker_row(
    cells: &mut [Cell],
    y: u16,
    screen_width: u16,
    screen_height: u16,
    items: &[ThemeMetadata],
    selected: usize,
) {
    let Some(layout) = picker_layout(screen_width, screen_height, items, selected) else {
        return;
    };
    let row = y as usize;
    if row < layout.top || row >= layout.top + layout.height {
        return;
    }
    let border = Rgb(82, 156, 190);
    let background = Rgb(8, 15, 29);
    let foreground = Rgb(190, 211, 226);
    for (column, cell) in cells
        .iter_mut()
        .enumerate()
        .take(layout.left + layout.width)
        .skip(layout.left)
    {
        let relative_x = column - layout.left;
        let relative_y = row - layout.top;
        let is_horizontal = relative_y == 0 || relative_y + 1 == layout.height;
        let is_vertical = relative_x == 0 || relative_x + 1 == layout.width;
        let mut glyph = ' ';
        let mut fg = foreground;
        let mut bg = background;
        if is_horizontal && is_vertical {
            glyph = match (relative_x == 0, relative_y == 0) {
                (true, true) => '┌',
                (false, true) => '┐',
                (true, false) => '└',
                (false, false) => '┘',
            };
            fg = border;
        } else if is_horizontal {
            glyph = '─';
            fg = border;
        } else if is_vertical {
            glyph = '│';
            fg = border;
        } else if relative_y == 1 {
            let title = "Selecionar tema";
            let title_start =
                layout.left + (layout.width.saturating_sub(title.chars().count()) / 2);
            glyph = char_at_cell(title, column, title_start);
            fg = Rgb(129, 222, 235);
        } else if relative_y >= 2 && relative_y < layout.height - 2 {
            let item_index = layout.first_item + relative_y - 2;
            if item_index < items.len() && relative_y - 2 < layout.visible_items {
                let is_selected = item_index == selected;
                let label = format!(
                    "{} {}",
                    if is_selected { "›" } else { " " },
                    items[item_index].name
                );
                glyph = char_at_cell(&label, relative_x, 2);
                if is_selected {
                    fg = Rgb(7, 22, 32);
                    bg = Rgb(75, 204, 207);
                }
            }
        } else if relative_y == layout.height - 2 {
            let footer = "↑/↓ mover Enter trocar Esc fechar";
            glyph = char_at_cell(footer, relative_x, 2);
            fg = Rgb(138, 158, 179);
        }
        cell.ch = glyph;
        cell.fg = fg;
        cell.bg = bg;
        cell.attrs = 0;
        cell.wide_continuation = false;
    }
}

fn char_at_cell(text: &str, column: usize, start: usize) -> char {
    column
        .checked_sub(start)
        .and_then(|offset| text.chars().nth(offset))
        .unwrap_or(' ')
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

fn apply_cursor(cell: &mut Cell, style: TerminalCursorStyle, has_text: bool) {
    if has_text {
        match style {
            TerminalCursorStyle::BlinkUnderline | TerminalCursorStyle::SteadyUnderline => {
                cell.attrs |= CELL_ATTR_UNDERLINE;
            }
            _ => {
                cell.attrs ^= CELL_ATTR_INVERSE;
            }
        }
    } else {
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
    use crate::engine::pty_shell::TerminalGrid;

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
    fn terminal_changes_invalidate_the_cached_text_shadow() {
        let mut terminal = TerminalGrid::new(5, 3);
        let mut renderer = Renderer::new(5, 3);
        renderer.update_text_shadow(&terminal);
        renderer.text_shadow_dirty = false;

        terminal.cells[7].ch = 'x';
        renderer.terminal_changed();
        assert!(renderer.text_shadow_dirty);
        renderer.update_text_shadow(&terminal);
        assert!(renderer.text_shadow[6]);
        assert!(renderer.text_shadow[8]);
    }

    #[test]
    fn cursor_is_rendered_as_a_static_marker() {
        let mut cell = Cell::default();
        apply_cursor(&mut cell, TerminalCursorStyle::BlinkVerticalBar, false);
        assert_eq!(cell.ch, '▏');
        assert_eq!(cell.fg, Rgb(240, 245, 255));

        apply_cursor(&mut cell, TerminalCursorStyle::BlinkBlock, false);
        assert_eq!(cell.ch, '█');

        apply_cursor(&mut cell, TerminalCursorStyle::BlinkUnderline, false);
        assert_eq!(cell.ch, '▁');
    }

    #[test]
    fn cursor_over_text_preserves_character_and_inverts() {
        let mut cell = Cell {
            ch: 'W',
            attrs: 0,
            ..Cell::default()
        };
        apply_cursor(&mut cell, TerminalCursorStyle::BlinkBlock, true);
        assert_eq!(cell.ch, 'W');
        assert_ne!(cell.attrs & CELL_ATTR_INVERSE, 0);

        let mut underline_cell = Cell {
            ch: 'W',
            attrs: 0,
            ..Cell::default()
        };
        apply_cursor(
            &mut underline_cell,
            TerminalCursorStyle::BlinkUnderline,
            true,
        );
        assert_eq!(underline_cell.ch, 'W');
        assert_ne!(underline_cell.attrs & CELL_ATTR_UNDERLINE, 0);
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
    fn wide_glyphs_are_clipped_at_the_edge_and_preserve_cli_cells() {
        let terminal = TerminalGrid::new(3, 1);
        let mut cells = vec![Cell::default(); 3];
        cells[0].ch = '界';
        cells[1].ch = '界';
        cells[2].ch = '界';

        normalize_wide_cells(&mut cells, &terminal, 0, (0, 0));

        assert!(cells[1].wide_continuation);
        assert_eq!(cells[2].ch, '·');
        assert!(!cells[2].wide_continuation);

        let mut terminal = TerminalGrid::new(3, 1);
        terminal.cells[1].ch = 'X';
        let mut cells = vec![Cell::default(); 3];
        cells[0].ch = '界';
        normalize_wide_cells(&mut cells, &terminal, 0, (0, 0));
        assert_eq!(cells[0].ch, '·');
        assert_eq!(cells[1].ch, '▀');
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

    #[test]
    fn picker_is_centered_and_scrolls_selected_item_into_view() {
        let items = [
            ThemeMetadata {
                id: "a",
                name: "First",
                description: "",
            },
            ThemeMetadata {
                id: "b",
                name: "Second",
                description: "",
            },
            ThemeMetadata {
                id: "c",
                name: "Third",
                description: "",
            },
        ];
        let layout = picker_layout(80, 24, &items, 2).unwrap();
        assert_eq!((layout.left, layout.top), (20, 8));
        assert_eq!(layout.first_item, 0);

        let short_layout = picker_layout(30, 7, &items, 2).unwrap();
        assert_eq!(short_layout.visible_items, 3);
        assert_eq!(short_layout.first_item, 0);
    }

    #[test]
    fn picker_text_does_not_repeat_first_character_in_padding() {
        let items = [ThemeMetadata {
            id: "space",
            name: "Space",
            description: "",
        }];
        let layout = picker_layout(80, 24, &items, 0).unwrap();
        let mut cells = vec![Cell::default(); 80];

        draw_picker_row(&mut cells, (layout.top + 1) as u16, 80, 24, &items, 0);
        let title_start = layout.left + (layout.width - "Selecionar tema".chars().count()) / 2;
        let title: Vec<char> = cells
            .iter()
            .skip(layout.left)
            .take(layout.width)
            .map(|cell| cell.ch)
            .collect();
        assert!(title[1..title_start - layout.left]
            .iter()
            .all(|glyph| *glyph == ' '));
        assert_eq!(
            title[title_start - layout.left
                ..title_start - layout.left + "Selecionar tema".chars().count()]
                .iter()
                .copied()
                .collect::<String>(),
            "Selecionar tema",
        );

        draw_picker_row(&mut cells, (layout.top + 2) as u16, 80, 24, &items, 0);
        let item_row: String = cells
            .iter()
            .skip(layout.left)
            .take(layout.width)
            .map(|cell| cell.ch)
            .collect();
        assert_eq!(item_row.chars().nth(1), Some(' '));
        assert_eq!(item_row.chars().nth(2), Some('›'));

        draw_picker_row(
            &mut cells,
            (layout.top + layout.height - 2) as u16,
            80,
            24,
            &items,
            0,
        );
        let footer_row: String = cells
            .iter()
            .skip(layout.left)
            .take(layout.width)
            .map(|cell| cell.ch)
            .collect();
        assert_eq!(footer_row.chars().nth(1), Some(' '));
        let footer = "↑/↓ mover Enter trocar Esc fechar";
        assert_eq!(
            footer_row
                .chars()
                .skip(2)
                .take(footer.chars().count())
                .collect::<String>(),
            footer
        );
    }

    #[test]
    fn notice_replaces_any_wide_continuation_in_the_background() {
        let mut cell = Cell {
            wide_continuation: true,
            attrs: CELL_ATTR_BOLD,
            ..Cell::default()
        };
        apply_notice(&mut cell, 'T', 2, Rgb(92, 207, 220));
        assert_eq!(cell.ch, 'T');
        assert_eq!(cell.fg, Rgb(92, 207, 220));
        assert_eq!(cell.attrs, 0);
        assert!(!cell.wide_continuation);
    }
}
