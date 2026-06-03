//! Application theme definitions and file-backed custom theme loading.

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const BUILTIN_THEME_FILES: &[&str] = &[
    include_str!("../assets/themes/snow_lv.json"),
    include_str!("../assets/themes/gruvbox_dark.json"),
    include_str!("../assets/themes/gruvbox_light.json"),
    include_str!("../assets/themes/terminal.json"),
    include_str!("../assets/themes/catppuccin_mocha.json"),
    include_str!("../assets/themes/catppuccin_latte.json"),
    include_str!("../assets/themes/dracula.json"),
    include_str!("../assets/themes/nord.json"),
    include_str!("../assets/themes/tokyo_night.json"),
    include_str!("../assets/themes/solarized_dark.json"),
    include_str!("../assets/themes/monokai.json"),
    include_str!("../assets/themes/high_contrast.json"),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeId {
    #[default]
    SnowLV,
    GruvboxDark,
    GruvboxLight,
    Terminal,
    CatppuccinMocha,
    CatppuccinLatte,
    Dracula,
    Nord,
    TokyoNight,
    SolarizedDark,
    Monokai,
    HighContrast,
}

impl ThemeId {
    pub const ALL: [Self; 12] = [
        Self::SnowLV,
        Self::GruvboxDark,
        Self::GruvboxLight,
        Self::Terminal,
        Self::CatppuccinMocha,
        Self::CatppuccinLatte,
        Self::Dracula,
        Self::Nord,
        Self::TokyoNight,
        Self::SolarizedDark,
        Self::Monokai,
        Self::HighContrast,
    ];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::SnowLV => "SnowLV",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::GruvboxLight => "Gruvbox Light",
            Self::Terminal => "Terminal",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::CatppuccinLatte => "Catppuccin Latte",
            Self::Dracula => "Dracula",
            Self::Nord => "Nord",
            Self::TokyoNight => "Tokyo Night",
            Self::SolarizedDark => "Solarized Dark",
            Self::Monokai => "Monokai",
            Self::HighContrast => "High Contrast",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::SnowLV => "snow_lv",
            Self::GruvboxDark => "gruvbox_dark",
            Self::GruvboxLight => "gruvbox_light",
            Self::Terminal => "terminal",
            Self::CatppuccinMocha => "catppuccin_mocha",
            Self::CatppuccinLatte => "catppuccin_latte",
            Self::Dracula => "dracula",
            Self::Nord => "nord",
            Self::TokyoNight => "tokyo_night",
            Self::SolarizedDark => "solarized_dark",
            Self::Monokai => "monokai",
            Self::HighContrast => "high_contrast",
        }
    }

    pub fn theme(self) -> AppTheme {
        builtin_theme(self.id())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppTheme {
    pub dark: bool,
    pub background: [u8; 3],
    pub panel: [u8; 3],
    pub panel_alt: [u8; 3],
    pub card: [u8; 3],
    pub card_hover: [u8; 3],
    pub text: [u8; 3],
    pub muted_text: [u8; 3],
    pub accent: [u8; 3],
    pub accent_alt: [u8; 3],
    pub warning: [u8; 3],
    pub error: [u8; 3],
    pub success: [u8; 3],
    pub playhead: [u8; 3],
    pub grid: [u8; 3],
    pub selection: [u8; 3],
    pub chart: Vec<[u8; 3]>,
}

impl AppTheme {
    pub fn color(&self, rgb: [u8; 3]) -> egui::Color32 {
        egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
    }

    pub fn readable_text_color(&self, background: egui::Color32) -> egui::Color32 {
        let candidates = [
            self.color(self.text),
            egui::Color32::BLACK,
            egui::Color32::WHITE,
        ];

        candidates
            .into_iter()
            .max_by(|a, b| {
                contrast_ratio(*a, background)
                    .partial_cmp(&contrast_ratio(*b, background))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(egui::Color32::WHITE)
    }

    pub fn visuals(&self) -> egui::Visuals {
        let mut visuals = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        visuals.panel_fill = self.color(self.panel);
        visuals.window_fill = self.color(self.panel);
        visuals.extreme_bg_color = self.color(self.background);
        visuals.faint_bg_color = self.color(self.card);
        visuals.code_bg_color = self.color(self.card);
        visuals.selection.bg_fill = self.color(self.selection);
        visuals.selection.stroke = egui::Stroke::new(1.0, self.color(self.accent));
        visuals.hyperlink_color = self.color(self.accent_alt);
        visuals.warn_fg_color = self.color(self.warning);
        visuals.error_fg_color = self.color(self.error);
        visuals.override_text_color = Some(self.color(self.text));

        let inactive = &mut visuals.widgets.inactive;
        inactive.bg_fill = self.color(self.card);
        inactive.weak_bg_fill = self.color(self.card);
        inactive.fg_stroke.color = self.color(self.text);
        inactive.bg_stroke.color = self.color(self.grid);

        let hovered = &mut visuals.widgets.hovered;
        hovered.bg_fill = self.color(self.card_hover);
        hovered.weak_bg_fill = self.color(self.card_hover);
        hovered.fg_stroke.color = self.color(self.text);
        hovered.bg_stroke.color = self.color(self.accent);

        let active = &mut visuals.widgets.active;
        active.bg_fill = self.color(self.selection);
        active.weak_bg_fill = self.color(self.selection);
        active.fg_stroke.color = self.color(self.text);
        active.bg_stroke.color = self.color(self.accent);

        visuals
    }
}

fn contrast_ratio(a: egui::Color32, b: egui::Color32) -> f32 {
    let a_luminance = relative_luminance(a);
    let b_luminance = relative_luminance(b);
    let lighter = a_luminance.max(b_luminance);
    let darker = a_luminance.min(b_luminance);

    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(color: egui::Color32) -> f32 {
    fn channel(value: u8) -> f32 {
        let normalized = value as f32 / 255.0;
        if normalized <= 0.03928 {
            normalized / 12.92
        } else {
            ((normalized + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ThemeFile {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(flatten)]
    theme: AppTheme,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeSource {
    BuiltIn,
    Custom(PathBuf),
}

#[derive(Clone, Debug)]
pub struct ThemeEntry {
    pub id: String,
    pub display_name: String,
    pub source: ThemeSource,
    pub theme: AppTheme,
}

#[derive(Clone, Debug)]
pub struct ThemeRegistry {
    themes: Vec<ThemeEntry>,
    load_errors: Vec<String>,
    fallback_id: String,
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

impl ThemeRegistry {
    pub fn builtin() -> Self {
        let themes = builtin_theme_entries();

        Self {
            themes,
            load_errors: Vec::new(),
            fallback_id: ThemeId::default().id().to_string(),
        }
    }

    pub fn load(custom_dir: Option<&Path>) -> Self {
        let mut registry = Self::builtin();
        if let Some(custom_dir) = custom_dir {
            registry.load_custom_themes(custom_dir);
        }
        registry
    }

    pub fn themes(&self) -> &[ThemeEntry] {
        &self.themes
    }

    pub fn load_errors(&self) -> &[String] {
        &self.load_errors
    }

    pub fn resolve_id(&self, requested: &str) -> String {
        if self.themes.iter().any(|theme| theme.id == requested) {
            requested.to_string()
        } else {
            self.fallback_id.clone()
        }
    }

    pub fn theme(&self, theme_id: &str) -> &AppTheme {
        self.themes
            .iter()
            .find(|theme| theme.id == theme_id)
            .or_else(|| {
                self.themes
                    .iter()
                    .find(|theme| theme.id == self.fallback_id)
            })
            .map(|theme| &theme.theme)
            .expect("theme registry must contain the fallback theme")
    }

    pub fn display_name(&self, theme_id: &str) -> &str {
        self.themes
            .iter()
            .find(|theme| theme.id == theme_id)
            .or_else(|| {
                self.themes
                    .iter()
                    .find(|theme| theme.id == self.fallback_id)
            })
            .map(|theme| theme.display_name.as_str())
            .unwrap_or("SnowLV")
    }

    pub fn ensure_default_theme_files(custom_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(custom_dir)
            .map_err(|e| format!("Failed to create themes directory: {}", e))?;

        for entry in builtin_theme_entries() {
            let path = custom_dir.join(format!("{}.json", entry.id));
            if path.exists() {
                continue;
            }

            let file = ThemeFile {
                id: entry.id,
                name: entry.display_name,
                theme: entry.theme,
            };
            let content = serde_json::to_string_pretty(&file)
                .map_err(|e| format!("Failed to serialize default theme: {}", e))?;
            std::fs::write(&path, content)
                .map_err(|e| format!("Failed to write default theme file: {}", e))?;
        }

        Ok(())
    }

    fn load_custom_themes(&mut self, custom_dir: &Path) {
        let Ok(entries) = std::fs::read_dir(custom_dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !is_theme_file(&path) {
                continue;
            }

            match load_theme_file(&path) {
                Ok(theme) => {
                    if let Some(existing_idx) = self
                        .themes
                        .iter()
                        .position(|existing| existing.id == theme.id)
                    {
                        if matches!(self.themes[existing_idx].source, ThemeSource::BuiltIn) {
                            self.themes[existing_idx] = theme;
                        } else {
                            self.load_errors.push(format!(
                                "{} uses duplicate theme id '{}'",
                                path.display(),
                                theme.id
                            ));
                        }
                    } else {
                        self.themes.push(theme);
                    }
                }
                Err(error) => self
                    .load_errors
                    .push(format!("{}: {}", path.display(), error)),
            }
        }
    }
}

fn is_theme_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase()),
        Some(ext) if ext == "json" || ext == "yaml" || ext == "yml"
    )
}

fn builtin_theme_entries() -> Vec<ThemeEntry> {
    BUILTIN_THEME_FILES
        .iter()
        .map(|content| {
            let theme: ThemeFile =
                serde_json::from_str(content).expect("embedded built-in theme must be valid JSON");
            theme_file_to_entry(theme, "", ThemeSource::BuiltIn)
                .expect("embedded built-in themes must be valid")
        })
        .collect()
}

fn builtin_theme(theme_id: &str) -> AppTheme {
    builtin_theme_entries()
        .into_iter()
        .find(|theme| theme.id == theme_id)
        .unwrap_or_else(|| {
            builtin_theme_entries()
                .into_iter()
                .find(|theme| theme.id == ThemeId::default().id())
                .expect("embedded built-in themes must include the fallback theme")
        })
        .theme
}

fn load_theme_file(path: &Path) -> Result<ThemeEntry, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read theme file: {}", e))?;
    let parsed: ThemeFile = match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("yaml" | "yml") => {
            serde_yml::from_str(&content).map_err(|e| format!("Invalid YAML: {}", e))?
        }
        _ => serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {}", e))?,
    };

    let fallback_id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("custom_theme");
    theme_file_to_entry(parsed, fallback_id, ThemeSource::Custom(path.to_path_buf()))
}

fn theme_file_to_entry(
    parsed: ThemeFile,
    fallback_id: &str,
    source: ThemeSource,
) -> Result<ThemeEntry, String> {
    let id = normalize_theme_id(if parsed.id.trim().is_empty() {
        fallback_id
    } else {
        &parsed.id
    });
    if id.is_empty() {
        return Err("Theme id must contain at least one ASCII letter or number".to_string());
    }
    if parsed.theme.chart.is_empty() {
        return Err("Theme chart palette must contain at least one color".to_string());
    }

    let display_name = if parsed.name.trim().is_empty() {
        humanize_theme_id(&id)
    } else {
        parsed.name.trim().to_string()
    };

    Ok(ThemeEntry {
        id,
        display_name,
        source,
        theme: parsed.theme,
    })
}

fn normalize_theme_id(raw: &str) -> String {
    let mut id = String::new();
    let mut last_was_separator = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if (ch == '_' || ch == '-' || ch.is_whitespace()) && !last_was_separator {
            id.push('_');
            last_was_separator = true;
        }
    }
    id.trim_matches('_').to_string()
}

fn humanize_theme_id(id: &str) -> String {
    id.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = first.to_ascii_uppercase().to_string();
                    word.push_str(chars.as_str());
                    word
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ThemeFile, ThemeId, ThemeRegistry, ThemeSource};

    #[test]
    fn all_themes_have_enough_distinct_chart_colors() {
        for theme_id in ThemeId::ALL {
            let theme = theme_id.theme();
            assert!(
                theme.chart.len() >= 10,
                "{} should have at least 10 chart colors",
                theme_id.display_name()
            );

            let unique: HashSet<[u8; 3]> = theme.chart.iter().copied().collect();
            assert_eq!(
                unique.len(),
                theme.chart.len(),
                "{} should not repeat chart colors",
                theme_id.display_name()
            );
        }
    }

    #[test]
    fn theme_accents_are_distinct() {
        let unique: HashSet<[u8; 3]> = ThemeId::ALL
            .iter()
            .map(|theme_id| theme_id.theme().accent)
            .collect();

        assert_eq!(
            unique.len(),
            ThemeId::ALL.len(),
            "built-in themes should have distinct accent colors"
        );
    }

    #[test]
    fn builtin_registry_loads_default_themes_from_file() {
        let registry = ThemeRegistry::builtin();
        assert!(registry.load_errors().is_empty());
        assert_eq!(registry.themes().len(), ThemeId::ALL.len());

        for theme_id in ThemeId::ALL {
            let entry = registry
                .themes()
                .iter()
                .find(|theme| theme.id == theme_id.id())
                .expect("built-in theme file should contain every default theme id");
            assert_eq!(entry.source, ThemeSource::BuiltIn);
            assert_eq!(entry.display_name, theme_id.display_name());
        }
    }

    #[test]
    fn ensure_default_theme_files_writes_each_default_theme() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("snowlv-default-theme-test-{}", unique));

        ThemeRegistry::ensure_default_theme_files(&dir).unwrap();

        let files: HashSet<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| {
                entry
                    .unwrap()
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        for theme_id in ThemeId::ALL {
            assert!(
                files.contains(&format!("{}.json", theme_id.id())),
                "{} should be written to the themes folder",
                theme_id.id()
            );
        }
        assert_eq!(files.len(), ThemeId::ALL.len());

        let registry = ThemeRegistry::load(Some(&dir));
        fs::remove_dir_all(&dir).ok();

        assert!(registry.load_errors().is_empty());
        assert_eq!(registry.themes().len(), ThemeId::ALL.len());
        for theme_id in ThemeId::ALL {
            let entry = registry
                .themes()
                .iter()
                .find(|theme| theme.id == theme_id.id())
                .expect("seeded default theme should load from the themes folder");
            assert!(matches!(entry.source, ThemeSource::Custom(_)));
            assert_eq!(entry.display_name, theme_id.display_name());
        }
    }

    #[test]
    fn registry_loads_custom_json_themes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("snowlv-theme-test-{}", unique));
        fs::create_dir_all(&dir).unwrap();

        let theme_file = ThemeFile {
            id: "my_track_theme".to_string(),
            name: "My Track Theme".to_string(),
            theme: ThemeId::SnowLV.theme(),
        };
        fs::write(
            dir.join("my-track-theme.json"),
            serde_json::to_string_pretty(&theme_file).unwrap(),
        )
        .unwrap();

        let registry = ThemeRegistry::load(Some(&dir));
        fs::remove_dir_all(&dir).ok();

        assert!(registry.load_errors().is_empty());
        assert_eq!(registry.resolve_id("my_track_theme"), "my_track_theme");
        assert_eq!(registry.display_name("my_track_theme"), "My Track Theme");
        assert_eq!(registry.theme("my_track_theme").chart.len(), 10);
    }

    #[test]
    fn registry_falls_back_for_missing_theme_ids() {
        let registry = ThemeRegistry::builtin();
        assert_eq!(registry.resolve_id("missing_theme"), ThemeId::SnowLV.id());
        assert_eq!(
            registry.display_name("missing_theme"),
            ThemeId::SnowLV.display_name()
        );
    }
}
