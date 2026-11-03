use std::collections::HashMap;
use std::sync::Arc;

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
    sources: HashMap<String, Vec<ImageSource>>,
    images: HashMap<String, Arc<Icon>>,
}

impl AvatarCache {
    pub fn begin_snapshot(&mut self) {
        self.sources.clear();
        if self.images.len() > 128 {
            self.images.clear();
        }
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
        if let Some(image) = self.images.get(source) {
            return Some(image.clone());
        }
        let image = load(source)?;
        self.images.insert(source.clone(), image.clone());
        Some(image)
    }
}

fn image_source(xml: &XmlDocument) -> Option<ImageSource> {
    let nodes = xml.GetElementsByTagName(&HSTRING::from("image")).ok()?;
    let source = (0..nodes.Length().ok()?.min(16))
        .filter_map(|i| nodes.Item(i).ok())
        .filter_map(|node| node.cast::<XmlElement>().ok())
        .find_map(|element| {
            (element.GetAttribute(&HSTRING::from("placement")).ok()? == "appLogoOverride")
                .then(|| element.GetAttribute(&HSTRING::from("src")).ok())
                .flatten()
                .map(|source| source.to_string())
                .filter(|source| !source.is_empty() && source.len() <= 4096)
        })?;
    let nodes = xml.GetElementsByTagName(&HSTRING::from("text")).ok()?;
    let lines: Vec<_> = (0..nodes.Length().ok()?.min(8))
        .filter_map(|i| nodes.Item(i).ok())
        .filter_map(|node| node.InnerText().ok())
        .map(|line| text(&line.to_string(), 1200))
        .filter(|line| !line.is_empty())
        .collect();
    Some(ImageSource {
        title: text(lines.first()?, 180),
        body: text(&lines.into_iter().skip(1).collect::<Vec<_>>().join("\n"), 1200),
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
    let uri = local_image_uri(source)?;
    let path = local_image_path(&uri)?;
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
    let decoder = BitmapDecoder::CreateAsync(stream).ok()?.join().ok()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
        let image = cache.resolve("test", "Sender", "Message").expect("local notification image should decode");
        assert_eq!(image.side, 96);
        assert_eq!(image.bgra.len(), 96 * 96 * 4);
        std::fs::remove_file(&path).unwrap();
        let cached = cache.resolve("test", "Sender", "Message").unwrap();
        assert!(Arc::ptr_eq(&image, &cached));
        Ok(())
    }

    #[test]
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
