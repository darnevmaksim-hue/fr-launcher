# FR-Launcher (Rust + Tauri)

Minecraft-лаунчер FR с родным интерфейсом и Rust-ядром (Tauri 2).

## Что это

- `ui/` — наш интерфейс (HTML/CSS/JS) из Electron-версии FR-Launcher.
- `src-tauri/` — Rust-ядро. За основу взято ядро HorizonUI-Launcher
  (https://github.com/nokarin-dev/HorizonUI-Launcher, GPL-3.0-or-later):
  загрузка Minecraft, библиотек и ассетов, установка Fabric/Forge/NeoForge,
  запуск игры, Modrinth.
- `ui/tauri-shim.js` — мост: `window.API` (контракт Electron-preload)
  → команда `api(method, args)` в Rust.

## Данные

- `%USERPROFILE%\.fr-launcher` — инстансы, настройки, логи, кэш, статистика.
- `%APPDATA%\.minecraft` — общие клиент, библиотеки и ассеты
  (не качаем 7 ГБ заново).

## Сборка

```powershell
cargo build --release --manifest-path src-tauri/Cargo.toml
```

Готовый файл: `src-tauri/target/release/fr_launcher.exe`.

## Лицензия

GPL-3.0-or-later (см. `LICENSE` и `NOTICE`).
