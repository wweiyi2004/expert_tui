use expert_tui::app::App;
use serde_json::json;

#[test]
fn project_and_check_commands_open_read_only_panels_without_running_commands() {
    let mut app = App::default();
    app.binding_ready = true;
    app.apply_snapshot(json!({"session":{"id":"s","mode":"work","status":"idle"},"messages":[],"steps":[],"models":[],"changes":[],"agents":[]}));
    for (command, topic) in [("/project", "wb.project"), ("/check", "wb.check")] {
        let commands = app.extra_command(command, "");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].method, "terminal.query");
        assert_eq!(commands[0].params["topic"], topic);
        assert!(commands[0].params.get("action").is_none());
    }
}

#[test]
fn project_save_receipt_preserves_notes_typed_while_waiting() {
    let mut app = App::default();
    app.binding_ready = true;
    app.apply_snapshot(json!({"session":{"id":"s","mode":"work","status":"idle"},"messages":[],"steps":[],"models":[],"changes":[],"agents":[]}));
    let open = app.extra_command("/project", "").remove(0);
    let panel = json!({"workbench":true,"title":"Project notes","fields":[{"key":"summary","label":"Progress","value":"original","editable":true}],"data":{"expected_revision":0}});
    app.command_response(&open, &panel);
    let mut save = app
        .workbench_query("wb.project", json!({"summary":"submitted"}))
        .remove(0);
    save.method = "terminal.action".into();
    save.params["action"] = json!("wb.project.save");
    app.form.as_mut().unwrap().fields[0].value = "new edits".into();
    let mut receipt = panel;
    receipt["fields"][0]["value"] = json!("submitted");
    receipt["data"]["expected_revision"] = json!(1);
    app.command_response(&save, &receipt);
    assert_eq!(app.form.as_ref().unwrap().fields[0].value, "new edits");
    assert_eq!(
        app.form.as_ref().unwrap().items[0]["data"]["expected_revision"],
        1
    );
    app.form = None;
    app.command_response(&save, &receipt);
    assert!(app.form.is_none());
}
