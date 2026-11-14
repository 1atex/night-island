use serde::{Deserialize, Serialize};

pub const MAX_ITEMS: usize = 32;
pub const MAX_FRAME: usize = 32768;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Active,
    Paused,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Pause,
    Resume,
    Cancel,
    Reveal,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pause => "Pause",
            Self::Resume => "Resume",
            Self::Cancel => "Cancel download",
            Self::Reveal => "Show in folder",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Download {
    pub id: String,
    pub name: String,
    pub state: State,
    pub current: u64,
    pub total: Option<u64>,
    pub actions: Vec<Action>,
}

impl Download {
    pub fn progress(&self) -> Option<f32> {
        self.total.filter(|n| *n > 0).map(|n| (self.current as f64 / n as f64).clamp(0.0, 1.0) as f32)
    }
    pub fn valid(&self) -> bool {
        !self.id.is_empty()
            && self.id.len() <= 64
            && self.id.chars().all(|c| c.is_ascii_alphanumeric() || "{}-".contains(c))
            && !self.name.is_empty()
            && self.name.chars().count() <= 160
            && !self.name.chars().any(|c| c.is_control() || c == '/' || c == '\\')
            && self.current <= 9_007_199_254_740_991
            && self.total.is_none_or(|n| n <= 9_007_199_254_740_991)
            && self.actions.len() <= 4
            && self.actions.iter().all(|action| match action {
                Action::Pause => self.state == State::Active,
                Action::Resume => matches!(self.state, State::Paused | State::Failed),
                Action::Cancel => matches!(self.state, State::Active | State::Paused | State::Failed),
                Action::Reveal => self.state == State::Complete,
            })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Frame {
    Hello { v: u8, capabilities: Vec<String> },
    Downloads { v: u8, items: Vec<Download> },
    Result { v: u8, request: String, ok: bool },
}

pub fn parse(bytes: &[u8]) -> Option<Frame> {
    if bytes.len() >= MAX_FRAME {
        return None;
    }
    let frame = serde_json::from_slice(bytes).ok()?;
    match &frame {
        Frame::Hello { v, capabilities }
            if *v == 1 && capabilities.len() <= 8 && capabilities.iter().any(|c| c == "downloads") =>
        {
            Some(frame)
        }
        Frame::Downloads { v, items }
            if *v == 1 && items.len() <= MAX_ITEMS && items.iter().all(Download::valid) && {
                let mut ids = std::collections::HashSet::new();
                items.iter().all(|item| ids.insert(&item.id))
            } =>
        {
            Some(frame)
        }
        Frame::Result { v, request, .. } if *v == 1 && request.len() <= 64 => Some(frame),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_protocol_and_capabilities() {
        assert!(parse(br#"{"v":1,"type":"hello","capabilities":["downloads"]}"#).is_some());
        assert!(parse(br#"{"v":2,"type":"hello","capabilities":["downloads"]}"#).is_none());
        assert!(parse(&vec![b' '; MAX_FRAME]).is_none());
        assert!(parse(br#"{"v":1,"type":"downloads","items":[],"url":"secret"}"#).is_none());
    }
    #[test]
    fn commands_only_match_download_state() {
        let mut item = Download {
            id: "a-1".into(),
            name: "file.zip".into(),
            state: State::Active,
            current: 100,
            total: Some(200),
            actions: vec![Action::Pause],
        };
        assert!(item.valid());
        assert_eq!(item.progress(), Some(0.5));
        item.actions = vec![Action::Reveal];
        assert!(!item.valid());
        item.state = State::Complete;
        assert!(item.valid());
        item.name = "../file.zip".into();
        assert!(!item.valid());
    }
}
