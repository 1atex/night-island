use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone, Timelike};

const MAX_TEXT_CHARS: usize = 300;
const MAX_DAYS: i64 = 3650;

pub struct Parsed {
    pub due: DateTime<Local>,
    pub text: String,
}

fn unit_seconds(unit: &str) -> Option<i64> {
    Some(match unit {
        "s" | "sec" | "secs" | "second" | "seconds" | "с" | "сек" | "секунд" | "секунду" | "секунды" => {
            1
        }
        "m" | "min" | "mins" | "minute" | "minutes" | "м" | "мин" | "минут" | "минуту" | "минуты" => {
            60
        }
        "h" | "hr" | "hrs" | "hour" | "hours" | "ч" | "час" | "часа" | "часов" => 3600,
        "d" | "day" | "days" | "д" | "дн" | "день" | "дня" | "дней" => 86_400,
        _ => return None,
    })
}

fn duration(tokens: &[&str]) -> Option<(Duration, usize)> {
    let first = tokens.first()?.to_lowercase();
    let digits: String = first.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 7 {
        return None;
    }
    let n: i64 = digits.parse().ok()?;
    let rest = &first[digits.len()..];
    let (secs, used) =
        if rest.is_empty() { (unit_seconds(&tokens.get(1)?.to_lowercase())?, 2) } else { (unit_seconds(rest)?, 1) };
    let total = n.checked_mul(secs)?;
    (total <= MAX_DAYS * 86_400).then(|| (Duration::seconds(total), used))
}

fn clock(token: &str) -> Option<NaiveTime> {
    let (h, m) = token.split_once(':')?;
    if h.is_empty() || h.len() > 2 || m.len() != 2 {
        return None;
    }
    NaiveTime::from_hms_opt(h.parse().ok()?, m.parse().ok()?, 0)
}

pub fn parse(input: &str, now: DateTime<Local>) -> Option<Parsed> {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let (mut i, mut after, mut day, mut at) = (0usize, None::<Duration>, None::<i64>, None::<NaiveTime>);
    while i < tokens.len() {
        let word = tokens[i].to_lowercase();
        match word.as_str() {
            "in" | "через" => match duration(&tokens[i + 1..]) {
                Some((d, used)) => {
                    after = Some(d);
                    i += 1 + used;
                    continue;
                }
                None => break,
            },
            "tomorrow" | "завтра" => {
                day = Some(1);
                i += 1;
                continue;
            }
            "today" | "сегодня" => {
                day = Some(0);
                i += 1;
                continue;
            }
            "at" | "в" => {
                if tokens.get(i + 1).is_some_and(|t| clock(t).is_some()) {
                    i += 1;
                    continue;
                }
                break;
            }
            _ => {}
        }
        if let Some(t) = clock(&word) {
            at = Some(t);
            i += 1;
            continue;
        }
        if let Some((d, used)) = duration(&tokens[i..]) {
            after = Some(d);
            i += used;
            continue;
        }
        break;
    }
    if after.is_none() && day.is_none() && at.is_none() {
        return None;
    }

    let due = if let Some(d) = after {
        now + d
    } else {
        let date = now.date_naive() + Duration::days(day.unwrap_or(0));
        let time = at.unwrap_or_else(|| NaiveTime::from_hms_opt(9, 0, 0).unwrap_or_default());
        let mut due = Local.from_local_datetime(&date.and_time(time)).single()?;
        if day.is_none() && due <= now {
            due += Duration::days(1);
        }
        due
    };
    if due <= now {
        return None;
    }
    let text: String = tokens[i..].join(" ").chars().take(MAX_TEXT_CHARS).collect();
    Some(Parsed { due, text: if text.is_empty() { "Reminder".to_string() } else { text } })
}

pub fn describe(due: DateTime<Local>, now: DateTime<Local>) -> String {
    let secs = (due - now).num_seconds();
    if secs <= 0 {
        return "now".into();
    }
    if secs < 3600 {
        return format!("in {} min", (secs + 59) / 60);
    }
    let clock = format!("{:02}:{:02}", due.hour(), due.minute());
    let days = (due.date_naive() - now.date_naive()).num_days();
    match days {
        0 => format!("Today {clock}"),
        1 => format!("Tomorrow {clock}"),
        _ => format!("{:02}.{:02} {clock}", due.day(), due.month()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 9, 15, 0, 0).single().expect("valid local time")
    }

    fn parsed(s: &str) -> Parsed {
        parse(s, now()).unwrap_or_else(|| panic!("`{s}` should parse"))
    }

    #[test]
    fn relative_times() {
        let p = parsed("in 30 min call mom");
        assert_eq!(p.due, now() + Duration::minutes(30));
        assert_eq!(p.text, "call mom");
        assert_eq!(parsed("2h drink water").due, now() + Duration::hours(2));
        assert_eq!(parsed("через 5 минут чай").due, now() + Duration::minutes(5));
        assert_eq!(parsed("через 2 часа").text, "Reminder");
        assert_eq!(parsed("10 sec x").due, now() + Duration::seconds(10));
    }

    #[test]
    fn clock_times() {
        let p = parsed("18:30 standup");
        assert_eq!((p.due.hour(), p.due.minute(), p.due.day()), (18, 30, 9));
        assert_eq!(p.text, "standup");
        assert_eq!(parsed("at 9:00 report").due.day(), 10);
        let t = parsed("tomorrow 7:15 gym");
        assert_eq!((t.due.day(), t.due.hour(), t.due.minute()), (10, 7, 15));
        assert_eq!(parsed("завтра в 21:00 фильм").text, "фильм");
        assert_eq!(parsed("tomorrow report").due.hour(), 9);
    }

    #[test]
    fn not_reminders() {
        assert!(parse("buy milk", now()).is_none());
        assert!(parse("30 apples", now()).is_none());
        assert!(parse("today 8:00 too late", now()).is_none());
        assert!(parse("in 99999999 days x", now()).is_none());
        assert!(parse("", now()).is_none());
    }

    #[test]
    fn words_in_the_message_are_kept() {
        assert_eq!(parsed("in 5 min at the door").text, "at the door");
    }

    #[test]
    fn descriptions() {
        assert_eq!(describe(now() + Duration::minutes(25), now()), "in 25 min");
        assert_eq!(describe(now() + Duration::hours(2), now()), "Today 17:00");
        assert_eq!(describe(now() + Duration::days(1), now()), "Tomorrow 15:00");
        assert_eq!(describe(now() + Duration::days(3), now()), "12.10 15:00");
        assert_eq!(describe(now() - Duration::seconds(5), now()), "now");
    }
}
