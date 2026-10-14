use windows::Foundation::Uri;
use windows::Storage::Streams::UnicodeEncoding;
use windows::Web::Http::Headers::HttpCredentialsHeaderValue;
use windows::Web::Http::{HttpClient, HttpMethod, HttpRequestMessage, HttpStringContent};
use windows::core::HSTRING;

const MAX_BODY_CHARS: usize = 600_000;

pub struct Response {
    pub status: u16,
    pub body: String,
}

pub fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn request(method: &str, url: &str, bearer: Option<&str>, form: Option<&str>) -> Option<Response> {
    let verb = match method {
        "GET" => HttpMethod::Get().ok()?,
        "POST" => HttpMethod::Post().ok()?,
        "PUT" => HttpMethod::Put().ok()?,
        "DELETE" => HttpMethod::Delete().ok()?,
        _ => return None,
    };
    let client = HttpClient::new().ok()?;
    let message = HttpRequestMessage::Create(&verb, &Uri::CreateUri(&HSTRING::from(url)).ok()?).ok()?;
    if let Some(token) = bearer {
        let value =
            HttpCredentialsHeaderValue::CreateFromSchemeWithToken(&HSTRING::from("Bearer"), &HSTRING::from(token))
                .ok()?;
        message.Headers().ok()?.SetAuthorization(&value).ok()?;
    }
    if let Some(body) = form {
        let content = HttpStringContent::CreateFromStringWithEncodingAndMediaType(
            &HSTRING::from(body),
            UnicodeEncoding::Utf8,
            &HSTRING::from("application/x-www-form-urlencoded"),
        )
        .ok()?;
        message.SetContent(&content).ok()?;
    }
    let reply = client.SendRequestAsync(&message).ok()?.join().ok()?;
    let status = reply.StatusCode().ok()?.0 as u16;
    let body = reply.Content().ok()?.ReadAsStringAsync().ok()?.join().ok()?.to_string();
    Some(Response { status, body: body.chars().take(MAX_BODY_CHARS).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_keeps_unreserved_only() {
        assert_eq!(pct("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
        assert_eq!(pct("Az09-_.~"), "Az09-_.~");
    }
}
