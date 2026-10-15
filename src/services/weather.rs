use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use chrono::{Datelike, Local, Timelike};
use windows::Foundation::Uri;
use windows::Web::Http::HttpClient;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::HSTRING;

pub const WM_WEATHER: u32 = WM_APP + 13;

const REFRESH: Duration = Duration::from_secs(30 * 60);
const RETRY: Duration = Duration::from_secs(3 * 60);
const MAX_BODY_CHARS: usize = 600_000;
const USER_AGENT: &str = "NightIsland/0.1 (desktop widget)";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sky {
    Clear,
    Partly,
    Cloudy,
    Fog,
    Rain,
    Sleet,
    Snow,
    Thunder,
}

impl Sky {
    pub fn from_symbol(code: &str) -> (Sky, bool) {
        let night = code.ends_with("_night");
        let sky = if code.contains("thunder") {
            Sky::Thunder
        } else if code.contains("sleet") {
            Sky::Sleet
        } else if code.contains("snow") {
            Sky::Snow
        } else if code.contains("rain") {
            Sky::Rain
        } else if code.contains("fog") {
            Sky::Fog
        } else if code.starts_with("partlycloudy") {
            Sky::Partly
        } else if code.starts_with("clearsky") || code.starts_with("fair") {
            Sky::Clear
        } else {
            Sky::Cloudy
        };
        (sky, night)
    }

    pub fn name(self) -> &'static str {
        match self {
            Sky::Clear => "Clear sky",
            Sky::Partly => "Partly cloudy",
            Sky::Cloudy => "Cloudy",
            Sky::Fog => "Fog",
            Sky::Rain => "Rain",
            Sky::Sleet => "Sleet",
            Sky::Snow => "Snow",
            Sky::Thunder => "Thunderstorm",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Hour {
    pub hour: u32,
    pub temp: f32,
    pub sky: Sky,
    pub night: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Day {
    pub weekday: u32,
    pub min: f32,
    pub max: f32,
    pub sky: Sky,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Weather {
    pub place: String,
    pub temp: f32,
    pub wind: f32,
    pub humidity: f32,
    pub sky: Sky,
    pub night: bool,
    pub hourly: Vec<Hour>,
    pub daily: Vec<Day>,
    pub city_missing: bool,
}

static SNAPSHOT: Mutex<Option<Weather>> = Mutex::new(None);
static ENABLED: AtomicBool = AtomicBool::new(true);
static WAKE: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);
static CITY: Mutex<String> = Mutex::new(String::new());
static PLACE: Mutex<Option<(f64, f64, String)>> = Mutex::new(None);
static MATCHES: Mutex<Vec<City>> = Mutex::new(Vec::new());

pub const WM_GEO: u32 = WM_APP + 18;

#[derive(Clone, Debug, PartialEq)]
pub struct City {
    pub label: String,
    pub lat: f64,
    pub lon: f64,
}

pub fn snapshot() -> Option<Weather> {
    SNAPSHOT.lock().ok().and_then(|g| g.clone())
}

fn wake() {
    if let Ok(g) = WAKE.lock()
        && let Some(tx) = g.as_ref()
    {
        let _ = tx.send(());
    }
}

pub fn set_city(city: &str) {
    if let Ok(mut c) = CITY.lock() {
        *c = city.trim().to_string();
    }
    if let Ok(mut p) = PLACE.lock() {
        *p = None;
    }
    wake();
}

pub fn set_place(label: &str, lat: f64, lon: f64) {
    if let Ok(mut p) = PLACE.lock() {
        *p = Some((lat, lon, label.to_string()));
    }
    wake();
}

pub fn matches() -> Vec<City> {
    MATCHES.lock().map(|m| m.clone()).unwrap_or_default()
}

fn read_cities(body: &str) -> Vec<City> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else { return Vec::new() };
    let Some(list) = v.get("features").and_then(|r| r.as_array()) else { return Vec::new() };
    list.iter()
        .filter_map(|feature| {
            let coords = feature.pointer("/geometry/coordinates")?.as_array()?;
            let (lon, lat) = (coords.first()?.as_f64()?, coords.get(1)?.as_f64()?);
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                return None;
            }
            let props = feature.get("properties")?;
            let part = |k: &str| {
                props
                    .get(k)
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.chars().take(40).collect::<String>())
            };
            let mut label = Vec::new();
            for piece in [part("name"), part("state"), part("country")].into_iter().flatten() {
                if label.last() != Some(&piece) {
                    label.push(piece);
                }
            }
            (!label.is_empty()).then(|| City { label: label.join(", "), lat, lon })
        })
        .collect()
}

