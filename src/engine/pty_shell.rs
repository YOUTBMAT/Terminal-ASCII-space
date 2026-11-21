use std::{
    io::{self, Read, Write},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

pub use crate::engine::canvas::Rgb;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use vt100::{Color, CursorStyle, MouseProtocolEncoding, MouseProtocolMode};

pub const CELL_ATTR_BOLD: u8 = 1;
pub const CELL_ATTR_DIM: u8 = 2;
pub const CELL_ATTR_ITALIC: u8 = 4;
pub const CELL_ATTR_UNDERLINE: u8 = 8;
pub const CELL_ATTR_INVERSE: u8 = 16;

#[derive(Debug, thiserror::Error)]
pub enum PtyShellError {
    #[error("terminal I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("PTY operation failed: {0}")]
    Pty(#[from] anyhow::Error),
    #[error("PTY output reader failed: {0}")]
    Reader(#[source] io::Error),
    #[error("PTY output reader thread panicked")]
    ReaderThreadPanicked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalCell {
    pub ch: char,
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub attrs: u8,
    pub wide_continuation: bool,
}

impl TerminalCell {
    fn blank() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg: None,
            attrs: 0,
            wide_continuation: false,
        }
    }
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self::blank()
    }
}

#[derive(Clone, Debug)]
pub struct TerminalGrid {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<TerminalCell>,
    pub cursor: (u16, u16),
    pub cursor_visible: bool,
    pub cursor_style: CursorStyle,
    pub alternate_screen: bool,
    pub application_cursor: bool,
    pub bracketed_paste: bool,
    pub mouse_mode: MouseProtocolMode,
    pub mouse_encoding: MouseProtocolEncoding,
}

impl TerminalGrid {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![TerminalCell::blank(); width as usize * height as usize],
            cursor: (0, 0),
            cursor_visible: true,
            cursor_style: CursorStyle::Default,
            alternate_screen: false,
            application_cursor: false,
            bracketed_paste: false,
            mouse_mode: MouseProtocolMode::None,
            mouse_encoding: MouseProtocolEncoding::Default,
        }
    }
}

struct Emulator {
    parser: vt100::Parser,
    width: u16,
    height: u16,
}

impl Emulator {
    fn new(width: u16, height: u16) -> Self {
        Self {
            parser: vt100::Parser::new(height, width, 0),
            width,
            height,
        }
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.parser.screen_mut().set_size(height, width);
    }

    fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    fn copy_grid_into(&self, target: &mut TerminalGrid) {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        target.width = cols;
        target.height = rows;
        target
            .cells
            .resize(cols as usize * rows as usize, TerminalCell::blank());
        let (row, col) = screen.cursor_position();
        target.cursor = (col, row);
        target.cursor_visible = !screen.hide_cursor();
        target.cursor_style = screen.cursor_style();
        target.alternate_screen = screen.alternate_screen();
        target.application_cursor = screen.application_cursor();
        target.bracketed_paste = screen.bracketed_paste();
        target.mouse_mode = screen.mouse_protocol_mode();
        target.mouse_encoding = screen.mouse_protocol_encoding();
        for row in 0..rows {
            for col in 0..cols {
                let index = row as usize * cols as usize + col as usize;
                let Some(cell) = screen.cell(row, col) else {
                    target.cells[index] = TerminalCell::blank();
                    continue;
                };
                let ch = cell.contents().chars().next().unwrap_or(' ');
                let mut attrs = 0;
                if cell.bold() {
                    attrs |= CELL_ATTR_BOLD;
                } else if cell.dim() {
                    attrs |= CELL_ATTR_DIM;
                }
                if cell.italic() {
                    attrs |= CELL_ATTR_ITALIC;
                }
                if cell.underline() {
                    attrs |= CELL_ATTR_UNDERLINE;
                }
                if cell.inverse() {
                    attrs |= CELL_ATTR_INVERSE;
                }
                target.cells[index] = TerminalCell {
                    ch,
                    fg: color_to_rgb(cell.fgcolor()),
                    bg: color_to_rgb(cell.bgcolor()),
                    attrs,
                    wide_continuation: cell.is_wide_continuation(),
                };
            }
        }
    }
}

fn color_to_rgb(color: Color) -> Option<Rgb> {
    match color {
        Color::Default => None,
        Color::Idx(index) => Some(ansi_256(index)),
        Color::Rgb(red, green, blue) => Some(Rgb(red, green, blue)),
    }
}

