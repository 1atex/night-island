use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

static RU: AtomicBool = AtomicBool::new(true);

pub fn set_ru(on: bool) {
    RU.store(on, Ordering::Relaxed);
}

pub fn is_ru() -> bool {
    RU.load(Ordering::Relaxed)
}

fn phrase(s: &str) -> Option<&'static str> {
    Some(match s {
        "Home" => "Главная",
        "Clipboard" => "Буфер",
        "Shelf" => "Полка",
        "Text" => "Текст",
        "Note" => "Заметки",
        "Record" => "Запись",
        "Weather" => "Погода",
        "Controls" => "Управление",
        "Settings" => "Настройки",
        "Wi-Fi" => "Wi-Fi",
        "Bluetooth" => "Bluetooth",
        "Dark theme" => "Тёмная тема",
        "Lock" => "Блокировка",
        "Sleep" => "Сон",
        "Restart" => "Перезагрузка",
        "Shut down" => "Выключение",
        "Microphone" => "Микрофон",
        "Battery" => "Батарея",
        "On" => "Включён",
        "Off" => "Выключен",
        "Unavailable" => "Недоступно",
        "Press again" => "Нажми ещё раз",
        "Press Enter again" => "Нажми Enter ещё раз",
        "On AC power" => "Питание от сети",
        "On battery" => "От батареи",
        "Charging" => "Заряжается",
        "Area" => "Область",
        "Full screen" => "Весь экран",
        "Text from screen" => "Текст с экрана",
        "QR code" => "QR-код",
        "Color picker" => "Пипетка",
        "Clean text" => "Чистый текст",
        "Screen recording" => "Запись экрана",
        "Command" => "Команда",
        "Drop shortcuts, folders or files here" => "Перетащи сюда ярлыки программ, папки или файлы",
        "Drop files here, drag them out when you need them" => "Бросай файлы сюда, вытаскивай когда нужно",
        "Copy something and it will show up here" => "Скопируй что-нибудь — и оно появится здесь",
        "Nothing is playing" => "Сейчас ничего не играет",
        "Click to close" => "Нажми, чтобы закрыть",
        "Paste clipboard" => "Из буфера",
        "Copy" => "Копировать",
        "Paste to window" => "Вставить в окно",
        "Clear" => "Очистить",
        "Paste or type text, then pick a tool" => "Вставь или набери текст и выбери инструмент",
        "Write something, it saves itself" => "Пиши — сохраняется само",
        "Remind me…  e.g. in 30 min call mom" => "Напомни…  например, через 30 мин позвонить маме",
        "Reminder" => "Напоминание",
        "Done" => "Готово",
        "Snooze 10 min" => "Отложить на 10 мин",
        "Snooze 1 hour" => "Отложить на час",
        "Record an area" => "Записать область",
        "Stop recording" => "Остановить запись",
        "Open the recordings folder" => "Открыть папку с записями",
        "Ctrl+Alt+R  ·  or click the pill" => "Ctrl+Alt+R  ·  или нажми на пилюлю",
        "Search apps, calculate, run commands" => "Поиск программ, расчёты, команды",
        "Language" => "Язык",
        "Placement" => "Положение",
        "Floating" => "Плавающая",
        "Docked" => "У края",
        "Shape" => "Форма",
        "Flat" => "Плоская",
        "Liquid" => "Жидкая",
        "Character" => "Персонаж",
        "Size" => "Размер",
        "Liquid reach" => "Дальность тяги",
        "Liquid depth" => "Глубина тяги",
        "Liquid softness" => "Мягкость тяги",
        "Panel opacity" => "Прозрачность",
        "Music on pill" => "Музыка на пилюле",
        "Title" => "Название",
        "Equalizer" => "Эквалайзер",
        "Edge spikes" => "Шипы по краю",
        "Open delay" => "Задержка откр.",
        "Close delay" => "Задержка закр.",
        "Open speed" => "Скорость откр.",
        "Close speed" => "Скорость закр.",
        "Auto-hide" => "Автоскрытие",
        "Acrylic blur" => "Размытие фона",
        "Hide in screenshots" => "Скрыть на снимках",
        "Hide island in my captures" => "Прятать при записи и снимке",
        "Game mode" => "Игровой режим",
        "REC" => "Запись",
        "Format" => "Формат",
        "Frame rate" => "Частота кадров",
        "System sound" => "Звук системы",
        "No character" => "Без персонажа",
        "Capture failed" => "Не удалось сделать снимок",
        "Copied" => "Скопировано",
        "Looking for a QR code…" => "Ищу QR-код…",
        "Reading text…" => "Читаю текст…",
        "Microphone muted" => "Микрофон выключен",
        "Microphone on" => "Микрофон включён",
        "No microphone found" => "Микрофон не найден",
        "No QR code found" => "QR-код не найден",
        "No text found" => "Текст не найден",
        "Nothing to convert" => "Нечего преобразовывать",
        "Nothing to paste" => "Нечего вставлять",
        "Pasted from the clipboard" => "Вставлено из буфера",
        "QR code copied" => "QR-код скопирован",
        "Recording failed" => "Запись не удалась",
        "Recording saved" => "Запись сохранена",
        "Saving the recording…" => "Сохраняю запись…",
        "Screenshot copied" => "Скриншот скопирован",
        "Screenshot copied and saved" => "Скриншот скопирован и сохранён",
        "The clipboard has no text" => "В буфере нет текста",
        "Too many reminders" => "Слишком много напоминаний",
        "Too much text for a QR code" => "Слишком много текста для QR-кода",
        "no system sound" => "нет системного звука",
        "mic was silent" => "микрофон молчал",
        "mic unavailable" => "микрофон недоступен",
        "no system sound device" => "нет устройства вывода",
        "software encoder" => "программный кодер",
        "GIF is silent" => "GIF без звука",
        "Cloudy" => "Облачно",
        "Clear sky" => "Ясно",
        "Partly cloudy" => "Переменная облачность",
        "Fog" => "Туман",
        "Rain" => "Дождь",
        "Showers" => "Ливни",
        "Snow" => "Снег",
        "Sleet" => "Дождь со снегом",
        "Thunderstorm" => "Гроза",
        "Feels like" => "Ощущается как",
        "Wind" => "Ветер",
        "Humidity" => "Влажность",
        "Locating…" => "Определяю место…",
        "No connection" => "Нет соединения",
        "Today" => "Сегодня",
        "Max" => "Макс",
        "Min" => "Мин",
        "Camera" => "Камера",
        "Weather is off" => "Погода выключена",
        "Pill blur" => "Размытие под пилюлей",
        "Start with Windows" => "Запуск с Windows",
        "Pill opacity" => "Прозрачность пилюли",
        "Weather on the pill" => "Погода на пилюле",
        "Battery on the pill" => "Батарея на пилюле",
        "Track title" => "Название трека",
        "Synced lyrics" => "Текст песни",
        "Tabs" => "Вкладки",
        "AI" => "Нейросети",
        "Own Client ID" => "Свой Client ID",
        "Optional: your Spotify app" => "По желанию: ваше приложение Spotify",
        "Save the Client ID" => "Сохранить Client ID",
        "Client ID saved" => "Client ID сохранён",
        "That does not look like a Client ID" => "Это не похоже на Client ID",
        "Mute the microphone" => "Выключить микрофон",
        "Unmute the microphone" => "Включить микрофон",
        "Undeafen" => "Включить звук",
        "Show the call overlay" => "Показать оверлей звонка",
        "Hide the call overlay" => "Скрыть оверлей звонка",
        "Discord must be running; you will see an Authorize prompt inside it." => {
            "Discord должен быть запущен: в нём появится запрос на авторизацию."
        }
        "discord.com/developers: New Application, OAuth2: Redirect http://127.0.0.1," => {
            "discord.com/developers: New Application, OAuth2: Redirect http://127.0.0.1,"
        }
        "App Testers: add yourself. The credentials stay on this computer." => {
            "App Testers: добавьте себя. Данные хранятся только на этом компьютере."
        }
        "Shuffle, repeat and likes for the playing track." => "Перемешивание, повтор и лайки для играющего трека.",
        "Connections to AI services will live here." => "Здесь будут подключения к нейросетям.",
        "Planned: Claude, OpenAI-compatible servers and Gemini, each with its own key." => {
            "В планах: Claude, OpenAI-совместимые серверы и Gemini, у каждого свой ключ."
        }
        "Keys will be stored encrypted for your Windows account, like the Spotify token." => {
            "Ключи будут храниться зашифрованными для вашего аккаунта Windows, как токен Spotify."
        }
        "Wrong layout" => "Не та раскладка",
        "UPPER" => "ВЕРХНИЙ",
        "lower" => "нижний",
        "Title Case" => "Каждое Слово",
        "Translit" => "Транслит",
        "Sort lines" => "Сортировка строк",
        "Unique lines" => "Без повторов",
        "Clean spaces" => "Убрать пробелы",
        "Count" => "Подсчёт",
        "JSON" => "JSON",
        "JSON min" => "JSON в строку",
        "Timestamp" => "Время (Unix)",
        "Developer" => "Разработчик",
        "Developer tools" => "Инструменты разработчика",
        "chars" => "симв.",
        "words" => "слов",
        "lines" => "строк",
        "bytes" => "байт",
        "Not valid JSON" => "Это не JSON",
        "Not valid Base64" => "Это не Base64",
        "Not valid hex" => "Это не hex",
        "Not a JWT" => "Это не JWT",
        "Not a date or a Unix time" => "Это не дата и не Unix-время",
        "Decoded data is not text" => "Результат не является текстом",
        "This tool does not produce text" => "Этот инструмент не выдаёт текст",
        "Notifications" => "Уведомления",
        "Microphone mute" => "Отключение микрофона",
        "Finished downloads" => "Завершённые загрузки",
        "Screenshots, text and QR" => "Скриншоты, текст и QR",
        "Discord and Spotify" => "Discord и Spotify",
        "Errors and limits" => "Ошибки и лимиты",
        "Copied to the clipboard" => "Копирование в буфер",
        "Show the copied text" => "Показывать скопированный текст",
        "Caps, Num and Scroll Lock" => "Caps, Num и Scroll Lock",
        "Charger and low battery" => "Зарядка и низкий заряд",
        "Caps Lock on" => "Caps Lock включён",
        "Caps Lock off" => "Caps Lock выключен",
        "Num Lock on" => "Num Lock включён",
        "Num Lock off" => "Num Lock выключен",
        "Scroll Lock on" => "Scroll Lock включён",
        "Scroll Lock off" => "Scroll Lock выключен",
        "Charger connected" => "Зарядка подключена",
        "Charger disconnected" => "Зарядка отключена",
        "Battery low" => "Низкий заряд батареи",
        "Reminders" => "Напоминания",
        "+5 min" => "+5 мин",
        "+15 min" => "+15 мин",
        "+30 min" => "+30 мин",
        "+1 hour" => "+1 час",
        "+2 hours" => "+2 часа",
        "Tomorrow 9:00" => "Завтра 9:00",
        "Tomorrow" => "Завтра",
        "Set" => "Поставить",
        "What to remind?  or type a time: in 30 min call mom" => {
            "О чём напомнить?  или время текстом: через 30 мин позвонить маме"
        }
        "Pick a time, or press Enter to use the time in the text" => {
            "Выберите время или нажмите Enter, чтобы взять время из текста"
        }
        "No time in the text: pick one above, or start with in 30 min · 18:30" => {
            "В тексте нет времени: выберите его выше или начните с «через 30 мин» · 18:30"
        }
        "That time does not exist" => "Такого времени не существует",
        "New note" => "Новая заметка",
        "New folder" => "Новая папка",
        "Notes limit reached" => "Достигнут лимит заметок",
        "General" => "Основные",
        "Island" => "Остров",
        "Appearance" => "Внешний вид",
        "About" => "О программе",
        "Logs folder" => "Папка логов",
        "Search settings" => "Поиск по настройкам",
        "Open" => "Открыть",
        "Spike tips in track colour" => "Кончики в цвет трека",
        "Hidden" => "Скрыта",
        "Icon only" => "Значок",
        "With percent" => "С процентами",
        "Pill and music" => "Пилюля и музыка",
        "AGY conversations" => "Чаты AGY",
        "Back to the list" => "К списку чатов",
        "limit: ok" => "лимит: ок",
        "limit reached" => "лимит исчерпан",
        "resets in" => "сброс через",
        "tokens" => "токенов",
        "agy is not installed" => "agy не найден",
        "No conversations found" => "Чаты не найдены",
        "Send" => "Отправить",
        "Stop" => "Стоп",
        "Message to this chat…" => "Сообщение в этот чат…",
        "Working…" => "Работает…",
        "Failed" => "Ошибка",
        "Write below; you will get a notice when the answer is in." => {
            "Напиши ниже: когда ответ будет готов, придёт уведомление."
        }
        "AGY auto-approve" => "AGY без подтверждений",
        "AGY: sent" => "AGY: отправлено",
        "Choose a conversation" => "Выбери чат",
        "City not found" => "Город не найден",
        "Choose a city" => "Выбери город",
        "City (empty: by IP address)" => "Город (пусто: по IP-адресу)",
        "Edge" => "Край",
        "Top edge" => "Сверху",
        "Bottom edge" => "Снизу",
        "Left edge" => "Слева",
        "Right edge" => "Справа",
        "Panel" => "Панель",
        "Accent" => "Акцент",
        "more" => "ещё",
        "Theme" => "Тема",
        "Panel hue" => "Оттенок панели",
        "Panel saturation" => "Насыщенность панели",
        "Panel brightness" => "Яркость панели",
        "Accent hue" => "Оттенок акцента",
        "Accent saturation" => "Насыщ. акцента",
        "Presets" => "Пресеты",
        "Night" => "Ночь",
        "Graphite" => "Графит",
        "Ocean" => "Океан",
        "Forest" => "Лес",
        "Rose" => "Роза",
        "Violet" => "Фиолет",
        "Amber" => "Янтарь",
        "Mono" => "Моно",
        "Click to open, right-click for more" => "Клик — открыть, правый клик — меню",
        "Show in folder" => "Показать в папке",
        "Run as administrator" => "Запуск от администратора",
        "Remove from the dock" => "Убрать из панели",
        "Add to the dock" => "Добавить в панель",
        "Type to search files with Everything" => "Пиши запрос: поиск файлов через Everything",
        "Everything is not running" => "Everything не запущен",
        "Everything did not answer" => "Everything не ответил",
        "Start Everything to search files" => "Запусти Everything, чтобы искать файлы",
        "Save" => "Сохранить",
        "Client ID" => "Client ID",
        "Client Secret" => "Client Secret",
        "discord.com/developers: New Application," => "discord.com/developers: New Application,",
        "OAuth2: Redirect http://127.0.0.1," => "OAuth2: Redirect http://127.0.0.1,",
        "App Testers: add yourself." => "App Testers: добавь себя.",
        "Discord must be running; you will see an" => "Discord должен быть запущен; в нём появится",
        "Authorize prompt inside it. Your account has to" => "окно Authorize. Твой аккаунт должен быть",
        "be an App Tester of the application." => "в App Testers этого приложения.",
        "Approve in Discord…" => "Подтверди в Discord…",
        "Not running" => "Discord не запущен",
        "Not in a voice channel" => "Ты не в голосовом канале",
        "Create an app at discord.com/developers, add" => "Создай приложение на discord.com/developers,",
        "the Redirect http://127.0.0.1, make yourself an" => "добавь Redirect http://127.0.0.1, внеси себя в",
        "App Tester, put its Client ID and secret in the file." => "App Testers, впиши Client ID и secret в файл.",
        "Integrations" => "Интеграции",
        "Connect" => "Подключить",
        "Disconnect" => "Отключить",
        "Connected" => "Подключено",
        "Not connected" => "Не подключено",
        "Open settings file" => "Открыть файл настроек",
        "Waiting for the browser…" => "Жду браузер…",
        "Coming soon" => "Скоро",
        "Needs a Client ID" => "Нужен Client ID",
        "Create an app at developer.spotify.com," => "Создай приложение на developer.spotify.com,",
        "add the Redirect URI http://127.0.0.1:8898/callback" => "добавь Redirect URI http://127.0.0.1:8898/callback",
        "and put its Client ID into the settings file." => "и впиши его Client ID в файл настроек.",
        "Who is in your voice channel and who writes to you." => "Кто в твоём голосовом канале и кто тебе пишет.",
        "Search the clipboard" => "Поиск по буферу",
        "Nothing found" => "Ничего не найдено",
        "MP4: hardware H.264 + AAC sound." => "MP4: H.264 на видеокарте + звук AAC.",
        "The GPU captures and encodes: a still" => "Видеокарта сама снимает и кодирует:",
        "screen costs almost nothing." => "статичный экран почти ничего не стоит.",
        "GIF: silent, up to 720 px wide." => "GIF: без звука, до 720 px по ширине.",
        "Only the part of a frame that" => "Сохраняется только изменившаяся часть",
        "changed is stored: small files." => "кадра: файлы получаются небольшими.",
        _ => return None,
    })
}

