use std::{
    io::{self, Write},
    panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use super::{
    pty_shell::PtyShell, reactivity::Reactivity, renderer::Renderer, Canvas, Rgb, Theme,
    ThemeAction,
};
use crate::themes;
use crossterm::{
    cursor::{Hide, SetCursorStyle, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    style::{Attribute, Print, SetAttribute},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

static ACTIVE: AtomicBool = AtomicBool::new(false);
const UNFOCUSED_FRAME_TIME: Duration = Duration::from_millis(200);
const CHILD_POLL_TIME: Duration = Duration::from_millis(100);
const MAX_FPS: u16 = 120;

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            Print("\x1b[?1004h"),
            Hide
        ) {
            let _ = execute!(
                stdout,
                Print("\x1b[?1004l"),
                DisableMouseCapture,
                LeaveAlternateScreen
            );
            let _ = terminal::disable_raw_mode();
            return Err(error);
        }
        ACTIVE.store(true, Ordering::SeqCst);
        Ok(Self)
    }

    fn restore() {
        if ACTIVE.swap(false, Ordering::SeqCst) {
            let mut stdout = io::stdout();
            let _ = execute!(
                stdout,
                Show,
                SetCursorStyle::DefaultUserShape,
                SetAttribute(Attribute::Reset),
                Print("\x1b[?1004l"),
                DisableMouseCapture,
                LeaveAlternateScreen
            );
            let _ = terminal::disable_raw_mode();
            let _ = stdout.flush();
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        Self::restore();
    }
}

pub(super) fn run(mut theme: Box<dyn Theme>, fps: u16) -> Result<(), Box<dyn std::error::Error>> {
    if !(1..=MAX_FPS).contains(&fps) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("FPS must be between 1 and {MAX_FPS}"),
        )
        .into());
    }
    panic::set_hook(Box::new(|info| {
        TerminalGuard::restore();
        eprintln!("space-terminal panic: {info}");
    }));

    let terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&terminate))?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&terminate))?;
    #[cfg(unix)]
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&terminate))?;

    let size = terminal::size()?;
    theme.resize(size.0, size.1);
    let mut shell = PtyShell::spawn(size.0, size.1)?;
    let mut terminal_grid = shell.grid();
    let _terminal = TerminalGuard::enter()?;
    let mut renderer = Renderer::new(size.0, size.1);
    let mut canvas = Canvas::new(size.0, size.1);
    let mut reactivity = Reactivity::start();
    let mut last_frame = Instant::now();
    let mut focused = true;
    let mut paused = false;
    let mut scene_dirty = false;
    let mut status_notice: Option<(String, Rgb, Instant)> = None;
    let mut last_output_bytes = 0_u64;
    let mut terminal_dirty = false;
    let mut picker_open = false;
    let mut picker_selection = themes::index_of(theme.id());
    let mut burst_until = Instant::now();
    let mut next_child_poll = Instant::now();

    loop {
        if terminate.load(Ordering::Relaxed) {
            break;
        }
        let loop_now = Instant::now();
        if loop_now >= next_child_poll {
            if shell.child_exited()? {
                break;
            }
            next_child_poll = loop_now + CHILD_POLL_TIME;
        }

        observe_output(
            &shell,
            &mut last_output_bytes,
            &mut burst_until,
            &mut terminal_dirty,
            loop_now,
        );
        expire_notice(
            &mut status_notice,
            &mut scene_dirty,
            &mut renderer,
            loop_now,
        );

        let until_frame = if scene_dirty || terminal_dirty {
            Duration::ZERO
        } else {
            frame_interval(paused, focused, fps)
                .map(|interval| interval.saturating_sub(loop_now.duration_since(last_frame)))
                .unwrap_or(CHILD_POLL_TIME)
        };
        let timeout = until_frame.min(next_child_poll.saturating_duration_since(loop_now));
        let event = match event::poll(timeout) {
            Ok(true) => match event::read() {
                Ok(event) => Some(event),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => None,
                Err(_) if terminate.load(Ordering::Relaxed) => break,
                Err(error) => return Err(error.into()),
            },
            Ok(false) => None,
            Err(_) if terminate.load(Ordering::Relaxed) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        if terminate.load(Ordering::Relaxed) {
            break;
        }
        if let Some(event) = event {
            match event {
                Event::Key(key) if key.kind != crossterm::event::KeyEventKind::Release => {
                    if key.code == KeyCode::F(10) {
                        break;
                    }
                    if is_alt_char(key, 't') {
                        let next_id = themes::next_id(theme.id());
                        let (name, preference_error) = activate_theme(
                            &mut theme,
                            next_id,
                            size_for(&terminal_grid),
                            &mut canvas,
                            &mut renderer,
                        )?;
                        status_notice = Some((
                            theme_notice(name, preference_error),
                            Rgb(92, 207, 220),
                            Instant::now() + Duration::from_secs(2),
                        ));
                        picker_open = false;
                        scene_dirty = true;
                        reactivity.note_input();
                        continue;
                    }
                    if is_alt_char(key, 'm') {
                        picker_open = !picker_open;
                        picker_selection = themes::index_of(theme.id());
                        scene_dirty = true;
                        renderer.force_redraw();
                        reactivity.note_input();
                        continue;
                    }
                    if picker_open {
                        match key.code {
                            KeyCode::Up => {
                                picker_selection = if picker_selection == 0 {
                                    themes::available().len() - 1
                                } else {
                                    picker_selection - 1
                                };
                                scene_dirty = true;
                                renderer.force_redraw();
                                continue;
                            }
                            KeyCode::Down => {
                                picker_selection =
                                    (picker_selection + 1) % themes::available().len();
                                scene_dirty = true;
                                renderer.force_redraw();
                                continue;
                            }
                            KeyCode::Enter => {
                                let selected = themes::available()[picker_selection];
                                let (name, preference_error) = activate_theme(
                                    &mut theme,
                                    selected.id,
                                    size_for(&terminal_grid),
                                    &mut canvas,
                                    &mut renderer,
                                )?;
                                status_notice = Some((
                                    theme_notice(name, preference_error),
                                    Rgb(92, 207, 220),
                                    Instant::now() + Duration::from_secs(2),
                                ));
                                picker_open = false;
                                scene_dirty = true;
                                reactivity.note_input();
                                continue;
                            }
                            KeyCode::Esc => {
                                picker_open = false;
                                scene_dirty = true;
                                renderer.force_redraw();
                                continue;
                            }
                            _ => {
                                picker_open = false;
                                scene_dirty = true;
                                renderer.force_redraw();
                            }
                        }
                    }
                    if key.code == KeyCode::F(9) {
                        theme.action(ThemeAction::ToggleFeature("warp-drive"));
                        scene_dirty = true;
                        renderer.force_redraw();
                        continue;
                    }
                    if key.code == KeyCode::F(5) {
                        status_notice = theme
                            .action(ThemeAction::ToggleFeature("solar-system"))
                            .map(|notice| {
                                (
                                    notice.text,
                                    notice.accent,
                                    Instant::now() + Duration::from_secs(2),
                                )
                            });
                        scene_dirty = true;
                        renderer.force_redraw();
                        continue;
                    }
                    if key.code == KeyCode::F(4)
                        || (key.code == KeyCode::Char('p')
                            && key
                                .modifiers
                                .contains(KeyModifiers::CONTROL | KeyModifiers::ALT))
                    {
                        status_notice = theme.action(ThemeAction::CyclePalette).map(|notice| {
                            (
                                notice.text,
                                notice.accent,
                                Instant::now() + Duration::from_secs(2),
                            )
                        });
                        scene_dirty = true;
                        renderer.force_redraw();
                        continue;
                    }
                    if key.code == KeyCode::F(12)
                        || (key.code == KeyCode::Char('s')
                            && key
                                .modifiers
                                .contains(KeyModifiers::CONTROL | KeyModifiers::ALT))
                    {
                        paused = !paused;
                        scene_dirty = true;
                        renderer.force_redraw();
                        continue;
                    }
                    reactivity.note_input();
                    shell.write_input(&encode_key(key, terminal_grid.application_cursor)?)?;
                }
                Event::Resize(width, height) => {
                    shell.resize(width, height)?;
                    shell.copy_grid_into(&mut terminal_grid);
                    theme.resize(width, height);
                    canvas.resize(width, height);
                    renderer.resize(width, height);
                    scene_dirty = true;
                }
                Event::Paste(text) => {
                    shell.write_paste(&text, terminal_grid.bracketed_paste)?;
                }
                Event::Mouse(mouse) => {
                    shell.write_mouse(mouse)?;
                }
                Event::FocusGained if !focused => {
                    focused = true;
                    scene_dirty = true;
                    renderer.force_redraw();
                }
                Event::FocusLost if focused => {
                    focused = false;
                    scene_dirty = true;
                    renderer.force_redraw();
                }
                _ => {}
            }
        }

        let now = Instant::now();
        observe_output(
            &shell,
            &mut last_output_bytes,
            &mut burst_until,
            &mut terminal_dirty,
            now,
        );
        expire_notice(&mut status_notice, &mut scene_dirty, &mut renderer, now);
        let frame_due = frame_interval(paused, focused, fps)
            .is_some_and(|interval| now.duration_since(last_frame) >= interval);
        if !frame_due && !scene_dirty && !terminal_dirty {
            continue;
        }
        let dt = now.duration_since(last_frame).as_secs_f32().min(0.25);
        last_frame = now;

        let metrics = reactivity.snapshot();
        theme.set_system_metrics(metrics.cpu_usage, metrics.memory_fraction);
        theme.set_output_burst(now < burst_until);
        if !paused {
            theme.update(dt, metrics.input_rate);
            scene_dirty = false;
        } else if scene_dirty {
            theme.refresh(metrics.input_rate);
            scene_dirty = false;
        }
        if terminal_dirty {
            shell.copy_grid_into(&mut terminal_grid);
            terminal_dirty = false;
        }
        let notice = status_notice
            .as_ref()
            .filter(|(_, _, deadline)| now < *deadline)
            .map(|(text, accent, _)| (text.as_str(), *accent));
        theme.render(&mut canvas);
        let picker = picker_open.then_some((themes::available(), picker_selection));
        if let Err(error) = renderer.draw(&canvas, &terminal_grid, !paused, notice, picker) {
            if terminate.load(Ordering::Relaxed) {
                break;
            }
            return Err(error.into());
        }
    }

    Ok(())
}

