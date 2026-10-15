use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde_json::json;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use crate::data::settings::{AiProvider, AiSettings};
use crate::services::{http, integrations};

pub const WM_AI: u32 = WM_APP + 21;

const USER_AGENT: &str = "NightIsland/0.1";
const MAX_PROMPT_CHARS: usize = 4000;
const MAX_ANSWER_CHARS: usize = 4000;
const MAX_OUTPUT_TOKENS: u32 = 600;
const SYSTEM_PROMPT: &str =
    "Answer briefly and plainly, in the language of the question, in at most a few sentences. No markdown.";

pub type Answer = Result<String, String>;

static CONFIG: OnceLock<Mutex<AiSettings>> = OnceLock::new();
static RESULT: Mutex<Option<Answer>> = Mutex::new(None);
static BUSY: AtomicBool = AtomicBool::new(false);

pub fn configure(settings: &AiSettings) {
    let cell = CONFIG.get_or_init(|| Mutex::new(AiSettings::default()));
    if let Ok(mut g) = cell.lock() {
        *g = settings.clone();
    }
}

fn config() -> AiSettings {
    CONFIG.get().and_then(|c| c.lock().ok().map(|g| g.clone())).unwrap_or_default()
}

pub fn active_provider() -> AiProvider {
    config().provider
}

fn secret_name(provider: AiProvider) -> String {
    format!("ai_{}", provider.id())
}

pub fn valid_key(key: &str) -> bool {
    let key = key.trim();
    (8..=300).contains(&key.len()) && key.chars().all(|c| c.is_ascii_graphic())
}

pub fn has_key(provider: AiProvider) -> bool {
    integrations::dir().is_some_and(|d| d.join(format!("{}.bin", secret_name(provider))).exists())
}

pub fn save_key(provider: AiProvider, key: &str) -> bool {
    if !valid_key(key) {
        return false;
    }
    integrations::save_secret(&secret_name(provider), key.trim());
    has_key(provider)
}

pub fn remove_key(provider: AiProvider) {
    integrations::delete_secret(&secret_name(provider));
}

pub fn valid_model(model: &str, provider: AiProvider) -> bool {
    let ok = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    !model.is_empty()
        && model.len() <= 80
        && model.chars().all(|c| ok(c) || (provider != AiProvider::Gemini && matches!(c, ':' | '/')))
}

pub fn clean_base(base: &str) -> Option<String> {
    let base = base.trim().trim_end_matches('/');
    let rest = base.strip_prefix("https://").or_else(|| {
        let local = base.strip_prefix("http://")?;
        let host = local.split(['/', ':']).next()?;
        matches!(host, "localhost" | "127.0.0.1" | "[::1]").then_some(local)
    })?;
    let valid = !rest.is_empty() && base.len() <= 200 && base.chars().all(|c| c.is_ascii_graphic());
    valid.then(|| base.to_string())
}

#[derive(Debug, PartialEq)]
pub struct Call {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: String,
}

pub fn build_call(settings: &AiSettings, key: &str, prompt: &str) -> Option<Call> {
    let provider = settings.provider;
    let model = settings.model(provider);
    if !valid_model(model, provider) {
        return None;
    }
    let prompt: String = prompt.chars().take(MAX_PROMPT_CHARS).collect();
    Some(match provider {
        AiProvider::Claude => Call {
            url: "https://api.anthropic.com/v1/messages".into(),
            headers: vec![("x-api-key", key.into()), ("anthropic-version", "2023-06-01".into())],
            body: json!({
                "model": model,
                "max_tokens": MAX_OUTPUT_TOKENS,
                "system": SYSTEM_PROMPT,
                "messages": [{"role": "user", "content": prompt}],
            })
            .to_string(),
        },
        AiProvider::OpenAi => Call {
            url: format!("{}/chat/completions", clean_base(&settings.openai_base)?),
            headers: vec![("Authorization", format!("Bearer {key}"))],
            body: json!({
                "model": model,
                "max_tokens": MAX_OUTPUT_TOKENS,
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": prompt},
                ],
            })
            .to_string(),
        },
        AiProvider::Gemini => Call {
            url: format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"),
            headers: vec![("x-goog-api-key", key.into())],
            body: json!({
                "systemInstruction": {"parts": [{"text": SYSTEM_PROMPT}]},
                "contents": [{"role": "user", "parts": [{"text": prompt}]}],
                "generationConfig": {"maxOutputTokens": MAX_OUTPUT_TOKENS},
            })
            .to_string(),
        },
    })
}

fn tidy(text: &str) -> String {
    let clean: String = text.chars().filter(|c| *c == '\n' || !c.is_control()).collect();
    clean.trim().chars().take(MAX_ANSWER_CHARS).collect()
}

