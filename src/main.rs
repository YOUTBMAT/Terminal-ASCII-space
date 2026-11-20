mod pty_shell;
mod reactivity;
mod renderer;
mod space_engine;

use std::{
    io::{self, Write},
    panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, SetCursorStyle, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    style::{Attribute, Print, SetAttribute},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use pty_shell::{PtyShell, Rgb};
use reactivity::Reactivity;
use renderer::Renderer;
use space_engine::SpaceEngine;

static ACTIVE: AtomicBool = AtomicBool::new(false);
const FRAME_TIME: Duration = Duration::from_millis(33);
const UNFOCUSED_FRAME_TIME: Duration = Duration::from_millis(200);
const CHILD_POLL_TIME: Duration = Duration::from_millis(100);

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    panic::set_hook(Box::new(|info| {
        TerminalGuard::restore();
        eprintln!("space-terminal panic: {info}");
    }));

    let terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&terminate))?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&terminate))?;

    let size = terminal::size()?;
    let mut shell = PtyShell::spawn(size.0, size.1)?;
    let mut terminal_grid = shell.grid();
    let _terminal = TerminalGuard::enter()?;
    let mut renderer = Renderer::new(size.0, size.1);
    let mut engine = SpaceEngine::new(size.0, size.1);
    let mut reactivity = Reactivity::start();
    let mut last_frame = Instant::now();
    let mut focused = true;
    let mut paused = false;
    let mut warp_drive = false;
    let mut scene_dirty = false;
    let mut status_notice: Option<(String, Rgb, Instant)> = None;
    let mut last_output_bytes = 0_u64;
    let mut terminal_dirty = false;
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
            frame_interval(paused, focused)
                .map(|interval| interval.saturating_sub(loop_now.duration_since(last_frame)))
                .unwrap_or(CHILD_POLL_TIME)
        };
        let timeout = until_frame.min(next_child_poll.saturating_duration_since(loop_now));
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind != crossterm::event::KeyEventKind::Release => {
                    if key.code == KeyCode::F(10) {
                        break;
                    }
                    if key.code == KeyCode::F(9) {
                        warp_drive = !warp_drive;
                        scene_dirty = true;
                        renderer.force_redraw();
                        continue;
                    }
                    if key.code == KeyCode::F(5) {
                        let active = engine.toggle_solar_system();
                        status_notice = Some((
                            if active {
                                "SOLAR SYSTEM ON".to_string()
                            } else {
                                "SOLAR SYSTEM OFF".to_string()
                            },
                            if active {
                                Rgb(126, 222, 255)
                            } else {
                                Rgb(170, 170, 190)
                            },
                            Instant::now() + Duration::from_secs(2),
                        ));
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
                        let palette = engine.next_palette();
                        status_notice = Some((
                            format!("{}/4 | {}", palette.index, palette.name),
                            palette.accent,
                            Instant::now() + Duration::from_secs(2),
                        ));
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
                    engine.resize(width, height);
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
        let frame_due = frame_interval(paused, focused)
            .is_some_and(|interval| now.duration_since(last_frame) >= interval);
        if !frame_due && !scene_dirty && !terminal_dirty {
            continue;
        }
        let dt = now.duration_since(last_frame).as_secs_f32().min(0.25);
        last_frame = now;

        let metrics = reactivity.snapshot();
        if !paused {
            engine.update(
                dt,
                metrics.input_rate,
                now < burst_until,
                metrics.cpu_usage,
                metrics.memory_fraction,
                warp_drive,
            );
            scene_dirty = false;
        } else if scene_dirty {
            engine.refresh(
                metrics.cpu_usage,
                metrics.memory_fraction,
                metrics.input_rate,
                warp_drive,
            );
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
        renderer.draw(&engine, &terminal_grid, !paused, notice)?;
    }

    Ok(())
}

fn frame_interval(paused: bool, focused: bool) -> Option<Duration> {
    if paused {
        None
    } else if focused {
        Some(FRAME_TIME)
    } else {
        Some(UNFOCUSED_FRAME_TIME)
    }
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
        assert_eq!(frame_interval(false, true), Some(FRAME_TIME));
        assert_eq!(frame_interval(false, false), Some(UNFOCUSED_FRAME_TIME));
        assert_eq!(frame_interval(true, true), None);
        assert_eq!(frame_interval(true, false), None);
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
    }
}