fn frame_interval(paused: bool, focused: bool, fps: u16) -> Option<Duration> {
    if paused {
        None
    } else {
        let requested = Duration::from_secs_f64(1.0 / fps as f64);
        Some(if focused {
            requested
        } else {
            requested.max(UNFOCUSED_FRAME_TIME)
        })
    }
}

fn activate_theme(
    theme: &mut Box<dyn Theme>,
    id: &str,
    (width, height): (u16, u16),
    canvas: &mut Canvas,
    renderer: &mut Renderer,
) -> io::Result<(&'static str, Option<String>)> {
    let mut replacement = themes::create(id, width, height).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("theme '{id}' is not registered"),
        )
    })?;
    replacement.resize(width, height);
    let name = replacement.name();
    *theme = replacement;
    let preference_error = crate::config::save_theme(id)
        .err()
        .map(|error| format!("Could not save theme preference: {error}"));
    canvas.clear();
    renderer.force_redraw();
    Ok((name, preference_error))
}

fn theme_notice(name: &str, preference_error: Option<String>) -> String {
    preference_error.unwrap_or_else(|| format!("Theme: {name}"))
}

fn size_for(terminal: &super::pty_shell::TerminalGrid) -> (u16, u16) {
    (terminal.width, terminal.height)
}

fn is_alt_char(key: KeyEvent, expected: char) -> bool {
    matches!(key.code, KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&expected))
        && key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::CONTROL)
}