fn ansi_256(index: u8) -> Rgb {
    if index < 16 {
        const COLORS: [Rgb; 16] = [
            Rgb(0, 0, 0),
            Rgb(205, 49, 49),
            Rgb(13, 188, 121),
            Rgb(229, 229, 16),
            Rgb(36, 114, 200),
            Rgb(188, 63, 188),
            Rgb(17, 168, 205),
            Rgb(229, 229, 229),
            Rgb(102, 102, 102),
            Rgb(241, 76, 76),
            Rgb(35, 209, 139),
            Rgb(245, 245, 67),
            Rgb(59, 142, 234),
            Rgb(214, 112, 214),
            Rgb(41, 184, 219),
            Rgb(255, 255, 255),
        ];
        return COLORS[index as usize];
    }
    if index < 232 {
        let n = index - 16;
        let component = |value: u8| if value == 0 { 0 } else { 55 + value * 40 };
        return Rgb(component(n / 36), component((n / 6) % 6), component(n % 6));
    }
    let gray = 8 + (index - 232) * 10;
    Rgb(gray, gray, gray)
}

pub struct PtyShell {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    emulator: Arc<Mutex<Emulator>>,
    output_bytes: Arc<std::sync::atomic::AtomicU64>,
    reader_error: Arc<Mutex<Option<io::Error>>>,
    reader_thread: Option<JoinHandle<()>>,
    stopped: bool,
}

impl PtyShell {
    pub fn spawn(width: u16, height: u16) -> Result<Self, PtyShellError> {
        Self::spawn_with_command(width, height, shell_command())
    }

