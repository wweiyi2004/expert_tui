//! Image decoding and protocol encoding are bounded background work. The main
//! thread only places a prepared protocol in a modal over the existing UI.
use crate::app::{App, text};
use anyhow::{Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use image::{DynamicImage, ImageReader, Limits};
use ratatui::{
    Frame,
    layout::{Rect, Size},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};
use ratatui_image::{Image, Resize, picker::Picker, protocol::Protocol};
use serde_json::Value;
use std::{
    io::Cursor,
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
};

const MAX_BYTES: usize = 5 * 1024 * 1024;
const MAX_PIXELS: u64 = 16 * 1024 * 1024;

/// Explicit CLI file selection uses the same bounded payload as the host.
pub fn file_payload(path: &std::path::Path) -> Result<Value> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_BYTES,
        "图片为空或超过 5 MiB 预览限制"
    );
    Ok(
        serde_json::json!({"image_preview":true,"name":path.file_name().unwrap_or_default().to_string_lossy(),"data":STANDARD.encode(bytes)}),
    )
}

pub struct Preview {
    pub id: u64,
    pub name: String,
    pub maximized: bool,
    pub dimensions: Option<(u32, u32)>,
    pub status: String,
    pub protocol_name: String,
    data: Arc<str>,
    last_requested: Option<Size>,
    rendered_size: Option<Size>,
    protocol: Option<Protocol>,
}
impl std::fmt::Debug for Preview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImagePreview")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("dimensions", &self.dimensions)
            .field("status", &self.status)
            .finish()
    }
}
pub fn popup_rect(width: u16, height: u16, maximized: bool) -> Rect {
    let w = if maximized {
        width.saturating_sub(2)
    } else {
        (u32::from(width) * 3 / 4).min(86) as u16
    }
    .min(width);
    let h = if maximized {
        height.saturating_sub(2)
    } else {
        (u32::from(height) * 3 / 4).min(32) as u16
    }
    .min(height);
    Rect::new((width - w) / 2, (height - h) / 2, w, h)
}
pub fn content_rect(width: u16, height: u16, maximized: bool) -> Rect {
    let r = popup_rect(width, height, maximized);
    Rect::new(
        r.x.saturating_add(1),
        r.y.saturating_add(2),
        r.width.saturating_sub(2),
        r.height.saturating_sub(4),
    )
}

impl App {
    pub fn open_image_preview(&mut self, result: &Value) {
        self.interaction.preview_payload = result.clone();
        self.interaction.preview_target = None;
        self.image_preview_generation = self.image_preview_generation.wrapping_add(1);
        let data = text(result, "data");
        let valid = !data.is_empty() && data.len() <= MAX_BYTES.div_ceil(3) * 4;
        self.image_preview = Some(Preview {
            id: self.image_preview_generation,
            name: crate::ui::clean(text(result, "name")),
            maximized: false,
            dimensions: None,
            status: if valid {
                "正在读取图片…"
            } else {
                "图片为空或超过 5 MiB 预览限制"
            }
            .into(),
            protocol_name: String::new(),
            data: if valid {
                Arc::from(data)
            } else {
                Arc::from("")
            },
            last_requested: None,
            rendered_size: None,
            protocol: None,
        });
        self.selection = None;
        self.redraw_requested = true;
    }
    pub fn close_image_preview(&mut self) {
        self.image_preview = None;
        self.redraw_requested = true;
    }
    pub fn image_preview_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => self.close_image_preview(),
            KeyCode::Char('f' | 'F') => {
                if let Some(p) = self.image_preview.as_mut() {
                    p.maximized = !p.maximized;
                }
                self.redraw_requested = true;
            }
            _ => {}
        }
    }
    pub fn image_preview_mouse(&mut self, mouse: MouseEvent) {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let Some(p) = self.image_preview.as_ref() else {
                return;
            };
            let r = popup_rect(self.viewport.0, self.viewport.1, p.maximized);
            if !r.contains((mouse.column, mouse.row).into())
                || mouse.row == r.y && mouse.column >= r.right().saturating_sub(5)
            {
                self.close_image_preview();
            }
        }
    }
}

enum Work {
    Render { id: u64, data: Arc<str>, size: Size },
    Clear,
}
struct Rendered {
    id: u64,
    size: Size,
    result: Result<(Protocol, (u32, u32))>,
}
pub struct Renderer {
    input: SyncSender<Work>,
    output: Receiver<Rendered>,
    active: Option<u64>,
    protocol_name: String,
}

