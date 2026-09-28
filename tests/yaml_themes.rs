use agent_progress::{
    dashboard::{self, View},
    live::{Snapshot, StepState},
    settings::{self, BackgroundState},
};
use ratatui::{Terminal, backend::TestBackend, style::Color};
use std::{fs, process::Command};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn yaml_merges_custom_presets_overrides_and_state_backgrounds() {
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join(".agent-progress")).unwrap();
    fs::write(settings::path(root.path()), "invalid older json").unwrap();
    fs::write(
        settings::yaml_path(root.path()),
        r##"
schema: 1
preset: my_ocean
background_brightness: 1.5
presets:
  my_ocean:
    extends: ocean
    theme: &colors
      fill: '#203040'
      accent: '#AACCEE'
theme:
  metadata: '#CCDDEE'
states:
  blocked:
    <<: *colors
    fill: '#401020'
  done:
    fill: '#104020'
"##,
    )
    .unwrap();
    let settings = settings::load(root.path()).unwrap();
    assert_eq!(settings.palette(None).unwrap().fill, Color::Rgb(48, 72, 96));
    assert_eq!(
        settings
            .palette(Some(BackgroundState::Blocked))
            .unwrap()
            .fill,
        Color::Rgb(96, 24, 48)
    );
    assert_eq!(
        settings.palette(Some(BackgroundState::Done)).unwrap().fill,
        Color::Rgb(24, 96, 48)
    );
    assert_eq!(
        settings.palette(None).unwrap().accent,
        Color::Rgb(170, 204, 238)
    );
    // Preset cycles fail closed; brightness NaN/out-of-range cannot enter renderer state.
    for text in [
        "preset: a\npresets:\n  a: {extends: b}\n  b: {extends: a}",
        "background_brightness: 0",
        "preset: missing",
        "states: {unknown: {fill: '#123456'}}",
    ] {
        fs::write(settings::yaml_path(root.path()), text).unwrap();
        assert!(settings::load(root.path()).is_err());
    }
}

#[test]
fn renderer_selects_background_from_observed_state_not_percentage() {
    let mut s = Snapshot::empty(Uuid::new_v4(), "/tmp/theme".into());
    s.apply_plan(
        vec![("Current task".into(), StepState::Active)],
        "test",
        "",
        "",
    )
    .unwrap();
    let mut settings = settings::Settings::default();
    settings.states.insert(
        BackgroundState::Working,
        settings::ThemePatch {
            track: Some("#123456".into()),
            ..Default::default()
        },
    );
    settings.states.insert(
        BackgroundState::Done,
        settings::ThemePatch {
            fill: Some("#654321".into()),
            ..Default::default()
        },
    );
    let mut view = View::default();
    for state in BackgroundState::ALL {
        view.state_palettes
            .insert(state, settings.palette(Some(state)).unwrap());
    }
    let mut terminal = Terminal::new(TestBackend::new(80, 5)).unwrap();
    terminal
        .draw(|f| dashboard::render(f, &s, &mut view))
        .unwrap();
    assert_eq!(
        terminal.backend().buffer()[(0, 0)].bg,
        Color::Rgb(18, 52, 86)
    );
    s.entries[0].state = StepState::Done;
    terminal
        .draw(|f| dashboard::render(f, &s, &mut view))
        .unwrap();
    assert_eq!(
        terminal.backend().buffer()[(0, 0)].bg,
        Color::Rgb(101, 67, 33)
    );
}

#[test]
fn standalone_config_yaml_and_file_cli_do_not_need_herdr() {
    let root = tempdir().unwrap();
    let ap = env!("CARGO_BIN_EXE_ap");
    let run = |args: &[&str]| {
        Command::new(ap)
            .args(args)
            .current_dir(root.path())
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("HERDR_ENV", "0")
            .output()
            .unwrap()
    };
    assert!(
        run(&["config", "set", "--preset", "forest", "--brightness", "1.2"])
            .status
            .success()
    );
    assert!(run(&["config", "init-yaml"]).status.success());
    assert!(settings::yaml_path(root.path()).exists());
    assert!(
        run(&["config", "set", "--preset", "ocean"])
            .status
            .success()
    );
    assert_eq!(settings::load(root.path()).unwrap().preset, "ocean");
    assert!(!run(&["config", "init-yaml"]).status.success());
    assert!(
        run(&[
            "--file",
            "plan.md",
            "init",
            "--project",
            "standalone",
            "--goal",
            "No Herdr needed"
        ])
        .status
        .success()
    );
    assert!(run(&["--file", "plan.md", "show"]).status.success());
}
