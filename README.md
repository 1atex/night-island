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

Open **More → App mixer** to adjust or mute each application's audio sessions. Paused applications remain available while they have an audio session. The mixer refreshes only while its panel is open.

Open **More → Notifications → Connect** and allow notification access if Windows asks. Enable **Do not disturb** through the **Windows settings** button to suppress ordinary Windows banners. New notifications unfold into a medium card from the island, queue while it is expanded and pause their timeout on hover. Click a card to open the island's notification history; close it to return to the island. Existing Windows notifications are never removed or replayed on connection. Notification contents stay in memory.

Windows priority exceptions, reminders and alarms can still display their own banners; adjust these in [Windows notification settings](https://support.microsoft.com/en-us/windows/experience/notifications-and-do-not-disturb-in-windows). Notification quick actions are not implemented yet. On Windows versions where notification events are unavailable to unpackaged apps, the enabled listener checks for new notifications about once per second. Disable the feature to stop these checks.

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

Открой **Ещё → Микшер приложений**, чтобы изменить или отключить звук отдельной программы. Приложение остаётся в списке на паузе, пока у него есть звуковой сеанс. Микшер обновляется только при открытой панели.

Открой **Ещё → Уведомления → Подключить** и разреши доступ, если Windows его запросит. Через кнопку **Параметры Windows** включи **«Не беспокоить»**, чтобы скрыть обычные системные баннеры. Новые уведомления плавно раскрываются из острова в карточку среднего размера; пока остров открыт, они ждут в очереди. Наведение приостанавливает исчезновение. Нажатие открывает историю уведомлений в острове, крестик возвращает обычный остров. При подключении старые уведомления не всплывают заново и не удаляются из центра Windows. Содержимое хранится только в памяти.

Приоритетные исключения, напоминания и будильники могут показывать свои баннеры — их можно настроить в [параметрах уведомлений Windows](https://support.microsoft.com/en-us/windows/experience/notifications-and-do-not-disturb-in-windows). Быстрые действия пока не добавлены. Если Windows не предоставляет события уведомлений приложению без установочного пакета, при включённой функции проверка выполняется примерно раз в секунду. Отключение функции останавливает проверки.

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
