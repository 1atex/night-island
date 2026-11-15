use crate::data::pocket::identifier;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Features {
    pub tabs: bool,
    pub pocket: bool,
    pub activity: bool,
    pub requests: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabItem {
    pub id: String,
    pub title: String,
    pub path: String,
    pub host: String,
    pub sleeping: bool,
    pub activity: Vec<String>,
    pub icon: String,
}
impl TabItem {
    pub fn valid(&self) -> bool {
        identifier(&self.id)
            && [&self.title, &self.path, &self.host]
                .iter()
                .all(|s| s.chars().count() <= 160 && !s.chars().any(char::is_control))
            && self.activity.len() <= 5
            && self
                .activity
                .iter()
                .all(|s| matches!(s.as_str(), "audio" | "muted" | "camera" | "microphone" | "screen"))
            && (self.icon.is_empty()
                || self.icon.len() == 1368
                    && self.icon.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b)))
    }
    pub fn icon(&self) -> Option<std::sync::Arc<crate::platform::apps::Icon>> {
        use base64::Engine;
        let bgra = base64::engine::general_purpose::STANDARD.decode(&self.icon).ok()?;
        (bgra.len() == 1024).then(|| std::sync::Arc::new(crate::platform::apps::Icon { side: 16, bgra }))
    }
    pub fn activity_label(&self) -> String {
        self.activity
            .iter()
            .filter_map(|s| match s.as_str() {
                "audio" if !self.activity.iter().any(|value| value == "muted") => Some("Audio"),
                "muted" => Some("Muted"),
                "camera" => Some("Camera"),
                "microphone" => Some("Microphone"),
                "screen" => Some("Screen sharing"),
                _ => None,
            })
            .map(|s| crate::ui::i18n::tr(s).into_owned())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestItem {
    pub id: String,
    pub method: String,
    pub host: String,
}
impl RequestItem {
    pub fn valid(&self) -> bool {
        identifier(&self.id)
            && !self.method.is_empty()
            && self.method.len() <= 20
            && self.method.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
            && self.host.chars().count() <= 160
            && !self.host.chars().any(char::is_control)
    }
}
