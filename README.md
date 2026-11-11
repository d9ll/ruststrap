# Ruststrap

**Ruststrap** — это нативный бутстраппер Roblox для Windows, написанный на Rust. Проект повторяет основное поведение [Bloxstrap](https://github.com/pizzaboxer/bloxstrap): установку и обновление клиента Roblox, запуск Player и Studio, управление fast flags и модификациями, регистрацию протоколов `roblox://`, а также Discord Rich Presence.

Интерфейс приложения реализован на `egui`/`eframe`, работает без .NET и WebView2.

## Возможности

- Установка Ruststrap в выбранную папку (по умолчанию `%LOCALAPPDATA%\Ruststrap`).
- Автоматическая загрузка актуальной версии Roblox через официальный deployment API.
- Проверка и восстановление пакетов по MD5, кэширование загрузок.
- Запуск Roblox Player, Roblox Studio и Studio с авторизацией.
- Регистрация протоколов `roblox`, `roblox-player`, `roblox-studio`, `roblox-studio-auth`, `Roblox.Place`, `.rbxl`, `.rbxlx`.
- Менеджер fast flags (`Modifications\ClientSettings\ClientAppSettings.json`) с пресетами MSAA, качества текстур и полноэкранного режима.
- Поддержка модификаций: файлы из `Modifications` копируются в папку версии Roblox при каждом запуске, удалённые модификации восстанавливаются из оригинальных пакетов.
- Кастомный шрифт интерфейса Roblox.
- Discord Rich Presence с названием игры, создателем, иконкой и кнопкой «View on Roblox».
- Определение текущей игры по логам Roblox.
- Создание ярлыков на рабочем столе и в меню «Пуск».
- Удаление Ruststrap с возможностью сохранить настройки и модификации.

## Требования

- Windows 10 или Windows 11 (x64).
- [Rust](https://rustup.rs/) с toolchain `stable-x86_64-pc-windows-msvc`.
- Visual Studio 2022 (Desktop development with C++) либо Build Tools 2022 — для компоновки MSVC.

## Сборка

```powershell
cd C:\Users\Глеб\Desktop\ruststrap
cargo build --release
```

Готовый файл: `target\release\ruststrap.exe`.

Для отладочной сборки:

```powershell
cargo build
.\target\debug\ruststrap.exe
```

## Запуск и использование

Запустите `ruststrap.exe` без аргументов — откроется главное окно. Если Ruststrap ещё не установлен, приложение предложит выбрать папку установки.

После установки Ruststrap перехватывает ссылки Roblox: запуск игры с сайта Roblox будет открывать Ruststrap, который проверит обновления и запустит клиент.

### Аргументы командной строки

| Аргумент | Описание |
| --- | --- |
| `-menu`, `-settings`, `-preferences` | Открыть настройки |
| `-player "<uri>"` | Запустить Roblox Player (поддерживается `roblox://` и `roblox-player://`) |
| `-studio "<uri или путь>"` | Запустить Roblox Studio или открыть `.rbxl`/`.rbxlx` |
| `-quiet` | Работать без графического интерфейса, показывать только ошибки |
| `-uninstall` | Открыть окно удаления |
| `-uninstall -quiet` | Удалить Ruststrap без подтверждения (данные сохраняются) |
| `-version <version-...>` | Запустить конкретную версию Roblox |
| `-channel <name>` | Использовать указанный канал Roblox |
| `-force` | Принудительно переустановить клиент |
| `-nolaunch` | Только установить/обновить, не запускать Roblox |
| `-watcher <base64>` | Служебный режим: слежение за процессом Roblox и Rich Presence |
| `-backgroundupdater Player\|Studio` | Фоновое обновление клиента |

## Каталоги

По умолчанию всё хранится в `%LOCALAPPDATA%\Ruststrap`:

```
Ruststrap\
├── Ruststrap.exe
├── Settings.json          # настройки приложения
├── State.json             # состояние бутстраппера
├── PlayerState.json       # установленная версия Player
├── StudioState.json       # установленная версия Studio
├── Downloads\             # кэш пакетов Roblox
├── Versions\              # распакованные версии клиента
├── Logs\                  # журналы Ruststrap
└── Modifications\         # пользовательские модификации
    ├── ClientSettings\ClientAppSettings.json
    └── content\fonts\CustomFont.ttf
```

Файлы настроек используют формат JSON с именами свойств в PascalCase.

## Модификации и fast flags

- Fast flags редактируются на вкладке **Fast flags** или напрямую в `Modifications\ClientSettings\ClientAppSettings.json`.
- Любой файл, положенный в `Modifications` с сохранением относительного пути, заменяет соответствующий файл в папке версии Roblox. Например, `Modifications\content\sounds\...` заменяет `content\sounds\...`.
- Если удалить файл из `Modifications`, при следующем запуске оригинал будет восстановлен из пакета Roblox.
- Для замены шрифта положите файл в `Modifications\content\fonts\CustomFont.ttf`.

## Удаление

Удалить Ruststrap можно из окна приложения (**Settings → Advanced → Uninstall Ruststrap**), через «Параметры → Приложения → Ruststrap» в Windows или командой:

```powershell
ruststrap.exe -uninstall
```

При удалении можно сохранить настройки и модификации. Протоколы Roblox будут возвращены официальному клиенту, если он установлен.

## Ограничения

- Поддерживается только Windows.
- Нет автоматического самообновления Ruststrap (`-upgrade`).
- Фоновое обновление клиента не реализовано: обновление выполняется при запуске.
- Не реализованы пользовательские интеграции Bloxstrap и установка WebView2 Runtime.
- Интерфейс приложения на английском языке; журналы и сообщения об ошибках также на английском.

## Лицензия

MIT License.

Ruststrap не связан с корпорацией Roblox. Roblox является товарным знаком Roblox Corporation. Оригинальный проект Bloxstrap создан pizzaboxer и участниками сообщества.
