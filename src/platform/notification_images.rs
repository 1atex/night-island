use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use windows::Data::Xml::Dom::{XmlDocument, XmlElement};
use windows::Foundation::Uri;
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode, ExifOrientationMode,
};
use windows::Storage::StorageFile;
use windows::Storage::Streams::IRandomAccessStreamWithContentType;
use windows::UI::Notifications::ToastNotificationManager;
use windows::core::{HSTRING, Interface};

use super::apps::Icon;
use super::notifications::text;

struct ImageSource {
    title: String,
    body: String,
    source: String,
}

#[derive(Default)]
pub(super) struct AvatarCache {
    history: Option<rusqlite::Connection>,
    sources: HashMap<String, Vec<ImageSource>>,
    images: HashMap<String, Arc<Icon>>,
}

impl AvatarCache {
    pub fn begin_snapshot(&mut self) {
        self.sources.clear();
        self.history = super::notification_payload::open();
        if self.images.len() > 128 {
            self.images.clear();
        }
    }

    pub fn resolve_notification(
        &mut self,
        id: u32,
        app_id: &str,
        title: &str,
        body: &str,
    ) -> (bool, Option<Arc<Icon>>) {
        if let Some(xml) =
            self.history.as_ref().and_then(|db| super::notification_payload::read(db, id, app_id, title, body))
        {
            let urgent = xml
                .DocumentElement()
                .and_then(|root| root.GetAttribute(&HSTRING::from("scenario")))
                .is_ok_and(|scenario| matches!(scenario.to_string().as_str(), "incomingCall" | "alarm" | "reminder"));
            let avatar = match image_source(&xml) {
                Some(item) => self.resolve_image(app_id, &item.source),
                None => self.resolve(app_id, title, body),
            };
            return (urgent, avatar);
        }
        (false, self.resolve(app_id, title, body))
    }

    pub fn resolve(&mut self, app_id: &str, title: &str, body: &str) -> Option<Arc<Icon>> {
        let sources = self.sources.entry(app_id.into()).or_insert_with(|| {
            ToastNotificationManager::History()
                .and_then(|history| history.GetHistoryWithId(&HSTRING::from(app_id)))
                .ok()
                .into_iter()
                .flat_map(|toasts| toasts.into_iter().take(128))
                .filter_map(|toast| toast.Content().ok())
                .filter_map(|xml| image_source(&xml))
                .collect()
        });
        let mut matches = sources.iter().filter(|item| item.title == title && item.body == body);
        let source = &matches.next()?.source;
        if matches.any(|item| &item.source != source) {
            return None;
        }
        let source = source.clone();
        self.resolve_image(app_id, &source)
    }

    fn resolve_image(&mut self, app_id: &str, source: &str) -> Option<Arc<Icon>> {
        if let Some(image) = self.images.get(source) {
            return Some(image.clone());
        }
        if app_id.to_ascii_lowercase().contains("discord") && discord_avatar_uri(source).is_some() {
            return remote_avatar(source);
        }
        let image = load(source)?;
        self.images.insert(source.to_string(), image.clone());
        Some(image)
    }
}

fn image_source(xml: &XmlDocument) -> Option<ImageSource> {
    let nodes = xml.GetElementsByTagName(&HSTRING::from("image")).ok()?;
    let elements: Vec<_> = (0..nodes.Length().ok()?.min(16))
        .filter_map(|i| nodes.Item(i).ok())
        .filter_map(|node| node.cast::<XmlElement>().ok())
        .collect();
    let legacy = xml
        .GetElementsByTagName(&HSTRING::from("binding"))
        .ok()?
        .Item(0)
        .ok()?
        .cast::<XmlElement>()
        .ok()?
        .GetAttribute(&HSTRING::from("template"))
        .ok()?
        .to_string()
        .starts_with("ToastImageAndText");
    let image = elements
        .iter()
        .find(|element| element.GetAttribute(&HSTRING::from("placement")).is_ok_and(|v| v == "appLogoOverride"))
        .or_else(|| {
            legacy
                .then(|| {
                    elements.iter().find(|element| element.GetAttribute(&HSTRING::from("id")).is_ok_and(|v| v == "1"))
                })
                .flatten()
        })?;
    let source = image.GetAttribute(&HSTRING::from("src")).ok()?.to_string();
    if source.is_empty() || source.len() > 4096 {
        return None;
    }
    let nodes = xml.GetElementsByTagName(&HSTRING::from("text")).ok()?;
    let lines: Vec<_> = (0..nodes.Length().ok()?.min(8))
        .filter_map(|i| nodes.Item(i).ok())
        .filter_map(|node| node.InnerText().ok())
        .map(|line| text(&line.to_string(), 16384))
        .filter(|line| !line.is_empty())
        .collect();
    Some(ImageSource {
        title: text(lines.first()?, 512),
        body: text(&lines.into_iter().skip(1).collect::<Vec<_>>().join("\n"), 16384),
        source,
    })
}

