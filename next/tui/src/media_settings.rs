use crate::app::{App, FormKind, TerminalForm, items, text};
use serde_json::json;

const FORMATS: &[(&str, &str)] = &[
    ("auto", "自动"),
    ("openAiCompatible", "OpenAI 兼容"),
    ("geminiNative", "Gemini 原生"),
];
const AUDIO_FORMATS: &[(&str, &str)] = &[("auto", "自动"), ("openAiCompatible", "OpenAI 兼容")];
const BOOLS: &[(&str, &str)] = &[("false", "否"), ("true", "是")];
const SPEECH: &[(&str, &str)] = &[
    ("openAiAudio", "OpenAI Audio"),
    ("mimoChatCompletions", "MiMo"),
    ("aliyunModelStudio", "百炼"),
];
const IMAGE_RESPONSE: &[(&str, &str)] = &[
    ("auto", "服务商默认"),
    ("b64_json", "Base64（避免失效链接）"),
    ("url", "图片链接"),
];
const IMAGE_MODES: &[(&str, &str)] = &[
    ("off", "关闭聊天生图"),
    ("auto", "按需生成"),
    ("always", "优先生图"),
];
const VISION_FORMATS: &[(&str, &str)] = &[
    ("auto", "自动"),
    ("openAiCompatible", "OpenAI 兼容"),
    ("openAiResponses", "Responses"),
    ("anthropicMessages", "Anthropic"),
    ("geminiNative", "Gemini 原生"),
];

impl App {
    pub fn edit_media(&mut self, kind: &str) {
        let title = match kind {
            "imageGeneration" => "图片生成 API",
            "vision" => "视觉理解 API",
            "asr" => "语音识别 API",
            "tts" => "语音合成 API",
            _ => return,
        };
        let item = if kind == "asr" && !self.snapshot["settings"]["media"][kind].is_object() {
            self.snapshot["settings"]["asr"].clone()
        } else {
            self.snapshot["settings"]["media"][kind].clone()
        };
        let mut fields = vec![
            Self::field("base_url", "Base URL", text(&item, "base_url"), false, true),
            Self::field("model", "模型 ID", text(&item, "model"), false, true),
            Self::choice(
                "api_format",
                "接口格式",
                item["api_format"].as_str().unwrap_or("auto"),
                if kind == "asr" || kind == "tts" {
                    AUDIO_FORMATS
                } else if kind == "vision" {
                    VISION_FORMATS
                } else {
                    FORMATS
                },
            ),
            Self::field(
                "api_key",
                if item["has_key"] == true {
                    "API Key（已保存，留空保留）"
                } else {
                    "API Key"
                },
                "",
                true,
                true,
            ),
            Self::choice("clear_key", "清除已保存 Key", "false", BOOLS),
        ];
        if kind == "imageGeneration" {
            fields.push(Self::choice(
                "mode",
                "聊天生图",
                item["mode"].as_str().unwrap_or("auto"),
                IMAGE_MODES,
            ));
            fields.push(Self::choice(
                "image_response_format",
                "图片返回格式",
                item["image_response_format"].as_str().unwrap_or("auto"),
                IMAGE_RESPONSE,
            ));
            fields.push(Self::field(
                "image_size",
                "图片尺寸 / 比例（如 1024x1024 / auto）",
                item["image_size"].as_str().unwrap_or("1024x1024"),
                false,
                true,
            ));
        }
        if kind == "asr" {
            fields.push(Self::choice(
                "enabled",
                "启用语音识别",
                if item["enabled"] == true {
                    "true"
                } else {
                    "false"
                },
                BOOLS,
            ));
        }
        if kind == "tts" {
            fields.push(Self::choice(
                "speech_protocol",
                "语音协议",
                item["speech_protocol"].as_str().unwrap_or("openAiAudio"),
                SPEECH,
            ));
            fields.push(Self::field(
                "voice",
                "音色 ID（可选）",
                text(&item, "voice"),
                false,
                true,
            ));
            fields.push(Self::field(
                "voice_design_prompt",
                "音色 / 风格描述（可选）",
                text(&item, "voice_design_prompt"),
                false,
                true,
            ));
            fields.push(Self::field(
                "voice_clone_path",
                "克隆音频绝对路径（可选）",
                text(&item, "voice_clone_path"),
                false,
                true,
            ));
        }
        fields.push(Self::field("__save", "保存配置", "", false, false));
        for provider in items(&self.snapshot["settings"], "providers") {
            if text(provider, "auth_mode") == "apiKey" && provider["has_key"] == true {
                fields.push(Self::field(
                    &format!("__import:{}", text(provider, "id")),
                    &format!("从 {} 导入地址与 Key", text(provider, "name")),
                    "",
                    false,
                    false,
                ));
            }
        }
        self.stack_dialog();
        self.form_revision = self.form_revision.wrapping_add(1);
        self.form = Some(TerminalForm {
            kind: FormKind::Media,
            title: title.into(),
            original_fields: fields.clone(),
            fields,
            selected: 0,
            editing: true,
            cursor: 0,
            notice: "Key 留空保留；保存不会发送生成请求。导入后请确认模型 ID。".into(),
            items: vec![json!({"kind":kind})],
            item_selected: 0,
        });
    }

    pub(crate) fn media_form_action(&mut self) -> Vec<crate::app::Command> {
        let Some(form) = self.form.as_ref() else {
            return vec![];
        };
        let field = &form.fields[form.selected];
        let kind = text(&form.items[0], "kind").to_owned();
        if field.key != "__save" && !field.key.starts_with("__import:") {
            return vec![];
        }
        let mut params = json!({"kind":kind});
        let method;
        if let Some(profile) = field.key.strip_prefix("__import:") {
            method = "media.import";
            params["profile_id"] = json!(profile);
        } else {
            method = "media.save";
            for f in &form.fields {
                if !f.editable || f.key == "api_key" && f.value.is_empty() {
                    continue;
                }
                params[&f.key] = if ["enabled", "clear_key"].contains(&f.key.as_str()) {
                    json!(f.value == "true")
                } else {
                    json!(f.value)
                };
            }
        }
        self.pending_form_values = form.fields.clone();
        let command = self.command(method, params);
        self.pending_form = Some((self.form_revision, command.clone()));
        vec![command]
    }
}
