use std::io::Read;
use std::path::Path;
use std::sync::Arc;

pub const MAX_FILES: usize = 4;
const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_PDF_BYTES: u64 = 12 * 1024 * 1024;
const MAX_TEXT_BYTES: u64 = 1024 * 1024;
pub const MAX_TEXT_CHARS: usize = 60_000;
const MAX_NAME_CHARS: usize = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    Pdf,
    Text,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Image => "image",
            Kind::Pdf => "pdf",
            Kind::Text => "text",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "image" => Kind::Image,
            "pdf" => Kind::Pdf,
            "text" => Kind::Text,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub name: String,
    pub kind: Kind,
    pub mime: &'static str,
    pub data: Arc<Vec<u8>>,
}

impl Attachment {
    pub fn kept(&self) -> bool {
        !self.data.is_empty()
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.data).into_owned()
    }

    pub fn restored(name: &str, kind: Kind, text: &str) -> Self {
        let (mime, data) = match kind {
            Kind::Text => ("text/plain", text.as_bytes().to_vec()),
            Kind::Image => ("image/png", Vec::new()),
            Kind::Pdf => ("application/pdf", Vec::new()),
        };
        Self { name: clean_name(name), kind, mime, data: Arc::new(data) }
    }
}

pub fn clean_name(name: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).collect();
    name.trim().chars().take(MAX_NAME_CHARS).collect()
}

fn image_mime(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn classify(name: &str, bytes: Vec<u8>, whole: bool) -> Result<Attachment, &'static str> {
    let name = clean_name(name);
    let make = |kind, mime, data: Vec<u8>| Attachment { name: name.clone(), kind, mime, data: Arc::new(data) };
    if let Some(mime) = image_mime(&bytes) {
        return if whole { Ok(make(Kind::Image, mime, bytes)) } else { Err("That image is too large") };
    }
    if bytes.starts_with(b"%PDF-") {
        return if whole { Ok(make(Kind::Pdf, "application/pdf", bytes)) } else { Err("That PDF is too large") };
    }
    let probe = &bytes[..bytes.len().min(8192)];
    if probe.contains(&0) {
        return Err("Only images, PDF and text files can be attached");
    }
    let text = match std::str::from_utf8(&bytes) {
        Ok(t) => t,
        Err(e) if !whole && e.error_len().is_none() => std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""),
        Err(_) => return Err("Only images, PDF and text files can be attached"),
    };
    let cut: String = text.chars().take(MAX_TEXT_CHARS).collect();
    Ok(make(Kind::Text, "text/plain", cut.into_bytes()))
}

pub fn load(path: &Path) -> Result<Attachment, &'static str> {
    let meta = std::fs::metadata(path).map_err(|_| "That file cannot be read")?;
    if !meta.is_file() {
        return Err("Only files can be attached");
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let size = meta.len();
    let unreadable = |_| "That file cannot be read";
    let mut file = std::fs::File::open(path).map_err(unreadable)?;
    let mut bytes = Vec::new();
    (&mut file).take(MAX_TEXT_BYTES).read_to_end(&mut bytes).map_err(unreadable)?;
    let media = if image_mime(&bytes).is_some() {
        Some((MAX_IMAGE_BYTES, "That image is too large"))
    } else if bytes.starts_with(b"%PDF-") {
        Some((MAX_PDF_BYTES, "That PDF is too large"))
    } else {
        None
    };
    if let Some((limit, too_large)) = media {
        if size > limit {
            return Err(too_large);
        }
        (&mut file).take(limit).read_to_end(&mut bytes).map_err(unreadable)?;
    }
    let whole = bytes.len() as u64 >= size;
    classify(&name, bytes, whole)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: [u8; 12] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];

    #[test]
    fn files_are_told_apart_by_their_bytes() {
        assert_eq!(classify("a.txt", PNG.to_vec(), true).unwrap().kind, Kind::Image, "a PNG named .txt is an image");
        assert_eq!(classify("x.png", b"%PDF-1.7 ...".to_vec(), true).unwrap().kind, Kind::Pdf);
        let t = classify("n.md", "привет, мир".as_bytes().to_vec(), true).unwrap();
        assert_eq!((t.kind, t.text().as_str()), (Kind::Text, "привет, мир"));
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        jpeg.extend_from_slice(&[0; 20]);
        assert_eq!(classify("p", jpeg, true).unwrap().mime, "image/jpeg");
        let mut webp = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
        webp.extend_from_slice(&[0; 8]);
        assert_eq!(classify("p", webp, true).unwrap().mime, "image/webp");
    }

    #[test]
    fn programs_and_binary_data_are_refused() {
        assert!(classify("a.exe", b"MZ\x90\0\x03\0\0\0".to_vec(), true).is_err());
        assert!(classify("a.zip", b"PK\x03\x04\xff\xfe\xfd".to_vec(), true).is_err());
        assert!(classify("a.bin", vec![0xC3, 0x28, 0xA0], true).is_err(), "invalid UTF-8");
    }

    #[test]
    fn long_text_is_cut_and_big_media_refused() {
        let long = "я".repeat(MAX_TEXT_CHARS + 500);
        let a = classify("big.txt", long.into_bytes(), true).unwrap();
        assert_eq!(a.text().chars().count(), MAX_TEXT_CHARS);
        assert!(classify("i.png", PNG.to_vec(), false).is_err(), "an image that did not fit is refused");
        let mut half = "ab".as_bytes().to_vec();
        half.push(0xD1);
        assert_eq!(classify("t", half, false).unwrap().text(), "ab");
    }

    #[test]
    fn names_are_one_clean_line() {
        assert_eq!(clean_name("a\nb\u{7}.txt"), "ab.txt");
        assert_eq!(clean_name(&"x".repeat(200)).chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn folders_and_missing_files_are_refused() {
        let dir = std::env::temp_dir();
        assert_eq!(load(&dir), Err("Only files can be attached"));
        assert!(load(&dir.join("night-island-no-such-file.txt")).is_err());
    }

    #[test]
    fn a_file_on_disk_is_loaded() {
        let path = std::env::temp_dir().join(format!("ni-attach-{}.txt", std::process::id()));
        std::fs::write(&path, "hello").unwrap();
        let a = load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!((a.kind, a.text().as_str()), (Kind::Text, "hello"));
    }

    #[test]
    fn a_reopened_media_file_is_not_kept() {
        let a = Attachment::restored("p.png", Kind::Image, "");
        assert!(!a.kept());
        assert!(Attachment::restored("n.txt", Kind::Text, "x").kept());
    }
}