fn decode(data: &str) -> Result<DynamicImage> {
    let bytes = STANDARD.decode(data)?;
    ensure!(bytes.len() <= MAX_BYTES, "图片超过 5 MiB 预览限制");
    let (w, h) = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()?
        .into_dimensions()?;
    ensure!(
        w > 0 && h > 0 && u64::from(w) * u64::from(h) <= MAX_PIXELS,
        "图片超过 1600 万像素预览限制"
    );
    let mut reader = ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?)
}
impl Renderer {
    pub fn new(mut picker: Picker) -> Self {
        // Flatten transparent pixels onto the same background as the popup.
        picker.set_background_color(image::Rgba([18, 22, 30, 255]));
        let protocol_name = format!("{:?}", picker.protocol_type());
        let (tx, input) = mpsc::sync_channel(1);
        let (output, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut cached: Option<(u64, DynamicImage)> = None;
            while let Ok(work) = input.recv() {
                let Work::Render { id, data, size } = work else {
                    cached = None;
                    continue;
                };
                let result = (|| {
                    if cached.as_ref().is_none_or(|(old, _)| *old != id) {
                        cached = Some((id, decode(&data)?));
                    }
                    let image = &cached.as_ref().unwrap().1;
                    let dimensions = (image.width(), image.height());
                    Ok((
                        picker.new_protocol(
                            image.clone(),
                            Rect::new(0, 0, size.width, size.height),
                            Resize::Fit(None),
                        )?,
                        dimensions,
                    ))
                })();
                if output.send(Rendered { id, size, result }).is_err() {
                    break;
                }
            }
        });
        Self {
            input: tx,
            output: rx,
            active: None,
            protocol_name,
        }
    }
    /// Called by the ordinary event loop. Old image/resize responses can never
    /// reopen a closed popup or replace a newer image.
    pub fn update(&mut self, app: &mut App, width: u16, height: u16) -> bool {
        let mut changed = false;
        while let Ok(frame) = self.output.try_recv() {
            if let Some(p) = app.image_preview.as_mut() {
                let r = content_rect(width, height, p.maximized);
                if p.id == frame.id && frame.size == r.as_size() {
                    match frame.result {
                        Ok((protocol, dimensions)) => {
                            p.protocol = Some(protocol);
                            p.dimensions = Some(dimensions);
                            p.status.clear();
                            p.rendered_size = Some(frame.size);
                        }
                        Err(error) => {
                            p.status = format!(
                                "无法预览图片：{}",
                                error.to_string().chars().take(160).collect::<String>()
                            );
                            p.protocol = None;
                        }
                    }
                    changed = true;
                }
            }
        }
        let Some(p) = app.image_preview.as_mut() else {
            if self.active.is_some() && self.input.try_send(Work::Clear).is_ok() {
                self.active = None;
            }
            return changed;
        };
        let size = content_rect(width, height, p.maximized).as_size();
        if !p.data.is_empty() && size.width > 0 && size.height > 0 && p.last_requested != Some(size)
        {
            match self.input.try_send(Work::Render {
                id: p.id,
                data: p.data.clone(),
                size,
            }) {
                Ok(()) => {
                    self.active = Some(p.id);
                    p.last_requested = Some(size);
                    p.protocol_name = self.protocol_name.clone();
                    p.status = "正在渲染图片…".into();
                    changed = true;
                }
                Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => {
                    p.status = "图片预览线程已退出，请重新启动终端".into();
                    p.last_requested = Some(size);
                    changed = true;
                }
            }
        }
        changed
    }
}

/// Draw last, after skin, selection, and link processing. No resampling, file
/// IO, or capability queries are performed in the frame renderer.
pub fn draw(frame: &mut Frame, app: &App) {
    let Some(p) = app.image_preview.as_ref() else {
        return;
    };
    let r = popup_rect(frame.area().width, frame.area().height, p.maximized);
    if r.width < 8 || r.height < 5 {
        return;
    }
    let style = Style::default()
        .bg(Color::Rgb(18, 22, 30))
        .fg(Color::Rgb(222, 226, 234));
    frame.render_widget(Clear, r);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(" 图片预览 · S 保存 · C 复制 · R 作为参考图 ")
            .style(style),
        r,
    );
    frame.render_widget(
        Paragraph::new("[×]").style(style),
        Rect::new(r.right() - 4, r.y, 3, 1),
    );
    let meta = match p.dimensions {
        Some((w, h)) => format!("{} · {w}×{h} · {}", p.name, p.protocol_name),
        None => p.name.clone(),
    };
    frame.render_widget(
        Paragraph::new(meta).style(style),
        Rect::new(r.x + 1, r.y + 1, r.width - 2, 1),
    );
    let area = content_rect(frame.area().width, frame.area().height, p.maximized);
    if let Some(protocol) = p
        .protocol
        .as_ref()
        .filter(|_| p.rendered_size == Some(area.as_size()))
    {
        let size = protocol.area();
        let centered = Rect::new(
            area.x + (area.width.saturating_sub(size.width)) / 2,
            area.y + (area.height.saturating_sub(size.height)) / 2,
            size.width.min(area.width),
            size.height.min(area.height),
        );
        frame.render_widget(Image::new(protocol), centered);
    } else {
        frame.render_widget(Paragraph::new(p.status.clone()).style(style), area);
    }
    frame.render_widget(
        Paragraph::new("Esc / Enter 关闭 · F 放大/还原").style(style),
        Rect::new(r.x + 1, r.bottom() - 2, r.width - 2, 1),
    );
}