fn local_image_uri(source: &str) -> Option<Uri> {
    if !source.get(..5).is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:")) {
        return None;
    }
    let uri = Uri::CreateUri(&HSTRING::from(source)).ok()?;
    if uri.SchemeName().ok()? != "file" || !uri.Host().ok()?.is_empty() {
        return None;
    }
    let path = uri.Path().ok()?.to_string();
    let path = path.trim_start_matches('/');
    let bytes = path.as_bytes();
    (bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && matches!(bytes[2], b'/' | b'\\'))
        .then_some(uri)
}

fn load(source: &str) -> Option<Arc<Icon>> {
    let path =
        if local_drive_path(source) { source.replace('/', "\\") } else { local_image_path(&local_image_uri(source)?)? };
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path)).ok()?.join().ok()?;
    let stream = file.OpenReadAsync().ok()?.join().ok()?;
    decode(&stream, 96)
}

fn local_image_path(uri: &Uri) -> Option<String> {
    let path = Uri::UnescapeComponent(&uri.Path().ok()?).ok()?.to_string();
    Some(path.trim_start_matches('/').replace('/', "\\"))
}

pub(super) fn decode(stream: &IRandomAccessStreamWithContentType, side: u32) -> Option<Arc<Icon>> {
    if stream.Size().ok()? > 4 * 1024 * 1024 {
        return None;
    }
    decode_bitmap(BitmapDecoder::CreateAsync(stream).ok()?.join().ok()?, side)
}

fn decode_bitmap(decoder: BitmapDecoder, side: u32) -> Option<Arc<Icon>> {
    if decoder.PixelWidth().ok()? > 4096 || decoder.PixelHeight().ok()? > 4096 {
        return None;
    }
    let transform = BitmapTransform::new().ok()?;
    transform.SetScaledWidth(side).ok()?;
    transform.SetScaledHeight(side).ok()?;
    let data = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .ok()?
        .join()
        .ok()?
        .DetachPixelData()
        .ok()?;
    (data.len() == (side * side * 4) as usize).then(|| Arc::new(Icon { side, bgra: data.to_vec() }))
}

fn local_drive_path(source: &str) -> bool {
    let bytes = source.as_bytes();
    bytes.len() > 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
        && !source.chars().any(char::is_control)
}

fn discord_avatar_uri(source: &str) -> Option<Uri> {
    let uri = Uri::CreateUri(&HSTRING::from(source)).ok()?;
    let host = uri.Host().ok()?.to_string().to_ascii_lowercase();
    let path = uri.Path().ok()?.to_string();
    (uri.SchemeName().ok()? == "https"
        && matches!(host.as_str(), "cdn.discordapp.com" | "media.discordapp.net")
        && uri.UserName().ok()?.is_empty()
        && uri.Password().ok()?.is_empty()
        && uri.Port().ok()? == 443
        && (path.starts_with("/avatars/")
            || path.starts_with("/embed/avatars/")
            || (path.starts_with("/guilds/") && path.contains("/users/") && path.contains("/avatars/"))))
    .then_some(uri)
}

type RemoteImages = HashMap<String, (Instant, Option<Arc<Icon>>)>;
static REMOTE_IMAGES: Mutex<Option<RemoteImages>> = Mutex::new(None);
static REMOTE_WORKER: OnceLock<mpsc::SyncSender<String>> = OnceLock::new();

fn remote_avatar(source: &str) -> Option<Arc<Icon>> {
    let worker = REMOTE_WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<String>(32);
        std::thread::spawn(move || {
            unsafe {
                let _ =
                    windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);
            }
            while let Ok(source) = rx.recv() {
                let image = download_avatar(&source);
                if let Ok(mut cache) = REMOTE_IMAGES.lock() {
                    cache.get_or_insert_with(HashMap::new).insert(source, (Instant::now(), image));
                }
                super::notifications::wake();
            }
        });
        tx
    });
    let mut cache = REMOTE_IMAGES.lock().ok()?;
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some((at, image)) = cache.get(source)
        && (image.is_some() || at.elapsed() < Duration::from_secs(60))
    {
        return image.clone();
    }
    if cache.len() >= 128
        && let Some(oldest) = cache.iter().min_by_key(|(_, v)| v.0).map(|(k, _)| k.clone())
    {
        cache.remove(&oldest);
    }
    worker.try_send(source.into()).ok()?;
    cache.insert(source.into(), (Instant::now(), None));
    None
}

