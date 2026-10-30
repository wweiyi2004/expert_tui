//! Welcome-screen pet driven entirely by a pack file.
//!
//! A pack is one JSON file: palette, same-sized frames, an idle loop and the
//! actions a click cycles through. Megumin is bundled as the default pack;
//! others are loaded by name from `<config dir>/pets/<name>.json` or by path.
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const DEFAULT_PACK: &str = "megumin";
const BUNDLED: &str = include_str!("../../../design/tui/pet/megumin.json");

#[derive(Debug, Deserialize)]
pub struct Art {
    #[serde(default)]
    pub name: String,
    pub palette: BTreeMap<char, String>,
    pub frames: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub idle: Idle,
    /// Action ids started by successive clicks, cycled in order.
    #[serde(default)]
    pub clicks: Vec<String>,
    #[serde(default)]
    pub actions: Vec<Action>,
    /// How long a line stays in the speech bubble after its action starts.
    #[serde(default = "default_quote_ms")]
    pub quote_ms: u64,
}
#[derive(Debug, Deserialize)]
pub struct Idle {
    /// `[frame, milliseconds]` pairs played in a loop.
    pub frames: Vec<(String, u64)>,
    #[serde(default)]
    pub blink: Option<String>,
}
impl Default for Idle {
    fn default() -> Self {
        Self {
            frames: vec![("idle".into(), 0)],
            blink: None,
        }
    }
}
#[derive(Debug, Deserialize)]
pub struct Action {
    pub id: String,
    pub frames: Vec<(String, u64)>,
    /// Loop the frames until this many milliseconds; defaults to one pass.
    #[serde(default)]
    pub duration: Option<u64>,
    /// Stay on the last frame until clicked instead of finishing.
    #[serde(default)]
    pub hold: bool,
    /// Action to continue with when this one finishes.
    #[serde(default)]
    pub next: Option<String>,
    /// Action to play instead when the terminal asks for reduced motion.
    #[serde(default)]
    pub reduced: Option<String>,
    /// `[start milliseconds, line]` pairs; the latest started line is shown.
    #[serde(default)]
    pub quotes: Vec<(u64, String)>,
    /// Line shown when a held action is dismissed by a click.
    #[serde(default)]
    pub release_quote: Option<String>,
}
impl Action {
    fn length(&self) -> u64 {
        self.duration
            .unwrap_or_else(|| self.frames.iter().map(|(_, ms)| ms).sum())
    }
}
fn default_quote_ms() -> u64 {
    2500
}
/// Frame of a `[frame, ms]` timeline at `elapsed`; loops when `cycle`, else holds the last.
fn timeline(frames: &[(String, u64)], elapsed: u64, cycle: bool) -> &str {
    let total: u64 = frames.iter().map(|(_, ms)| ms).sum();
    let mut t = if cycle && total > 0 {
        elapsed % total
    } else {
        elapsed
    };
    for (name, ms) in frames {
        if t < *ms {
            return name;
        }
        t -= ms;
    }
    &frames[frames.len() - 1].0
}

