use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

fn theme_path() -> io::Result<PathBuf> {
    let config_dir = env::var_os("XDG_CONFIG_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "home/config directory unavailable")
        })?;
    Ok(config_dir.join("space-terminal").join("theme"))
}

pub fn load_theme() -> io::Result<Option<String>> {
    load_theme_at(&theme_path()?)
}

fn load_theme_at(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(theme) => {
            let theme = theme.trim();
            Ok((!theme.is_empty()).then(|| theme.to_string()))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn save_theme(theme: &str) -> io::Result<()> {
    save_theme_at(&theme_path()?, theme)
}

fn save_theme_at(path: &Path, theme: &str) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("theme preference path has no parent directory"))?;
    fs::create_dir_all(directory)?;

    let temporary = directory.join(format!("theme.tmp.{}", std::process::id()));
    fs::write(&temporary, format!("{theme}\n"))?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(temporary);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn saved_theme_is_read_back_from_the_config_path() {
        let directory = env::temp_dir().join(format!(
            "space-terminal-config-{}-{}",
            std::process::id(),
            NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        let path = directory.join("theme");

        save_theme_at(&path, "ocean").unwrap();

        assert_eq!(load_theme_at(&path).unwrap().as_deref(), Some("ocean"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_theme_preference_is_not_an_error() {
        let path = env::temp_dir().join(format!(
            "space-terminal-missing-theme-{}-{}",
            std::process::id(),
            NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        assert_eq!(load_theme_at(&path).unwrap(), None);
    }
}
