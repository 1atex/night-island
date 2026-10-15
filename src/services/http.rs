use windows::Foundation::Uri;
use windows::Storage::Streams::{DataReader, DataWriter, InMemoryRandomAccessStream, UnicodeEncoding};
use windows::Web::Http::Headers::HttpCredentialsHeaderValue;
use windows::Web::Http::{HttpBufferContent, HttpClient, HttpMethod, HttpRequestMessage, HttpStringContent};
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
    request_as(method, url, bearer, form, None)
}

pub fn request_as(
    method: &str,
    url: &str,
    bearer: Option<&str>,
    form: Option<&str>,
    agent: Option<&str>,
) -> Option<Response> {
    let verb = match method {
        "GET" => HttpMethod::Get().ok()?,
        "POST" => HttpMethod::Post().ok()?,
        "PUT" => HttpMethod::Put().ok()?,
        "DELETE" => HttpMethod::Delete().ok()?,
        _ => return None,
    };
    let client = HttpClient::new().ok()?;
    let message = HttpRequestMessage::Create(&verb, &Uri::CreateUri(&HSTRING::from(url)).ok()?).ok()?;
    if let Some(agent) = agent {
        message.Headers().ok()?.UserAgent().ok()?.TryParseAdd(&HSTRING::from(agent)).ok()?;
    }
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

pub struct BytesReply {
    pub status: u16,
    pub body: Vec<u8>,
}

pub fn get_bytes(url: &str, max_bytes: usize, agent: Option<&str>) -> Option<BytesReply> {
    send_bytes("GET", url, &[], "", max_bytes, agent)
}

pub fn send_bytes(
    method: &str,
    url: &str,
    body: &[u8],
    content_type: &str,
    max_bytes: usize,
    agent: Option<&str>,
) -> Option<BytesReply> {
    let verb = match method {
        "GET" => HttpMethod::Get().ok()?,
        "POST" => HttpMethod::Post().ok()?,
        "PUT" => HttpMethod::Put().ok()?,
        _ => return None,
    };
    let client = HttpClient::new().ok()?;
    let message = HttpRequestMessage::Create(&verb, &Uri::CreateUri(&HSTRING::from(url)).ok()?).ok()?;
    if let Some(agent) = agent {
        message.Headers().ok()?.UserAgent().ok()?.TryParseAdd(&HSTRING::from(agent)).ok()?;
    }
    if !body.is_empty() {
        let stream = InMemoryRandomAccessStream::new().ok()?;
        let writer = DataWriter::CreateDataWriter(&stream).ok()?;
        writer.WriteBytes(body).ok()?;
        let content = HttpBufferContent::CreateFromBuffer(&writer.DetachBuffer().ok()?).ok()?;
        content
            .Headers()
            .ok()?
            .TryAppendWithoutValidation(&HSTRING::from("Content-Type"), &HSTRING::from(content_type))
            .ok()?;
        message.SetContent(&content).ok()?;
    }
    let reply = client.SendRequestAsync(&message).ok()?.join().ok()?;
    let status = reply.StatusCode().ok()?.0 as u16;
    let content = reply.Content().ok()?;
    if let Ok(len) = content.Headers().and_then(|h| h.ContentLength())
        && let Ok(len) = len.Value()
        && len > max_bytes as u64
    {
        return None;
    }
    let buffer = content.ReadAsBufferAsync().ok()?.join().ok()?;
    let len = buffer.Length().ok()? as usize;
    if len > max_bytes {
        return None;
    }
    let mut bytes = vec![0u8; len];
    if len > 0 {
        DataReader::FromBuffer(&buffer).ok()?.ReadBytes(&mut bytes).ok()?;
    }
    Some(BytesReply { status, body: bytes })
}

pub fn post_json(url: &str, headers: &[(&str, &str)], body: &str, agent: Option<&str>) -> Option<Response> {
    let client = HttpClient::new().ok()?;
    let message =
        HttpRequestMessage::Create(&HttpMethod::Post().ok()?, &Uri::CreateUri(&HSTRING::from(url)).ok()?).ok()?;
    let sent = message.Headers().ok()?;
    if let Some(agent) = agent {
        sent.UserAgent().ok()?.TryParseAdd(&HSTRING::from(agent)).ok()?;
    }
    for (name, value) in headers {
        if !sent.TryAppendWithoutValidation(&HSTRING::from(*name), &HSTRING::from(*value)).ok()? {
            return None;
        }
    }
    let content = HttpStringContent::CreateFromStringWithEncodingAndMediaType(
        &HSTRING::from(body),
        UnicodeEncoding::Utf8,
        &HSTRING::from("application/json"),
    )
    .ok()?;
    message.SetContent(&content).ok()?;
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