impl Art {
    /// The bundled default pack.
    pub fn load() -> Self {
        Self::parse(BUNDLED).expect("bundled pet JSON validated")
    }
    pub fn parse(json: &str) -> Result<Self, String> {
        let art: Art = serde_json::from_str(json).map_err(|e| format!("宠物包 JSON 无效：{e}"))?;
        art.validate()?;
        Ok(art)
    }
    pub fn dimensions(&self) -> (usize, usize) {
        let frame = &self.frames[&self.idle.frames[0].0];
        (frame[0].len(), frame.len())
    }
    fn action(&self, id: &str) -> Option<usize> {
        self.actions.iter().position(|a| a.id == id)
    }
    fn validate(&self) -> Result<(), String> {
        for (key, hex) in &self.palette {
            if !key.is_ascii_graphic() || *key == '.' {
                return Err(format!("调色板键 {key:?} 必须是除 '.' 外的单个 ASCII 字符"));
            }
            if crate::theme::parse_color(hex).is_none() || hex.len() != 7 {
                return Err(format!("调色板 {key} 的颜色 {hex} 不是 #RRGGBB"));
            }
        }
        let Some(first) = self.frames.values().next() else {
            return Err("宠物包没有任何帧".into());
        };
        let (w, h) = (first.first().map_or(0, String::len), first.len());
        if !(1..=256).contains(&w) || !(1..=256).contains(&h) {
            return Err("帧尺寸须在 1–256 像素之间".into());
        }
        for (name, rows) in &self.frames {
            if rows.len() != h || rows.iter().any(|r| r.len() != w) {
                return Err(format!("帧 {name} 尺寸与其他帧不一致（应为 {w}×{h}）"));
            }
            if let Some(c) = rows
                .iter()
                .flat_map(|r| r.chars())
                .find(|c| *c != '.' && !self.palette.contains_key(c))
            {
                return Err(format!("帧 {name} 使用了调色板中没有的字符 {c:?}"));
            }
        }
        let frame = |name: &str| {
            self.frames
                .contains_key(name)
                .then_some(())
                .ok_or(format!("引用了不存在的帧 {name}"))
        };
        if self.idle.frames.is_empty() {
            return Err("idle 至少需要一帧".into());
        }
        for (name, _) in &self.idle.frames {
            frame(name)?;
        }
        if let Some(name) = &self.idle.blink {
            frame(name)?;
        }
        let action = |id: &str| {
            self.action(id)
                .map(|_| ())
                .ok_or(format!("引用了不存在的动作 {id}"))
        };
        for id in &self.clicks {
            action(id)?;
        }
        for a in &self.actions {
            if a.frames.is_empty() {
                return Err(format!("动作 {} 没有帧", a.id));
            }
            for (name, _) in &a.frames {
                frame(name)?;
            }
            for id in a.next.iter().chain(&a.reduced) {
                action(id)?;
            }
            if !a.hold && a.length() == 0 {
                return Err(format!(
                    "动作 {} 时长为 0；非 hold 动作需要帧时长或 duration",
                    a.id
                ));
            }
        }
        Ok(())
    }
}

/// Directory searched for packs named in the configuration.
pub fn pack_dir() -> PathBuf {
    crate::config::path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
        .join("pets")
}
/// Resolve a configured pack: the bundled default, a `.json` path, or a name in [`pack_dir`].
pub fn load_pack(name: &str) -> Result<Art, String> {
    if name == DEFAULT_PACK {
        return Ok(Art::load());
    }
    let path = if name.ends_with(".json") || name.contains(['/', '\\']) {
        PathBuf::from(name)
    } else {
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
        {
            return Err(format!("宠物包名称 {name} 无效"));
        }
        pack_dir().join(format!("{name}.json"))
    };
    let data =
        std::fs::read_to_string(&path).map_err(|_| format!("找不到宠物包 {}", path.display()))?;
    Art::parse(&data).map_err(|e| format!("宠物包 {name}：{e}"))
}
/// Bundled pack followed by the packs installed in [`pack_dir`].
pub fn available() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(pack_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            (path.extension()? == "json")
                .then(|| path.file_stem()?.to_str().map(str::to_owned))
                .flatten()
        })
        .filter(|n| n != DEFAULT_PACK)
        .collect();
    names.sort();
    names.insert(0, DEFAULT_PACK.into());
    names
}

