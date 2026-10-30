use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use expert_tui::{app::App, interaction, ui};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;
use std::time::Instant;

fn app() -> App {
    let mut app = App::default();
    app.apply_snapshot(json!({"session":{"id":"s","status":"idle"},"messages":[{"id":"a","role":"assistant","text":"修改完成"}],"models":[{"id":"one","choice":"provider/one","label":"模型一","source":"本机","efforts":["low","high"]}]}));
    app.viewport = (80, 24);
    app.interaction_tick();
    app
}
fn key(a: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Vec<expert_tui::app::Command> {
    a.key(KeyEvent::new(code, modifiers), Instant::now(), 80)
}

#[test]
fn predicted_text_is_only_an_editable_draft_and_stale_receipts_are_ignored() {
    let mut a = app();
    a.snapshot["session"]["status"] = json!("running");
    a.interaction_tick();
    a.snapshot["session"]["status"] = json!("idle");
    a.interaction_tick();
    let c = a.outbox.pop().unwrap();
    assert_eq!(c.params["action"], "ux.predict");
    a.command_response(&c, &json!({"ux":"prediction","text":"运行测试"}));
    assert!(a.input.is_empty());
    assert!(key(&mut a, KeyCode::Tab, KeyModifiers::NONE).is_empty());
    assert_eq!(a.input, "运行测试");
    a.input = "自己的草稿".into();
    a.command_response(&c, &json!({"ux":"prediction","text":"不能覆盖"}));
    assert_eq!(a.input, "自己的草稿");
    a.input.clear();
    a.snapshot["session"]["id"] = json!("other");
    a.command_response(&c, &json!({"ux":"prediction","text":"旧会话"}));
    assert!(a.interaction.prediction.is_none());
}
#[test]
fn typing_dismisses_prediction_and_disabling_does_not_issue_a_request() {
    let mut a = app();
    a.interaction.prediction = Some("预测".into());
    key(&mut a, KeyCode::Char('新'), KeyModifiers::NONE);
    assert!(a.interaction.prediction.is_none());
    assert_eq!(a.input, "新");
    a.interaction_command("/suggest", "off");
    a.snapshot["session"]["status"] = json!("running");
    a.interaction_tick();
    a.snapshot["session"]["status"] = json!("idle");
    a.interaction_tick();
    assert!(a.outbox.is_empty());
}
#[test]
fn main_composer_undo_redo_and_stash_keep_text_and_attachments() {
    let mut a = app();
    key(&mut a, KeyCode::Char('甲'), KeyModifiers::NONE);
    a.interaction_tick();
    key(&mut a, KeyCode::Char('乙'), KeyModifiers::NONE);
    a.interaction_tick();
    key(&mut a, KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(a.input, "甲");
    key(&mut a, KeyCode::Char('y'), KeyModifiers::CONTROL);
    assert_eq!(a.input, "甲乙");
    a.insert_attachment_mark(
        json!({"id":"image","token":"scoped-token","kind":"image","_tag":"[Image 1]"}),
    );
    let expected = a.input.clone();
    key(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(a.input.is_empty());
    assert!(a.attachments.is_empty());
    a.interaction_command("/drafts", "");
    key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.input, expected);
    assert_eq!(a.attachments[0]["token"], "scoped-token");
}
#[test]
fn palette_search_preserves_draft_and_uses_existing_command_paths() {
    let mut a = app();
    a.input = "保留草稿".into();
    a.cursor = a.input.len();
    key(&mut a, KeyCode::F(1), KeyModifiers::NONE);
    a.paste("思考");
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| {
        ui::draw(f, &a);
        interaction::draw(f, &a);
    })
    .unwrap();
    assert!(ui::buffer_text(term.backend().buffer()).contains("思考"));
    key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.input, "保留草稿");
    assert!(a.drawer.is_some() || a.form.is_some());
}
#[test]
fn model_favorites_persist_and_selection_is_not_an_optimistic_change() {
    let mut a = app();
    a.open_quick_models();
    let menu = a.interaction.menu.as_mut().unwrap();
    menu.query = "模型一".into();
    key(&mut a, KeyCode::F(2), KeyModifiers::NONE);
    assert!(a.interaction.favorites.contains(&"provider/one".to_owned()));
    let loaded = interaction::Interaction::load(&a.interaction.preferences());
    assert_eq!(loaded.favorites, a.interaction.favorites);
    a.interaction.menu.as_mut().unwrap().query = "模型一".into();
    let commands = key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(commands[0].method, "model.set");
    assert!(a.snapshot["session"]["model_choice"].is_null());
}
#[test]
fn next_task_queue_is_separate_from_steering_and_waits_for_receipt() {
    let mut a = app();
    a.snapshot["session"]["status"] = json!("running");
    a.input = "之后再检查文档".into();
    a.cursor = a.input.len();
    let commands = key(&mut a, KeyCode::Enter, KeyModifiers::ALT);
    assert_eq!(commands[0].params["action"], "ux.followup.add");
    assert_eq!(a.input, "之后再检查文档");
    a.command_response(
        &commands[0],
        &json!({"ux":"queued","accepted":true,"id":"q"}),
    );
    assert!(a.input.is_empty());
    a.input = "当前不要改数据库".into();
    a.cursor = a.input.len();
    let commands = key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(commands[0].method, "queue.add");
}
#[test]
fn queued_message_failure_keeps_draft_and_queue_edit_does_not_overwrite_new_text() {
    let mut a = app();
    a.snapshot["session"]["status"] = json!("running");
    a.input = "排队内容".into();
    let commands = key(&mut a, KeyCode::Enter, KeyModifiers::ALT);
    a.command_error(&commands[0], "队列不可用");
    assert_eq!(a.input, "排队内容");
    a.interaction_command("/queue", "");
    let c = a.command("terminal.query", json!({"topic":"ux.queue"}));
    a.command_response(
        &c,
        &json!({"ux":"queue","entries":[{"id":"q","kind":"next_task","text":"旧内容"}]}),
    );
    let commands = key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(commands[0].params["action"], "ux.send_now");
}
#[test]
fn code_toolbar_resolves_source_bytes_under_scroll_and_narrow_windows() {
    for (w, h) in [(40, 16), (80, 24), (160, 32)] {
        let mut a = app();
        a.apply_snapshot(json!({"session":{"id":"s","status":"idle"},"messages":[{"id":"old","role":"user","text":"old\n".repeat(40)},{"id":"code","role":"assistant","text":"```python\n\tprint('甲')\n```\n"}]}));
        ui::focus_message(&mut a, "code");
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| ui::draw(f, &a)).unwrap();
        let mut found = false;
        for y in 0..h {
            for x in 0..w {
                if let Some((save, language, source)) = ui::code_action_hit(&a, w, h, x, y) {
                    assert_eq!(language, "python");
                    assert_eq!(source, "\tprint('甲')\n");
                    if !save {
                        found = true;
                    }
                }
            }
        }
        assert!(found, "{w}x{h}");
    }
}
#[test]
fn image_save_is_explicit_and_cannot_overwrite_an_existing_file() {
    let mut a = app();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("result.txt");
    std::fs::write(&path, b"original").unwrap();
    a.save_code_block("txt", "replacement");
    a.interaction.menu.as_mut().unwrap().query = path.to_string_lossy().into();
    key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    assert!(a.interaction.menu.is_some());
    let new = dir.path().join("new.txt");
    a.interaction.menu.as_mut().unwrap().query = new.to_string_lossy().into();
    key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(std::fs::read(new).unwrap(), b"replacement");
    assert!(a.interaction.menu.is_none());
}
#[test]
fn changing_session_closes_stale_dialog_and_modal_keys_do_not_change_the_composer() {
    let mut a = app();
    a.input = "草稿".into();
    a.open_command_palette();
    a.snapshot["session"]["id"] = json!("other");
    key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert!(a.interaction.menu.is_none());
    assert_eq!(a.input, "草稿");
    a.open_command_palette();
    a.paste("功能");
    assert_eq!(a.input, "草稿");
}

