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

Left-click the **speaker on Home** to switch between system volume and the current player's application volume. Right-click it to unfold the application mixer; right-click again or press Escape to close it. The full panel is also available under **More → App mixer**. Paused applications remain available while they have an audio session. The mixer refreshes only while open.

Open **More → Notifications → Connect** and allow notification access if Windows asks. Enable **Do not disturb** through **Windows settings** to suppress ordinary Windows banners. Hover over a notification to open a Home preview occupying one, two or four tiles, as selected in notification settings. Its expand button opens the full, scrollable message. **Open sender app** is always available in the full message; an optional setting also makes the notification itself open the sender. Existing Windows notifications are never removed or replayed on connection. Notification contents stay in memory.

Windows priority exceptions, reminders and alarms can still display their own banners; adjust these in [Windows notification settings](https://support.microsoft.com/en-us/windows/experience/notifications-and-do-not-disturb-in-windows). Notification quick actions are not implemented yet. On Windows versions where notification events are unavailable to unpackaged apps, the enabled listener checks for new notifications about once per second. Disable the feature to stop these checks.

The **Setup guide** inside Notifications explains all three steps: notification access, hiding an application's Windows banners while keeping its notification-center entries, and enabling **Use Windows notifications** in Telegram's notification settings. Telegram's custom popup windows do not pass through the Windows listener.

Cards and history display the sender/chat image with a small application badge. A read-only fallback to the current user's Windows notification database recovers images and original activation arguments unavailable through the public listener. Records must match notification ID, sender ID and full text; schema changes or locked databases safely fall back to the listener. Local paths, including Discord's mixed path separators, are supported. Discord CDN avatar requests are bounded and use an allowlist; decoded images are cached in memory. Missing images fall back to the app icon.

Opening a notification uses its original registered Windows notification callback and arguments, or a supported sender-owned protocol link. The sender's registered shortcut is the fallback. Some applications provide no destination in their toast, so opening a specific conversation depends on their activation handler. No message text is interpreted as a command, and notification quick actions/replies are not replayed.

### Header customization

Use **More → Customize header…** to add a tab without removing another, explicitly replace a tab, or move a pinned tab back to More. Up to five additional tabs have full labels; more than five switch to compact circular icons with tooltips. Tabs that do not fit remain in More. Liquid deformation is decorative: only the original island body and its attached character capture mouse input.

### Yandex Music: My Wave

Open **More → My Wave → Connect Yandex Music** or **Settings → Integrations → Yandex Music**, copy the code and confirm it at [ya.ru/device](https://ya.ru/device). The island never asks for your password. The access token is encrypted for your Windows account; **Sign out** stops playback and removes the local token.

The Home card and My Wave share playback, seeking and track colors. Yandex cover metadata supplies the background, mini-player, accent and text palette; missing colors fall back to the cover. Colors transition smoothly with readable contrast. Select **My Wave · Settings** on Home or **Open main player** in My Wave to move between them without restarting playback.

Start your personal wave and adjust activity, variety, mood and language with the arrows. Available choices come from Yandex. Changes apply after the current track; **Next** applies them immediately. Playback continues when the island is collapsed, with cover colors, progress and controls on Home and the desktop companion. **Stop wave** releases control to your other music apps. Automatic playback on application startup is opt-in.

This integration uses the unofficial Yandex Music API and native Windows audio playback (MP3). An eligible subscription and region are required. Protocol behavior was researched in [yamusic-md3](https://github.com/yaskorinov/yamusic-md3); the implementation uses Night Island's native UI and workers. Account-specific playback has been confirmed by the user; automated checks cover device authorization endpoints, API requests, queue metadata, native audio and UI rendering.

### Desktop dock

An optional bottom dock shares Island's process and graphics device. Enable it in **Settings → Desktop dock** after closing another dock. It includes pinned apps and files, running-window indicators, click to launch/restore/minimize, a window list on right-click, pinned-icon reordering and file drops. Scroll when the app list exceeds the screen width.

Settings control icon size, hover magnification, native backdrop blur, tint hue/saturation/brightness, opacity, bottom margin, monitor, auto-hide and 30/60 fps motion. Blur and plain materials remember separate opacity values; unsupported systems keep the configured tint without the blur effect. Icons load asynchronously at the physical size needed for DPI and zoom, preserve their aspect ratio and alpha, and reuse cached GPU bitmaps. Open Island settings appear in the running-app list while helper windows remain excluded.

Right-click menus show application/window state and provide window activation, restore/minimize/normal close, file location/properties, copying the executable path, pinning and app-specific dock behavior. Long menus scroll within the current monitor and support keyboard navigation.

Fullscreen and borderless apps hide the dock on their monitor; ordinary maximized windows do not. For windowed games, use **Dock behavior → Hide dock while this app is active**; rules can be removed in dock settings. Hidden dock windows do not intercept the pointer or run animation/icon-refresh work. Alt+Tab restores the dock without activating it. Game compatibility is checked with native window fixtures, not with every game or anti-cheat. Animation frames stop at rest, Windows reduced-motion settings are respected, and disabling the dock releases its window and timers. The Windows taskbar settings link lets you enable its auto-hide; Island does not replace Explorer or change taskbar visibility automatically.

**Import applications from MyDockFinder** reads `ico.ini` from the default Steam installation, or asks for the file when it is elsewhere. It merges application paths in order without copying MyDockFinder binaries, themes or icon packs. Icons come from Windows. Notification badges and third-party window-minimization effects are not included. The upper panel remains Island.

Hover briefly over a running app to open its live Windows thumbnail, switch between its windows, open or close a window, and use the controls that app exposes. Media controls target the hovered application and selected session, even when Island shows another player; app volume/mute affects only its audio sessions. Discord microphone/deafen controls require the existing authorized Discord integration and a confirmed voice-settings connection to that exact Discord client. Unavailable controls stay disabled; the popup links to integration settings when disconnected. Hover cards reuse Island’s popup panel, theme and rounded surfaces. Discord places the voice channel and participant status above a compact window preview, with circular mute/deafen controls; app audio uses the same slider style as Home. Previews never restore or activate a source window on hover; minimized apps may supply a cached image, and protected/unavailable sources show a fallback. Escape or leaving the card dismisses it. The hover option can be disabled in dock settings; **Window preview and controls** remains available on right-click.

Dock contours preserve their antialiased edge pixels at fractional DPI, and switching/minimizing windows triggers a single delayed backdrop refresh rather than a continuous capture loop.

### Interface motion

Home action tiles, playback buttons, header selection and control tiles use short hover transitions. Pressed Home buttons compress slightly; settings switches slide their thumb and segmented controls move their selection. Repeated input retargets from the currently visible value. The feedback timer stops when transitions finish.

When Home is opened from the music pill, its cover moves between the compact slot and the player card; closing reverses that movement. Changing a track or player fades the artwork, replaces the labels in two phases for readability and blends the card tint. Rapid track changes preserve the current visual mixture. Cover motion is cleaned up on hiding, layout changes and navigation. New motion respects Windows' animation setting.

### Desktop pets

Drag a character out of the collapsed island to release it; drag it back over the island to dock it. Pets breathe, follow the pointer smoothly, blink, react to gentle pointer strokes, stretch while carried and settle when released. Bear joins the character list. Music controls and the mini AI chat remain available beside every detached character, including custom pets.

**Settings → Island → Pet animation quality** offers reduced motion, economical (up to 20 FPS), balanced (up to 30 FPS) and smooth (up to 60 FPS). The pet has its own frame budget, lowers its refresh rate at rest and while sleeping, and stops drawing in gaming mode.

**Pet walks, rests and sleeps on its own** enables short walks along the height where you leave the detached pet, sitting breaks and occasional naps. Pets stay inside the current monitor’s working area. Approaching, dragging, using the player or opening either AI chat pauses roaming and wakes the pet. There is a short pause after interaction before the next walk. Disabling autonomy preserves manual dragging and pointer reactions; reduced motion also prevents walking. Movement never activates the pet window or saves settings on each frame.

Open **Pet studio** in either AI chat. Describe a pet or select **Pet photo**, then send your request using your configured AI provider. The model produces a stylized animated character with editable colors, body, markings and accessories. Review the preview and select **Apply pet**; further messages revise that design. Photo input requires a vision-capable model. **Text only** excludes the attached photo and lets you describe the desired appearance instead. The chosen pet survives restarts; generated designs cannot contain code or external assets. The studio shows a live portrait, the selected photo and a color palette, with distinct preview, applied and error states. Its animation follows the pet quality setting and pauses when hidden; the mini chat also pauses it when unfocused. Rounded silhouettes, soft shading and expressive eyes carry across both the island and detached pets.

### Pet speech extension

`src/ui/pet_behavior.rs` exposes time-based activity transitions as `BehaviorEvent` values. `src/ui/pet_speech.rs` accepts typed `SpeechCue` values (`Quiet`, `Listening`, `Thinking`, `Speaking { amplitude }`) and smooths a bounded mouth envelope. The controller in `src/island/character.rs` derives listening/thinking cues from both chats; a future voice adapter can set `Pet.speech_cue` while playing audio and restore `Quiet` when finished or cancelled. Speaking keeps the pet attentive and stops roaming. `Motion.mouth` drives mouth shapes (or eye modulation for Eyes) and honors reduced motion. No microphone capture, speech synthesis or extra AI requests are enabled by this foundation.

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

**ЛКМ по динамику на главной** переключает системную громкость и громкость приложения текущего плеера. **ПКМ** раскрывает микшер; повторный ПКМ или Escape закрывает его. Полная панель остаётся в **Ещё → Микшер приложений**. Приложение остаётся в списке на паузе, пока у него есть звуковой сеанс. Микшер обновляется только при открытой панели.

Открой **Ещё → Уведомления → Подключить** и разреши доступ, если Windows его запросит. Через **Параметры Windows** включи **«Не беспокоить»**, чтобы скрыть обычные системные баннеры. Наведение на уведомление раскрывает его на главной: размер в одну, две или четыре плитки выбирается в настройках уведомлений. Кнопка разворачивания открывает полный текст с прокруткой. **«Открыть приложение»** доступно всегда в полном уведомлении; отдельная настройка позволяет переходить в приложение и по клику на саму карточку. Старые уведомления при подключении не показываются заново и не удаляются из центра Windows. Содержимое хранится только в памяти.

Приоритетные исключения, напоминания и будильники могут показывать свои баннеры — их можно настроить в [параметрах уведомлений Windows](https://support.microsoft.com/en-us/windows/experience/notifications-and-do-not-disturb-in-windows). Быстрые действия пока не добавлены. Если Windows не предоставляет события уведомлений приложению без установочного пакета, при включённой функции проверка выполняется примерно раз в секунду. Отключение функции останавливает проверки.

В уведомлениях есть вкладка **«Как настроить»**: доступ к уведомлениям, отключение баннеров Windows при сохранении записей в центре и включение **«Системного окна Windows» / «Использовать уведомления Windows»** в настройках Telegram. Собственные всплывающие окна Telegram не попадают в системный обработчик.

Карточки и история показывают фото отправителя/чата со значком приложения. Недостающие изображения и аргументы открытия берутся из базы уведомлений текущего пользователя Windows, доступ к которой выполняется только для чтения. Проверяется совпадение ID уведомления, приложения и полного текста. Если база недоступна или её формат изменился, продолжает работать системный обработчик. Поддерживаются локальные пути, включая смешанные разделители Discord, и ограниченная загрузка аватарок с CDN Discord. Готовые картинки сохраняются в памяти; при отсутствии изображения остаётся значок приложения.

Переход использует зарегистрированный обработчик уведомлений Windows с исходными аргументами или поддерживаемую ссылку самого приложения. Запасной вариант — зарегистрированный ярлык отправителя. Некоторые приложения не передают адрес назначения: открытие конкретного чата зависит от их обработчика. Текст сообщений не выполняется как команда. Быстрые действия и ответы не воспроизводятся.

### Настройка шапки

В **Ещё → Настроить шапку…** можно добавить вкладку, явно заменить другую или вернуть закреплённую вкладку в «Ещё». До пяти вкладок помимо главной показываются полные подписи; при большем количестве — круглые кнопки с иконками и подсказками. Не поместившиеся вкладки остаются в «Ещё». Тянущийся край декоративный: мышь перехватывают только исходное тело острова и прикреплённый персонаж.

### Яндекс Музыка: Моя волна

Открой **Ещё → Моя волна → Подключить Яндекс Музыку** или **Настройки → Интеграции → Яндекс Музыка**, скопируй код и подтверди вход на [ya.ru/device](https://ya.ru/device). Остров не запрашивает пароль. Токен шифруется для текущего пользователя Windows; **Выйти** останавливает музыку и удаляет сохранённый токен.

Главная карточка и «Моя волна» используют общий плеер, перемотку и палитру трека. Цвета фона, мини-плеера, акцента и текста приходят из Яндекса; недостающие вычисляются из обложки. Цвета сменяются плавно с сохранением читаемости. «Моя волна · Настроить» на главной и «К плееру» в волне переключают вкладки без перезапуска музыки.

Запусти свою волну и стрелками выбери занятие, характер, настроение и язык. Варианты загружаются из Яндекса. Новые настройки применяются после текущего трека; кнопка **Следующий** переключает сразу. Музыка играет и в свёрнутом острове: работают цвета обложки, прогресс, управление на главной и у персонажа. **Остановить** возвращает управление другим плеерам. Запуск волны вместе с островом включается отдельно.

Используется неофициальный API Яндекс Музыки и встроенный аудиоплеер Windows (MP3). Нужны подходящая подписка и доступность сервиса в регионе. Связь с API изучена по [yamusic-md3](https://github.com/yaskorinov/yamusic-md3), интерфейс и обработчики написаны для Night Island. Пользователь подтвердил работу персональной волны после входа; автоматические проверки покрывают получение кода, запросы API, данные очереди, воспроизведение Windows и отрисовку вкладки.

### Нижний док

Отдельная нижняя панель работает внутри процесса Island на общем графическом устройстве. Включается в **Настройки → Нижний док**, после закрытия другого дока. Поддерживает закреплённые приложения и файлы, индикаторы запущенных окон, запуск/возврат/сворачивание по клику, список окон по ПКМ, перестановку значков и добавление перетаскиванием. Если приложения не помещаются, список прокручивается колесом.

В настройках: размер, увеличение при наведении, системное размытие фона, оттенок/насыщенность/яркость панели, непрозрачность, отступ, монитор, автоскрытие и анимации 30/60 кадров/с. Размытая и обычная панели запоминают непрозрачность отдельно. Если системное размытие недоступно, сохраняется выбранный цвет панели. Иконки загружаются в фоне под масштаб Windows и увеличение, сохраняют пропорции и прозрачность, а готовые изображения переиспользуются на GPU. Открытые настройки Island видны в списке запущенных приложений; служебные окна туда не попадают.

Меню ПКМ показывает состояние приложения и окон: можно выбрать окно, восстановить/свернуть/закрыть его обычным способом, перейти к файлу, открыть свойства, скопировать путь, закрепить значок и настроить поведение дока. Длинные меню прокручиваются внутри монитора и поддерживают клавиатуру.

Полноэкранные приложения и игры без рамки скрывают док на своём мониторе; обычное развёрнутое окно — нет. Для игры в окне: **Поведение дока → Скрывать док в этом приложении**. Правило удаляется в настройках дока. Скрытый док не перехватывает мышь, не рисует анимации и не обновляет значки. После Alt+Tab возвращается без захвата фокуса. Проверки выполнены на нативных тестовых окнах, а не на всех играх и античитах. В покое отрисовка останавливается; учитывается отключение анимаций Windows. При выключении освобождаются окно и таймеры дока. По отдельной кнопке открываются параметры панели задач Windows для включения её автоскрытия; Island сам не скрывает панель задач и не заменяет Explorer.

Кнопка **Импорт приложений из MyDockFinder** читает `ico.ini` из стандартной папки Steam или предлагает выбрать файл. Приложения добавляются в исходном порядке без дубликатов; значки берутся из Windows. Бинарники, темы и наборы иконок MyDockFinder не копируются. Счётчики уведомлений и эффекты сворачивания чужих окон пока не реализованы. Верхнюю панель пока заменяет сам Island.

Задержи курсор над запущенным приложением: появится живой предпросмотр Windows, переключение между его окнами, открытие/закрытие и доступные быстрые действия. Музыкальные кнопки управляют выбранным плеером именно этого приложения, даже если Island показывает другой источник. Громкость и выключение звука относятся только к приложению. Для микрофона и звука Discord нужна подключённая интеграция с подтверждённым доступом к голосовым настройкам именно этого клиента Discord. Недоступные действия отключены; без подключения карточка предлагает перейти в настройки. Карточки используют общую панель, тему и скругления Island. У Discord сверху показаны голосовой канал, участники и круглые кнопки микрофона/наушников; ниже — компактное окно и ползунок громкости в стиле главной. Наведение не активирует и не разворачивает окно. Для свёрнутого окна Windows может показывать последний кадр; для защищённого или недоступного — заглушку. Escape или уход курсора закрывает карточку. Предпросмотр при наведении отключается в настройках дока; пункт **Предпросмотр и управление** остаётся в меню ПКМ.

Контур панели сохраняет сглаженные края при дробном масштабе Windows. После переключения и сворачивания окон фон обновляется один раз с небольшой задержкой, без постоянного захвата экрана.

### Анимации интерфейса

На главной плавно подсвечиваются плитки действий и кнопки плеера; нажатие слегка сжимает кнопку. В шапке плавно меняется выделение, а в настройках двигаются бегунок переключателя и выделение сегментов. Повторное действие продолжает переход из текущего состояния; после завершения таймер останавливается.

При открытии главной из музыкального острова обложка переходит в карточку плеера и возвращается при закрытии. При смене трека или источника обложка и оттенок карточки плавно меняются, а старая подпись уступает место новой без наложения текста. Быстрые переключения сохраняют текущую смесь изображений. Новые эффекты учитывают настройку анимаций Windows.

### Питомцы на рабочем столе

Вытяни персонажа из свёрнутого острова, чтобы выпустить его; перетащи обратно на остров, чтобы вернуть. Питомцы дышат, плавно следят за курсором, моргают, реагируют на поглаживание курсором, тянутся при переносе и мягко успокаиваются после отпускания. В списке появился медведь. Управление музыкой и мини-чат с ИИ доступны рядом с любым отдельным персонажем, включая собственного.

В **Настройки → Остров → Анимация питомца** доступны режимы без движения, экономный (до 20 FPS), сбалансированный (до 30 FPS) и плавный (до 60 FPS). Частота питомца настраивается отдельно от острова, снижается в покое и во сне. В игровом режиме отрисовка останавливается.

**Самостоятельные прогулки, отдых и сон** включают короткие прогулки на той высоте, где ты оставил отдельного питомца, отдых сидя и сон. Питомец остаётся в рабочей области текущего монитора. При приближении курсора, перетаскивании, работе с плеером или открытом ИИ-чате он останавливается и просыпается; после взаимодействия ждёт несколько секунд. Отключение самостоятельности сохраняет ручное перетаскивание и реакции на курсор. Режим «Без движения» также отключает прогулки. Перемещение не забирает фокус и не записывает настройки на каждом кадре.

Нажми **Питомец** в основном или мини-чате с ИИ. Опиши внешность или выбери **Фото** и отправь запрос настроенной модели. Получится стилизованный анимированный персонаж с выбранными цветами, формой, узором и аксессуарами. Проверь предпросмотр и нажми **Применить**; следующим сообщением можно изменить результат. Для фотографий нужна модель с поддержкой изображений. Кнопка **Без фото** исключает фотографию из запроса: вместо неё опиши внешность словами. Выбранный питомец сохраняется после перезапуска; результат модели не может содержать код или внешние ресурсы. В редакторе видны живой портрет, миниатюра выбранного фото и палитра; состояния предпросмотра, применения и ошибки различаются. Анимация следует настройке качества и останавливается в скрытом окне, а в мини-чате — также без фокуса. Скруглённые силуэты, мягкие тени и выразительные глаза сохраняются и в острове, и у отдельного питомца.

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