fn error_message(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let err = value.get("error")?;
    let text = err.get("message").and_then(|m| m.as_str()).or_else(|| err.as_str())?;
    Some(tidy(text).chars().take(140).collect())
}

pub fn parse_reply(provider: AiProvider, status: u16, body: &str) -> Answer {
    if status == 401 || status == 403 {
        return Err("The key was rejected".into());
    }
    if status == 429 {
        return Err("Too many requests, try again later".into());
    }
    if !(200..300).contains(&status) {
        let why = error_message(body);
        if why
            .as_deref()
            .is_some_and(|w| w.to_lowercase().contains("api key") && w.to_lowercase().contains("not valid"))
        {
            return Err("The key was rejected".into());
        }
        return Err(match why {
            Some(why) if !why.is_empty() => format!("Error {status}: {why}"),
            _ => format!("Error {status}"),
        });
    }
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| "The reply was not understood")?;
    let text = match provider {
        AiProvider::Claude => value["content"]
            .as_array()
            .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("")),
        AiProvider::OpenAi => value["choices"][0]["message"]["content"].as_str().map(str::to_string),
        AiProvider::Gemini => value["candidates"][0]["content"]["parts"]
            .as_array()
            .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("")),
    };
    match text.map(|t| tidy(&t)) {
        Some(t) if !t.is_empty() => Ok(t),
        _ => Err("The answer was empty".into()),
    }
}

pub fn take_result() -> Option<Answer> {
    RESULT.lock().ok()?.take()
}

pub fn ask(hwnd: HWND, prompt: &str) -> bool {
    if BUSY.swap(true, Ordering::SeqCst) {
        return false;
    }
    let (raw, prompt, settings) = (hwnd.0 as isize, prompt.to_string(), config());
    std::thread::spawn(move || {
        let answer = run(&settings, &prompt);
        if let Ok(mut r) = RESULT.lock() {
            *r = Some(answer);
        }
        BUSY.store(false, Ordering::SeqCst);
        unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_AI, WPARAM(0), LPARAM(0));
        }
    });
    true
}