#[test]
fn stale_prediction_does_not_consume_the_newer_pending_request() {
    let mut a = app();
    a.snapshot["session"]["status"] = json!("running");
    a.interaction_tick();
    a.snapshot["session"]["status"] = json!("idle");
    a.interaction_tick();
    let command = a.outbox.pop().unwrap();
    let mut stale = command.clone();
    stale.params["prediction_generation"] = json!(9999);
    a.command_response(&stale, &json!({"ux":"prediction","text":"旧预测"}));
    assert!(a.interaction.prediction.is_none());
    a.command_response(&command, &json!({"ux":"prediction","text":"新的有效预测"}));
    assert_eq!(a.interaction.prediction.as_deref(), Some("新的有效预测"));
}
#[test]
fn task_center_switches_to_the_selected_session_and_supports_type_filters() {
    let mut a = app();
    let command = a.command("terminal.query", json!({"topic":"ux.tasks"}));
    a.command_response(&command,&json!({"ux":"tasks","entries":[{"kind":"session","id":"other","title":"另一个会话","status":"running"}]}));
    key(&mut a, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(a.interaction.menu.as_ref().unwrap().category, "session");
    let commands = key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(commands[0].method, "session.open");
    assert_eq!(commands[0].params["session_id"], "other");
}
#[test]
fn question_and_background_session_events_request_notifications_once() {
    let mut a = app();
    a.snapshot["sessions"] = json!([{"id":"background","title":"后台任务","status":"running"}]);
    a.interaction_tick();
    assert!(a.interaction.notify_requested.is_none());
    a.snapshot["sessions"][0]["status"] = json!("completed");
    a.interaction_tick();
    assert!(
        a.interaction
            .notify_requested
            .take()
            .unwrap()
            .contains("后台任务")
    );
    a.interaction_tick();
    assert!(a.interaction.notify_requested.is_none());
    a.snapshot["questions"] = json!([{"id":"question"}]);
    a.interaction_tick();
    assert!(
        a.interaction
            .notify_requested
            .take()
            .unwrap()
            .contains("需要回答")
    );
    a.interaction_tick();
    assert!(a.interaction.notify_requested.is_none());
}

#[test]
fn convenience_popups_fit_narrow_and_double_width_terminals() {
    for (w, h) in [(40, 12), (80, 24), (160, 40)] {
        for ambiguous in [1, 2] {
            let mut a = app();
            a.ambiguous_width = ambiguous;
            a.viewport = (w, h);
            a.open_command_palette();
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| {
                ui::draw(f, &a);
                interaction::draw(f, &a);
            })
            .unwrap();
            for row in ui::buffer_text(term.backend().buffer()).lines() {
                assert!(ui::width(row, ambiguous) <= w as usize, "{w}x{h}: {row}");
            }
        }
    }
}

#[test]
fn previous_model_includes_the_initial_default_and_keeps_source_identity() {
    let mut a=app();a.snapshot["session"]["model"]=json!("one");a.snapshot["session"]["model_source_id"]=json!("p");
    a.snapshot["models"][0]["source_id"]=json!("p");
    let c=a.command("model.set",json!({"choice":"other/two"}));a.command_response(&c,&json!({"accepted":true}));
    assert_eq!(a.interaction.recent_models,["other/two","provider/one"]);
}