fn download_avatar(source: &str) -> Option<Arc<Icon>> {
    use windows::Storage::Streams::{Buffer, DataWriter, InMemoryRandomAccessStream, InputStreamOptions};
    use windows::Web::Http::Filters::HttpBaseProtocolFilter;
    use windows::Web::Http::{HttpClient, HttpCompletionOption};
    let filter = HttpBaseProtocolFilter::new().ok()?;
    filter.SetAllowAutoRedirect(false).ok()?;
    let client = HttpClient::Create(&filter).ok()?;
    let op = client.GetWithOptionAsync(&discord_avatar_uri(source)?, HttpCompletionOption::ResponseHeadersRead).ok()?;
    let deadline = Instant::now() + Duration::from_secs(6);
    while op.Status().ok()?.0 == 0 {
        if Instant::now() >= deadline {
            let _ = op.Cancel();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let response = op.GetResults().ok()?;
    if response.StatusCode().ok()?.0 != 200 {
        return None;
    }
    let content = response.Content().ok()?;
    if content.Headers().ok()?.ContentLength().ok().and_then(|v| v.Value().ok()).is_some_and(|v| v > 4 * 1024 * 1024) {
        return None;
    }
    let input = content.ReadAsInputStreamAsync().ok()?.join().ok()?;
    let memory = InMemoryRandomAccessStream::new().ok()?;
    let writer = DataWriter::CreateDataWriter(&memory).ok()?;
    let mut total = 0u32;
    loop {
        let buffer = Buffer::Create(65536).ok()?;
        let op = input.ReadAsync(&buffer, 65536, InputStreamOptions::Partial).ok()?;
        while op.Status().ok()?.0 == 0 {
            if Instant::now() >= deadline {
                let _ = op.Cancel();
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let buffer = op.GetResults().ok()?;
        let length = buffer.Length().ok()?;
        if length == 0 {
            break;
        }
        total += length;
        if total > 4 * 1024 * 1024 {
            return None;
        }
        writer.WriteBuffer(&buffer).ok()?;
    }
    writer.StoreAsync().ok()?.join().ok()?;
    writer.DetachStream().ok()?;
    memory.Seek(0).ok()?;
    decode_bitmap(BitmapDecoder::CreateAsync(&memory).ok()?.join().ok()?, 96)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Downloads one public default Discord avatar without account data"]
    fn public_discord_avatar_decodes() -> windows::core::Result<()> {
        unsafe {
            windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED)?;
        }
        let image = download_avatar("https://cdn.discordapp.com/embed/avatars/0.png");
        unsafe {
            windows::Win32::System::WinRT::RoUninitialize();
        }
        let image = image.expect("public Discord avatar did not download and decode");
        assert_eq!(image.side, 96);
        assert_eq!(image.bgra.len(), 96 * 96 * 4);
        Ok(())
    }

    #[test]
    #[ignore = "Needs WinRT (Uri, toast XML, imaging), which wine does not provide"]
    fn discord_avatars_accept_cdn_and_native_paths_without_other_remote_content() {
        assert!(discord_avatar_uri("https://cdn.discordapp.com/avatars/123/avatar.png?size=128").is_some());
        assert!(discord_avatar_uri("https://media.discordapp.net/embed/avatars/0.png").is_some());
        for url in [
            "https://cdn.discordapp.com.evil.test/avatars/1/a.png",
            "https://cdn.discordapp.com/attachments/a.png",
            "http://cdn.discordapp.com/avatars/1/a.png",
            "https://cdn.discordapp.com:123/avatars/1/a.png",
            "https://user:pass@cdn.discordapp.com/avatars/1/a.png",
        ] {
            assert!(discord_avatar_uri(url).is_none(), "{url}");
        }
        assert!(local_drive_path(r"C:\Users\User\AppData\Local\Temp\discord.png"));
        assert!(!local_drive_path(r"\\server\avatar.png"));
        assert!(!local_drive_path("C:avatar.png"));
    }

    #[test]
    #[ignore = "Needs WinRT (Uri, toast XML, imaging), which wine does not provide"]
    fn legacy_discord_toasts_preserve_sender_image_and_long_message() -> windows::core::Result<()> {
        let xml = XmlDocument::new()?;
        let body = "message ".repeat(300);
        xml.LoadXml(&HSTRING::from(format!(r#"<toast><visual><binding template="ToastImageAndText02"><image id="1" src="C:\Temp\discord.png"/><text id="1">Sender</text><text id="2">{body}</text></binding></visual></toast>"#)))?;
        let source = image_source(&xml).unwrap();
        assert!(local_drive_path(&source.source));
        assert_eq!(source.title, "Sender");
        assert_eq!(source.body, body.trim());
        Ok(())
    }

    #[test]
    #[ignore = "Needs WinRT (Uri, toast XML, imaging), which wine does not provide"]
    fn decoded_avatar_survives_removal_of_the_temporary_sender_image() -> windows::core::Result<()> {
        use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED)?;
        }
        struct Runtime;
        impl Drop for Runtime {
            fn drop(&mut self) {
                unsafe {
                    RoUninitialize();
                }
            }
        }
        let _runtime = Runtime;
        let path = std::env::temp_dir().join(format!(
            "night-island-avatar-{}-{}.png",
            std::process::id(),
            super::super::notifications::now_ticks()
        ));
        struct File(std::path::PathBuf);
        impl Drop for File {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _file = File(path.clone());
        let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(&[100, 120, 180, 255].repeat(4)).unwrap();
        let source = format!("file:///{}", path.to_string_lossy().replace('\\', "/"));
        let mut cache = AvatarCache::default();
        cache
            .sources
            .insert("test".into(), vec![ImageSource { title: "Sender".into(), body: "Message".into(), source }]);
        let mixed = path.to_string_lossy().replace("\\", "/");
        assert!(load(&mixed).is_some());
        let image = cache.resolve("test", "Sender", "Message").expect("local notification image should decode");
        assert_eq!(image.side, 96);
        assert_eq!(image.bgra.len(), 96 * 96 * 4);
        std::fs::remove_file(&path).unwrap();
        let cached = cache.resolve("test", "Sender", "Message").unwrap();
        assert!(Arc::ptr_eq(&image, &cached));
        Ok(())
    }

    #[test]
    #[ignore = "Needs WinRT (Uri, toast XML, imaging), which wine does not provide"]
    fn avatar_uri_accepts_local_files_and_rejects_remote_sources() {
        assert!(local_image_uri("file:///C:/Users/Test/avatar.png").is_some());
        assert!(local_image_uri("file:///C:\\Users\\Test\\avatar.png").is_some());
        for source in [
            "https://example.com/a.png",
            "http://localhost/a.png",
            "file://server/share/a.png",
            "file:////server/share/a.png",
            "data:image/png;base64,eA==",
            "C:\\test.png",
        ] {
            assert!(local_image_uri(source).is_none(), "{source}");
        }
    }

    #[test]
    #[ignore = "Needs WinRT (Uri, toast XML, imaging), which wine does not provide"]
    fn toast_avatar_uses_the_sender_image_and_matches_the_complete_message() -> windows::core::Result<()> {
        let xml = XmlDocument::new()?;
        xml.LoadXml(&HSTRING::from(r#"<toast><visual><binding template="ToastGeneric"><image placement="hero" src="wrong.png"/><image placement="appLogoOverride" src="file:///C:/avatar.png"/><text>Александр</text><text>Привет &amp; спасибо</text><text>Ещё строка</text></binding></visual></toast>"#))?;
        let item = image_source(&xml).unwrap();
        assert_eq!(item.source, "file:///C:/avatar.png");
        assert_eq!(item.title, "Александр");
        assert_eq!(item.body, "Привет & спасибо\nЕщё строка");
        let mut cache = AvatarCache::default();
        cache.images.insert(item.source.clone(), Arc::new(Icon { side: 1, bgra: vec![0, 0, 0, 255] }));
        cache.sources.insert("test".into(), vec![item]);
        assert!(cache.resolve("test", "Александр", "Другой текст").is_none());
        assert!(cache.resolve("test", "Александр", "Привет & спасибо\nЕщё строка").is_some());
        cache.sources.get_mut("test").unwrap().push(ImageSource {
            title: "Александр".into(),
            body: "Привет & спасибо\nЕщё строка".into(),
            source: "file:///C:/other.png".into(),
        });
        assert!(cache.resolve("test", "Александр", "Привет & спасибо\nЕщё строка").is_none());
        Ok(())
    }
}