fn run(settings: &AiSettings, prompt: &str) -> Answer {
    let provider = settings.provider;
    let Some(key) = integrations::load_secret(&secret_name(provider)) else {
        return Err("No key is saved for this service".into());
    };
    let Some(call) = build_call(settings, &key, prompt) else {
        return Err("The model or the server address in the settings is not valid".into());
    };
    let headers: Vec<(&str, &str)> = call.headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    match http::post_json(&call.url, &headers, &call.body, Some(USER_AGENT)) {
        Some(reply) => {
            crate::log_debug!("AI request answered with status {}", reply.status);
            parse_reply(provider, reply.status, &reply.body)
        }
        None => {
            crate::log_warn!("AI request could not be made");
            Err("No connection to the service".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(provider: AiProvider) -> AiSettings {
        AiSettings { provider, ..AiSettings::default() }
    }

    #[test]
    fn claude_call_has_the_key_in_a_header_only() {
        let call = build_call(&settings(AiProvider::Claude), "sk-secret-key", "Hi?").unwrap();
        assert_eq!(call.url, "https://api.anthropic.com/v1/messages");
        assert!(call.headers.contains(&("x-api-key", "sk-secret-key".to_string())));
        assert!(call.headers.contains(&("anthropic-version", "2023-06-01".to_string())));
        assert!(!call.body.contains("sk-secret-key") && !call.url.contains("sk-secret-key"));
        let body: serde_json::Value = serde_json::from_str(&call.body).unwrap();
        assert_eq!(body["messages"][0]["content"], "Hi?");
        assert_eq!(body["max_tokens"], MAX_OUTPUT_TOKENS);
    }

    #[test]
    fn openai_and_gemini_calls() {
        let mut s = settings(AiProvider::OpenAi);
        s.openai_base = "https://openrouter.ai/api/v1/".into();
        let call = build_call(&s, "k1234567", "Hi?").unwrap();
        assert_eq!(call.url, "https://openrouter.ai/api/v1/chat/completions");
        assert!(call.headers.contains(&("Authorization", "Bearer k1234567".to_string())));
        let g = build_call(&settings(AiProvider::Gemini), "k1234567", "Hi?").unwrap();
        assert!(g.url.starts_with("https://generativelanguage.googleapis.com/v1beta/models/"));
        assert!(g.url.ends_with(":generateContent") && !g.url.contains("k1234567"));
        assert!(g.headers.contains(&("x-goog-api-key", "k1234567".to_string())));
    }

    #[test]
    fn unsafe_models_and_addresses_are_refused() {
        let mut s = settings(AiProvider::Gemini);
        s.gemini_model = "../evil?x=1".into();
        assert!(build_call(&s, "k1234567", "x").is_none());
        s.gemini_model = "gemini/x".into();
        assert!(build_call(&s, "k1234567", "x").is_none(), "no slashes in a URL path");
        let mut o = settings(AiProvider::OpenAi);
        o.openai_model = "openai/gpt-4o".into();
        assert!(valid_model(&o.openai_model, AiProvider::OpenAi), "OpenRouter style names are fine");
        o.openai_base = "http://example.com/v1".into();
        assert!(build_call(&o, "k1234567", "x").is_none(), "plain HTTP only for this computer");
        o.openai_base = "http://localhost:1234/v1".into();
        assert!(build_call(&o, "k1234567", "x").is_some());
        assert_eq!(clean_base("https://a.b/v1/"), Some("https://a.b/v1".into()));
        assert_eq!(clean_base("ftp://a.b"), None);
        assert_eq!(clean_base("https://a b"), None);
        assert_eq!(clean_base("http://localhost.evil.com/v1"), None);
    }

    #[test]
    fn long_questions_are_cut() {
        let call = build_call(&settings(AiProvider::Claude), "k1234567", &"я".repeat(50_000)).unwrap();
        let body: serde_json::Value = serde_json::from_str(&call.body).unwrap();
        assert_eq!(body["messages"][0]["content"].as_str().unwrap().chars().count(), MAX_PROMPT_CHARS);
    }

    #[test]
    fn replies_are_read_per_provider() {
        let claude = r#"{"content":[{"type":"text","text":"Hello "},{"type":"text","text":"there"}]}"#;
        assert_eq!(parse_reply(AiProvider::Claude, 200, claude), Ok("Hello there".into()));
        let openai = r#"{"choices":[{"message":{"role":"assistant","content":"  Hi\u0007!  "}}]}"#;
        assert_eq!(parse_reply(AiProvider::OpenAi, 200, openai), Ok("Hi!".into()));
        let gemini = r#"{"candidates":[{"content":{"parts":[{"text":"Hey"}]}}]}"#;
        assert_eq!(parse_reply(AiProvider::Gemini, 200, gemini), Ok("Hey".into()));
        assert!(parse_reply(AiProvider::Gemini, 200, r#"{"candidates":[]}"#).is_err());
        assert!(parse_reply(AiProvider::Claude, 200, "not json").is_err());
    }

    #[test]
    fn failures_have_short_reasons_without_secrets() {
        assert_eq!(parse_reply(AiProvider::Claude, 401, "{}"), Err("The key was rejected".into()));
        assert_eq!(parse_reply(AiProvider::OpenAi, 429, ""), Err("Too many requests, try again later".into()));
        let body = r#"{"error":{"message":"model not found"}}"#;
        assert_eq!(parse_reply(AiProvider::OpenAi, 404, body), Err("Error 404: model not found".into()));
        assert_eq!(parse_reply(AiProvider::Gemini, 500, "<html>"), Err("Error 500".into()));
        let gemini_bad_key = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key."}}"#;
        assert_eq!(parse_reply(AiProvider::Gemini, 400, gemini_bad_key), Err("The key was rejected".into()));
        let long = format!(r#"{{"error":{{"message":"{}"}}}}"#, "x".repeat(1000));
        assert!(parse_reply(AiProvider::OpenAi, 400, &long).unwrap_err().chars().count() < 170);
    }

    #[test]
    #[ignore = "needs the network"]
    fn the_real_services_refuse_a_made_up_key() {
        for provider in [AiProvider::Claude, AiProvider::OpenAi, AiProvider::Gemini] {
            let call = build_call(&settings(provider), "not-a-real-key-12345", "Hi").unwrap();
            let headers: Vec<(&str, &str)> = call.headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let reply = http::post_json(&call.url, &headers, &call.body, Some(USER_AGENT)).expect("a connection");
            println!("{provider:?}: status {}", reply.status);
            let answer = parse_reply(provider, reply.status, &reply.body);
            assert!(answer.is_err(), "{provider:?} accepted a made-up key");
            assert!(matches!(reply.status, 400 | 401 | 403), "{provider:?} answered {}", reply.status);
        }
    }

    #[test]
    fn keys_must_look_like_keys() {
        assert!(valid_key("sk-ant-api03-abcdefgh"));
        assert!(!valid_key("short"));
        assert!(!valid_key("has a space in it"));
        assert!(!valid_key(&"k".repeat(400)));
        assert!(!valid_key("línea-con-acentos-1234"));
    }
}
