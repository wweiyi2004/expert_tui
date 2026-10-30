use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use expert_tui::app::App;

#[test]
fn update_and_version_are_local_and_do_not_send_host_commands() {
    let mut app = App::default();
    for (text, install) in [
        ("/update check", false),
        ("/update", true),
        ("/update install", true),
    ] {
        app.drawer = None;
        app.update_busy = false;
        app.input = text.into();
        app.cursor = text.len();
        let commands = app.key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            std::time::Instant::now(),
            100,
        );
        assert!(commands.is_empty());
        assert_eq!(app.update_requested.take(), Some(install));
        assert!(app.update_busy);
        assert!(!app.exit);
    }
    app.drawer = None;
    app.input = "/version".into();
    app.cursor = app.input.len();
    assert!(
        app.key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            std::time::Instant::now(),
            100
        )
        .is_empty()
    );
    assert!(
        app.info_lines
            .iter()
            .any(|line| line.contains(env!("CARGO_PKG_VERSION")))
    );
    assert!(!app.exit);
}