const PREFIXES: [(&str, &str); 7] = [
    ("Downloaded ", "Скачано "),
    ("Downloading ", "Загрузка "),
    ("Recording failed: ", "Запись не удалась: "),
    ("Saved · ", "Сохранено · "),
    ("Text copied (", "Текст скопирован ("),
    ("Search the web for ", "Искать в сети: "),
    ("Set ", "Поставлено: "),
];

pub fn tr(s: &str) -> Cow<'_, str> {
    if !is_ru() {
        return Cow::Borrowed(s);
    }
    if let Some(ru) = phrase(s) {
        return Cow::Borrowed(ru);
    }
    for (en, ru) in PREFIXES {
        if let Some(rest) = s.strip_prefix(en) {
            let tail = phrase(rest).unwrap_or(rest);
            let tail = tail.replace(" characters)", " симв.)");
            return Cow::Owned(format!("{ru}{tail}"));
        }
    }
    Cow::Borrowed(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_and_passes_through() {
        set_ru(true);
        assert_eq!(tr("Home"), "Главная");
        assert_eq!(tr("Saved · mic was silent"), "Сохранено · микрофон молчал");
        assert_eq!(tr("unknown text"), "unknown text");
        set_ru(false);
        assert_eq!(tr("Home"), "Home");
        set_ru(true);
    }
}
