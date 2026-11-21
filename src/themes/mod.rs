mod aurora;
mod ocean;
mod space;

use crate::engine::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

const AVAILABLE_THEMES: [ThemeMetadata; 3] = [
    ThemeMetadata {
        id: "space",
        name: "Space",
        description:
            "An animated deep-space scene with stars, nebulae, planets, ships, and meteors.",
    },
    ThemeMetadata {
        id: "aurora",
        name: "Aurora",
        description: "Slow ribbons of teal, emerald, and violet polar light.",
    },
    ThemeMetadata {
        id: "ocean",
        name: "Ocean",
        description: "A layered seabed with drifting schools, bubbles, sharks, and tropical life.",
    },
];

pub fn available() -> &'static [ThemeMetadata] {
    &AVAILABLE_THEMES
}

pub fn create(id: &str, width: u16, height: u16) -> Option<Box<dyn Theme>> {
    let requested = id.trim();
    let registered_id = AVAILABLE_THEMES
        .iter()
        .find(|theme| {
            theme.id.eq_ignore_ascii_case(requested) || theme.name.eq_ignore_ascii_case(requested)
        })?
        .id;
    match registered_id {
        "space" => Some(Box::new(space::SpaceTheme::new(width, height))),
        "aurora" => Some(Box::new(aurora::AuroraTheme::new(width, height))),
        "ocean" => Some(Box::new(ocean::OceanTheme::new(width, height))),
        _ => None,
    }
}

pub fn next_id(current_id: &str) -> &'static str {
    let current_index = AVAILABLE_THEMES
        .iter()
        .position(|theme| theme.id.eq_ignore_ascii_case(current_id))
        .unwrap_or(0);
    AVAILABLE_THEMES[(current_index + 1) % AVAILABLE_THEMES.len()].id
}

pub fn index_of(id: &str) -> usize {
    AVAILABLE_THEMES
        .iter()
        .position(|theme| theme.id.eq_ignore_ascii_case(id))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Canvas, ThemeAction};

    #[test]
    fn registry_lists_and_creates_the_space_theme_by_id() {
        let metadata = available();
        assert_eq!(metadata.len(), 3);
        assert_eq!(metadata[0].id, "space");

        let mut theme = create("Space", 80, 24).expect("space theme should be registered");
        assert_eq!(theme.id(), metadata[0].id);
        assert_eq!(theme.name(), metadata[0].name);
        assert_eq!(theme.description(), metadata[0].description);
        assert!(theme
            .action(ThemeAction::CyclePalette)
            .is_some_and(|notice| notice.text.starts_with("2/4")));
        assert!(theme
            .action(ThemeAction::ToggleFeature("solar-system"))
            .is_some_and(|notice| notice.text == "SOLAR SYSTEM ON"));
        assert_eq!(next_id("space"), "aurora");
        assert_eq!(next_id("aurora"), "ocean");
        assert_eq!(index_of("Aurora"), 1);
        assert_eq!(create("Aurora", 80, 24).unwrap().id(), "aurora");
        assert_eq!(create("Ocean", 80, 24).unwrap().id(), "ocean");

        let mut canvas = Canvas::new(80, 24);
        theme.render(&mut canvas);
        assert_eq!((canvas.width, canvas.height), (80, 24));
        theme.resize(40, 12);
        theme.update(0.033, 0.0);
        theme.render(&mut canvas);
        assert_eq!((canvas.width, canvas.height), (40, 12));
        assert!(create("unknown", 80, 24).is_none());
    }
}