fn lookup(query: &str, limit: u32) -> Vec<City> {
    let lang = if crate::ui::i18n::is_ru() { "default" } else { "en" };
    let url = format!(
        "https://photon.komoot.io/api/?q={}&limit={limit}&lang={lang}&osm_tag=place",
        crate::services::http::pct(query)
    );
    get(&url).map(|b| read_cities(&b)).unwrap_or_default()
}

pub fn search(hwnd: HWND, query: &str) {
    let query: String = query.trim().chars().filter(|c| !c.is_control()).take(60).collect();
    let raw = hwnd.0 as isize;
    if query.chars().count() < 2 {
        if let Ok(mut m) = MATCHES.lock() {
            m.clear();
        }
        unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_GEO, WPARAM(0), LPARAM(0));
        }
        return;
    }
    std::thread::spawn(move || {
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let found = lookup(&query, 5);
        if let Ok(mut m) = MATCHES.lock() {
            *m = found;
        }
        unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_GEO, WPARAM(0), LPARAM(0));
        }
    });
}

fn geocode(city: &str) -> Option<(f64, f64, String)> {
    let first = lookup(city, 1).into_iter().next()?;
    Some((first.lat, first.lon, first.label))
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    if !on && let Ok(mut g) = SNAPSHOT.lock() {
        *g = None;
    }
    wake();
}

fn get(url: &str) -> Option<String> {
    let client = HttpClient::new().ok()?;
    client.DefaultRequestHeaders().ok()?.UserAgent().ok()?.TryParseAdd(&HSTRING::from(USER_AGENT)).ok()?;
    let text = client.GetStringAsync(&Uri::CreateUri(&HSTRING::from(url)).ok()?).ok()?.join().ok()?;
    let text = text.to_string();
    (text.chars().count() <= MAX_BODY_CHARS).then_some(text)
}

fn locate() -> Option<(f64, f64, String)> {
    let body = get("https://ipwho.is/?fields=success,latitude,longitude,city")?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    if v.get("success").and_then(|s| s.as_bool()) == Some(false) {
        return None;
    }
    let (lat, lon) = (v.get("latitude")?.as_f64()?, v.get("longitude")?.as_f64()?);
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let city = v.get("city").and_then(|c| c.as_str()).unwrap_or("").chars().take(40).collect();
    Some((lat, lon, city))
}

fn symbol(entry: &serde_json::Value) -> Option<(Sky, bool)> {
    let data = &entry["data"];
    let code = data["next_1_hours"]["summary"]["symbol_code"]
        .as_str()
        .or_else(|| data["next_6_hours"]["summary"]["symbol_code"].as_str())
        .or_else(|| data["next_12_hours"]["summary"]["symbol_code"].as_str())?;
    Some(Sky::from_symbol(code))
}

fn local(entry: &serde_json::Value) -> Option<chrono::DateTime<Local>> {
    let t = chrono::DateTime::parse_from_rfc3339(entry["time"].as_str()?).ok()?;
    Some(t.with_timezone(&Local))
}

pub fn parse(json: &str, place: &str) -> Option<Weather> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let series = v["properties"]["timeseries"].as_array()?;
    let first = series.first()?;
    let details = &first["data"]["instant"]["details"];
    let temp = details["air_temperature"].as_f64()? as f32;
    let wind = details["wind_speed"].as_f64().unwrap_or(0.0) as f32;
    let humidity = details["relative_humidity"].as_f64().unwrap_or(0.0) as f32;
    let (sky, night) = symbol(first).unwrap_or((Sky::Cloudy, false));

    let hourly = (1..=6)
        .filter_map(|i| {
            let e = series.get(i * 2)?;
            let t = e["data"]["instant"]["details"]["air_temperature"].as_f64()? as f32;
            let (sky, night) = symbol(e)?;
            Some(Hour { hour: local(e)?.hour(), temp: t, sky, night })
        })
        .collect();

    type DayAcc = (chrono::NaiveDate, f32, f32, Option<(Sky, i64)>);
    let mut days: Vec<DayAcc> = Vec::new();
    for e in series {
        let (Some(at), Some(t)) = (local(e), e["data"]["instant"]["details"]["air_temperature"].as_f64()) else {
            continue;
        };
        let t = t as f32;
        let date = at.date_naive();
        if days.last().map(|d| d.0) != Some(date) {
            days.push((date, t, t, None));
        }
        let Some(d) = days.last_mut() else { continue };
        d.1 = d.1.min(t);
        d.2 = d.2.max(t);
        if let Some((sky, _)) = symbol(e) {
            let distance = (i64::from(at.hour()) - 13).abs();
            if d.3.is_none_or(|(_, best)| distance < best) {
                d.3 = Some((sky, distance));
            }
        }
    }
    let daily = days
        .into_iter()
        .take(4)
        .map(|(date, min, max, sky)| Day {
            weekday: date.weekday().num_days_from_monday(),
            min,
            max,
            sky: sky.map_or(Sky::Cloudy, |s| s.0),
        })
        .collect();

    Some(Weather { place: place.to_string(), temp, wind, humidity, sky, night, hourly, daily, city_missing: false })
}

