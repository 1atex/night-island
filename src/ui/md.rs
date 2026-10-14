#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Bold,
    Italic,
    Code,
    Block,
    Heading(u8),
    Link,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub len: usize,
    pub style: Style,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Doc {
    pub text: String,
    pub spans: Vec<Span>,
}

struct Out {
    text: String,
    utf16: usize,
    spans: Vec<Span>,
}

impl Out {
    fn push(&mut self, s: &str) {
        self.text.push_str(s);
        self.utf16 += s.encode_utf16().count();
    }

    fn styled(&mut self, s: &str, style: Style) {
        let start = self.utf16;
        self.push(s);
        if self.utf16 > start {
            self.spans.push(Span { start, len: self.utf16 - start, style });
        }
    }
}

fn find(chars: &[char], from: usize, needle: &[char]) -> Option<usize> {
    if needle.is_empty() || chars.len() < needle.len() {
        return None;
    }
    (from..=chars.len() - needle.len()).find(|&i| chars[i..i + needle.len()] == *needle)
}

fn inline(line: &str, out: &mut Out, outer: Option<Style>) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Out| {
        if !plain.is_empty() {
            match outer {
                Some(style) => out.styled(plain, style),
                None => out.push(plain),
            }
            plain.clear();
        }
    };
    while i < chars.len() {
        let c = chars[i];
        let boundary = i == 0 || !chars[i - 1].is_alphanumeric();
        if c == '`'
            && let Some(end) = find(&chars, i + 1, &['`'])
        {
            flush(&mut plain, out);
            out.styled(&chars[i + 1..end].iter().collect::<String>(), Style::Code);
            i = end + 1;
            continue;
        }
        if c == '*'
            && chars.get(i + 1) == Some(&'*')
            && let Some(end) = find(&chars, i + 2, &['*', '*'])
        {
            flush(&mut plain, out);
            inline(&chars[i + 2..end].iter().collect::<String>(), out, Some(Style::Bold));
            i = end + 2;
            continue;
        }
        if (c == '*' || c == '_')
            && boundary
            && chars.get(i + 1).is_some_and(|n| !n.is_whitespace() && *n != c)
            && let Some(end) = find(&chars, i + 1, &[c])
            && end > i + 1
            && !chars[end - 1].is_whitespace()
            && chars.get(end + 1).is_none_or(|n| !n.is_alphanumeric())
        {
            flush(&mut plain, out);
            inline(&chars[i + 1..end].iter().collect::<String>(), out, Some(Style::Italic));
            i = end + 1;
            continue;
        }
        if c == '['
            && let Some(close) = find(&chars, i + 1, &[']', '('])
            && let Some(paren) = find(&chars, close + 2, &[')'])
        {
            flush(&mut plain, out);
            let label: String = chars[i + 1..close].iter().collect();
            inline(&label, out, Some(Style::Link));
            i = paren + 1;
            continue;
        }
        plain.push(c);
        i += 1;
    }
    flush(&mut plain, out);
}

pub fn parse(md: &str) -> Doc {
    let mut out = Out { text: String::new(), utf16: 0, spans: Vec::new() };
    let mut in_block = false;
    let mut block_start = 0;
    let mut blank = true;
    for raw in md.replace("\r\n", "\n").lines() {
        let line = raw.trim_end();
        if line.trim_start().starts_with("```") {
            if in_block {
                if out.utf16 > block_start {
                    out.spans.push(Span { start: block_start, len: out.utf16 - block_start, style: Style::Block });
                }
                in_block = false;
                out.push("\n");
                blank = false;
            } else {
                if !out.text.is_empty() && !out.text.ends_with("\n\n") {
                    out.push("\n");
                }
                in_block = true;
                block_start = out.utf16;
            }
            continue;
        }
        if in_block {
            out.push(raw);
            out.push("\n");
            continue;
        }
        if line.trim().is_empty() {
            if !blank {
                out.push("\n");
            }
            blank = true;
            continue;
        }
        blank = false;
        let trimmed = line.trim_start();
        let indent = (line.len() - trimmed.len()) / 2;
        let heading = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=4).contains(&heading) && trimmed[heading..].starts_with(' ') {
            let start = out.utf16;
            inline(trimmed[heading..].trim(), &mut out, None);
            let len = out.utf16 - start;
            if len > 0 {
                out.spans.push(Span { start, len, style: Style::Heading(heading as u8) });
            }
        } else if let Some(rest) =
            trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).or_else(|| trimmed.strip_prefix("+ "))
        {
            out.push(&format!("{}• ", "  ".repeat(indent.min(4))));
            inline(rest, &mut out, None);
        } else if let Some(rest) = trimmed.strip_prefix("> ") {
            let start = out.utf16;
            inline(rest, &mut out, None);
            out.spans.push(Span { start, len: out.utf16 - start, style: Style::Italic });
        } else {
            let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
            if digits > 0 && trimmed[digits..].starts_with(". ") {
                out.push(&format!("{}{} ", "  ".repeat(indent.min(4)), &trimmed[..digits + 1]));
                inline(&trimmed[digits + 2..], &mut out, None);
            } else {
                inline(trimmed, &mut out, None);
            }
        }
        out.push("\n");
    }
    let text = out.text.trim_end().to_string();
    let limit = text.encode_utf16().count();
    let spans = out
        .spans
        .into_iter()
        .filter(|s| s.start < limit)
        .map(|s| Span { len: s.len.min(limit - s.start), ..s })
        .collect();
    Doc { text, spans }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styled(d: &Doc, style: Style) -> Vec<String> {
        d.spans
            .iter()
            .filter(|s| s.style == style)
            .map(|s| {
                let units: Vec<u16> = d.text.encode_utf16().skip(s.start).take(s.len).collect();
                String::from_utf16_lossy(&units)
            })
            .collect()
    }

    #[test]
    fn inline_styles_are_found_and_markers_removed() {
        let d = parse("a **bold** and *it* and `code` and [link](http://x)");
        assert_eq!(d.text, "a bold and it and code and link");
        assert_eq!(styled(&d, Style::Bold), ["bold"]);
        assert_eq!(styled(&d, Style::Italic), ["it"]);
        assert_eq!(styled(&d, Style::Code), ["code"]);
        assert_eq!(styled(&d, Style::Link), ["link"]);
    }

    #[test]
    fn snake_case_and_math_stay_plain() {
        let d = parse("use my_var_name and 2 * 3 * 4");
        assert_eq!(d.text, "use my_var_name and 2 * 3 * 4");
        assert!(d.spans.is_empty());
    }

    #[test]
    fn blocks_headings_and_lists() {
        let d = parse("# Title\n\n- one\n- two\n\n```rust\nlet x = 1;\n```\n1. first");
        assert_eq!(styled(&d, Style::Heading(1)), ["Title"]);
        assert!(d.text.contains("• one\n• two"));
        assert_eq!(styled(&d, Style::Block), ["let x = 1;\n"]);
        assert!(d.text.ends_with("1. first"));
    }

    #[test]
    fn utf16_offsets_follow_wide_characters() {
        let d = parse("😀 **b**");
        let s = d.spans[0];
        assert_eq!((s.start, s.len), (3, 1));
    }

    #[test]
    fn unfinished_markers_are_left_alone() {
        assert_eq!(parse("**open and `tick").text, "**open and `tick");
    }
}
