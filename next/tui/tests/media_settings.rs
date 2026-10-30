use expert_tui::{
    app::{App, FormKind},
    ui,
};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;

fn settings() -> App {
    let mut app = App::default();
    app.apply_snapshot(json!({"session":{"id":"media-session","status":"idle"},"messages":[],
      "settings":{"providers":[],"mcp":[],"media":{"imageGeneration":{"base_url":"https://synthetic.invalid/v1","model":"image","has_key":true,"image_response_format":"auto"}}}}));
    app.open_settings();
    app
}
#[test]
fn settings_exposes_all_media_configs_and_masked_keys_at_small_sizes() {
    for (key, kind) in [
        ("media.imageGeneration", "imageGeneration"),
        ("media.vision", "vision"),
        ("asr", "asr"),
        ("media.tts", "tts"),
    ] {
        let mut app = settings();
        let i = app
            .form
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .position(|f| f.key == key)
            .unwrap();
        app.click_form_field(i);
        assert_eq!(app.form.as_ref().unwrap().kind, FormKind::Media);
        assert_eq!(app.form.as_ref().unwrap().items[0]["kind"], kind);
        let form = app.form.as_mut().unwrap();
        form.fields
            .iter_mut()
            .find(|f| f.key == "api_key")
            .unwrap()
            .value = "SYNTHETIC_SECRET".into();
        for (w, h) in [(80, 24), (40, 24)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|f| ui::draw(f, &app)).unwrap();
            let output = ui::buffer_text(terminal.backend().buffer());
            assert!(!output.contains("SYNTHETIC_SECRET"));
        }
        let save = app
            .form
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .position(|f| f.key == "__save")
            .unwrap();
        let c = app.click_form_field(save).remove(0);
        assert_eq!(c.method, "media.save");
        assert_eq!(c.params["kind"], kind);
        assert_eq!(c.params["api_key"], "SYNTHETIC_SECRET");
        app.command_error(&c, "fixture failure");
        assert_eq!(
            app.form
                .as_ref()
                .unwrap()
                .fields
                .iter()
                .find(|f| f.key == "api_key")
                .unwrap()
                .value,
            "SYNTHETIC_SECRET"
        );
    }
}
#[test]
fn media_actions_open_config_locally_without_generating() {
    let mut app = settings();
    let commands = app.workbench_query("wb.media", json!({}));
    let query = &commands[0];
    app.command_response(query,&json!({"workbench":true,"title":"创作与媒体","fields":[
      {"key":"__config","label":"图片生成 API","kind":"imageGeneration","editable":false,"action":"client.media.settings"}],"data":{}}));
    let commands = app.click_form_field(0);
    assert!(commands.is_empty());
    assert_eq!(app.form.as_ref().unwrap().kind, FormKind::Media);
}
#[test]
fn late_media_job_poll_does_not_replace_a_settings_editor() {
    let mut app = settings();
    let c = app
        .workbench_query("wb.job", json!({"job_id":"job"}))
        .remove(0);
    app.edit_media("imageGeneration");
    app.command_response(&c,&json!({"workbench":true,"title":"late result","job_id":"job","done":true,"fields":[],"data":{}}));
    assert_eq!(app.form.as_ref().unwrap().kind, FormKind::Media);
}
