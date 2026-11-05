use std::net::IpAddr;

use crate::services::http;

pub const MAX_PAGE_CHARS: usize = 3500;
const MAX_PAGE_BYTES: usize = 2_000_000;
const MAX_HITS: usize = 8;
const BROWSER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

const BLOCK_TAGS: [&str; 22] = [
    "p",
    "div",
    "br",
    "li",
    "ul",
    "ol",
    "tr",
    "table",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "section",
    "article",
    "header",
    "footer",
    "blockquote",
    "pre",
    "hr",
    "dd",
];
const SKIPPED_TAGS: [&str; 6] = ["script", "style", "noscript", "svg", "template", "head"];

pub fn public_host(host: &str) -> bool {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    if host.is_empty() || host == "localhost" {
        return false;
    }
    if [".localhost", ".local", ".internal", ".lan", ".home"].iter().any(|s| host.ends_with(s)) {
        return false;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            !(ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.octets()[0] == 0
                || (ip.octets()[0] == 100 && (64..128).contains(&ip.octets()[1])))
        }
        Ok(IpAddr::V6(ip)) => {
            let first = ip.segments()[0];
            !(ip.is_loopback() || ip.is_unspecified() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80)
        }
        Err(_) => host.contains('.'),
    }
}

pub fn check_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or("Only http and https addresses can be opened")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') || authority.is_empty() {
        return Err("That address is not allowed".into());
    }
    let host = if let Some(inner) = authority.strip_prefix('[') {
        inner.split(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    if public_host(host) { Ok(()) } else { Err("That address is not allowed".into()) }
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest.find(';').filter(|&end| end <= 10).and_then(|end| {
            let name = &rest[1..end];
            let ch = match name {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                "nbsp" => ' ',
                "mdash" => '\u{2014}',
                "ndash" => '\u{2013}',
                "hellip" => '\u{2026}',
                "laquo" => '\u{ab}',
                "raquo" => '\u{bb}',
                _ => {
                    let number = name.strip_prefix('#')?;
                    let code = match number.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => number.parse().ok()?,
                    };
                    char::from_u32(code)?
                }
            };
            Some((ch, end + 1))
        });
        match decoded {
            Some((ch, used)) => {
                out.push(ch);
                rest = &rest[used..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn tidy_text(raw: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        let flat = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if flat.is_empty() && lines.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        lines.push(flat);
    }
    lines.join("\n").trim().to_string()
}

pub fn html_to_text(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len() / 3);
    let mut at = 0;
    while at < html.len() {
        if html.as_bytes()[at] != b'<' {
            let next = html[at..].find('<').map_or(html.len(), |e| at + e);
            out.push_str(&html[at..next]);
            at = next;
            continue;
        }
        if lower[at..].starts_with("<!--") {
            at = lower[at + 4..].find("-->").map_or(html.len(), |e| at + 4 + e + 3);
            continue;
        }
        let end = lower[at..].find('>').map_or(html.len(), |e| at + e + 1);
        let inner = &lower[at + 1..end.saturating_sub(1).max(at + 1)];
        let closing = inner.starts_with('/');
        let name: String = inner.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        if !closing && SKIPPED_TAGS.contains(&name.as_str()) {
            let close = format!("</{name}");
            at = match lower[end..].find(&close) {
                Some(e) => lower[end + e..].find('>').map_or(html.len(), |g| end + e + g + 1),
                None => html.len(),
            };
            continue;
        }
        if BLOCK_TAGS.contains(&name.as_str()) && !out.ends_with('\n') {
            out.push('\n');
        }
        at = end;
    }
    tidy_text(&decode_entities(&out))
}

fn page_text(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let head: String = text.chars().take(2000).collect::<String>().to_ascii_lowercase();
    let readable = if head.contains("<html") || head.contains("<body") || head.contains("<div") || head.contains("<p") {
        html_to_text(&text)
    } else {
        tidy_text(&text)
    };
    let cut: String = readable.chars().take(MAX_PAGE_CHARS).collect();
    if readable.chars().count() > MAX_PAGE_CHARS { format!("{cut}\n[cut]") } else { cut }
}

pub fn fetch(url: &str) -> Result<String, String> {
    check_url(url)?;
    let reply = http::get_bytes(url.trim(), MAX_PAGE_BYTES, Some(BROWSER_AGENT))
        .ok_or("The page could not be loaded or is too large")?;
    if reply.status >= 400 {
        return Err(format!("The site answered with status {}", reply.status));
    }
    let text = page_text(&reply.body);
    if text.is_empty() { Err("The page has no readable text".into()) } else { Ok(text) }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes.get(at + 1..at + 3).and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[at], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                at += 1;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn result_url(href: &str) -> Option<String> {
    let href = decode_entities(href);
    let url = match href.find("uddg=") {
        Some(at) => percent_decode(href[at + 5..].split('&').next().unwrap_or("")),
        None if href.starts_with("//") => format!("https:{href}"),
        None => href,
    };
    (url.starts_with("http") && !url.contains("duckduckgo.com/y.js")).then_some(url)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub fn parse_results(html: &str) -> Vec<Hit> {
    let marker = "class=\"result__a\"";
    let mut hits = Vec::new();
    let mut from = 0;
    while let Some(found) = html[from..].find(marker) {
        let at = from + found;
        let next = html[at + marker.len()..].find(marker).map_or(html.len(), |e| at + marker.len() + e);
        from = at + marker.len();
        let Some(open) = html[..at].rfind("<a ") else { continue };
        let Some(gt) = html[at..].find('>').map(|e| at + e) else { continue };
        let tag = &html[open..gt];
        let Some(href) = tag.find("href=\"").map(|e| &tag[e + 6..]).and_then(|r| r.split('"').next()) else { continue };
        let Some(url) = result_url(href) else { continue };
        let title_end = html[gt..].find("</a>").map_or(next, |e| gt + e);
        let title = html_to_text(&html[gt + 1..title_end]);
        let snippet = html[title_end..next]
            .find("result__snippet")
            .and_then(|s| {
                let from_snippet = title_end + s;
                let start = html[from_snippet..].find('>')? + from_snippet + 1;
                let stop = ["</a>", "</div>", "</td>"]
                    .iter()
                    .filter_map(|t| html[start..].find(t))
                    .min()
                    .map_or(next, |e| start + e);
                Some(html_to_text(&html[start..stop.min(next)]))
            })
            .unwrap_or_default();
        if !title.is_empty() {
            hits.push(Hit { title, url, snippet });
        }
        if hits.len() == MAX_HITS {
            break;
        }
    }
    hits
}

pub fn format_hits(hits: &[Hit]) -> String {
    hits.iter()
        .enumerate()
        .map(|(i, h)| {
            let snippet: String = h.snippet.chars().take(220).collect();
            format!("{}. {}\n{}\n{}", i + 1, h.title, h.url, snippet).trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn search(query: &str) -> Result<String, String> {
    let query: String = query.chars().filter(|c| !c.is_control()).collect();
    let query = query.trim();
    if query.is_empty() {
        return Err("The search is empty".into());
    }
    let url = format!("https://html.duckduckgo.com/html/?q={}", http::pct(query));
    let reply = http::get_bytes(&url, MAX_PAGE_BYTES, Some(BROWSER_AGENT)).ok_or("The search could not be made")?;
    if reply.status >= 400 {
        return Err(format!("The search service answered with status {}", reply.status));
    }
    let hits = parse_results(&String::from_utf8_lossy(&reply.body));
    if hits.is_empty() { Err("The search found nothing".into()) } else { Ok(format_hits(&hits)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_and_private_addresses_are_not_fetched() {
        for url in [
            "http://localhost/x",
            "http://127.0.0.1:8080/",
            "https://192.168.1.1/",
            "http://10.0.0.5",
            "http://169.254.169.254/latest/meta-data",
            "http://[::1]/",
            "http://[fd00::1]/",
            "http://printer.local/",
            "http://user:pw@example.com/",
            "file:///C:/Windows/win.ini",
            "ftp://example.com/",
            "http://intranet/",
        ] {
            assert!(check_url(url).is_err(), "{url}");
        }
        for url in ["https://example.com/a?b=c", "http://93.184.216.34/", "https://sub.example.org:8443/x"] {
            assert!(check_url(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn a_page_becomes_readable_text() {
        let html = "<html><head><title>T</title><style>p{color:red}</style></head><body><h1>Head &amp; tail</h1>\
<script>var x = '<p>no</p>';</script><p>One <b>bold</b> word</p><!-- hidden --><p>Two&nbsp;&#33; &#x41;</p></body></html>";
        assert_eq!(html_to_text(html), "Head & tail\nOne bold word\nTwo ! A");
    }

    #[test]
    fn broken_markup_does_not_panic() {
        for html in ["<", "<p", "<script>never closed", "<!-- open", "a < b > c", "&;&#;&#xZZ;&", "<div>日本語</div>"]
        {
            let _ = html_to_text(html);
        }
        assert_eq!(html_to_text("<div>日本語</div>"), "日本語");
    }

    #[test]
    fn plain_text_is_kept_and_long_pages_are_cut() {
        assert_eq!(page_text(b"  just   text \n\n\n and more "), "just text\n\nand more");
        let long = page_text("<p>word </p>".repeat(2000).as_bytes());
        assert!(long.ends_with("[cut]") && long.chars().count() <= MAX_PAGE_CHARS + 6);
    }

    #[test]
    fn search_results_are_read_from_the_page() {
        let html = "<div class=\"result\"><h2><a rel=\"nofollow\" class=\"result__a\" href=\"//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa%3Fb%3D1&amp;rut=abc\">Example <b>one</b></a></h2>\
<a class=\"result__snippet\" href=\"x\">First <b>snippet</b> here</a></div>\
<div class=\"result\"><h2><a class=\"result__a\" href=\"https://second.org/\">Second</a></h2></div>\
<div class=\"result\"><a class=\"result__a\" href=\"//duckduckgo.com/y.js?ad=1\">Ad</a></div>";
        let hits = parse_results(html);
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0],
            Hit {
                title: "Example one".into(),
                url: "https://example.com/a?b=1".into(),
                snippet: "First snippet here".into()
            }
        );
        assert_eq!(hits[1].url, "https://second.org/");
        assert!(format_hits(&hits).starts_with("1. Example one\nhttps://example.com/a?b=1\nFirst snippet here"));
        assert!(parse_results("<html>nothing</html>").is_empty());
    }

    #[test]
    fn percent_decoding_handles_utf8_and_junk() {
        assert_eq!(percent_decode("a%20b%D0%B0+c"), "a bа c");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    #[ignore = "needs the internet"]
    fn a_live_search_and_page_work() {
        let found = search("rust programming language").expect("search");
        assert!(found.contains("http"));
        let page = fetch("https://www.rust-lang.org/").expect("page");
        assert!(page.len() > 100);
    }
}