#[derive(Debug)]
pub struct Pet {
    pub enabled: bool,
    /// Configured pack name, kept so `/pet on` restores it.
    pub pack: String,
    pub art: Art,
    /// 0 is idle; `n` is `art.actions[n - 1]`.
    pub action: usize,
    pub clicks: u32,
    pub started: Option<Instant>,
    /// Line left over after an action ends, shown until the instant.
    pub lingering: Option<(String, Instant)>,
    pub visible: bool,
    pub blink_at: Option<Instant>,
    pub blink_seed: u64,
    idle_started: Instant,
    reduced: bool,
}
impl Default for Pet {
    fn default() -> Self {
        Self {
            enabled: true,
            pack: DEFAULT_PACK.into(),
            art: Art::load(),
            action: 0,
            clicks: 0,
            started: None,
            lingering: None,
            visible: false,
            blink_at: None,
            blink_seed: 17,
            idle_started: Instant::now(),
            reduced: false,
        }
    }
}
impl Pet {
    /// Switch to another pack, resetting the animation.
    pub fn select(&mut self, name: &str) -> Result<(), String> {
        let art = load_pack(name)?;
        *self = Self {
            enabled: self.enabled,
            pack: name.into(),
            art,
            ..Self::default()
        };
        Ok(())
    }
    fn current(&self) -> Option<&Action> {
        self.action.checked_sub(1).map(|i| &self.art.actions[i])
    }
    fn elapsed(&self, now: Instant) -> u64 {
        self.started
            .map_or(0, |t| now.saturating_duration_since(t).as_millis() as u64)
    }
    fn start(&mut self, id: &str, now: Instant) {
        self.action = self.art.action(id).map_or(0, |i| i + 1);
        self.started = Some(now);
        self.lingering = None;
    }
    pub fn click(&mut self, now: Instant, reduced: bool) {
        if !self.enabled {
            return;
        }
        match self.current() {
            Some(action) if action.hold => {
                let release = action.release_quote.clone();
                self.action = 0;
                self.started = Some(now);
                self.lingering =
                    release.map(|q| (q, now + Duration::from_millis(self.art.quote_ms)));
            }
            Some(_) => {}
            None if self.art.clicks.is_empty() => {}
            None => {
                self.clicks += 1;
                let index = (self.clicks as usize - 1) % self.art.clicks.len();
                let mut id = self.art.clicks[index].clone();
                if reduced
                    && let Some(alt) = self.art.actions[self.art.action(&id).unwrap()]
                        .reduced
                        .clone()
                {
                    id = alt;
                }
                self.start(&id, now);
            }
        }
    }
    pub fn tick(&mut self, now: Instant, reduced: bool) {
        self.reduced = reduced;
        if !self.visible {
            return;
        }
        if let Some(action) = self.current()
            && !action.hold
            && self.elapsed(now) >= action.length()
        {
            match action.next.clone() {
                Some(next) => self.start(&next, now),
                None => {
                    let quote = self.quote(now).to_owned();
                    let until =
                        self.started.unwrap_or(now) + Duration::from_millis(self.art.quote_ms);
                    self.action = 0;
                    self.lingering = (!quote.is_empty()).then_some((quote, until));
                }
            }
        }
        if reduced || self.art.idle.blink.is_none() {
            return;
        }
        if self.blink_at.is_none() {
            self.blink_seed = self
                .blink_seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1);
            self.blink_at = Some(now + Duration::from_millis(3000 + self.blink_seed % 3001));
        } else if self
            .blink_at
            .is_some_and(|t| now >= t + Duration::from_millis(150))
        {
            self.blink_at = None;
        }
    }
    pub fn frame(&self, now: Instant) -> &str {
        if let Some(action) = self.current() {
            if self.reduced {
                return &action.frames[0].0;
            }
            return timeline(&action.frames, self.elapsed(now), action.duration.is_some());
        }
        if let Some(blink) = &self.art.idle.blink
            && self.blink_at.is_some_and(|t| now >= t)
        {
            return blink;
        }
        if self.reduced {
            return &self.art.idle.frames[0].0;
        }
        let idle = now.saturating_duration_since(self.idle_started).as_millis() as u64;
        timeline(&self.art.idle.frames, idle, true)
    }
    pub fn quote(&self, now: Instant) -> &str {
        if let Some(action) = self.current() {
            let elapsed = self.elapsed(now);
            if elapsed >= self.art.quote_ms {
                return "";
            }
            return action
                .quotes
                .iter()
                .rev()
                .find(|(at, _)| *at <= elapsed)
                .map_or("", |(_, q)| q);
        }
        match &self.lingering {
            Some((quote, until)) if now < *until => quote,
            _ => "",
        }
    }
}
