use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct Release {
    #[prost(string, tag = "1")]
    pub version: String,
    #[prost(bool, tag = "2")]
    pub beta: bool,
    #[prost(string, tag = "3")]
    pub url: String,
    #[prost(bytes = "vec", tag = "4")]
    pub sha256: Vec<u8>,
    #[prost(uint64, tag = "5")]
    pub size: u64,
    #[prost(string, tag = "6")]
    pub notes: String,
    #[prost(int64, tag = "7")]
    pub signed_at: i64,
    #[prost(string, tag = "8")]
    pub channel: String,
    #[prost(string, tag = "9")]
    pub os: String,
    #[prost(string, tag = "10")]
    pub app: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SignedRelease {
    #[prost(bytes = "vec", tag = "1")]
    pub release: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub signature: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Lyrics {
    #[prost(string, tag = "1")]
    pub lrc: String,
    #[prost(string, tag = "2")]
    pub plain: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct LyricsReply {
    #[prost(bool, tag = "1")]
    pub found: bool,
    #[prost(message, optional, tag = "2")]
    pub lyrics: Option<Lyrics>,
}

#[derive(Clone, PartialEq, Message)]
pub struct LyricsPut {
    #[prost(bytes = "vec", tag = "1")]
    pub id: Vec<u8>,
    #[prost(message, optional, tag = "2")]
    pub lyrics: Option<Lyrics>,
    #[prost(bytes = "vec", tag = "3")]
    pub client_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub signature: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROTO: &str = include_str!("../../proto/night_island.proto");

    #[test]
    fn field_numbers_match_the_proto_file() {
        let fields = [
            ("Release", "version", 1),
            ("Release", "beta", 2),
            ("Release", "url", 3),
            ("Release", "sha256", 4),
            ("Release", "size", 5),
            ("Release", "notes", 6),
            ("Release", "signed_at", 7),
            ("Release", "channel", 8),
            ("Release", "os", 9),
            ("Release", "app", 10),
            ("SignedRelease", "release", 1),
            ("SignedRelease", "signature", 2),
            ("Lyrics", "lrc", 1),
            ("Lyrics", "plain", 2),
            ("LyricsReply", "found", 1),
            ("LyricsReply", "lyrics", 2),
            ("LyricsPut", "id", 1),
            ("LyricsPut", "lyrics", 2),
            ("LyricsPut", "client_key", 3),
            ("LyricsPut", "signature", 4),
        ];
        for (message, field, number) in fields {
            let start = PROTO.find(&format!("message {message} {{")).unwrap_or_else(|| panic!("no message {message}"));
            let body = &PROTO[start..start + PROTO[start..].find("\n}").unwrap()];
            let line = format!(" {field} = {number};");
            assert!(body.contains(&line), "{message}.{field} is not field {number} in the .proto");
        }
    }

    #[test]
    fn messages_round_trip() {
        let put = LyricsPut {
            id: vec![7; 32],
            lyrics: Some(Lyrics { lrc: "[00:01.00]hi".into(), plain: "hi".into() }),
            client_key: vec![1; 32],
            signature: vec![2; 64],
        };
        assert_eq!(LyricsPut::decode(put.encode_to_vec().as_slice()).unwrap(), put);
        assert!(LyricsPut::decode(&b"\xff\xff garbage"[..]).is_err());
        let miss = LyricsReply { found: false, lyrics: None };
        assert_eq!(miss.encode_to_vec(), Vec::<u8>::new(), "a miss is an empty body in proto3");
    }
}
