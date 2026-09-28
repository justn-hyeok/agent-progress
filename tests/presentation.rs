use agent_progress::{
    dashboard::{self, View},
    live::Snapshot,
    settings,
};
use ratatui::{Terminal, backend::TestBackend, style::Color};
use std::{fs, process::Command};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn colors_are_validated_before_atomic_settings_change_and_reset_recovers() {
    let dir = tempdir().unwrap();
    let ap = env!("CARGO_BIN_EXE_ap");
    let run = |args: &[&str]| {
        Command::new(ap)
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap()
    };
    let defaults = run(&["config", "show"]);
    assert!(defaults.status.success());
    assert!(!dir.path().join(".agent-progress").exists());
    assert!(
        run(&[
            "config",
            "set",
            "--accent",
            "#FF8899",
            "--fill",
            "#224455",
            "--position",
            "above",
            "--auto-open",
            "false"
        ])
        .status
        .success()
    );
    let settings = settings::load(dir.path()).unwrap();
    assert_eq!(settings.position, settings::Position::Above);
    assert!(!settings.auto_open);
    let before = fs::read(settings::path(dir.path())).unwrap();
    assert!(
        !run(&["config", "set", "--accent", "red", "--fill", "#123456"])
            .status
            .success()
    );
    assert_eq!(fs::read(settings::path(dir.path())).unwrap(), before);
    fs::write(settings::path(dir.path()), "broken").unwrap();
    assert!(run(&["config", "reset"]).status.success());
    assert_eq!(
        settings::load(dir.path()).unwrap(),
        settings::Settings::default()
    );
    for value in ["#12345", "#1234567", "#12GG56", "#한글글", "\x1b[31m"] {
        assert!(settings::color(value).is_err());
    }
}

#[test]
fn custom_palette_changes_visible_cells_without_changing_snapshot() {
    let snapshot = Snapshot::empty(Uuid::nil(), "/tmp/presentation".into());
    let before = serde_json::to_vec(&snapshot).unwrap();
    let theme = settings::Theme {
        track: "#112233".into(),
        accent: "#FF8899".into(),
        ..Default::default()
    };
    let mut view = View::default();
    view.palette = theme.palette().unwrap();
    let mut terminal = Terminal::new(TestBackend::new(80, 5)).unwrap();
    terminal
        .draw(|f| dashboard::render(f, &snapshot, &mut view))
        .unwrap();
    use unicode_width::UnicodeWidthStr;
    for y in 0..5 {
        let mut x = 0;
        while x < 80 {
            let cell = &terminal.backend().buffer()[(x, y)];
            assert_eq!(cell.bg, Color::Rgb(17, 34, 51));
            x += cell.symbol().width().max(1) as u16;
        }
    }
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|cell| cell.fg == Color::Rgb(255, 136, 153))
    );
    assert_eq!(serde_json::to_vec(&snapshot).unwrap(), before);
}
