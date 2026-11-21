mod config;
pub mod engine;
pub mod themes;

use std::{env, error::Error};

struct Options {
    theme: Option<String>,
    fps: u16,
    list: bool,
    help: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = parse_args(env::args().skip(1))?;
    if options.help {
        print_help();
        return Ok(());
    }
    if options.list {
        for theme in themes::available() {
            println!("{} - {}", theme.id, theme.description);
        }
        return Ok(());
    }

    let saved_theme = match config::load_theme() {
        Ok(theme) => theme,
        Err(error) => {
            eprintln!("Could not read saved theme preference: {error}");
            None
        }
    };
    let requested_theme = options
        .theme
        .as_deref()
        .or(saved_theme.as_deref())
        .unwrap_or("space");
    let (width, height) = crossterm::terminal::size()?;
    let theme = match themes::create(requested_theme, width, height) {
        Some(theme) => theme,
        None if options.theme.is_none() => {
            eprintln!("Saved theme '{requested_theme}' is unavailable; using 'space'.");
            themes::create("space", width, height).expect("space theme is registered")
        }
        None => {
            let available = themes::available()
                .iter()
                .map(|theme| theme.id)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "unknown theme '{}'. Available themes: {available}",
                requested_theme
            )
            .into());
        }
    };
    if let Err(error) = config::save_theme(theme.id()) {
        eprintln!("Could not save theme preference: {error}");
    }
    engine::run(theme, options.fps)
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        theme: None,
        fps: 30,
        list: false,
        help: false,
    };
    let mut args = args.into_iter();
    let mut positional_theme = false;
    while let Some(argument) = args.next() {
        if argument == "--list" || argument == "--list-themes" {
            options.list = true;
        } else if argument == "--help" || argument == "-h" {
            options.help = true;
        } else if argument == "--theme" {
            let value = args
                .next()
                .ok_or_else(|| "--theme requires a theme name".to_string())?;
            if value.starts_with('-') {
                return Err("--theme requires a theme name".to_string());
            }
            options.theme = Some(value);
        } else if let Some(value) = argument.strip_prefix("--theme=") {
            if value.is_empty() {
                return Err("--theme requires a theme name".to_string());
            }
            options.theme = Some(value.to_string());
        } else if argument == "--fps" {
            options.fps = parse_fps(
                &args
                    .next()
                    .ok_or_else(|| "--fps requires a value from 1 to 120".to_string())?,
            )?;
        } else if let Some(value) = argument.strip_prefix("--fps=") {
            options.fps = parse_fps(value)?;
        } else if argument.starts_with('-') {
            return Err(format!("unknown option '{argument}'"));
        } else if !positional_theme {
            options.theme = Some(argument);
            positional_theme = true;
        } else {
            return Err(format!("unexpected argument '{argument}'"));
        }
    }
    if options.list && options.help {
        return Err("--list and --help cannot be combined".to_string());
    }
    if options
        .theme
        .as_deref()
        .is_some_and(|theme| theme.trim().is_empty())
    {
        return Err("--theme requires a non-empty theme name".to_string());
    }
    Ok(options)
}

fn parse_fps(value: &str) -> Result<u16, String> {
    let fps = value
        .parse::<u16>()
        .map_err(|_| format!("invalid FPS '{value}': expected an integer from 1 to 120"))?;
    if !(1..=120).contains(&fps) {
        return Err(format!(
            "invalid FPS '{value}': expected an integer from 1 to 120"
        ));
    }
    Ok(fps)
}

fn print_help() {
    println!(
        "Terminal ASCII Space\n\
         Usage: space-terminal [--theme <name>] [--fps <1..120>] [--list]\n\
         \n\
         Options:\n\
           --theme <name>  Start with a registered theme (default: last selected, then space)\n\
           --fps <value>   Focused frame rate (default: 30, range: 1..120)\n\
           --list          List registered theme IDs and descriptions (--list-themes alias)\n\
           -h, --help      Show this help\n\
         \n\
         Runtime keys:\n\
           Alt+T           Switch to the next theme\n\
           Alt+M           Open/close theme picker; arrows move, Enter selects, Esc closes\n\
           F4 / Ctrl+Alt+P Cycle the current theme palette\n\
           F5              Toggle the current theme's solar-system feature\n\
           F9              Toggle warp drive; F12 / Ctrl+Alt+S pause; F10 exits"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<Options, String> {
        parse_args(arguments.iter().map(|value| (*value).to_string()))
    }

    #[test]
    fn cli_options_select_theme_list_and_fps() {
        let options = parse(&["--theme", "aurora", "--fps", "60"]).unwrap();
        assert_eq!(options.theme.as_deref(), Some("aurora"));
        assert_eq!(options.fps, 60);
        assert!(parse(&["--list"]).unwrap().list);
        assert!(parse(&["--help"]).unwrap().help);
    }

    #[test]
    fn cli_rejects_missing_or_out_of_range_values() {
        assert!(parse(&["--theme"]).is_err());
        assert!(parse(&["--theme", "--fps", "30"]).is_err());
        assert!(parse(&["--fps", "0"]).is_err());
        assert!(parse(&["--fps=121"]).is_err());
        assert!(parse(&["--unknown"]).is_err());
        assert!(parse(&["space", "aurora"]).is_err());
    }

    #[test]
    fn no_theme_argument_leaves_preference_resolution_to_startup() {
        assert_eq!(parse(&[]).unwrap().theme, None);
    }
}
