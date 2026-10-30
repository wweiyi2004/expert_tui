use base64::{Engine as _, engine::general_purpose::STANDARD};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use expert_tui::{
    app::App,
    image_preview::{self, Renderer},
    ui,
};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use ratatui::{Terminal, backend::TestBackend};
use ratatui_image::picker::Picker;
use serde_json::json;
use std::{
    io::Cursor,
    thread,
    time::{Duration, Instant},
};

fn payload() -> serde_json::Value {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_fn(64, 64, |x, _| {
        if x < 32 {
            Rgba([0, 120, 255, 255])
        } else {
            Rgba([255, 60, 30, 255])
        }
    }));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).unwrap();
    json!({"image_preview":true,"name":"fixture.png","data":STANDARD.encode(bytes.into_inner())})
}
fn wait_for(renderer: &mut Renderer, app: &mut App, w: u16, h: u16) {
    for _ in 0..300 {
        renderer.update(app, w, h);
        if app.image_preview.as_ref().unwrap().status.is_empty()
            || app
                .image_preview
                .as_ref()
                .unwrap()
                .status
                .contains("无法预览")
        {
            return;
        }
        thread::sleep(Duration::from_millis(2));
    }
    panic!("image worker did not finish: {:?}", app.image_preview);
}
#[test]
fn preview_renders_image_inside_small_modal_and_preserves_parent_and_draft() {
    for (w, h) in [(120, 40), (80, 24), (40, 16)] {
        let mut app = App::default();
        app.input = "keep my draft".into();
        app.cursor = app.input.len();
        app.open_settings();
        let fields = app.form.as_ref().unwrap().fields.len();
        app.open_image_preview(&payload());
        let mut renderer = Renderer::new(Picker::halfblocks());
        wait_for(&mut renderer, &mut app, w, h);
        assert_eq!(
            app.image_preview.as_ref().unwrap().dimensions,
            Some((64, 64))
        );
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|f| {
                ui::draw(f, &app);
                image_preview::draw(f, &app)
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(ui::buffer_text(buffer).contains("图片预览"));
        assert!(
            buffer.content.iter().any(|cell| matches!(
                cell.bg,
                ratatui::style::Color::Rgb(0, 120, 255) | ratatui::style::Color::Rgb(255, 60, 30)
            )),
            "image pixels must be rendered"
        );
        app.paste("must not enter background");
        app.key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            Instant::now(),
            w,
        );
        assert_eq!(app.input, "keep my draft");
        app.key(KeyCode::Esc.into(), Instant::now(), w);
        assert!(app.image_preview.is_none());
        assert_eq!(app.form.as_ref().unwrap().fields.len(), fields);
        assert!(app.redraw_requested);
    }
}
#[test]
fn rapid_resize_reopen_and_closed_popup_ignore_stale_worker_results() {
    let mut app = App::default();
    let mut renderer = Renderer::new(Picker::halfblocks());
    app.open_image_preview(&payload());
    renderer.update(&mut app, 80, 24);
    app.close_image_preview();
    renderer.update(&mut app, 80, 24);
    app.open_image_preview(&payload());
    let id = app.image_preview.as_ref().unwrap().id;
    for (w, h) in [(120, 40), (60, 20), (40, 16), (80, 24)] {
        renderer.update(&mut app, w, h);
    }
    wait_for(&mut renderer, &mut app, 80, 24);
    assert_eq!(app.image_preview.as_ref().unwrap().id, id);
    app.image_preview_key(KeyCode::Char('f').into());
    wait_for(&mut renderer, &mut app, 80, 24);
    assert!(app.image_preview.as_ref().unwrap().maximized);
    app.close_image_preview();
    for _ in 0..10 {
        renderer.update(&mut app, 80, 24);
        thread::sleep(Duration::from_millis(2));
    }
    assert!(app.image_preview.is_none());
}
#[test]
fn invalid_image_is_a_popup_error_and_rectangles_always_fit() {
    let mut app = App::default();
    app.open_image_preview(&json!({"name":"broken.png","data":STANDARD.encode(b"not an image")}));
    let mut renderer = Renderer::new(Picker::halfblocks());
    wait_for(&mut renderer, &mut app, 80, 24);
    assert!(
        app.image_preview
            .as_ref()
            .unwrap()
            .status
            .contains("无法预览图片")
    );
    for (w, h) in [(0, 0), (4, 3), (40, 16), (65535, 65535)] {
        for max in [true, false] {
            let r = image_preview::popup_rect(w, h, max);
            assert!(r.right() <= w && r.bottom() <= h);
        }
    }
}
#[test]
fn workbench_image_receipt_does_not_replace_or_repoll_the_parent_form() {
    let mut app = App::default();
    app.snapshot = json!({"session":{"id":"s"}});
    let c = app.workbench_query("wb.media", json!({})).remove(0);
    app.command_response(
        &c,
        &json!({"workbench":true,"title":"image result","fields":[],"data":{}}),
    );
    app.wb_job = Some("old".into());
    let c = app.command(
        "terminal.action",
        json!({"action":"wb.media.preview","_workbench":true}),
    );
    app.command_response(&c, &payload());
    assert!(app.image_preview.is_some());
    assert!(app.wb_job.is_none());
    assert_eq!(app.form.as_ref().unwrap().title, "image result");
}

#[test]
fn generated_image_file_rows_are_clickable_and_scoped_after_scroll_and_resize() {
    for (w, h) in [(40, 16), (80, 24), (160, 32)] {
        let mut app = App::default();
        app.apply_snapshot(json!({"session":{"id":"images","status":"done"},
            "messages":[
                {"id":"question","role":"user","text":"draw a character"},
                {"id":"old","role":"assistant","text":"older output\n".repeat(50),
                 "attachments":[{"id":"old-image","mime":"image/png","name":"result.png","size":10}]},
                {"id":"answer","role":"assistant","text":"result.png is ready",
                 "attachments":[
                     {"id":"first","mime":"image/png","name":"result.png","size":20},
                     {"id":"document","mime":"application/pdf","name":"notes.pdf","size":30},
                     {"id":"second","mime":"image/png","name":"result.png","size":40}]}],"steps":[]}));
        app.viewport = (w, h);
        ui::focus_message(&mut app, "answer");
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| ui::draw(f, &app)).unwrap();
        let frame = terminal.backend().buffer();
        let mut hits = vec![];
        for y in 0..h {
            if let Some(hit) = ui::image_message_hit(&app, frame, 5, y) {
                assert!(ui::image_message_hit(&app, frame, w - 1, y).is_none());
                if hit.0 == "answer" {
                    hits.push(hit.1);
                } else {
                    assert_eq!(hit, ("old".into(), "old-image".into()));
                }
            }
        }
        assert_eq!(hits, ["first", "second"], "viewport {w}x{h}");
        app.open_settings();
        assert!((0..h).all(|y| ui::image_message_hit(&app, frame, 5, y).is_none()));
    }
}

#[test]
fn message_and_draft_image_receipts_open_modal_without_changing_draft() {
    for action in ["attachment.message.preview", "attachment.preview"] {
        let mut app = App::default();
        app.snapshot = json!({"session":{"id":"images"}});
        app.input = "keep this draft".into();
        let command = app.command("terminal.action", json!({"action":action}));
        app.command_response(&command, &payload());
        assert!(app.image_preview.is_some());
        assert!(app.form.is_none());
        assert_eq!(app.input, "keep this draft");
        app.close_image_preview();
        app.snapshot["session"]["id"] = json!("other-session");
        app.command_response(&command, &payload());
        assert!(app.image_preview.is_none(), "late receipt must not open in another session");
    }
}
