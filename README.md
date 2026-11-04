<div align="center">

# Night Island

A lightweight Dynamic Island for Windows 10/11, written in Rust.

[English](#english) · [Русский](#русский)

</div>

## English

A small floating pill at the top of the screen that grows into a panel on hover. It shows what is playing, what you copied, what is downloading and what the weather is, and keeps a few tools one click away. It draws only when something changes, so it stays out of the way: about 3 MB executable, no polling without a reason.

### Features

- Media card with cover, progress and optional synced lyrics; Spotify and Discord integrations
- Clipboard history with pinning and search, a file shelf, notes with folders and reminders
- Weather, battery, notifications and download progress right on the pill
- Screen recording to MP4 or GIF, quick toggles (Wi-Fi, Bluetooth, theme, lock)
- Command bar on `Alt+Space`: apps, calculator, text tools, QR codes
- Themes, transparency and blur, free or docked position, global hotkeys
- Russian and English interface

### App volume and Windows notifications

Press the **speaker on Home** to unfold the application mixer above the master volume slider. Adjust or mute each application and scroll to reach additional apps. Press the speaker again or Escape to close it. The full panel remains available under **More → App mixer**. Paused applications remain available while they have an audio session. The mixer refreshes only while its panel is open.

Open **More → Notifications → Connect** and allow notification access if Windows asks. Enable **Do not disturb** through the **Windows settings** button to suppress ordinary Windows banners. New notifications unfold into a medium card from the island, queue while it is expanded and pause their timeout on hover. Click a card to open the island's notification history; close it to return to the island. Existing Windows notifications are never removed or replayed on connection. Notification contents stay in memory.

Windows priority exceptions, reminders and alarms can still display their own banners; adjust these in [Windows notification settings](https://support.microsoft.com/en-us/windows/experience/notifications-and-do-not-disturb-in-windows). Notification quick actions are not implemented yet. On Windows versions where notification events are unavailable to unpackaged apps, the enabled listener checks for new notifications about once per second. Disable the feature to stop these checks.

The **Setup guide** inside Notifications explains all three steps: notification access, hiding an application's Windows banners while keeping its notification-center entries, and enabling **Use Windows notifications** in Telegram's notification settings. Telegram's custom popup windows do not pass through the Windows listener.

Cards and history display the sender/chat image supplied by the system toast, with a small application badge. Images are matched to the complete notification text and read from local image files only, then cached in memory. Telegram deletes its temporary avatar files after about a minute, so old notifications whose files have already disappeared fall back to the app icon. No Telegram login or network image requests are used.

### Yandex Music: My Wave

Open **More → My Wave → Connect Yandex Music**, copy the code and confirm it at [ya.ru/device](https://ya.ru/device). The island never asks for your password. The access token is encrypted for your Windows account; **Sign out** stops playback and removes the local token.

The Home card and My Wave share playback, seeking and track colors. Yandex cover metadata supplies the background, mini-player, accent and text palette; missing colors fall back to the cover. Colors transition smoothly with readable contrast. Select **My Wave · Settings** on Home or **Open main player** in My Wave to move between them without restarting playback.

Start your personal wave and adjust activity, variety, mood and language with the arrows. Available choices come from Yandex. Changes apply after the current track; **Next** applies them immediately. Playback continues when the island is collapsed, with cover colors, progress and controls on Home and the desktop companion. **Stop wave** releases control to your other music apps. Automatic playback on application startup is opt-in.

This integration uses the unofficial Yandex Music API and native Windows audio playback (MP3). An eligible subscription and region are required. Protocol behavior was researched in [yamusic-md3](https://github.com/yaskorinov/yamusic-md3); the implementation uses Night Island's native UI and workers. Account-specific playback has been confirmed by the user; automated checks cover device authorization endpoints, API requests, queue metadata, native audio and UI rendering.

### Build

Requires Windows, a recent stable Rust toolchain (edition 2024) and a C compiler (MSVC Build Tools or mingw-w64).

```
cargo build --release
```

The executable is `target/release/night-island.exe`. Settings and data live in `%APPDATA%\NightIsland`.

### Contributing

Issues and pull requests are welcome, see [CONTRIBUTING.md](CONTRIBUTING.md). Security reports: [SECURITY.md](SECURITY.md).

### License

[MIT](LICENSE). Bundled icons keep their own licenses, see `assets/material/LICENSE` and `assets/weather/src/LICENSE`.

## Русский

Небольшая плавающая «пилюля» у верхнего края экрана, которая раскрывается в панель при наведении. Показывает, что играет, что скопировано, что скачивается и какая погода, а ещё держит под рукой несколько инструментов. Рисует только при изменениях, поэтому не мешает системе: exe около 3 МБ, опросов без причины нет.

### Возможности

- Карточка плеера с обложкой, прогрессом и синхронным текстом песни; интеграции со Spotify и Discord
- История буфера обмена с закреплением и поиском, полка для файлов, заметки с папками и напоминания
- Погода, батарея, уведомления и ход загрузок прямо на пилюле
- Запись экрана в MP4 или GIF, быстрые переключатели (Wi-Fi, Bluetooth, тема, блокировка)
- Командная строка на `Alt+Space`: приложения, калькулятор, текстовые утилиты, QR-коды
- Темы, прозрачность и размытие, свободное или закреплённое положение, глобальные горячие клавиши
- Интерфейс на русском и английском

### Громкость приложений и уведомления Windows

Нажми **динамик на главной**, чтобы раскрыть микшер над общей громкостью. Меняй громкость отдельных программ, отключай звук и прокручивай список. Повторное нажатие или Escape закрывает микшер. Полная панель остаётся в **Ещё → Микшер приложений**. Приложение остаётся в списке на паузе, пока у него есть звуковой сеанс. Микшер обновляется только при открытой панели.

Открой **Ещё → Уведомления → Подключить** и разреши доступ, если Windows его запросит. Через кнопку **Параметры Windows** включи **«Не беспокоить»**, чтобы скрыть обычные системные баннеры. Новые уведомления плавно раскрываются из острова в карточку среднего размера; пока остров открыт, они ждут в очереди. Наведение приостанавливает исчезновение. Нажатие открывает историю уведомлений в острове, крестик возвращает обычный остров. При подключении старые уведомления не всплывают заново и не удаляются из центра Windows. Содержимое хранится только в памяти.

Приоритетные исключения, напоминания и будильники могут показывать свои баннеры — их можно настроить в [параметрах уведомлений Windows](https://support.microsoft.com/en-us/windows/experience/notifications-and-do-not-disturb-in-windows). Быстрые действия пока не добавлены. Если Windows не предоставляет события уведомлений приложению без установочного пакета, при включённой функции проверка выполняется примерно раз в секунду. Отключение функции останавливает проверки.

В уведомлениях есть вкладка **«Как настроить»**: доступ к уведомлениям, отключение баннеров Windows при сохранении записей в центре и включение **«Системного окна Windows» / «Использовать уведомления Windows»** в настройках Telegram. Собственные всплывающие окна Telegram не попадают в системный обработчик.

Карточки и история показывают фото отправителя/чата из системного уведомления с маленьким значком приложения. Фото сопоставляется с полным текстом уведомления, читается только из локального файла и сохраняется в памяти. Telegram удаляет временные аватарки примерно через минуту: у старых уведомлений с уже удалённым файлом отображается значок приложения. Вход в Telegram и загрузка изображений по сети не используются.

### Яндекс Музыка: Моя волна

Открой **Ещё → Моя волна → Подключить Яндекс Музыку**, скопируй код и подтверди вход на [ya.ru/device](https://ya.ru/device). Остров не запрашивает пароль. Токен шифруется для текущего пользователя Windows; **Выйти** останавливает музыку и удаляет сохранённый токен.

Главная карточка и «Моя волна» используют общий плеер, перемотку и палитру трека. Цвета фона, мини-плеера, акцента и текста приходят из Яндекса; недостающие вычисляются из обложки. Цвета сменяются плавно с сохранением читаемости. «Моя волна · Настроить» на главной и «К плееру» в волне переключают вкладки без перезапуска музыки.

Запусти свою волну и стрелками выбери занятие, характер, настроение и язык. Варианты загружаются из Яндекса. Новые настройки применяются после текущего трека; кнопка **Следующий** переключает сразу. Музыка играет и в свёрнутом острове: работают цвета обложки, прогресс, управление на главной и у персонажа. **Остановить** возвращает управление другим плеерам. Запуск волны вместе с островом включается отдельно.

Используется неофициальный API Яндекс Музыки и встроенный аудиоплеер Windows (MP3). Нужны подходящая подписка и доступность сервиса в регионе. Связь с API изучена по [yamusic-md3](https://github.com/yaskorinov/yamusic-md3), интерфейс и обработчики написаны для Night Island. Пользователь подтвердил работу персональной волны после входа; автоматические проверки покрывают получение кода, запросы API, данные очереди, воспроизведение Windows и отрисовку вкладки.

### Сборка

Нужны Windows, свежий стабильный Rust (edition 2024) и компилятор C (MSVC Build Tools или mingw-w64).

```
cargo build --release
```

Исполняемый файл: `target/release/night-island.exe`. Настройки и данные лежат в `%APPDATA%\NightIsland`.

### Участие

Issues и pull request приветствуются, см. [CONTRIBUTING.md](CONTRIBUTING.md). Сообщения об уязвимостях: [SECURITY.md](SECURITY.md).

### Лицензия

[MIT](LICENSE). Встроенные иконки сохраняют свои лицензии: `assets/material/LICENSE` и `assets/weather/src/LICENSE`.
