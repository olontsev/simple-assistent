# Simple Assistant

Менеджер **llama.cpp** (`llama-server`) в системном трее на Tauri + React + TypeScript.

## Запуск

```bash
npm install
npm run tauri dev
```

При старте окно скрыто — приложение живёт в трее. Левый клик по иконке или пункт «Настройки» открывает окно.

## Возможности

- Запуск / остановка `llama-server`
- Загрузка / выгрузка модели (через перезапуск процесса)
- Выбор модели (рекурсивный скан `.gguf`) и профиля в меню трея
- Настройки: пути, автозапуск с Windows, редактор профилей (строка аргументов)
- Иконка трея отражает статус сервера (серый / жёлтый / зелёный / красный)

Конфиг: `%APPDATA%\com.ryuky.simple-assistant\settings.json`  
Лог сервера: `%APPDATA%\com.ryuky.simple-assistant\llama-server.log`
