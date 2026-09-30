//! Optional project-local presentation settings. Independent of goals and native agent data.
use crate::recovery;
use anyhow::{Result, ensure};
use clap::ValueEnum;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    #[value(alias = "up", alias = "top")]
    Above,
    #[default]
    #[value(alias = "down", alias = "bottom")]
    Below,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ThemePatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub muted: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}
impl ThemePatch {
    fn apply(&self, theme: &mut Theme) -> Result<()> {
        for (target, value) in [
            (&mut theme.track, &self.track),
            (&mut theme.fill, &self.fill),
            (&mut theme.accent, &self.accent),
            (&mut theme.text, &self.text),
            (&mut theme.muted, &self.muted),
            (&mut theme.metadata, &self.metadata),
            (&mut theme.warning, &self.warning),
        ] {
            if let Some(value) = value {
                color(value)?;
                *target = value.clone();
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundState {
    Working,
    Waiting,
    Blocked,
    Paused,
    Error,
    Done,
    Empty,
}
impl BackgroundState {
    pub const ALL: [Self; 7] = [
        Self::Working,
        Self::Waiting,
        Self::Blocked,
        Self::Paused,
        Self::Error,
        Self::Done,
        Self::Empty,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Preset {
    pub extends: String,
    pub theme: ThemePatch,
}
impl Default for Preset {
    fn default() -> Self {
        Self {
            extends: "signal".into(),
            theme: Default::default(),
        }
    }
}

pub fn builtin(name: &str) -> Option<Theme> {
    let mut theme = Theme::default();
    match name {
        "signal" => {}
        "forest" => {
            theme.track = "#0F1418".into();
            theme.fill = "#34543B".into();
            theme.muted = "#C4D0C6".into();
            theme.metadata = "#B6C4BA".into();
        }
        "ocean" => {
            theme.track = "#0F1720".into();
            theme.fill = "#244858".into();
            theme.accent = "#8AD9FF".into();
            theme.muted = "#C1CFD8".into();
            theme.metadata = "#B8C9D3".into();
        }
        "amber" => {
            theme.track = "#191610".into();
            theme.fill = "#55432A".into();
            theme.accent = "#FFCF7C".into();
            theme.muted = "#D6CEBD".into();
            theme.metadata = "#C9C2B2".into();
        }
        _ => return None,
    }
    Some(theme)
}
impl Position {
    pub fn direction(self) -> &'static str {
        match self {
            Self::Above => "up",
            Self::Below => "down",
        }
    }
    pub fn ratio(self) -> &'static str {
        match self {
            Self::Above => "0.30",
            Self::Below => "0.70",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub track: String,
    pub fill: String,
    pub accent: String,
    pub text: String,
    pub muted: String,
    pub metadata: String,
    pub warning: String,
}
impl Default for Theme {
    fn default() -> Self {
        Self {
            track: "#0F1214".into(),
            fill: "#1F271B".into(),
            accent: "#C7F96C".into(),
            text: "#F2F5EE".into(),
            muted: "#9BA49B".into(),
            metadata: "#89988C".into(),
            warning: "#F5C26F".into(),
        }
    }
}
pub fn color(value: &str) -> Result<Color> {
    ensure!(
        value.len() == 7
            && value.starts_with('#')
            && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit),
        "color must be #RRGGBB"
    );
    Ok(Color::Rgb(
        u8::from_str_radix(&value[1..3], 16)?,
        u8::from_str_radix(&value[3..5], 16)?,
        u8::from_str_radix(&value[5..7], 16)?,
    ))
}
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub track: Color,
    pub fill: Color,
    pub accent: Color,
    pub text: Color,
    pub muted: Color,
    pub metadata: Color,
    pub warning: Color,
}
impl Default for Palette {
    fn default() -> Self {
        Theme::default().palette().expect("built-in palette")
    }
}
impl Theme {
    pub fn palette(&self) -> Result<Palette> {
        Ok(Palette {
            track: color(&self.track)?,
            fill: color(&self.fill)?,
            accent: color(&self.accent)?,
            text: color(&self.text)?,
            muted: color(&self.muted)?,
            metadata: color(&self.metadata)?,
            warning: color(&self.warning)?,
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub schema: u32,
    pub position: Position,
    /// Percentage of the source/progress split given to the progress pane.
    pub pane_size_percent: u8,
    pub auto_open: bool,
    pub preset: String,
    pub background_brightness: f64,
    pub theme: ThemePatch,
    pub presets: BTreeMap<String, Preset>,
    pub states: BTreeMap<BackgroundState, ThemePatch>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: 1,
            position: Position::Below,
            pane_size_percent: 30,
            auto_open: true,
            preset: "signal".into(),
            background_brightness: 1.0,
            theme: Default::default(),
            presets: Default::default(),
            states: Default::default(),
        }
    }
}
pub fn path(root: &Path) -> PathBuf {
    root.join(".agent-progress/ui.json")
}
pub fn yaml_path(root: &Path) -> PathBuf {
    root.join(".agent-progress/ui.yaml")
}
impl Settings {
    fn resolve_preset(&self, name: &str, seen: &mut BTreeSet<String>) -> Result<Theme> {
        ensure!(
            seen.len() < 16 && seen.insert(name.into()),
            "preset inheritance cycle or excessive depth"
        );
        if let Some(theme) = builtin(name) {
            return Ok(theme);
        }
        let preset = self
            .presets
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("unknown preset: {name}"))?;
        let mut theme = self.resolve_preset(&preset.extends, seen)?;
        preset.theme.apply(&mut theme)?;
        Ok(theme)
    }
    pub fn resolved_theme(&self, state: Option<BackgroundState>) -> Result<Theme> {
        let mut theme = self.resolve_preset(&self.preset, &mut BTreeSet::new())?;
        self.theme.apply(&mut theme)?;
        if let Some(state) = state
            && let Some(patch) = self.states.get(&state)
        {
            patch.apply(&mut theme)?;
        }
        Ok(theme)
    }
    pub fn palette(&self, state: Option<BackgroundState>) -> Result<Palette> {
        let mut p = self.resolved_theme(state)?.palette()?;
        let scale = |color: Color| match color {
            Color::Rgb(r, g, b) => {
                let c = |v: u8| {
                    (f64::from(v) * self.background_brightness)
                        .round()
                        .clamp(0.0, 255.0) as u8
                };
                Color::Rgb(c(r), c(g), c(b))
            }
            other => other,
        };
        p.track = scale(p.track);
        p.fill = scale(p.fill);
        Ok(p)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema == 1, "unsupported presentation schema");
        ensure!(
            (10..=50).contains(&self.pane_size_percent),
            "pane size must be between 10 and 50 percent"
        );
        ensure!(
            self.background_brightness.is_finite()
                && (0.25..=2.0).contains(&self.background_brightness),
            "background brightness must be between 0.25 and 2.0"
        );
        ensure!(self.presets.len() <= 64, "too many presets");
        for name in self.presets.keys() {
            ensure!(
                !name.is_empty()
                    && name.len() <= 80
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    && builtin(name).is_none(),
                "invalid or reserved preset name"
            );
            self.resolve_preset(name, &mut BTreeSet::new())?;
        }
        for state in BackgroundState::ALL {
            self.palette(Some(state))?;
        }
        Ok(())
    }

    pub fn source_split_ratio(&self) -> String {
        format!("{:.2}", 1.0 - f64::from(self.pane_size_percent) / 100.0)
    }

    pub fn progress_rows(&self, total_rows: usize) -> usize {
        ((total_rows * usize::from(self.pane_size_percent) + 50) / 100)
            .clamp(1, total_rows.saturating_sub(3).max(1))
    }
}

pub fn parse_pane_size_choice(choice: &str, current: u8) -> Result<u8> {
    let size = match choice.trim() {
        "" => current,
        "1" => 10,
        "2" => 20,
        "3" => 30,
        "4" => 40,
        value => value.parse::<u8>()?,
    };
    ensure!(
        (10..=50).contains(&size),
        "pane size must be between 10 and 50 percent"
    );
    Ok(size)
}
pub fn load(root: &Path) -> Result<Settings> {
    ensure!(
        !root.join(".agent-progress").is_symlink(),
        "refusing symlink presentation directory"
    );
    let yaml = yaml_path(root);
    let path = if yaml.exists() {
        yaml.clone()
    } else {
        path(root)
    };
    if !path.exists() {
        return Ok(Settings::default());
    }
    let bytes = recovery::read(&path)?;
    ensure!(bytes.len() <= 65536, "presentation settings exceed 64 KiB");
    let value: Settings = if path == yaml {
        let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_slice(&bytes)?;
        value.apply_merge()?;
        serde_yaml_ng::from_value(value)?
    } else {
        serde_json::from_slice(&bytes)?
    };
    value.validate()?;
    Ok(value)
}

pub fn load_for_cwd(cwd: &Path) -> Result<Settings> {
    let project = crate::project::Project::discover(cwd)?;
    load(project.as_ref().map(|p| p.root()).unwrap_or(cwd))
}
pub fn save(root: &Path, value: &Settings) -> Result<()> {
    value.validate()?;
    ensure!(
        !root.join(".agent-progress").is_symlink(),
        "refusing symlink presentation directory"
    );
    fs::create_dir_all(root.join(".agent-progress"))?;
    if yaml_path(root).exists() {
        recovery::write(
            &yaml_path(root),
            serde_yaml_ng::to_string(value)?.as_bytes(),
            true,
        )
    } else {
        recovery::write(&path(root), &serde_json::to_vec_pretty(value)?, true)
    }
}

pub fn init_yaml(root: &Path) -> Result<()> {
    let value = load(root)?;
    ensure!(
        !yaml_path(root).exists(),
        "ui.yaml already exists; preserve it"
    );
    fs::create_dir_all(root.join(".agent-progress"))?;
    recovery::write(
        &yaml_path(root),
        serde_yaml_ng::to_string(&value)?.as_bytes(),
        false,
    )
}