fn observe_output(
    shell: &PtyShell,
    last_output_bytes: &mut u64,
    burst_until: &mut Instant,
    terminal_dirty: &mut bool,
    now: Instant,
) {
    let output_bytes = shell.output_bytes();
    let output_delta = output_bytes.saturating_sub(*last_output_bytes);
    if output_delta == 0 {
        return;
    }
    if output_delta > 1024 {
        *burst_until = now + Duration::from_millis(900);
    }
    *last_output_bytes = output_bytes;
    *terminal_dirty = true;
}

fn expire_notice(
    notice: &mut Option<(String, Rgb, Instant)>,
    scene_dirty: &mut bool,
    renderer: &mut Renderer,
    now: Instant,
) {
    if notice
        .as_ref()
        .is_some_and(|(_, _, deadline)| now >= *deadline)
    {
        *notice = None;
        *scene_dirty = true;
        renderer.force_redraw();
    }
}

fn encode_key(key: KeyEvent, application_cursor: bool) -> Result<Vec<u8>, io::Error> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let mut bytes = match key.code {
        KeyCode::Char(ch) if ctrl && ch.is_ascii_alphabetic() => {
            vec![(ch.to_ascii_lowercase() as u8) - b'a' + 1]
        }
        KeyCode::Char(ch) => {
            let mut buffer = [0; 4];
            ch.encode_utf8(&mut buffer).as_bytes().to_vec()
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => {
            if application_cursor {
                b"\x1bOA".to_vec()
            } else {
                b"\x1b[A".to_vec()
            }
        }
        KeyCode::Down => {
            if application_cursor {
                b"\x1bOB".to_vec()
            } else {
                b"\x1b[B".to_vec()
            }
        }
        KeyCode::Right => {
            if application_cursor {
                b"\x1bOC".to_vec()
            } else {
                b"\x1b[C".to_vec()
            }
        }
        KeyCode::Left => {
            if application_cursor {
                b"\x1bOD".to_vec()
            } else {
                b"\x1b[D".to_vec()
            }
        }
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::F(n @ 1..=4) => format!("\x1bO{}", (b'P' + n - 1) as char).into_bytes(),
        KeyCode::F(n @ 5..=12) => format!("\x1b[{}~", 15 + (n - 5) * 2).into_bytes(),
        _ => return Ok(Vec::new()),
    };
    if alt {
        bytes.insert(0, 0x1b);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rate_depends_on_focus_and_pause_state_not_idle_time() {
        assert_eq!(
            frame_interval(false, true, 30),
            Some(Duration::from_secs_f64(1.0 / 30.0))
        );
        assert_eq!(frame_interval(false, false, 30), Some(UNFOCUSED_FRAME_TIME));
        assert_eq!(
            frame_interval(false, true, 60),
            Some(Duration::from_secs_f64(1.0 / 60.0))
        );
        assert_eq!(frame_interval(true, true, 30), None);
        assert_eq!(frame_interval(true, false, 30), None);
    }

    #[test]
    fn arrow_keys_follow_application_cursor_mode() {
        let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(encode_key(up, false).unwrap(), b"\x1b[A");
        assert_eq!(encode_key(up, true).unwrap(), b"\x1bOA");
    }

    #[test]
    fn shifted_tab_and_control_keys_keep_terminal_encodings() {
        let back_tab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert_eq!(encode_key(back_tab, false).unwrap(), b"\x1b[Z");
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(encode_key(ctrl_c, false).unwrap(), [3]);
        let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(encode_key(ctrl_d, false).unwrap(), [4]);
        for (ch, control_byte) in [('o', 15), ('x', 24), ('g', 7)] {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL);
            assert_eq!(encode_key(key, false).unwrap(), [control_byte]);
        }
        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(encode_key(tab, false).unwrap(), b"\t");
        for ch in ['h', 'j', 'k', 'l', 'o'] {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
            assert_eq!(encode_key(key, false).unwrap(), ch.to_string().as_bytes());
        }
    }

    #[test]
    fn only_alt_theme_shortcuts_are_intercepted() {
        assert!(is_alt_char(
            KeyEvent::new(KeyCode::Char('T'), KeyModifiers::ALT),
            't'
        ));
        assert!(!is_alt_char(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
            't'
        ));
        assert!(!is_alt_char(
            KeyEvent::new(
                KeyCode::Char('t'),
                KeyModifiers::ALT | KeyModifiers::CONTROL
            ),
            't'
        ));
    }
}
