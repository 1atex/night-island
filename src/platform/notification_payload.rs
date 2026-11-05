use rusqlite::{Connection, OpenFlags};
use windows::Data::Xml::Dom::XmlDocument;
use windows::core::HSTRING;

pub(super) fn open() -> Option<Connection> {
    let path = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join(r"Microsoft\Windows\Notifications\wpndatabase.db");
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    connection.busy_timeout(std::time::Duration::from_millis(10)).ok()?;
    Some(connection)
}

pub(super) fn read(connection: &Connection, id: u32, app_id: &str, title: &str, body: &str) -> Option<XmlDocument> {
    let payload: Vec<u8> = connection.query_row(
        "SELECT n.Payload FROM Notification n JOIN NotificationHandler h ON h.RecordId = n.HandlerId WHERE n.Id = ?1 AND h.PrimaryId = ?2 AND length(n.Payload) <= 1048576 LIMIT 1",
        rusqlite::params![id, app_id],
        |row| row.get(0),
    ).ok()?;
    let source = std::str::from_utf8(&payload).ok()?;
    let xml = XmlDocument::new().ok()?;
    xml.LoadXml(&HSTRING::from(source)).ok()?;
    let (xml_title, xml_body) = content(&xml)?;
    (xml_title == title && xml_body == body).then_some(xml)
}

pub(super) fn content(xml: &XmlDocument) -> Option<(String, String)> {
    let nodes = xml.GetElementsByTagName(&HSTRING::from("text")).ok()?;
    let lines: Vec<_> = (0..nodes.Length().ok()?.min(8))
        .filter_map(|i| nodes.Item(i).ok()?.InnerText().ok())
        .map(|line| super::notifications::text(&line.to_string(), 16384))
        .filter(|line| !line.is_empty())
        .collect();
    Some((
        super::notifications::text(lines.first()?, 512),
        super::notifications::text(&lines.into_iter().skip(1).collect::<Vec<_>>().join("\n"), 16384),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Needs WinRT, which wine does not provide"]
    fn payload_requires_matching_id_app_and_content() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE Notification(Id INTEGER, HandlerId INTEGER, Payload BLOB); CREATE TABLE NotificationHandler(RecordId INTEGER, PrimaryId TEXT);").unwrap();
        db.execute("INSERT INTO NotificationHandler VALUES(1, 'discord')", []).unwrap();
        let xml = br#"<toast><visual><binding template="ToastGeneric"><text>Sender</text><text>Body</text></binding></visual></toast>"#;
        db.execute("INSERT INTO Notification VALUES(7, 1, ?1)", [xml.as_slice()]).unwrap();
        assert!(read(&db, 7, "discord", "Sender", "Body").is_some());
        assert!(read(&db, 8, "discord", "Sender", "Body").is_none());
        assert!(read(&db, 7, "other", "Sender", "Body").is_none());
        assert!(read(&db, 7, "discord", "Other sender", "Body").is_none());
        assert!(read(&db, 7, "discord", "Sender", "Other body").is_none());
        assert!(read(&Connection::open_in_memory().unwrap(), 7, "discord", "Sender", "Body").is_none());
    }
}
