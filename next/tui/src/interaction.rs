//! Convenience actions share the ordinary command, session and receipt paths.
//! Suggestions are drafts; queued follow-ups are owned and dispatched by Host.
use crate::app::{App, Command, Draft, Drawer, View, items, text};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct Entry {
    pub label: String,
    pub detail: String,
    pub action: String,
    pub data: Value,
}
impl Entry {
    fn new(label: impl Into<String>, detail: impl Into<String>, action: &str, data: Value) -> Self {
        Self {
            label: crate::ui::clean(&label.into()).replace(['\n', '\r', '\t'], " "),
            detail: crate::ui::clean(&detail.into()).replace(['\n', '\r', '\t'], " "),
            action: action.into(),
            data,
        }
    }
}
#[derive(Debug)]
pub struct Menu {
    pub category: String,
    pub title: String,
    pub query: String,
    pub selected: usize,
    pub entries: Vec<Entry>,
    pub kind: String,
}
impl Menu {
    fn filtered(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| self.category.is_empty() || e.detail.starts_with(&self.category))
            .filter(|e| {
                self.query.split_whitespace().all(|q| {
                    format!("{} {}", e.label, e.detail)
                        .to_lowercase()
                        .contains(&q.to_lowercase())
                })
            })
            .collect()
    }
}
#[derive(Default, Debug)]
struct History {
    undo: Vec<Draft>,
    redo: Vec<Draft>,
    last: Option<Draft>,
}
#[derive(Debug)]
pub struct Interaction {
    pub menu: Option<Menu>,
    pub predictions: bool,
    pub notifications: bool,
    pub focused: bool,
    pub favorites: Vec<String>,
    pub recent_models: Vec<String>,
    pub recent_commands: Vec<String>,
    pub prediction: Option<String>,
    prediction_request: Option<(String, String, u64)>,
    generation: u64,
    seen_session: String,
    seen_status: String,
    seen_question: String,
    session_states: HashMap<String, String>,
    deferred_route: Option<Value>,
    histories: HashMap<String, History>,
    stashes: Vec<(String, Draft)>,
    pub preferences_dirty: bool,
    pub notify_requested: Option<String>,
    pub open_requested: Option<String>,
    pub image_copy_requested: Option<String>,
    pub preview_payload: Value,
    pub preview_target: Option<(String, String)>,
    save_data: Vec<u8>,
    menu_session: String,
    pending_edit: Option<Value>,
    last_prediction_at: Option<Instant>,
    pub last_error: String,
}
impl Default for Interaction {
    fn default() -> Self {
        Self {
            menu: None,
            predictions: true,
            notifications: true,
            focused: true,
            favorites: vec![],
            recent_models: vec![],
            recent_commands: vec![],
            prediction: None,
            prediction_request: None,
            generation: 0,
            seen_session: String::new(),
            seen_status: String::new(),
            seen_question: String::new(),
            session_states: HashMap::new(),
            deferred_route: None,
            histories: HashMap::new(),
            stashes: vec![],
            preferences_dirty: false,
            notify_requested: None,
            open_requested: None,
            image_copy_requested: None,
            preview_payload: Value::Null,
            preview_target: None,
            save_data: vec![],
            menu_session: String::new(),
            pending_edit: None,
            last_prediction_at: None,
            last_error: String::new(),
        }
    }
}
impl Interaction {
    pub fn load(value: &Value) -> Self {
        let strings = |key| {
            items(value, key)
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .take(40)
                .collect()
        };
        Self {
            predictions: value["predictions"] != false,
            notifications: value["notifications"] != false,
            favorites: strings("favorites"),
            recent_models: strings("recent_models"),
            recent_commands: strings("recent_commands"),
            ..Self::default()
        }
    }
    pub fn preferences(&self) -> Value {
        json!({"predictions":self.predictions,"notifications":self.notifications,"favorites":self.favorites,"recent_models":self.recent_models,"recent_commands":self.recent_commands})
    }
}
fn remember(list: &mut Vec<String>, value: String) {
    list.retain(|v| v != &value);
    list.insert(0, value);
    list.truncate(24);
}
pub fn code_blocks(body: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    let mut active: Option<(String, String, char, usize)> = None;
    for row in body.split_inclusive('\n') {
        let trimmed = row.trim_start();
        if let Some((_, _, marker, count)) = &active {
            if trimmed.starts_with(&marker.to_string().repeat(*count)) {
                let (language, source, _, _) = active.take().unwrap();
                out.push((language, source));
                continue;
            }
        } else if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker = trimmed.chars().next().unwrap();
            let n = trimmed.chars().take_while(|c| *c == marker).count();
            active = Some((trimmed[n..].trim().to_owned(), String::new(), marker, n));
            continue;
        }
        if let Some((_, source, _, _)) = &mut active {
            source.push_str(row);
        }
    }
    out
}
impl App {
    pub fn save_code_block(&mut self, language: &str, source: &str) {
        let extension = match language {
            "python" => "py",
            "rust" => "rs",
            "javascript" => "js",
            "typescript" => "ts",
            "" => "txt",
            v => v,
        };
        self.begin_save(&format!("snippet.{extension}"), source.as_bytes().to_vec());
    }
    fn interaction_menu(&mut self, title: &str, kind: &str, entries: Vec<Entry>) {
        self.interaction.menu_session = self.session_id().unwrap_or("").into();
        self.interaction.menu = Some(Menu {
            category: String::new(),
            title: title.into(),
            kind: kind.into(),
            query: String::new(),
            selected: 0,
            entries,
        });
        self.interaction.prediction = None;
    }
    pub fn open_command_palette(&mut self) {
        let mut entries = vec![];
        for command in crate::app::COMMANDS
            .iter()
            .chain(crate::commands::EXTRA_COMMANDS)
            .copied()
        {
            if !crate::commands::is_primary_command(command)
                || entries.iter().any(|e: &Entry| e.data == command)
            {
                continue;
            }
            let (description, usage) = crate::app::command_metadata(command);
            entries.push(Entry::new(
                command,
                format!("{description} · {usage}"),
                "command",
                json!(command),
            ));
        }
        entries.sort_by_key(|e| {
            (
                self.interaction
                    .recent_commands
                    .iter()
                    .position(|v| v == e.data.as_str().unwrap_or(""))
                    .unwrap_or(usize::MAX),
                e.label.clone(),
            )
        });
        entries.extend([
            Entry::new(
                "设置 · 图片生成接口",
                "地址、协议、模型和密钥",
                "settings_image",
                Value::Null,
            ),
            Entry::new(
                "设置 · 模型来源",
                "管理 API 来源",
                "command",
                json!("/provider"),
            ),
            Entry::new(
                "设置 · 工具服务 MCP",
                "管理工具连接",
                "command",
                json!("/mcp"),
            ),
            Entry::new(
                "设置 · 输入预测开关",
                "当前模型生成短建议，/suggest on|off",
                "command",
                json!("/suggest"),
            ),
            Entry::new(
                "设置 · 任务提醒开关",
                "窗口失焦时提醒，/notifications on|off",
                "command",
                json!("/notifications"),
            ),
        ]);
        self.interaction_menu("功能搜索 · 命令 / 设置 / 快捷操作", "commands", entries);
    }
    pub fn open_quick_models(&mut self) {
        let mut entries = vec![
            Entry::new(
                "思考强度",
                "调整当前模型思考强度",
                "command",
                json!("/effort"),
            ),
            Entry::new("加速选项", "优先处理选项", "command", json!("/fast")),
            Entry::new("管理来源 / 登录", "进入设置", "command", json!("/settings")),
        ];
        let mut models = items(&self.snapshot, "models").to_vec();
        models.sort_by_key(|m| {
            let c = text(m, "choice");
            (
                !self.interaction.favorites.iter().any(|v| v == c),
                self.interaction
                    .recent_models
                    .iter()
                    .position(|v| v == c)
                    .unwrap_or(usize::MAX),
                text(m, "title").to_owned(),
            )
        });
        for model in models {
            let choice = text(&model, "choice");
            let starred = self.interaction.favorites.iter().any(|v| v == choice);
            let label = ["label", "title", "id"]
                .into_iter()
                .map(|k| text(&model, k))
                .find(|v| !v.is_empty())
                .unwrap_or(choice);
            entries.push(Entry::new(
                format!("{} {label}", if starred { "★" } else { " " }),
                format!("{} · F2 收藏/取消", text(&model, "source")),
                "model",
                model,
            ));
        }
        self.interaction_menu(
            "模型 · 收藏 / 最近使用 / 思考强度 / 加速",
            "models",
            entries,
        );
    }
    fn run_palette_command(&mut self, command: &str) -> Vec<Command> {
        remember(&mut self.interaction.recent_commands, command.into());
        self.interaction.preferences_dirty = true;
        let draft = self.current_draft();
        self.input = command.into();
        self.cursor = self.input.len();
        let commands = self.submit();
        if self.input.is_empty() {
            self.load_draft(draft);
        }
        commands
    }
    pub fn interaction_command(&mut self, command: &str, arg: &str) -> Option<Vec<Command>> {
        match command {
            "/commands" => self.open_command_palette(),
            "/model" if arg.is_empty() => self.open_quick_models(),
            "/model" if arg == "previous" => {
                let current = text(&self.snapshot["session"], "model_choice");
                if let Some(choice) = self
                    .interaction
                    .recent_models
                    .iter()
                    .find(|c| c.as_str() != current)
                    .cloned()
                {
                    if let Some(model) = items(&self.snapshot, "models")
                        .iter()
                        .find(|m| m["choice"] == choice)
                        .cloned()
                    {
                        return Some(self.interaction_choose(Entry::new("", "", "model", model)));
                    }
                }
                self.notice = "没有其他最近使用的模型".into();
            }
            "/actions" => self.open_content_actions(),
            "/drafts" => {
                let session = self.session_id().unwrap_or("");
                let entries = self
                    .interaction
                    .stashes
                    .iter()
                    .enumerate()
                    .filter(|(_, (s, _))| s == session)
                    .map(|(i, (_, d))| {
                        Entry::new(
                            d.text.lines().next().unwrap_or("附件草稿"),
                            format!(
                                "{} 个附件 · Enter 取回（当前草稿会先暂存）",
                                d.attachments.len()
                            ),
                            "draft",
                            json!(i),
                        )
                    })
                    .collect();
                self.interaction_menu("草稿暂存架 · 当前窗口 / 当前会话", "drafts", entries);
            }
            "/stash" => self.stash_draft(),
            "/queue" => {
                return Some(vec![
                    self.command("terminal.query", json!({"topic":"ux.queue"})),
                ]);
            }
            "/tasks" => {
                return Some(vec![
                    self.command("terminal.query", json!({"topic":"ux.tasks"})),
                ]);
            }
            "/diagnose" => {
                return Some(vec![
                    self.command("terminal.query", json!({"topic":"ux.diagnose"})),
                ]);
            }
            "/suggest" | "/notifications" => {
                let enabled = match arg {
                    "on" => true,
                    "off" => false,
                    _ => {
                        if command == "/suggest" {
                            !self.interaction.predictions
                        } else {
                            !self.interaction.notifications
                        }
                    }
                };
                if command == "/suggest" {
                    self.interaction.predictions = enabled;
                    self.interaction.prediction = None;
                    self.interaction.prediction_request = None;
                } else {
                    self.interaction.notifications = enabled;
                }
                self.interaction.preferences_dirty = true;
                self.notice = format!(
                    "{}已{}",
                    if command == "/suggest" {
                        "输入预测（当前模型短请求）"
                    } else {
                        "任务提醒"
                    },
                    if enabled { "开启" } else { "关闭" }
                );
            }
            _ => return None,
        }
        Some(vec![])
    }
    pub fn stash_draft(&mut self) {
        if self.interaction.stashes.len() >= 24 {
            self.notice = "暂存架已满，请先 /drafts 取回或 Delete 移除；当前草稿已保留".into();
            return;
        }
        if self.input.is_empty() && self.attachments.is_empty() {
            self.notice = "没有需要暂存的草稿".into();
            return;
        }
        self.interaction
            .stashes
            .push((self.session_id().unwrap_or("").into(), self.current_draft()));
        self.load_draft(Draft::default());
        self.notice = "已暂存草稿 · /drafts 取回".into();
    }
    pub fn open_content_actions(&mut self) {
        let mut entries = vec![];
        if let Some(selection) = &self.selection {
            let selected = selection.text();
            if !selected.is_empty() {
                entries.push(Entry::new(
                    "引用选中文字到草稿",
                    "保留当前草稿",
                    "quote",
                    json!(selected),
                ));
                entries.push(Entry::new(
                    "在旁问中解释选中文字",
                    "主任务继续",
                    "side_quote",
                    json!(selected),
                ));
                entries.push(Entry::new("复制选中文字", "", "copy", json!(selected)));
            }
        }
        for message in items(&self.snapshot, "messages").iter().rev().take(12) {
            let id = text(message, "id");
            let body = text(message, "text");
            if text(message, "role") == "assistant" && !body.is_empty() {
                entries.push(Entry::new(
                    format!(
                        "复制回复 · {}",
                        body.lines()
                            .next()
                            .unwrap_or("")
                            .chars()
                            .take(35)
                            .collect::<String>()
                    ),
                    "复制原文，保留段落",
                    "copy",
                    json!(body),
                ));
                for (i, (language, source)) in code_blocks(body).into_iter().enumerate() {
                    entries.push(Entry::new(
                        format!("复制代码块 {} · {language}", i + 1),
                        "保留原始缩进和换行",
                        "copy",
                        json!(source),
                    ));
                    entries.push(Entry::new(format!("保存代码块 {} · {language}",i+1),"另存为新文件","save_text",json!({"text":source,"name":format!("snippet.{}",match language.as_str(){"python"=>"py","rust"=>"rs","javascript"=>"js","typescript"=>"ts",""=>"txt",v=>v})})));
                }
            }
            for attachment in items(message, "attachments") {
                let data = json!({"message_id":id,"attachment_id":attachment["id"],"name":attachment["name"],"path":attachment["path"]});
                if text(attachment, "mime").starts_with("image/") {
                    entries.push(Entry::new(
                        format!("预览图片 · {}", text(attachment, "name")),
                        "双击文件行也可预览",
                        "image",
                        data.clone(),
                    ));
                    entries.push(Entry::new(
                        format!("作为参考图 · {}", text(attachment, "name")),
                        "加入当前草稿，再输入修改要求",
                        "reference",
                        data,
                    ));
                } else if !text(attachment, "path").is_empty() {
                    entries.push(Entry::new(
                        format!("打开文件 · {}", text(attachment, "name")),
                        text(attachment, "path"),
                        "open",
                        attachment["path"].clone(),
                    ));
                    entries.push(Entry::new(
                        "复制文件路径",
                        text(attachment, "path"),
                        "copy",
                        attachment["path"].clone(),
                    ));
                }
            }
        }
        self.interaction_menu("内容操作 · 选区 / 回复 / 代码 / 附件", "actions", entries);
    }
    fn begin_save(&mut self, name: &str, data: Vec<u8>) {
        self.interaction.save_data = data;
        let base = std::env::var_os("USERPROFILE")
            .map(std::path::PathBuf::from)
            .map(|p| p.join("Downloads"))
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        self.interaction_menu("另存为 · 输入绝对路径 · 不覆盖已有文件", "save", vec![]);
        self.interaction.menu.as_mut().unwrap().query = base
            .join(std::path::Path::new(name).file_name().unwrap_or_default())
            .to_string_lossy()
            .into();
    }
    fn interaction_choose(&mut self, entry: Entry) -> Vec<Command> {
        self.interaction.menu = None;
        match entry.action.as_str(){
            "command"=>return self.run_palette_command(entry.data.as_str().unwrap_or("/help")),
            "task_details"=>self.open(Drawer::Tasks),
            "copy"=>self.copy_requested=entry.data.as_str().map(str::to_owned),
            "open"=>self.interaction.open_requested=entry.data.as_str().map(str::to_owned),
            "quote"=>{let quote=entry.data.as_str().unwrap_or("").lines().map(|s|format!("> {s}")).collect::<Vec<_>>().join("\n");self.insert(&format!("\n{quote}\n\n"));},
            "side_quote"=>return self.extra_command("/btw",&format!("请解释这段内容：\n{}",entry.data.as_str().unwrap_or(""))),
            "save_text"=>self.begin_save(text(&entry.data,"name"),text(&entry.data,"text").as_bytes().to_vec()),
            "image"=>return vec![self.command("terminal.action",json!({"action":"attachment.message.preview","message_id":entry.data["message_id"],"attachment_id":entry.data["attachment_id"]}))],
            "settings_image"=>self.edit_media("imageGeneration"),
            "reference"=>return vec![self.command("terminal.action",json!({"action":"ux.image.reference","message_id":entry.data["message_id"],"attachment_id":entry.data["attachment_id"]}))],
            "draft"=>{
                if let Some(index)=entry.data.as_u64().map(|v|v as usize).filter(|i|*i<self.interaction.stashes.len()) {
                    let (session,draft)=self.interaction.stashes.remove(index);
                    if self.session_id()!=Some(session.as_str()){self.notice="草稿属于另一会话，请先返回原会话".into();return vec![];}
                    if !self.input.is_empty() || !self.attachments.is_empty(){self.stash_draft();}
                    self.load_draft(draft);
                }
            }
            "model"=>{
                if self.active(){self.notice="当前任务结束后才能切换模型".into();return vec![];}
                let choice=text(&entry.data,"choice").to_owned();if choice.is_empty(){return vec![];}
                let efforts=items(&entry.data,"efforts");
                let effort=efforts.iter().find(|v|v.as_str()==Some("high")).or_else(||efforts.first()).cloned().unwrap_or(json!("high"));
                return vec![self.command("model.set",json!({"choice":choice,"thinking":true,"effort":effort}))];
            }
            "session"=>return vec![Command{method:"session.open".into(),params:json!({"session_id":entry.data})}],
            "route"=>{
                let kind=text(&entry.data,"kind");
                let session=if kind=="session" {text(&entry.data,"id")}else{text(&entry.data,"session_id")};
                if Some(session)!=self.session_id() && !session.is_empty() {
                    if kind!="session" {self.interaction.deferred_route=Some(entry.data.clone());}
                    return vec![Command{method:"session.open".into(),params:json!({"session_id":session})}];
                }
                match kind {
                    "agent"=>return self.workbench_query("wb.agent.poll",json!({"agent_id":entry.data["id"]})),
                    "job"=>return self.workbench_query("wb.job",json!({"job_id":entry.data["id"]})),
                    "shell"=>{self.shell_state=json!({"job_id":entry.data["id"],"session_id":session,"cursor":0});return self.open_shell();}
                    _=>{}
                }
            }
            "job"=>return self.workbench_query("wb.job",json!({"job_id":entry.data})),
            "withdraw"=>{
                let kind=text(&entry.data,"kind");
                self.interaction.pending_edit=Some(entry.data.clone());
                return vec![if kind=="steer" {self.command("queue.remove",json!({"id":entry.data["id"]}))}else{self.command("terminal.action",json!({"action":"ux.followup.remove","id":entry.data["id"]}))}];
            }
            "move"=>return vec![self.command("terminal.action",json!({"action":"ux.followup.move","id":entry.data["id"],"text":entry.data["text"],"direction":entry.data["direction"]}))],
            "connection"=>return vec![self.command("terminal.action",json!({"action":"ux.connection.test"}))],
            "send_now"=>{
                let text=self.input.clone();
                if text.trim().is_empty(){self.notice="请先填写新的指示".into();return vec![];}
                if !self.attachments.is_empty(){self.notice="停止改方向操作仅支持文字；附件已保留".into();return vec![];}
                self.interaction.pending_edit=Some(json!({"text":text}));
                return vec![self.command("terminal.action",json!({"action":"ux.send_now","text":text}))];
            }
            "diagnosis_copy"=>self.copy_requested=Some(self.interaction.last_error.clone()),
            "preview_save"=>{
                if let Ok(bytes)=STANDARD.decode(text(&self.interaction.preview_payload,"data")){self.close_image_preview();self.begin_save(text(&entry.data,"name"),bytes);}
            }
            _=>{}
        }
        vec![]
    }
    pub fn interaction_key(&mut self, key: KeyEvent) -> Option<Vec<Command>> {
        if self.image_preview.is_none() && (self.rejection || self.status() == "approval") {
            self.interaction.menu = None;
            return None;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if self.interaction.menu.is_some() {
            if self.interaction.menu_session != self.session_id().unwrap_or("") {
                self.interaction.menu = None;
                return Some(vec![]);
            }
            if key.code == KeyCode::Esc {
                self.interaction.menu = None;
                return Some(vec![]);
            }
            let menu = self.interaction.menu.as_mut().unwrap();
            if menu.kind == "save" {
                match key.code {
                    KeyCode::Enter => {
                        let path = std::path::PathBuf::from(menu.query.trim());
                        if !path.is_absolute() {
                            self.notice = "请输入绝对路径".into();
                            return Some(vec![]);
                        }
                        use std::io::Write;
                        let result = std::fs::OpenOptions::new()
                            .create_new(true)
                            .write(true)
                            .open(&path)
                            .and_then(|mut f| {
                                f.write_all(&self.interaction.save_data)
                                    .and_then(|_| f.sync_all())
                            });
                        match result {
                            Ok(()) => {
                                self.notice = format!("已保存 {}", path.display());
                                self.interaction.menu = None;
                                self.interaction.save_data.clear();
                            }
                            Err(e) => self.notice = format!("保存失败（保留已有文件）：{e}"),
                        };
                    }
                    KeyCode::Backspace => {
                        menu.query.pop();
                    }
                    KeyCode::Char('u') if ctrl => menu.query.clear(),
                    KeyCode::Char(c) if !ctrl => menu.query.push(c),
                    _ => {}
                }
                return Some(vec![]);
            }
            if key.code == KeyCode::F(2) && menu.kind == "models" {
                if let Some(e) = menu
                    .filtered()
                    .get(menu.selected)
                    .filter(|e| e.action == "model")
                {
                    let choice = text(&e.data, "choice").to_owned();
                    if self.interaction.favorites.contains(&choice) {
                        self.interaction.favorites.retain(|c| c != &choice);
                    } else {
                        self.interaction.favorites.push(choice);
                    }
                    self.interaction.preferences_dirty = true;
                    self.open_quick_models();
                }
                return Some(vec![]);
            }
            match key.code {
                KeyCode::Delete if menu.kind == "drafts" => {
                    if let Some(index) = menu
                        .filtered()
                        .get(menu.selected)
                        .and_then(|e| e.data.as_u64())
                        .map(|v| v as usize)
                    {
                        self.interaction.stashes.remove(index);
                        return self.interaction_command("/drafts", "");
                    }
                }
                KeyCode::Tab if menu.kind == "tasks" => {
                    let categories = ["", "session", "agent", "job", "shell"];
                    let current = categories
                        .iter()
                        .position(|v| *v == menu.category)
                        .unwrap_or(0);
                    menu.category = categories[(current + 1) % categories.len()].into();
                    menu.selected = 0;
                }
                KeyCode::Up => menu.selected = menu.selected.saturating_sub(1),
                KeyCode::Down => {
                    menu.selected = (menu.selected + 1).min(menu.filtered().len().saturating_sub(1))
                }
                KeyCode::Enter => {
                    let entry = menu.filtered().get(menu.selected).map(|e| (*e).clone());
                    if let Some(entry) = entry {
                        return Some(self.interaction_choose(entry));
                    }
                }
                KeyCode::Backspace => {
                    menu.query.pop();
                    menu.selected = 0;
                }
                KeyCode::Char(c) if !ctrl && !alt => {
                    menu.query.push(c);
                    menu.selected = 0;
                }
                _ => {}
            }
            return Some(vec![]);
        }
        if self.image_preview.is_some() {
            match key.code {
                KeyCode::Char('r' | 'R') => {
                    if let Some((message, attachment)) = self
                        .interaction
                        .preview_target
                        .clone()
                        .filter(|(m, a)| !m.is_empty() && !a.is_empty())
                    {
                        self.close_image_preview();
                        return Some(vec![self.command("terminal.action",json!({"action":"ux.image.reference","message_id":message,"attachment_id":attachment}))]);
                    }
                    self.notice = "当前图片已在草稿中或没有会话来源".into();
                    return Some(vec![]);
                }
                KeyCode::Char('s' | 'S') => {
                    let payload = self.interaction.preview_payload.clone();
                    if let Ok(bytes) = STANDARD.decode(text(&payload, "data")) {
                        self.close_image_preview();
                        self.begin_save(text(&payload, "name"), bytes);
                    }
                    return Some(vec![]);
                }
                KeyCode::Char('c' | 'C') => {
                    self.interaction.image_copy_requested =
                        Some(text(&self.interaction.preview_payload, "data").into());
                    return Some(vec![]);
                }
                _ => return None,
            }
        }
        if self.side_panel.as_ref().is_some_and(|panel|panel.focused) {return None;}
        if self.form.is_some()
            || self.drawer.is_some()
            || self.trust_prompt.is_some()
            || self.workspace_pending
            || self.startup_phase.is_some()
            || self.question_popup_visible()
            || self.view != View::Chat
        {
            return None;
        }
        if key.code == KeyCode::F(1) {
            self.open_command_palette();
            return Some(vec![]);
        }
        if alt && key.code == KeyCode::Char('a') {
            self.open_content_actions();
            return Some(vec![]);
        }
        if ctrl && key.code == KeyCode::Char('s') {
            self.stash_draft();
            return Some(vec![]);
        }
        if alt && key.code == KeyCode::Char('m') {
            self.open_quick_models();
            return Some(vec![]);
        }
        if alt && key.code == KeyCode::Char('q') {
            return self.interaction_command("/queue", "");
        }
        if alt && key.code == KeyCode::Enter && self.active() {
            if self.interaction.pending_edit.is_some() {
                self.notice = "上一条队列操作仍待确认，草稿已保留".into();
                return Some(vec![]);
            }
            if self.input.trim_start().starts_with(['/', '!']) {
                self.notice = "下一任务排队用于普通文字；命令请通过原入口执行".into();
                return Some(vec![]);
            }
            if !self.attachments.is_empty() {
                self.notice = "下一任务排队目前仅支持文字；附件和草稿已保留".into();
                return Some(vec![]);
            }
            if self.input.trim().is_empty() {
                return Some(vec![]);
            }
            let text = self.input.clone();
            self.interaction.pending_edit = Some(json!({"text":text,"session":self.session_id()}));
            return Some(vec![self.command(
                "terminal.action",
                json!({"action":"ux.followup.add","text":text}),
            )]);
        }
        if ctrl && matches!(key.code, KeyCode::Char('z' | 'y')) {
            self.observe_input();
            let session = self.session_id().unwrap_or("").to_owned();
            let current = self.current_draft();
            let history = self.interaction.histories.entry(session).or_default();
            let value = if key.code == KeyCode::Char('z') {
                if let Some(v) = history.undo.pop() {
                    history.redo.push(current);
                    Some(v)
                } else {
                    None
                }
            } else {
                if let Some(v) = history.redo.pop() {
                    history.undo.push(current);
                    Some(v)
                } else {
                    None
                }
            };
            if let Some(draft) = value {
                history.last = Some(draft.clone());
                self.load_draft(draft);
            }
            return Some(vec![]);
        }
        if key.code == KeyCode::Tab && self.input.is_empty() && self.completion().is_empty() {
            if let Some(prediction) = self.interaction.prediction.take() {
                self.insert(&prediction);
                return Some(vec![]);
            }
        }
        if key.code == KeyCode::Esc
            && self.input.is_empty()
            && self.interaction.prediction.take().is_some()
        {
            return Some(vec![]);
        }
        if matches!(
            key.code,
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
        ) {
            self.interaction.prediction = None;
        }
        self.observe_input();
        None
    }
    fn observe_input(&mut self) {
        let session = self.session_id().unwrap_or("").to_owned();
        let draft = self.current_draft();
        let h = self.interaction.histories.entry(session).or_default();
        if h.last
            .as_ref()
            .is_some_and(|v| v.text != draft.text || v.attachments != draft.attachments)
        {
            h.undo.push(h.last.take().unwrap());
            if h.undo.len() > 80 {
                h.undo.remove(0);
            }
            h.redo.clear();
        }
        h.last = Some(draft);
    }
    pub fn interaction_tick(&mut self) {
        for session in items(&self.snapshot, "sessions") {
            let id = text(session, "id").to_owned();
            let status = text(session, "status").to_owned();
            if let Some(previous) = self.interaction.session_states.insert(id, status.clone()) {
                if self.interaction.notifications
                    && matches!(previous.as_str(), "running" | "retrying")
                    && !matches!(status.as_str(), "running" | "retrying")
                {
                    self.interaction.notify_requested = Some(format!(
                        "Expert · {} · {}",
                        text(session, "title"),
                        match status.as_str() {
                            "failed" => "任务失败",
                            "approval" => "需要批准",
                            "stopped" => "已停止",
                            _ => "任务已结束",
                        }
                    ));
                }
            }
        }
        if self.interaction.session_states.len() > 256 {
            self.interaction.session_states.clear();
        }
        let question = text(&self.snapshot["questions"][0], "id").to_owned();
        if question != self.interaction.seen_question {
            if !question.is_empty() && self.interaction.notifications {
                self.interaction.notify_requested = Some(format!(
                    "Expert · {} · 需要回答",
                    text(&self.snapshot["session"], "title")
                ));
            }
            self.interaction.seen_question = question;
        }
        if !self.rejection
            && self.status() != "approval"
            && self.form.is_none()
            && self.drawer.is_none()
            && self.interaction.menu.is_none()
            && !self.question_popup_visible()
        {
            self.observe_input();
        }
        let session = self.session_id().unwrap_or("").to_owned();
        let status = self.status().to_owned();
        if session != self.interaction.seen_session {
            self.interaction.generation = self.interaction.generation.wrapping_add(1);
            self.interaction.prediction = None;
            self.interaction.prediction_request = None;
            self.interaction.menu = None;
            self.interaction.seen_session = session;
            self.interaction.seen_status = status;
            self.interaction.pending_edit = None;
            if let Some(route) = self.interaction.deferred_route.take() {
                if text(&route, "session_id") == self.session_id().unwrap_or("") {
                    let commands = self.interaction_choose(Entry::new("", "", "route", route));
                    self.outbox.extend(commands);
                }
            }
            return;
        }
        let previous = self.interaction.seen_status.clone();
        if previous != status {
            self.interaction.seen_status = status.clone();
            if self.interaction.notifications
                && matches!(previous.as_str(), "running" | "retrying")
                && !matches!(status.as_str(), "running" | "retrying")
            {
                self.interaction.notify_requested = Some(format!(
                    "Expert · {} · {}",
                    text(&self.snapshot["session"], "title"),
                    match status.as_str() {
                        "approval" => "需要批准",
                        "failed" => "任务失败",
                        "stopped" => "任务已停止",
                        "waiting_acceptance" => "等待验收",
                        _ => "任务已结束",
                    }
                ));
            }
            if self.interaction.predictions
                && self.input.is_empty()
                && self.attachments.is_empty()
                && matches!(previous.as_str(), "running" | "retrying")
                && matches!(
                    status.as_str(),
                    "idle" | "completed" | "accepted" | "waiting_acceptance"
                )
                && self
                    .interaction
                    .last_prediction_at
                    .is_none_or(|at| at.elapsed() > Duration::from_secs(15))
                && !self.snapshot_stale
            {
                if let Some(message) = items(&self.snapshot, "messages")
                    .iter().rev()
                    .find(|m| text(m, "role") == "assistant" && !text(m, "text").is_empty())
                {
                    let id = text(message, "id").to_owned();
                    self.interaction.generation = self.interaction.generation.wrapping_add(1);
                    self.interaction.prediction_request = Some((
                        self.session_id().unwrap_or("").into(),
                        id.clone(),
                        self.interaction.generation,
                    ));
                    self.interaction.last_prediction_at = Some(Instant::now());
                    self.outbox.push(self.command("terminal.action",json!({"action":"ux.predict","message_id":id,"prediction_generation":self.interaction.generation})));
                }
            }
        }
        if self.active() || !self.input.is_empty() {
            self.interaction.prediction = None;
        }
    }
    pub fn interaction_response(&mut self, command: &Command, result: &Value) -> bool {
        if command.method == "model.set" && result["accepted"] != false {
            let previous = if !text(&self.snapshot["session"], "model_choice").is_empty() {
                text(&self.snapshot["session"], "model_choice").to_owned()
            } else {
                let session=&self.snapshot["session"];
                items(&self.snapshot,"models").iter().find(|m|m["id"]==session["model"] && (text(session,"model_source_id").is_empty() || m["source_id"]==session["model_source_id"]))
                    .map(|m|text(m,"choice").to_owned()).unwrap_or_default()
            };
            if !previous.is_empty() {
                remember(&mut self.interaction.recent_models, previous);
            }
            remember(
                &mut self.interaction.recent_models,
                text(&command.params, "choice").into(),
            );
            self.interaction.preferences_dirty = true;
        }
        let op = text(&command.params, "topic");
        let action = text(&command.params, "action");
        if !op.starts_with("ux.") && !action.starts_with("ux.") {
            if command.method == "queue.remove" && self.interaction.pending_edit.take().is_some() {
                if command.params["session_id"].as_str() == self.session_id() {
                    self.restore_withdrawn(text(result, "text"));
                }
                return true;
            }
            return false;
        }
        if command.params["session_id"].as_str() != self.session_id() {
            if action == "ux.followup.add" && result["accepted"] == true {
                if let Some(draft) = self
                    .session_drafts
                    .get_mut(text(&command.params, "session_id"))
                {
                    if draft.text == text(&command.params, "text") {
                        draft.text.clear();
                        draft.cursor = 0;
                    }
                }
            }
            return true;
        }
        match text(result, "ux") {
            "prediction" => {
                let generation = command.params["prediction_generation"]
                    .as_u64()
                    .unwrap_or(0);
                let valid = self.interaction.prediction_request.as_ref().is_some_and(
                    |(session, message, n)| {
                        Some(session.as_str()) == self.session_id()
                            && message == text(&command.params, "message_id")
                            && *n == generation
                    },
                );
                if valid
                    && self.interaction.predictions
                    && self.input.is_empty()
                    && !self.active()
                    && !self.snapshot_stale
                {
                    let suggestion = crate::ui::clean(text(result, "text"))
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_owned();
                    self.interaction.prediction = (!suggestion.is_empty()
                        && !suggestion.starts_with(['/', '!']))
                    .then_some(suggestion);
                }
                if valid {
                    self.interaction.prediction_request = None;
                }
            }
            "queue" => {
                let mut entries = vec![];
                if self.active() && !self.input.trim().is_empty() {
                    entries.push(Entry::new(
                        "停止当前任务并发送草稿",
                        "选择后停止并发送；原排队消息保留",
                        "send_now",
                        Value::Null,
                    ));
                }
                for entry in items(result, "entries") {
                    let next = entry["kind"] == "next_task";
                    entries.push(Entry::new(
                        format!(
                            "{} · {}",
                            if next {
                                "下一任务"
                            } else {
                                "当前任务补充"
                            },
                            text(entry, "text")
                        ),
                        "Enter 撤回到草稿，再编辑发送",
                        "withdraw",
                        entry.clone(),
                    ));
                    if next {
                        for direction in ["up", "down"] {
                            let mut data = entry.clone();
                            data["direction"] = json!(direction);
                            entries.push(Entry::new(
                                if direction == "up" {
                                    "↑ 提前一条"
                                } else {
                                    "↓ 延后一条"
                                },
                                text(entry, "text"),
                                "move",
                                data,
                            ));
                        }
                    }
                }
                self.interaction_menu("消息队列 · 插嘴 / 下一任务", "queue", entries);
            }
            "queued" => {
                if let Some(pending) = self.interaction.pending_edit.take() {
                    if self.input == text(&pending, "text") {
                        self.load_draft(Draft::default());
                    }
                }
                self.notice = "已排队为下一任务 · Alt+Q 查看/撤回/编辑".into();
            }
            "sent" => {
                if let Some(pending) = self.interaction.pending_edit.take() {
                    if self.input == text(&pending, "text") {
                        self.load_draft(Draft::default());
                    }
                }
                self.notice = "已停止原任务并提交新指示".into();
            }
            "attachment" => {
                self.insert_attachment_mark(result["attachment"].clone());
                self.notice = "参考图已加入草稿，请输入修改要求后发送".into();
            }
            "withdraw" => {
                self.interaction.pending_edit = None;
                self.restore_withdrawn(text(result, "text"));
            }
            "queue_changed" => self
                .outbox
                .push(self.command("terminal.query", json!({"topic":"ux.queue"}))),
            "tasks" => {
                let mut entries = vec![
                    Entry::new(
                        "任务与待办详情",
                        "当前会话验收、待办、批准和问题",
                        "task_details",
                        Value::Null,
                    ),
                    Entry::new(
                        "子代理详情",
                        "打开现有代理视图",
                        "command",
                        json!("/agents"),
                    ),
                    Entry::new(
                        "后台操作详情",
                        "打开当前窗口后台操作",
                        "command",
                        json!("/jobs"),
                    ),
                ];
                for entry in items(result, "entries") {
                    let kind = text(entry, "kind");
                    entries.push(Entry::new(
                        format!("[{}] {}", text(entry, "status"), text(entry, "title")),
                        format!("{kind} · Enter 定位"),
                        "route",
                        entry.clone(),
                    ));
                }
                self.interaction_menu("任务中心 · 主任务 / 子代理 / 后台操作", "tasks", entries);
            }
            "diagnosis" => {
                self.interaction.last_error = items(result, "lines")
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n");
                let mut entries: Vec<_> = items(result, "lines")
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|line| Entry::new(line, "", "noop", Value::Null))
                    .collect();
                entries.extend([
                    Entry::new(
                        "测试当前连接",
                        "发送一次独立短请求，不携带会话和附件",
                        "connection",
                        Value::Null,
                    ),
                    Entry::new("复制脱敏诊断", "", "diagnosis_copy", Value::Null),
                    Entry::new("打开来源设置", "", "command", json!("/settings")),
                ]);
                self.interaction_menu("状态与诊断", "diagnosis", entries);
            }
            _ => {}
        }
        true
    }
    fn restore_withdrawn(&mut self, text: &str) {
        if !self.input.is_empty() || !self.attachments.is_empty() {
            self.stash_draft();
        }
        self.input = text.into();
        self.cursor = self.input.len();
        self.notice = "已撤回到草稿；Enter 插嘴，Alt+Enter 重新排到下一任务".into();
    }
    pub fn interaction_error(&mut self, command: &Command, message: &str) -> bool {
        let op = text(&command.params, "topic");
        if op == "ux.predict" || text(&command.params, "action") == "ux.predict" {
            if self
                .interaction
                .prediction_request
                .as_ref()
                .is_some_and(|(_, _, generation)| {
                    Some(*generation) == command.params["prediction_generation"].as_u64()
                })
            {
                self.interaction.prediction_request = None;
            }
            return true;
        }
        if command.params["session_id"].as_str() != self.session_id()
            && (op.starts_with("ux.") || text(&command.params, "action").starts_with("ux."))
        {
            return true;
        }
        self.interaction.last_error = message.into();
        if op.starts_with("ux.") || text(&command.params, "action").starts_with("ux.") {
            self.interaction.pending_edit = None;
            self.notice = format!("{message} · /diagnose 查看诊断");
            return true;
        }
        false
    }
    pub fn interaction_mouse(&mut self, x: u16, y: u16) -> Vec<Command> {
        let Some(menu) = self.interaction.menu.as_ref() else {
            return vec![];
        };
        let r = popup(self.viewport.0, self.viewport.1);
        if !r.contains((x, y).into()) {
            self.interaction.menu = None;
            return vec![];
        }
        if y >= r.y + 3 && y < r.bottom().saturating_sub(2) {
            let count = r.height.saturating_sub(5) as usize;
            let start = menu
                .selected
                .saturating_sub(count / 2)
                .min(menu.filtered().len().saturating_sub(count));
            let index = start + (y - r.y - 3) as usize;
            if let Some(entry) = menu.filtered().get(index).map(|v| (*v).clone()) {
                return self.interaction_choose(entry);
            }
        }
        vec![]
    }
}
fn popup(w: u16, h: u16) -> Rect {
    let width = w.saturating_sub(4).min(110);
    let height = h.saturating_sub(2).min(28);
    Rect::new((w - width) / 2, (h - height) / 2, width, height)
}
pub fn draw(f: &mut Frame, a: &App) {
    if a.trust_prompt.is_some() || a.workspace_pending || a.startup_phase.is_some(){return;}
    let Some(menu) = a.interaction.menu.as_ref() else {
        return;
    };
    let r = popup(f.area().width, f.area().height);
    if r.width < 8 || r.height < 5 {
        return;
    }
    let accent = crate::ui::Theme::new(a).accent;
    f.render_widget(Clear, r);
    let mut block = Block::default()
        .borders(Borders::ALL)
        .title(crate::ui::truncate(
            &format!(
                " {} ",
                if a.ambiguous_width == 2 {
                    menu.title.replace('·', ":")
                } else {
                    menu.title.clone()
                }
            ),
            r.width.saturating_sub(2) as usize,
            a.ambiguous_width,
        ))
        .border_style(accent);
    if a.ascii || a.ambiguous_width == 2 {
        block = block.border_set(ratatui::symbols::border::Set {
            top_left: "+",
            top_right: "+",
            bottom_left: "+",
            bottom_right: "+",
            vertical_left: "|",
            vertical_right: "|",
            horizontal_top: "-",
            horizontal_bottom: "-",
        });
    }
    f.render_widget(block, r);
    let query = if menu.kind == "tasks" {
        format!(
            "Tab 分类：{} · 搜索：{}",
            match menu.category.as_str() {
                "session" => "主任务",
                "agent" => "子代理",
                "job" => "后台操作",
                "shell" => "终端命令",
                _ => "全部",
            },
            menu.query
        )
    } else if menu.kind == "save" {
        format!("路径：{}", menu.query)
    } else {
        format!("搜索：{}", menu.query)
    };
    crate::ui::render_text(
        f,
        a,
        crate::ui::truncate(
            &query,
            r.width.saturating_sub(4) as usize,
            a.ambiguous_width,
        ),
        Rect::new(r.x + 2, r.y + 1, r.width - 4, 1),
    );
    let entries = menu.filtered();
    let count = r.height.saturating_sub(5) as usize;
    let start = menu
        .selected
        .saturating_sub(count / 2)
        .min(entries.len().saturating_sub(count));
    let lines: Vec<_> = entries
        .iter()
        .enumerate()
        .skip(start)
        .take(count)
        .map(|(i, e)| {
            Line::from(Span::styled(
                crate::ui::truncate(
                    &format!(
                        "{} {}  {}",
                        if i == menu.selected { "›" } else { " " },
                        e.label,
                        e.detail
                    ),
                    r.width.saturating_sub(4) as usize,
                    a.ambiguous_width,
                ),
                if i == menu.selected {
                    crate::ui::Theme::new(a).selected
                } else {
                    Style::default()
                },
            ))
        })
        .collect();
    crate::ui::render_text(
        f,
        a,
        if lines.is_empty() {
            vec![Line::from(if menu.kind == "save" {
                "Enter 保存 · Ctrl+U 清空路径"
            } else {
                "暂无条目"
            })]
        } else {
            lines
        },
        Rect::new(r.x + 2, r.y + 3, r.width - 4, r.height - 5),
    );
    crate::ui::render_text(
        f,
        a,
        "↑↓ 选择 · Enter 执行 · 输入筛选 · Esc 返回",
        Rect::new(r.x + 2, r.bottom() - 2, r.width - 4, 1),
    );
}
