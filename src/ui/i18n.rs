use std::borrow::Cow;
#[cfg(not(test))]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(test))]
static RU: AtomicBool = AtomicBool::new(true);

#[cfg(test)]
thread_local! {
    static RU: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

pub fn set_ru(on: bool) {
    #[cfg(not(test))]
    RU.store(on, Ordering::Relaxed);
    #[cfg(test)]
    RU.set(on);
}

pub fn is_ru() -> bool {
    #[cfg(not(test))]
    {
        RU.load(Ordering::Relaxed)
    }
    #[cfg(test)]
    RU.get()
}

fn phrase(s: &str) -> Option<&'static str> {
    Some(match s {
        "Window previews and controls on hover" => "Предпросмотр окон и управление при наведении",
        "Window preview and controls" => "Предпросмотр и управление",
        "Voice chat" => "Голосовой чат",
        "Voice channel" => "Голосовой канал",
        "No voice participants" => "В канале пока никого нет",
        "Connect to control your call" => "Подключи для управления звонком",
        "Close preview" => "Закрыть предпросмотр",
        "Open window" => "Открыть окно",
        "Previous window" => "Предыдущее окно",
        "Next window" => "Следующее окно",
        "Close window" => "Закрыть окно",
        "Previous player" => "Предыдущий плеер",
        "Previous track" => "Предыдущий трек",
        "Next track" => "Следующий трек",
        "Play" => "Воспроизвести",
        "Mute application" => "Выключить звук приложения",
        "Unmute application" => "Включить звук приложения",
        "Quieter" => "Тише",
        "Louder" => "Громче",
        "Application volume" => "Громкость",
        "Connect Discord in settings" => "Подключить Discord в настройках",
        "Discord voice controls require connection" => "Для управления голосом подключите Discord",
        "Discord voice controls unavailable" => "Управление голосом Discord недоступно",
        "Waiting for Discord voice settings" => "Получение настроек голоса Discord…",
        "Window minimized" => "Окно свёрнуто",
        "Preview unavailable" => "Предпросмотр недоступен",
        "Window is no longer available" => "Окно больше недоступно",
        "Running" => "Запущено",
        "Windows" => "Окна",
        "Minimized" => "Свёрнуто",
        "Active" => "Активно",
        "Properties" => "Свойства",
        "Copy application path" => "Копировать путь приложения",
        "Restore all windows" => "Восстановить все окна",
        "Minimize all windows" => "Свернуть все окна",
        "Close windows normally" => "Закрыть окна",
        "Stop hiding dock for this app" => "Не скрывать док в этом приложении",
        "Hide dock while this app is active" => "Скрывать док в этом приложении",
        "Keep dock visible" => "Оставлять док видимым",
        "Auto-hide dock" => "Автоматически скрывать док",
        "Island settings" => "Настройки Island",
        "Dock behavior…" => "Поведение дока…",
        "Application actions…" => "Действия с приложением…",
        "Windows…" => "Окна…",
        "Night Island · Settings" => "Night Island · Настройки",

        "Fullscreen apps hide the dock only on their monitor. Alt+Tab brings it back." => {
            "Полноэкранные приложения скрывают док на своём мониторе. Alt+Tab вернёт его."
        }
        "Games and application rules" => "Игры и правила приложений",
        "For a windowed game, right-click its dock icon and choose Hide dock in this app." => {
            "Для игры в окне: ПКМ по значку → «Скрывать док в этом приложении»."
        }
        "Remove rule" => "Убрать правило",
        "Blur behind the dock" => "Размытие фона за доком",
        "Dock tint" => "Оттенок панели",
        "Dock tint saturation" => "Насыщенность оттенка",
        "Dock tint brightness" => "Яркость панели",
        "Applications" => "Приложения",
        "Pinned applications" => "Закреплённые приложения",
        "Add" => "Добавить",
        "Monitor" => "Монитор",
        "Desktop dock" => "Нижний док",
        "Enable desktop dock" => "Включить нижний док",
        "One process with Island. Enable after closing your other dock." => {
            "Работает внутри Island. Перед включением закрой другой док."
        }
        "Automatically hide the dock" => "Автоматически скрывать док",
        "Show running applications" => "Показывать запущенные приложения",
        "Hide over fullscreen applications" => "Скрывать поверх полноэкранных приложений",
        "Economy animations (30 fps)" => "Экономичные анимации (30 кадров/с)",
        "Dock icon size" => "Размер иконок",
        "Magnification on hover" => "Увеличение при наведении",
        "Dock opacity" => "Непрозрачность дока",
        "Bottom margin" => "Отступ снизу",
        "Switch dock monitor" => "Переключить монитор дока",
        "Primary monitor" => "Основной монитор",
        "Import applications from MyDockFinder" => "Импорт приложений из MyDockFinder",
        "Add application or file" => "Добавить приложение или файл",
        "Drop shortcuts onto the dock. Drag pinned icons to reorder." => {
            "Перетащи ярлыки на док. Закреплённые значки можно переставлять."
        }
        "Right-click for windows, pinning and settings. Scroll for more apps." => {
            "ПКМ — окна и закрепление. Колесо — прокрутка приложений."
        }
        "Windows taskbar settings" => "Параметры панели задач Windows",
        "To free the bottom edge, enable taskbar auto-hide in Windows." => {
            "Чтобы освободить низ экрана, включи автоскрытие панели Windows."
        }
        "Could not create the desktop dock" => "Не удалось создать док",
        "Application added to the dock" => "Приложение добавлено в док",
        "Already pinned or dock is full (32)" => "Уже закреплено или достигнут лимит (32)",
        "Could not read ico.ini" => "Не удалось прочитать ico.ini",
        "Imported applications" => "Добавлено приложений",
        "Open new window" => "Открыть новое окно",
        "Unpin from desktop dock" => "Открепить от дока",
        "Pin to desktop dock" => "Закрепить в доке",
        "Move left" => "Передвинуть влево",
        "Move right" => "Передвинуть вправо",
        "Desktop dock settings" => "Настройки нижнего дока",
        "Dock configuration is too large" => "Файл настроек дока слишком большой",
        "Unsupported dock configuration encoding" => "Неподдерживаемая кодировка настроек дока",
        "No applications found in ico.ini" => "В ico.ini не найдены приложения",
        "Import" => "Импортировать",
        "Switch" => "Переключить",

        "Create a pet from this photo" => "Создай питомца по этой фотографии",
        "Preview ready. Apply your pet or describe another change below." => {
            "Питомец готов. Нажми «Применить» или опиши, что ещё изменить."
        }
        "Building a preview. Your current pet stays with you." => {
            "Готовлю предпросмотр. Текущий питомец пока остаётся с тобой."
        }
        "Shaping your pet…" => "Готовим новый образ…",
        "Describe another change below." => "Опиши новые изменения ниже.",
        "Apply your pet or describe a change." => "Примени или опиши изменения.",
        "Describe your pet below." => "Опиши питомца ниже.",
        "Describe your pet or add a photo." => "Опиши питомца или добавь фото.",
        "Back to chat" => "К чату",
        "Needs attention" => "Нужно уточнить",
        "Creating…" => "Создаём образ",
        "With you" => "Уже с тобой",
        "Preview" => "Предпросмотр",
        "Palette" => "Палитра",
        "Your pet is taking shape. Your current companion stays with you." => {
            "Готовим новый образ. Текущий питомец остаётся с тобой."
        }
        "Describe a look below or add a photo for a vision model." => {
            "Опиши внешность ниже или добавь фото для модели с распознаванием изображений."
        }
        "Photo attached. Tell us what to keep or change." => "Фото добавлено. Напиши, что сохранить или изменить.",
        "Pet studio" => "Питомец",
        "Create or change your pet" => "Создать или изменить питомца",
        "Create your pet" => "Создай питомца",
        "Creating your pet…" => "Создаём питомца…",
        "Apply pet" => "Применить",
        "Applied" => "Применён",
        "Pet photo" => "Фото",
        "Text only" => "Без фото",
        "Describe your pet…" => "Опиши питомца или изменения…",
        "Your pet is ready. Describe another change below." => "Питомец готов. Ниже можно описать новые изменения.",
        "Describe the shape, colors and details. Photos will not be sent." => {
            "Опиши форму, цвета и детали. Фото не будет отправлено."
        }
        "Describe your pet or attach a photo. A model with vision is needed for photos." => {
            "Опиши питомца или добавь фото. Для фото нужна модель с распознаванием изображений."
        }
        "Photo attached. Add your wishes below, then press Send." => {
            "Фото добавлено. Напиши пожелания ниже и нажми отправить."
        }
        "Could not build this pet. Try a simpler description." => {
            "Не удалось создать питомца. Попробуй описать его проще."
        }
        "This model did not accept the photo. Select Text only and describe your pet, or choose a vision model." => {
            "Модель не приняла фото. Нажми «Без фото» и опиши питомца или выбери модель с распознаванием изображений."
        }
        "Choose a PNG, JPEG or WebP photo for your pet" => "Выбери фото питомца в PNG, JPEG или WebP",
        "Bear" => "Медведь",
        "Custom pet" => "Свой питомец",
        "My pet" => "Мой питомец",
        "Pet animation quality" => "Анимация питомца",
        "Pet walks, rests and sleeps on its own" => "Самостоятельные прогулки, отдых и сон",
        "Pet: reduced motion" => "Без движения",
        "Pet: economical · up to 20 FPS" => "Экономно · до 20 FPS",
        "Pet: balanced · up to 30 FPS" => "Баланс · до 30 FPS",
        "Pet: smooth · up to 60 FPS" => "Плавно · до 60 FPS",
        "My Wave · Settings" => "Моя волна · Настроить",
        "Open My Wave settings" => "Открыть настройки Моей волны",
        "Open main player" => "К плееру",
        "Could not remove the saved sign-in. Try signing out again." => {
            "Не удалось удалить сохранённый вход. Повтори выход из аккаунта."
        }
        "Cancel" => "Отмена",
        "Yandex Music" => "Яндекс Музыка",
        "Dismiss" => "Закрыть",
        "Back" => "Назад",
        "Expand notification" => "Развернуть уведомление",
        "Could not open the sender app" => "Не удалось открыть приложение-отправитель",
        "Open sender app" => "Открыть приложение",
        "4 tiles" => "4 плитки",
        "2 tiles" => "2 плитки",
        "1 tile" => "1 плитка",
        "Click a notification to open its app" => "Клик по уведомлению открывает приложение",
        "Notification preview size" => "Размер превью уведомления",
        "Code" => "Код",
        "Start My Wave with the island" => "Запускать волну вместе с островом",
        "Open sign-in page" => "Открыть страницу входа",
        "Open My Wave" => "Открыть Мою волну",
        "Wave activity" => "Занятие",
        "Connected for this session; Windows could not save the sign-in" => {
            "Вход выполнен, но Windows не сохранила его для следующего запуска"
        }
        "Sign out" => "Выйти",
        "Resume" => "Продолжить",
        "Pause" => "Пауза",
        "Connecting…" => "Подключение…",
        "Loading music…" => "Загружаем…",

        "My Wave" => "Моя волна",
        "Yandex Music inside your island" => "Яндекс Музыка в острове",
        "Sign in in your browser. Your password stays with Yandex." => {
            "Войди через браузер. Пароль остаётся у Яндекса."
        }
        "Enter this code at ya.ru/device" => "Введи этот код на ya.ru/device",
        "Waiting for confirmation in your browser…" => "Ждём подтверждения в браузере…",
        "Requires Yandex Music access. The connection uses an unofficial API." => {
            "Нужен доступ к Яндекс Музыке по подписке. Подключение использует неофициальный API."
        }
        "Connect Yandex Music" => "Подключить Яндекс Музыку",
        "Open Yandex" => "Открыть Яндекс",
        "Copy code" => "Скопировать код",
        "Made for you" => "Подобрано для тебя",
        "Music for this moment" => "Музыка под настроение",
        "Variety" => "Характер",
        "Mood" => "Настроение",
        "Any" => "Любое",
        "Start wave" => "Запустить",
        "Stop wave" => "Остановить",
        "Start wave when Island starts" => "Запускать при старте острова",
        "Could not connect to Yandex Music. Try again." => "Не удалось связаться с Яндекс Музыкой. Попробуй снова.",
        "Sign in to Yandex Music again" => "Войди в Яндекс Музыку снова",
        "Yandex Music requires an active subscription and an available region" => {
            "Нужна действующая подписка и доступность Яндекс Музыки в регионе"
        }
        "Yandex Music is busy. Wait a moment and try again." => {
            "Яндекс Музыка занята. Подожди немного и попробуй снова."
        }
        "Yandex Music could not complete the request. Try again." => {
            "Яндекс Музыка не выполнила запрос. Попробуй снова."
        }
        "This track is unavailable with your subscription or in your region" => {
            "Трек недоступен по подписке или в твоём регионе"
        }
        "Could not save My Wave settings" => "Не удалось сохранить настройки волны",
        "Could not start Yandex sign-in. Try again." => "Не удалось начать вход. Попробуй снова.",
        "Could not play this track. Check your connection and start the wave again." => {
            "Трек не воспроизводится. Проверь соединение и запусти волну снова."
        }
        "My Wave returned no tracks. Try different settings." => "Волна не вернула треки. Попробуй другие настройки.",
        "Could not start the Windows audio player" => "Не удалось запустить аудиоплеер Windows",
        "The sign-in code expired. Connect again." => "Код входа истёк. Нажми «Подключить» снова.",
        "Yandex sign-in was cancelled or expired. Connect again." => "Вход отменён или код истёк. Подключись снова.",
        "Wave settings are unavailable. Press Try again to reload." => {
            "Настройки волны недоступны. Нажми «Повторить», чтобы загрузить снова."
        }

        "Close the app mixer" => "Закрыть микшер приложений",
        "Prefer the player that is playing" => "Приоритет играющего плеера",
        "Player priority: pause My Wave and start a video to control the browser from Home. My Wave keeps its own controls." => {
            "При включённом приоритете: поставьте волну на паузу и запустите видео — главная переключится на браузер. Во вкладке «Моя волна» остаётся управление Яндексом."
        }
        "Player volume: left-click for system, right-click for mixer" => {
            "Громкость плеера: ЛКМ — система, ПКМ — микшер"
        }
        "System volume: left-click for player, right-click for mixer" => "Громкость системы: ЛКМ — плеер, ПКМ — микшер",
        "Player volume is unavailable. Right-click for the mixer" => {
            "Громкость этого плеера недоступна. ПКМ — открыть микшер"
        }
        "App mixer: adjust each application's volume" => "Микшер: громкость каждого приложения",
        "Setup guide" => "Как настроить",
        "Connected. The island receives Windows system notifications" => {
            "Подключено. Остров получает системные уведомления Windows"
        }
        "1. Connect notification access" => "1. Подключи уведомления",
        "Press Connect and allow access when Windows asks." => {
            "Нажми «Подключить» и разреши доступ, если Windows его запросит."
        }
        "2. Keep notifications, hide duplicate banners" => "2. Убери дубликаты справа снизу",
        "Windows settings → your app: banners off, notification center on. Do not disturb alone can have exceptions." => {
            "Включи «Не беспокоить». В параметрах нужного приложения: баннеры выкл., центр уведомлений вкл."
        }
        "3. Telegram needs Windows notifications" => "3. Включи системные уведомления в Telegram",
        "Telegram → Settings → Notifications and Sounds → Use Windows notifications. Custom Telegram popups cannot reach the island." => {
            "Telegram → Настройки → Уведомления и звуки: включи «Системное окно Windows» или «Использовать уведомления Windows»."
        }
        "Windows notifications" => "Уведомления Windows",
        "Windows notifications from the island" => "Уведомления Windows из острова",
        "Windows notification settings" => "Настройки уведомлений Windows",
        "Show Windows notifications from the island" => "Показывать уведомления Windows из острова",
        "Allow notification access to connect the island" => "Разреши острову доступ к уведомлениям",
        "Notification access is disabled in Windows privacy settings" => {
            "Доступ к уведомлениям отключён в настройках конфиденциальности Windows"
        }
        "Turn on Do not disturb in Windows to hide the standard banners" => {
            "Включи «Не беспокоить» в Windows, чтобы скрыть стандартные баннеры"
        }
        "Connected. New notifications will appear from the island" => {
            "Подключено. Новые уведомления будут появляться из острова"
        }
        "Windows notifications are unavailable. Try connecting again" => {
            "Уведомления Windows недоступны. Попробуй подключиться ещё раз"
        }
        "Allow access" => "Дать доступ",
        "Disable" => "Отключить",
        "Try again" => "Повторить",
        "Windows settings" => "Параметры Windows",
        "Show example" => "Показать пример",
        "Notification history" => "Центр уведомлений",
        "New notifications will be listed here" => "Здесь появятся уведомления Windows",
        "Notifications from the island" => "Уведомления из острова",
        "Your Windows notifications will appear here. Hover to keep reading." => {
            "Здесь будут появляться уведомления Windows. Наведи курсор, чтобы спокойно прочитать."
        }
        "App mixer" => "Микшер приложений",
        "New notification" => "Новое уведомление",
        "Separate volume for each app" => "Отдельная громкость для каждого приложения",
        "System sounds" => "Системные звуки",
        "Looking for audio apps…" => "Ищем приложения со звуком…",
        "Audio mixer is unavailable" => "Не удалось открыть микшер. Проверь устройство вывода звука.",
        "Play music, a video or a game to see its volume here" => {
            "Запусти музыку, видео или игру — приложение появится здесь."
        }
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
        "Not set" => "Не задано",
        "Rebuild and restart" => "Пересобрать и перезапустить",
        "Panel blur" => "Размытие панели",
        "Hide in screenshots" => "Скрыть на снимках",
        "Hide island in my captures" => "Прятать при записи и снимке",
        "Game mode" => "Игровой режим",
        "REC" => "Запись",
        "Format" => "Формат",
        "Frame rate" => "Частота кадров",
        "System sound" => "Звук системы",
        "No character" => "Без персонажа",
        "More" => "Ещё",
        "Weather position" => "Положение погоды",
        "Left of the clock" => "Слева от времени",
        "Right of the clock" => "Справа от времени",
        "Customize header…" => "Настроить шапку…",
        "Add to header" => "Добавить в шапку",
        "On the header" => "В шапке",
        "In More" => "В «Ещё»",
        "Replace:" => "Вместо:",
        "Restore default header" => "Восстановить шапку",
        "Your companion is here" => "Ваш персонаж рядом",
        "More tabs and actions" => "Другие вкладки и действия",
        "AI settings" => "Настройки ИИ",
        "You" => "Вы",
        "Ask me anything. This chat stays beside your character." => {
            "Задайте мне вопрос. Этот чат открывается рядом с вашим персонажем."
        }
        "Enter to send · Shift+Enter for a new line" => "Enter — отправить · Shift+Enter — новая строка",
        "Connect your AI provider in AI settings." => "Подключите нейросеть в настройках ИИ.",
        "Release the pet from the island" => "Выпустить питомца на рабочий стол",
        "Pets pause near the pointer and during chat. Reduced motion stops walking." => {
            "Рядом с курсором и во время чата питомец останавливается. Режим «Без движения» отключает прогулки."
        }
        "Release character" => "Выпустить персонажа",
        "Click or drag to release the character" => "Нажмите или потяните, чтобы выпустить персонажа",
        "Click for mini AI chat, right-click to hide/show the island. Drag to move." => {
            "ЛКМ — мини-чат с ИИ, ПКМ — скрыть/показать остров. Наведение — музыка. Перенесите на остров для возврата."
        }
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
        "Mostly clear" => "Малооблачно",
        "Overcast" => "Пасмурно",
        "Light rain" => "Небольшой дождь",
        "Heavy rain" => "Сильный дождь",
        "Rain showers" => "Кратковременный дождь",
        "Heavy rain showers" => "Сильный кратковременный дождь",
        "Sleet showers" => "Кратковременный дождь со снегом",
        "Heavy snow" => "Сильный снег",
        "Snow showers" => "Снег с прояснениями",
        "Heavy snow showers" => "Сильный снег с прояснениями",
        "Thunderstorm with rain" => "Гроза с дождём",
        "Thunderstorm with sleet" => "Гроза с дождём и снегом",
        "Thunderstorm with snow" => "Гроза со снегом",
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
        "Song progress strip" => "Полоска прогресса песни",
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
        "Cover shape" => "Форма обложки",
        "January" => "Январь",
        "February" => "Февраль",
        "March" => "Март",
        "April" => "Апрель",
        "May" => "Май",
        "June" => "Июнь",
        "July" => "Июль",
        "August" => "Август",
        "September" => "Сентябрь",
        "October" => "Октябрь",
        "November" => "Ноябрь",
        "December" => "Декабрь",
        "Mo" => "Пн",
        "Tu" => "Вт",
        "We" => "Ср",
        "Th" => "Чт",
        "Fr" => "Пт",
        "Sa" => "Сб",
        "Su" => "Вс",
        "Pick date and time" => "Выбрать дату и время",
        "Unpin" => "Открепить",
        "Equalizer while the island is open" => "Эквалайзер при открытом острове",
        "One colour" => "Один цвет",
        "Gradient outwards" => "Градиент наружу",
        "Liquid edge colour while music plays" => "Цвет жидкого края при музыке",
        "Equalizer colours" => "Цвета эквалайзера",
        "Progress colours" => "Цвета полосы прогресса",
        "Gradient left to right" => "Градиент слева направо",
        "Progress style" => "Вид прогресса",
        "Strip along the bottom" => "Полоска снизу",
        "Outline from the left side" => "Контур от левой стороны",
        "Outline, the bottom fills" => "Контур, заполняется низ",
        "Fill the island" => "Заливка острова",
        "30 FPS" => "30 кадров/с",
        "60 FPS" => "60 кадров/с",
        "Monitor refresh rate" => "Частота монитора",
        "Adaptive" => "Адаптивная",
        "Equalizer shape" => "Форма эквалайзера",
        "Equalizer strength" => "Сила эквалайзера",
        "Spikes" => "Шипы",
        "Wave" => "Волна",
        "Bars" => "Столбики",
        "Halo around the pill" => "Лучи вокруг пилюли",
        "Aura around the pill" => "Свечение вокруг пилюли",
        "Settings of this tab" => "Настройки этой вкладки",
        "Rename" => "Переименовать",
        "Delete (press twice)" => "Удалить (нажми дважды)",
        "Make it a plain note" => "Сделать обычной заметкой",
        "Make it a reminder" => "Сделать напоминанием",
        "Pin or unpin" => "Закрепить / открепить",
        "Saved chats" => "Сохранённые чаты",
        "Attach a file" => "Прикрепить файл",
        "Delete the chat" => "Удалить чат",
        "Previous" => "Назад",
        "Play or pause" => "Играть / пауза",
        "Next" => "Дальше",
        "Shuffle" => "Перемешать",
        "Repeat" => "Повтор",
        "Like" => "Нравится",
        "Next player" => "Следующий плеер",
        "Choose the tiles" => "Выбрать плитки",
        "Volume of this program: press for the system" => "Громкость программы: нажми для системной",
        "System volume: press for the playing program" => "Системная громкость: нажми для программы",
        "Sound output" => "Вывод звука",
        "Mute" => "Выключить звук",
        "Deafen" => "Заглушить",
        "Voice overlay" => "Оверлей голоса",
        "Fold the dock" => "Свернуть",
        "Show the whole dock" => "Показать все закреплённые",
        "-1 d" => "-1 д",
        "+1 d" => "+1 д",
        "-1 h" => "-1 ч",
        "+1 h" => "+1 ч",
        "-10 m" => "-10 мин",
        "+10 m" => "+10 мин",
        "In 1 h" => "Через 1 ч",
        "Look and feel" => "Внешний вид",
        "Monitoring" => "Мониторинг",
        "Services" => "Сервисы",
        "Program" => "Программа",
        "Notes" => "Заметки",
        "Move tabs by dragging" => "Перетаскивать вкладки",
        "Stop moving tabs" => "Не перетаскивать вкладки",
        "Pinned" => "Закреплено",
        "Click to open, drag to reorder, right-click for more" => {
            "Клик открывает, перетаскивание меняет порядок, правый клик — меню"
        }
        "Case" => "Регистр",
        "Lines" => "Строки",
        "Convert" => "Формат",
        "Encode" => "Кодек",
        "Info" => "Инфо",
        "Dev tools" => "Dev",
        "All tools…" => "Все инструменты…",
        "Close the catalogue" => "Закрыть каталог",
        "List" => "Список",
        "Chips and catalogue" => "Кнопки и каталог",
        "Text tools" => "Инструменты текста",
        "Text tab" => "Вкладка «Текст»",
        "Fix text typed in the wrong layout" => "Исправить текст в неверной раскладке",
        "Make everything UPPERCASE" => "ВСЁ ЗАГЛАВНЫМИ",
        "Make everything lowercase" => "всё строчными",
        "Capitalise Each Word" => "Каждое Слово С Заглавной",
        "Cyrillic to Latin and back" => "Кириллица ↔ латиница",
        "Sort the lines A to Z" => "Сортировка строк А–Я",
        "Remove repeated lines" => "Убрать повторы строк",
        "Trim and collapse extra spaces" => "Убрать лишние пробелы",
        "Characters, words, lines, bytes" => "Символы, слова, строки, байты",
        "Show the text as a QR code" => "Показать текст как QR-код",
        "Words to snake_case" => "Слова в snake_case",
        "Words to camelCase" => "Слова в camelCase",
        "Words to kebab-case" => "Слова в kebab-case",
        "Format JSON with indents" => "JSON с отступами",
        "Squeeze JSON into one line" => "JSON в одну строку",
        "Encode the text as Base64" => "Текст в Base64",
        "Decode Base64 back to text" => "Base64 обратно в текст",
        "Percent-encode for a URL" => "Кодирование для URL",
        "Decode a percent-encoded URL" => "Раскодировать URL",
        "Read a JWT's header and payload" => "Заголовок и данные JWT",
        "Unix time and date (now if empty)" => "Unix-время и дата (пусто — сейчас)",
        "Text as hexadecimal bytes" => "Текст в шестнадцатеричном виде",
        "Hexadecimal bytes back to text" => "Hex обратно в текст",
        "SHA-256 hash of the text" => "Хеш SHA-256 текста",
        "Insert a new random UUID" => "Вставить новый UUID",
        "Animations" => "Анимации",
        "Animation curve" => "Кривая анимации",
        "Tab change" => "Смена вкладки",
        "Tab change speed" => "Скорость смены вкладки",
        "Curve" => "Кривая",
        "Spring" => "Пружина",
        "Smooth" => "Плавная",
        "Ease out" => "Замедление",
        "Linear" => "Линейная",
        "Fade" => "Затухание",
        "Slide" => "Сдвиг",
        "Zoom" => "Масштаб",
        "Rise" => "Подъём",
        "Application" => "Приложение",
        "Behaviour" => "Поведение",
        "Advanced" => "Дополнительно",
        "Ghost mode" => "Режим призрака",
        "Placement and look" => "Положение и вид",
        "Open and close" => "Открытие и закрытие",
        "Window" => "Окно",
        "Liquid shape" => "Жидкая форма",
        "Pill" => "Пилюля",
        "Music" => "Музыка",
        "Edges" => "Края",
        "Clipboard and keys" => "Буфер и клавиши",
        "Apps and services" => "Программы и сервисы",
        "Preview and presets" => "Просмотр и пресеты",
        "Crop to square" => "Обрезать до квадрата",
        "Stretch" => "Сжать",
        "Keep proportions" => "Сохранить пропорции",
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
        "Press the new keys (Esc clears)…" => "Нажмите новые клавиши (Esc — убрать)…",
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
