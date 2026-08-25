Simple manager **llama.cpp** (`llama-server`) in the system tray on Tauri + React + TypeScript.

## Launch

```
npm install
npm run tauri dev
```

## Build

```
npm run tauri build
```

At startup, the window is hidden - the application lives in the tray. Left click on the icon or the “Settings” item opens a window.

## Features

- Fast Start/Stop llama-server
- Loading/unloading model
- Selecting a model (recursive scan .gguf) and profile in the tray menu
- Settings: paths, autorun with Windows, profile editor (argument string)
- The tray icon reflects the server status (gray / yellow / green / red)