    fn spawn_with_command(
        width: u16,
        height: u16,
        command: CommandBuilder,
    ) -> Result<Self, PtyShellError> {
        let system = native_pty_system();
        let pair = system.openpty(PtySize {
            rows: height,
            cols: width,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let child = pair.slave.spawn_command(command)?;
        drop(pair.slave);
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let emulator = Arc::new(Mutex::new(Emulator::new(width, height)));
        let output_bytes = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let reader_error = Arc::new(Mutex::new(None));
        let reader_thread = start_reader(
            reader,
            Arc::clone(&emulator),
            Arc::clone(&output_bytes),
            Arc::clone(&reader_error),
        );
        Ok(Self {
            master: pair.master,
            writer,
            child,
            emulator,
            output_bytes,
            reader_error,
            reader_thread: Some(reader_thread),
            stopped: false,
        })
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    pub fn resize(&mut self, width: u16, height: u16) -> Result<(), PtyShellError> {
        self.master
            .resize(PtySize {
                rows: height,
                cols: width,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(PtyShellError::Pty)?;
        self.emulator
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .resize(width, height);
        Ok(())
    }

    pub fn grid(&self) -> TerminalGrid {
        let emulator = self.emulator.lock().unwrap_or_else(|p| p.into_inner());
        let mut grid = TerminalGrid::new(emulator.width, emulator.height);
        emulator.copy_grid_into(&mut grid);
        grid
    }

    pub fn copy_grid_into(&self, target: &mut TerminalGrid) {
        let emulator = self.emulator.lock().unwrap_or_else(|p| p.into_inner());
        emulator.copy_grid_into(target);
    }

    pub fn write_paste(&mut self, text: &str, bracketed: bool) -> io::Result<()> {
        if bracketed {
            self.writer.write_all(b"\x1b[200~")?;
        }
        self.writer.write_all(text.as_bytes())?;
        if bracketed {
            self.writer.write_all(b"\x1b[201~")?;
        }
        self.writer.flush()
    }

    pub fn write_mouse(&mut self, event: MouseEvent) -> io::Result<()> {
        let emulator = self.emulator.lock().unwrap_or_else(|p| p.into_inner());
        let screen = emulator.parser.screen();
        let mode = screen.mouse_protocol_mode();
        if mode == MouseProtocolMode::None {
            return Ok(());
        }
        let encoding = screen.mouse_protocol_encoding();
        drop(emulator);
        let Some((mut button, released, motion)) = mouse_code(event.kind) else {
            return Ok(());
        };
        if event.modifiers.contains(KeyModifiers::SHIFT) {
            button += 4;
        }
        if event.modifiers.contains(KeyModifiers::ALT) {
            button += 8;
        }
        if event.modifiers.contains(KeyModifiers::CONTROL) {
            button += 16;
        }
        let allowed = match mode {
            MouseProtocolMode::None => false,
            MouseProtocolMode::Press => !released && !motion,
            MouseProtocolMode::PressRelease => !motion,
            MouseProtocolMode::ButtonMotion => {
                !motion || matches!(event.kind, MouseEventKind::Drag(_))
            }
            MouseProtocolMode::AnyMotion => true,
        };
        if !allowed {
            return Ok(());
        }
        let x = event.column.saturating_add(1);
        let y = event.row.saturating_add(1);
        match encoding {
            MouseProtocolEncoding::Sgr => {
                let final_char = if released { 'm' } else { 'M' };
                self.write_input(format!("\x1b[<{button};{x};{y}{final_char}").as_bytes())
            }
            MouseProtocolEncoding::Default | MouseProtocolEncoding::Utf8 => {
                let release_code = if released { 3 } else { button };
                let mut bytes = vec![0x1b, b'[', b'M'];
                if encoding == MouseProtocolEncoding::Utf8 {
                    for value in [release_code, x, y] {
                        if let Some(character) = utf8_mouse_character(value) {
                            let mut buffer = [0; 4];
                            bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                        }
                    }
                } else {
                    bytes.extend([
                        32_u16.saturating_add(release_code).min(255) as u8,
                        32_u16.saturating_add(x.min(223)).min(255) as u8,
                        32_u16.saturating_add(y.min(223)).min(255) as u8,
                    ]);
                }
                self.write_input(&bytes)
            }
        }
    }

    pub fn output_bytes(&self) -> u64 {
        self.output_bytes.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn child_exited(&mut self) -> Result<bool, PtyShellError> {
        if self.child.try_wait()?.is_some() {
            return Ok(true);
        }
        if let Some(error) = self
            .reader_error
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            return Err(PtyShellError::Reader(error));
        }
        Ok(false)
    }

    pub fn shutdown(&mut self) -> Result<(), PtyShellError> {
        if self.stopped {
            return Ok(());
        }

        let mut shutdown_error = None;
        #[cfg(unix)]
        if let Some(process_group) = self.master.process_group_leader() {
            if process_group > 1 {
                let result = unsafe { libc::kill(-process_group, libc::SIGHUP) };
                if result != 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::ESRCH) {
                        shutdown_error = Some(PtyShellError::Io(error));
                    }
                }
            }
        }

        match self.child.try_wait() {
            Ok(None) => {
                if let Err(error) = self.child.kill() {
                    if self.child.try_wait().is_ok_and(|status| status.is_none())
                        && shutdown_error.is_none()
                    {
                        shutdown_error = Some(PtyShellError::Io(error));
                    }
                }
            }
            Ok(Some(_)) => {}
            Err(error) => {
                if shutdown_error.is_none() {
                    shutdown_error = Some(PtyShellError::Io(error));
                }
                if let Err(error) = self.child.kill() {
                    if shutdown_error.is_none() {
                        shutdown_error = Some(PtyShellError::Io(error));
                    }
                }
            }
        }

        if let Err(error) = self.child.wait() {
            if shutdown_error.is_none() {
                shutdown_error = Some(PtyShellError::Io(error));
            }
        }
        if let Some(reader_thread) = self.reader_thread.take() {
            if reader_thread.join().is_err() && shutdown_error.is_none() {
                shutdown_error = Some(PtyShellError::ReaderThreadPanicked);
            }
        }
        self.stopped = true;

        shutdown_error.map_or(Ok(()), Err)
    }
}

impl Drop for PtyShell {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Could not fully shut down PTY shell: {error}");
        }
    }
}

fn start_reader(
    mut reader: Box<dyn Read + Send>,
    emulator: Arc<Mutex<Emulator>>,
    output_bytes: Arc<std::sync::atomic::AtomicU64>,
    reader_error: Arc<Mutex<Option<io::Error>>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let mut state = emulator.lock().unwrap_or_else(|p| p.into_inner());
                    state.process(&buffer[..count]);
                    drop(state);
                    output_bytes.fetch_add(count as u64, std::sync::atomic::Ordering::Relaxed);
                }

                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    *reader_error.lock().unwrap_or_else(|p| p.into_inner()) = Some(error);
                    break;
                }
            }
        }
    })
}

fn mouse_code(kind: MouseEventKind) -> Option<(u16, bool, bool)> {
    match kind {
        MouseEventKind::Down(button) => Some((mouse_button_code(button), false, false)),
        MouseEventKind::Up(button) => Some((mouse_button_code(button), true, false)),
        MouseEventKind::Drag(button) => Some((mouse_button_code(button) + 32, false, true)),
        MouseEventKind::Moved => Some((35, false, true)),
        MouseEventKind::ScrollUp => Some((64, false, false)),
        MouseEventKind::ScrollDown => Some((65, false, false)),
        MouseEventKind::ScrollLeft => Some((66, false, false)),
        MouseEventKind::ScrollRight => Some((67, false, false)),
    }
}

fn mouse_button_code(button: MouseButton) -> u16 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

fn utf8_mouse_character(value: u16) -> Option<char> {
    let mut codepoint = 32 + u32::from(value);
    if (0xd800..=0xdfff).contains(&codepoint) {
        codepoint = 0xe000 + (codepoint - 0xd800);
    }
    char::from_u32(codepoint)
}

