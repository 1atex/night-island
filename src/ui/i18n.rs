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
        "Lock dragging" => "Заблокировать перетаскивание",
        "Unlock dragging" => "Разблокировать перетаскивание",
        "Keep open" => "Не закрывать",
        "Let it close" => "Разрешить закрытие",
        "Mute microphone" => "Выключить микрофон",
        "Unmute microphone" => "Включить микрофон",
        "Camera in use" => "Камера используется",
        "Open / close island" => "Открыть / закрыть остров",
        "Reset position" => "Сбросить положение",
        "Exit" => "Выход",
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
        "Shared lyrics cache" => "Общий кэш текстов",
        "Check for updates" => "Проверять обновления",
        "Receive beta versions" => "Получать бета-версии",
        "New version available" => "Доступна новая версия",
        "Updates are off: this build has no release key" => "Обновления выключены: в этой сборке нет ключа релизов",
        "Not checked yet" => "Ещё не проверялось",
        "Checking…" => "Проверяю…",
        "You have the latest version" => "У вас последняя версия",
        "Check now" => "Проверить сейчас",
        "No connection to the update server" => "Нет связи с сервером обновлений",
        "The update server has no release" => "На сервере обновлений нет релиза",
        "The update information was not understood" => "Сведения об обновлении не удалось разобрать",
        "Bad signature" => "Подпись не подошла",
        "The download failed" => "Скачивание не удалось",
        "The download has the wrong size" => "Скачанный файл другого размера",
        "The download does not match its signature" => "Скачанный файл не совпадает с подписанным",
        "The download is not a program" => "Скачанный файл не программа",
        "The update could not be installed" => "Не удалось установить обновление",
        "Service" => "Сервис",
        "OpenAI-compatible" => "OpenAI-совместимый",
        "AI answers" => "Ответы нейросети",
        "API key" => "API-ключ",
        "Model" => "Модель",
        "Server address" => "Адрес сервера",
        "Key saved" => "Ключ сохранён",
        "No key yet" => "Ключа пока нет",
        "Saved: type a new one to replace it" => "Сохранён: введите новый, чтобы заменить",
        "Paste the key here" => "Вставьте ключ сюда",
        "Remove the saved key" => "Удалить сохранённый ключ",
        "Ask in the command bar (Alt+Space): start with ?, for example  ? what is DPAPI" => {
            "Спрашивайте в командной строке (Alt+Пробел): начните с ?, например  ? что такое DPAPI"
        }
        "The key is stored encrypted for your Windows account and is never shown again." => {
            "Ключ хранится зашифрованным для вашего аккаунта Windows и больше не показывается."
        }
        "OpenAI-compatible: OpenAI, OpenRouter, or a local server such as LM Studio." => {
            "OpenAI-совместимые: OpenAI, OpenRouter или локальный сервер, например LM Studio."
        }
        "That does not look like an API key" => "Это не похоже на API-ключ",
        "That model name is not valid" => "Недопустимое имя модели",
        "The server address must be https:// (or local http://)" => {
            "Адрес сервера: https:// (или http:// только для этого компьютера)"
        }
        "AI settings saved" => "Настройки нейросети сохранены",
        "The key was rejected" => "Ключ не принят",
        "Too many requests, try again later" => "Слишком много запросов, попробуйте позже",
        "The reply was not understood" => "Ответ не удалось разобрать",
        "The answer was empty" => "Ответ пустой",
        "No key is saved for this service" => "Для этого сервиса не сохранён ключ",
        "The model or the server address in the settings is not valid" => {
            "Модель или адрес сервера в настройках недопустимы"
        }
        "No connection to the service" => "Нет связи с сервисом",
        "Thinking…" => "Думаю…",
        "AI is not set up" => "Нейросеть не настроена",
        "Open Settings, AI: add a key" => "Откройте Настройки, Нейросети: добавьте ключ",
        "Type your question after the ?" => "Напишите вопрос после ?",
        "AI answer · Enter to copy" => "Ответ нейросети · Enter скопирует",
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
        "Keys" => "Клавиши",
        "Open or close the island" => "Открыть или закрыть остров",
        "Clipboard history" => "История буфера",
        "Paste as plain text" => "Вставить как простой текст",
        "Command bar" => "Командная строка",
        "Microphone on or off" => "Микрофон вкл/выкл",
        "Screenshot of an area" => "Снимок области",
        "Screenshot of the screen" => "Снимок экрана",
        "Hide the pill for a few seconds" => "Спрятать пилюлю на пару секунд",
        "Hide the pill for" => "Прятать пилюлю на",
        "Cover art on the pill" => "Обложка на пилюле",
        "Chat" => "Чат",
        "New chat" => "Новый чат",
        "New" => "Новый",
        "Chats" => "Чаты",
        "Attach" => "Файл",
        "No saved chats yet" => "Сохранённых чатов пока нет",
        "Drop files to attach" => "Бросьте файлы, чтобы прикрепить",
        "Only files can be attached" => "Прикрепить можно только файлы",
        "That file cannot be read" => "Этот файл не удалось прочитать",
        "That image is too large" => "Картинка слишком большая",
        "That PDF is too large" => "PDF слишком большой",
        "Only images, PDF and text files can be attached" => "Можно прикрепить картинки, PDF и текстовые файлы",
        "Up to 4 files go with one message" => "К одному сообщению — не больше 4 файлов",
        "Ask anything" => "Спросите о чём угодно",
        "Message…" => "Сообщение…",
        "Set up an AI service in the settings to chat" => "Чтобы общаться, настройте нейросеть в настройках",
        "Open AI settings" => "Открыть настройки ИИ",
        "Another request is running, try again in a moment" => "Другой запрос ещё идёт, повторите чуть позже",
        "Hotkey makes the pill a ghost" => "Хоткей делает пилюлю призраком",
        "Ghost visibility" => "Видимость призрака",
        "Ghost ignores the mouse" => "Призрак не реагирует на мышь",
        "Ghost only while held" => "Призрак, пока клавиши зажаты",
        "Press the new keys…" => "Нажмите новые клавиши…",
        "Click a key, then press the new combination with Ctrl, Alt, Shift or Win." => {
            "Нажмите на клавишу, затем новое сочетание с Ctrl, Alt, Shift или Win."
        }
        "Esc cancels, Delete brings the default back." => "Esc отменяет, Delete возвращает стандартное.",
        "Reset all keys" => "Сбросить все клавиши",
        "Add Ctrl, Alt, Shift or Win" => "Добавьте Ctrl, Alt, Shift или Win",
        "That key cannot be used" => "Эту клавишу нельзя использовать",
        "Another program already uses these keys" => "Эти клавиши уже заняты другой программой",
        "Players for lyrics" => "Плееры для текстов песен",
        "Any player. Add one to take lyrics only from the players you trust." => {
            "Любой плеер. Добавьте плеер, чтобы брать тексты только у доверенных."
        }
        "Lyrics are looked up only while one of these plays." => {
            "Тексты ищутся, только пока играет один из этих плееров."
        }
        "Add the player (start playing first)" => "Добавить плеер (сначала включите музыку)",
        "Remove" => "Убрать",
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
        "Privacy" => "Приватность",
        "Activity" => "Активность",
        "System" => "Система",
        "Agents" => "Агенты",
        "Steam games (started, ended)" => "Игры Steam (запуск, завершение)",
        "Played" => "Играл:",
        "Playing" => "Играешь:",
        "Coding agents" => "Кодовые агенты",
        "Coding agents (waiting, finished)" => "Кодовые агенты (ждут, закончили)",
        "The island shows when an agent works, waits for you or finishes." => {
            "Остров показывает, когда агент работает, ждёт вас или закончил."
        }
        "Nothing is changed in their settings until you connect." => "Пока вы не подключите, их настройки не меняются.",
        "Claude Code: hooks in ~/.claude/settings.json. Codex CLI: its notify command, finished only." => {
            "Claude Code: хуки в ~/.claude/settings.json. Codex CLI: его команда notify, только «закончил»."
        }
        "Sessions" => "Сессии",
        "No sessions heard from yet." => "Пока от сессий ничего не приходило.",
        "working" => "работает",
        "waiting for you" => "ждёт вас",
        "finished" => "закончил",
        "The Claude Code settings file is not valid JSON" => "Файл настроек Claude Code повреждён (не JSON)",
        "The hooks in the Claude Code settings are not an object" => "Раздел hooks в настройках Claude Code не объект",
        "A hook list in the Claude Code settings is not a list" => "Список хуков в настройках Claude Code не список",
        "Could not write the Claude Code settings file" => "Не удалось записать настройки Claude Code",
        "Codex already has a notify command of its own" => "У Codex уже есть своя команда notify",
        "Could not write the Codex configuration" => "Не удалось записать настройки Codex",
        "The user folder was not found" => "Папка пользователя не найдена",
        "The program path is unknown" => "Путь к программе неизвестен",
        "Figures" => "Показатели",
        "Read every" => "Опрашивать каждые",
        "Processor" => "Процессор",
        "Memory" => "Память",
        "Graphics card" => "Видеокарта",
        "System disk" => "Системный диск",
        "Download speed" => "Скорость загрузки",
        "Upload speed" => "Скорость отдачи",
        "Uptime" => "Время работы",
        "On the tab" => "На вкладке",
        "On the pill" => "На пилюле",
        "Tab and pill" => "Вкладка и пилюля",
        "Click a figure to move it: hidden, the System tab, the pill, or both." => {
            "Нажмите на показатель, чтобы переставить: скрыт, вкладка «Система», пилюля или оба."
        }
        "Nothing is read while no figure is on screen." => {
            "Пока ни один показатель не на экране, ничего не опрашивается."
        }
        "No figures chosen: pick some in Settings → System" => {
            "Показатели не выбраны: выберите их в Настройки → Система"
        }
        "Reading…" => "Считываю…",
        "Microphone, camera and screen" => "Микрофон, камера и экран",
        "Who used them, and when" => "Кто и когда ими пользовался",
        "Nothing yet: no program has used them since the log began" => {
            "Пока пусто: с начала журнала ими никто не пользовался"
        }
        "Yesterday" => "Вчера",
        "now" => "сейчас",
        "is capturing the screen" => "снимает экран",
        "Screen capture by other programs" => "Запись экрана другими программами",
        "Track activity (time in programs)" => "Следить за активностью (время в программах)",
        "Time in the program in front" => "Время в активной программе",
        "Activity tracking is off. It only notes which program is in front and for how long, on this computer." => {
            "Слежение выключено. Оно лишь отмечает, какая программа активна и сколько, и хранит это на этом компьютере."
        }
        "Turn tracking on" => "Включить слежение",
        "7 days" => "7 дней",
        "Total" => "Всего",
        "Sort unknown programs with AI" => "Определять неизвестные программы нейросетью",
        "Off by default. Programs are grouped by a built-in table first, and by your choices." => {
            "По умолчанию выключено. Сначала работает встроенная таблица и ваши выборы."
        }
        "On: the file names of programs nothing else knows (nothing more) go to your AI service." => {
            "Включено: имена файлов программ, которых никто не знает (и ничего больше), уходят в вашу нейросеть."
        }
        "AI requests today:" => "Запросов к нейросети сегодня:",
        "Groups" => "Группы",
        "Time at the computer" => "Время за компьютером",
        "Next reminder" => "Ближайшее напоминание",
        "Time and date" => "Время и дата",
        "Tracking off" => "Учёт выключен",
        "No battery" => "Нет батареи",
        "None" => "Нет",
        "Not playing" => "Не играете",
        "Idle" => "Спокойно",
        "Off" => "Выключен",
        "Up to 8 tiles fit: take one off first" => "Помещается до 8 плиток: сначала уберите одну",
        "Library" => "Библиотека",
        "Store" => "Магазин",
        "Downloads" => "Загрузки",
        "Friends" => "Друзья",
        "Steam is running" => "Steam запущен",
        "Steam is not running" => "Steam не запущен",
        "Steam is not installed on this computer" => "Steam не установлен на этом компьютере",
        "Start Steam" => "Запустить Steam",
        "Click again" => "Нажмите ещё раз",
        "No installed games found" => "Установленных игр не найдено",
        "playing now" => "играете сейчас",
        "Search games" => "Поиск игр",
        "Update" => "Обновить",
        "Updating…" => "Обновляется…",
        "Updating" => "Обновляется",
        "Updates" => "Обновления",
        "Online" => "В сети",
        "Away" => "Отошёл",
        "Invisible" => "Невидимка",
        "Offline" => "Не в сети",
        "min" => "мин",
        "h" => "ч",
        "Switching Steam to" => "Steam переключается на",
        "Steam is signing in as" => "Steam входит как",
        "Steam is already signed in with this account" => "Steam уже вошёл в этот аккаунт",
        "Steam does not remember that account: sign in to it once in Steam" => {
            "Steam не помнит этот аккаунт: войдите в него один раз в самом Steam"
        }
        "Steam was not found" => "Steam не найден",
        "That account is not known to Steam" => "Steam не знает такого аккаунта",
        "Steam did not close" => "Steam не закрылся",
        "Could not change Steam's sign-in setting" => "Не удалось изменить настройку входа Steam",
        "Timer" => "Таймер",
        "Shut down? Say the name and \"yes\"" => "Выключить компьютер? Скажите имя и «да»",
        "Restart? Say the name and \"yes\"" => "Перезагрузить? Скажите имя и «да»",
        "Sleep? Say the name and \"yes\"" => "Спящий режим? Скажите имя и «да»",
        "There is nothing to confirm" => "Подтверждать нечего",
        "Cancelled" => "Отменено",
        "Claude Code, Gemini CLI, Cursor, Copilot CLI: hooks in their settings files." => {
            "Claude Code, Gemini CLI, Cursor, Copilot CLI: хуки в их файлах настроек."
        }
        "Codex CLI: its notify command (finished only); a command it already has keeps running." => {
            "Codex CLI: его команда notify (только завершение); уже заданная команда продолжит работать."
        }
        "Perplexity has no hooks or command line to connect to." => {
            "У Perplexity нет ни хуков, ни командной строки, к которым можно подключиться."
        }
        "Same server: the key of the chat service is used" => "Тот же сервер: используется ключ сервиса чата",
        "VPN shield and button (Happ)" => "Щит и кнопка VPN (Happ)",
        "Switching" => "Переключение",
        "VPN connected" => "VPN подключён",
        "VPN disconnected" => "VPN отключён",
        "Happ is not installed" => "Happ не установлен",
        "Happ could not be started" => "Happ не удалось запустить",
        "choose a group" => "выберите группу",
        "Move to a group" => "Перенести в группу",
        "New group" => "Новая группа",
        "Name" => "Название",
        "Add group" => "Добавить группу",
        "Delete" => "Удалить",
        "Open a group in the Activity tab and click a program to move it to the next group." => {
            "Откройте группу на вкладке «Активность» и нажмите на программу, чтобы перенести её в следующую группу."
        }
        "That group name is taken, empty or one group too many" => {
            "Такое название занято, пустое или групп уже слишком много"
        }
        "Development" => "Разработка",
        "Communication" => "Общение",
        "Browsing" => "Браузеры",
        "Games" => "Игры",
        "Media" => "Медиа",
        "Work" => "Работа",
        "Other" => "Прочее",
        "Nothing recorded yet" => "Пока ничего не записано",
        "Dictate by voice" => "Диктовка голосом",
        "Paste dictated text" => "Вставлять надиктованное",
        "Listening… press the keys again" => "Слушаю… нажмите клавиши ещё раз",
        "Recognizing…" => "Распознаю…",
        "Dictation needs Gemini, an OpenAI-compatible service or a separate speech service" => {
            "Для диктовки нужен Gemini, OpenAI-совместимый сервис или отдельный сервис речи"
        }
        "Save a key for the speech service first" => "Сначала сохраните ключ сервиса речи",
        "Separate speech service" => "Отдельный сервис речи",
        "Voice command name" => "Имя для голосовых команд",
        "Save command name" => "Сохранить имя",
        "Command name saved" => "Имя сохранено",
        "Dictate the name first: \"Island, next track\", \"Island, remind me in 10 min\". Empty: off." => {
            "Скажите имя первым: «Айленд, следующий трек», «Айленд, напомни через 10 минут». Пусто: выключено."
        }
        "Up to 5 names of up to 24 characters, separated by commas" => "До 5 имён по 24 символа, через запятую",
        "Say a command after the name" => "Скажите команду после имени",
        "Done" => "Готово",
        "No time in that command" => "В команде нет времени",
        "That command is not something I can do by voice" => "Это я голосом сделать не могу",
        "Speech API key" => "Ключ API речи",
        "Speech model" => "Модель речи",
        "Speech server address" => "Адрес сервера речи",
        "Save speech service" => "Сохранить сервис речи",
        "Remove the speech key" => "Удалить ключ сервиса речи",
        "Speech service saved" => "Сервис речи сохранён",
        "Any OpenAI-compatible /audio/transcriptions server: OpenAI, Groq, your own Whisper." => {
            "Любой OpenAI-совместимый сервер /audio/transcriptions: OpenAI, Groq, свой Whisper."
        }
        "The microphone could not be opened" => "Не удалось открыть микрофон",
        "Too short, nothing was recorded" => "Слишком коротко, ничего не записано",
        "Nothing was heard (is the microphone muted?)" => "Ничего не слышно (микрофон выключен?)",
        "No speech was recognized" => "Речь не распознана",
        "This service cannot turn speech into text; choose Gemini or an OpenAI-compatible one" => {
            "Этот сервис не распознаёт речь: выберите Gemini или OpenAI-совместимый"
        }
        "Needs a Client ID" => "Нужен Client ID",
        "Create an app at developer.spotify.com," => "Создай приложение на developer.spotify.com,",
        "add the Redirect URI http://127.0.0.1:8898/callback" => "добавь Redirect URI http://127.0.0.1:8898/callback",
        "and put its Client ID into the settings file." => "и впиши его Client ID в файл настроек.",
        "Who is in your voice channel and who writes to you." => "Кто в твоём голосовом канале и кто тебе пишет.",
        "Search the clipboard" => "Поиск по буферу",
        "Nothing found" => "Ничего не найдено",
        "Sure?" => "Точно?",
        "Searching…" => "Ищу…",
        "MP4: hardware H.264 + AAC sound." => "MP4: H.264 на видеокарте + звук AAC.",
        "The GPU captures and encodes: a still" => "Видеокарта сама снимает и кодирует:",
        "screen costs almost nothing." => "статичный экран почти ничего не стоит.",
        "GIF: silent, up to 720 px wide." => "GIF: без звука, до 720 px по ширине.",
        "Only the part of a frame that" => "Сохраняется только изменившаяся часть",
        "changed is stored: small files." => "кадра: файлы получаются небольшими.",
        _ => return None,
    })
}

const PREFIXES: [(&str, &str); 9] = [
    ("Add the player ", "Добавить плеер "),
    ("Already used by: ", "Уже занято: "),
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