fn fetch() -> Option<Weather> {
    let wanted = CITY.lock().map(|c| c.clone()).unwrap_or_default();
    let picked = PLACE.lock().ok().and_then(|p| p.clone());
    let mut missing = false;
    let (lat, lon, city) = if let Some(place) = picked {
        place
    } else if wanted.is_empty() {
        locate()?
    } else if let Some(found) = geocode(&wanted) {
        found
    } else {
        missing = true;
        locate()?
    };
    let url = format!("https://api.met.no/weatherapi/locationforecast/2.0/compact?lat={lat:.4}&lon={lon:.4}");
    let mut w = parse(&get(&url)?, &city)?;
    w.city_missing = missing;
    Some(w)
}

pub fn start(hwnd: HWND, enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    let raw = hwnd.0 as isize;
    let (tx, rx) = mpsc::channel::<()>();
    if let Ok(mut g) = WAKE.lock() {
        *g = Some(tx);
    }
    std::thread::spawn(move || {
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let notify = || unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_WEATHER, WPARAM(0), LPARAM(0));
        };
        loop {
            let wait = if ENABLED.load(Ordering::Relaxed) {
                match fetch() {
                    Some(w) => {
                        if let Ok(mut g) = SNAPSHOT.lock() {
                            *g = Some(w);
                        }
                        REFRESH
                    }
                    None => {
                        crate::log_warn!("weather update failed, retrying later");
                        RETRY
                    }
                }
            } else {
                Duration::from_secs(24 * 3600)
            };
            notify();
            if let Err(mpsc::RecvTimeoutError::Disconnected) = rx.recv_timeout(wait) {
                return;
            }
        }
    });
}

#[cfg(test)]
mod city_tests {
    use super::*;

    #[test]
    fn matches_are_labelled_and_bad_ones_dropped() {
        let body = r#"{"features":[
            {"geometry":{"coordinates":[13.39,52.51]},"properties":{"name":"Berlin","country":"Germany"}},
            {"geometry":{"coordinates":[-71.18,44.46]},"properties":{"name":"Berlin","state":"New Hampshire","country":"United States"}},
            {"geometry":{"coordinates":[0.0,999.0]},"properties":{"name":"Nowhere"}}]}"#;
        let cities = read_cities(body);
        assert_eq!(cities.len(), 2);
        assert_eq!(cities[0].label, "Berlin, Germany");
        assert_eq!(cities[1].label, "Berlin, New Hampshire, United States");
        assert!(read_cities("not json").is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_map_to_skies() {
        assert_eq!(Sky::from_symbol("clearsky_night"), (Sky::Clear, true));
        assert_eq!(Sky::from_symbol("partlycloudy_day"), (Sky::Partly, false));
        assert_eq!(Sky::from_symbol("lightrainshowersandthunder_day").0, Sky::Thunder);
        assert_eq!(Sky::from_symbol("heavysleet").0, Sky::Sleet);
        assert_eq!(Sky::from_symbol("fog").0, Sky::Fog);
        assert_eq!(Sky::from_symbol("who_knows").0, Sky::Cloudy);
    }

    #[test]
    fn parses_a_compact_forecast_and_rejects_junk() {
        let mut series = Vec::new();
        for h in 0..30 {
            series.push(format!(
                r#"{{"time":"2026-10-09T{:02}:00:00Z","data":{{"instant":{{"details":{{"air_temperature":{}.5,"wind_speed":3.2,"relative_humidity":71.0}}}},"next_1_hours":{{"summary":{{"symbol_code":"rain"}}}}}}}}"#,
                h % 24,
                10 + h % 5
            ));
        }
        let json = format!(r#"{{"properties":{{"timeseries":[{}]}}}}"#, series.join(","));
        let w = parse(&json, "Kyiv").expect("valid forecast");
        assert_eq!(w.place, "Kyiv");
        assert_eq!(w.sky, Sky::Rain);
        assert_eq!(w.hourly.len(), 6);
        assert!(!w.daily.is_empty() && w.daily.len() <= 4);
        assert!(parse("not json", "x").is_none());
        assert!(parse(r#"{"properties":{"timeseries":[]}}"#, "x").is_none());
    }
}