fn shell_command() -> CommandBuilder {
    #[cfg(windows)]
    {
        let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string());
        let mut cmd = CommandBuilder::new(shell);
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }
        cmd.env("SPACE_TERMINAL_ACTIVE", "1");
        cmd
    }
    #[cfg(not(windows))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut cmd = CommandBuilder::new(shell);
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("SPACE_TERMINAL_ACTIVE", "1");
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vt_parser_tracks_cursor_attributes_and_truecolor() {
        let mut emulator = Emulator::new(12, 3);
        emulator.process(b"\x1b[2;3H\x1b[1;2;3;4;7;38;2;12;34;56;48;5;25mX");
        let mut grid = TerminalGrid::new(12, 3);
        emulator.copy_grid_into(&mut grid);

        assert_eq!(grid.cursor, (3, 1));
        let cell = grid.cells[14];
        assert_eq!(cell.ch, 'X');
        assert_eq!(cell.fg, Some(Rgb(12, 34, 56)));
        assert_eq!(cell.bg, Some(ansi_256(25)));
        assert_eq!(
            cell.attrs,
            CELL_ATTR_DIM | CELL_ATTR_ITALIC | CELL_ATTR_UNDERLINE | CELL_ATTR_INVERSE
        );
        emulator.process(b"\x1b[1;1H\x1b[1mB");
        emulator.copy_grid_into(&mut grid);
        assert_ne!(grid.cells[0].attrs & CELL_ATTR_BOLD, 0);
        emulator.process(b"\x1b[6 q");
        emulator.copy_grid_into(&mut grid);
        assert_eq!(grid.cursor_style, CursorStyle::SteadyVerticalBar);
    }

    #[test]
    fn vt_parser_preserves_normal_screen_across_alternate_screen_and_tracks_modes() {
        let mut emulator = Emulator::new(16, 3);
        emulator.process(b"shell\x1b[?1049h\x1b[?25l\x1b[?1h\x1b[?2004h\x1b[?1000h\x1b[?1006hmenu");
        let mut grid = TerminalGrid::new(16, 3);
        emulator.copy_grid_into(&mut grid);

        assert!(grid.alternate_screen);
        assert!(!grid.cursor_visible);
        assert!(grid.application_cursor);
        assert!(grid.bracketed_paste);
        assert_eq!(grid.mouse_mode, MouseProtocolMode::PressRelease);
        assert_eq!(grid.mouse_encoding, MouseProtocolEncoding::Sgr);
        assert_eq!(grid.cells[0].ch, 'm');

        emulator.process(b"\x1b[?1049l");
        emulator.copy_grid_into(&mut grid);
        assert!(!grid.alternate_screen);
        assert_eq!(grid.cells[0].ch, 's');
    }

    #[test]
    fn vt_parser_places_wide_characters_in_two_terminal_columns() {
        let mut emulator = Emulator::new(8, 2);
        emulator.process("A界B".as_bytes());
        let mut grid = TerminalGrid::new(8, 2);
        emulator.copy_grid_into(&mut grid);

        assert_eq!(grid.cells[0].ch, 'A');
        assert_eq!(grid.cells[1].ch, '界');
        assert!(grid.cells[2].wide_continuation);
        assert_eq!(grid.cells[3].ch, 'B');
        assert_eq!(grid.cursor, (4, 0));
    }

    #[test]
    fn mouse_events_follow_active_mode_and_use_sgr_coordinates() {
        let mut emulator = Emulator::new(8, 2);
        emulator.process(b"\x1b[?1000h\x1b[?1006h");
        let mut grid = TerminalGrid::new(8, 2);
        emulator.copy_grid_into(&mut grid);
        assert_eq!(grid.mouse_mode, MouseProtocolMode::PressRelease);
        assert_eq!(grid.mouse_encoding, MouseProtocolEncoding::Sgr);
        assert_eq!(
            mouse_code(MouseEventKind::Down(MouseButton::Left)),
            Some((0, false, false))
        );
        assert_eq!(
            mouse_code(MouseEventKind::Up(MouseButton::Right)),
            Some((2, true, false))
        );
        assert_eq!(utf8_mouse_character(1), Some('!'));
    }

    #[cfg(unix)]
    #[test]
    fn shutdown_stops_foreground_command_and_joins_reader() {
        let mut command = CommandBuilder::new("/bin/sh");
        command.arg("-c");
        command.arg("printf 'ready\\n'; sleep 30");
        let start = std::time::Instant::now();
        let mut shell = PtyShell::spawn_with_command(80, 24, command).unwrap();
        let output_deadline = start + std::time::Duration::from_secs(2);
        while shell.output_bytes() == 0 && std::time::Instant::now() < output_deadline {
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(shell.output_bytes() > 0);

        shell.shutdown().unwrap();

        assert!(start.elapsed() < std::time::Duration::from_secs(2));
        assert!(shell.reader_thread.is_none());
    }
}
