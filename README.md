```
                  _         _ 
                 | |       (_)
  _ __ ___  _   _| |_ _   _ _ 
 | '_ ` _ \| | | | __| | | | |
 | | | | | | |_| | |_| |_| | |
 |_| |_| |_|\__,_|\__|\__,_|_|
                              
```

`mutui` is a lightweight terminal music player for Linux, macOS, and Windows.

https://github.com/user-attachments/assets/5ba80a81-db08-4168-9a7a-47bb53afb577

It supports:
- YouTube search and playback (`yt-dlp` + `mpv`)
- Local music library folders (supports Artists & Albums)
- Queue and playlists
- Background playback

## Requirements

- `mpv`
- `yt-dlp`
- Rust toolchain (`cargo`) to build

Install deps:

```bash
# Arch Linux
sudo pacman -S mpv yt-dlp

# Ubuntu/Debian
sudo apt install mpv yt-dlp

# Fedora
sudo dnf install mpv yt-dlp

# macOS (Homebrew)
brew install mpv yt-dlp

# Windows (winget)
winget install --id mpv.net -e
winget install --id yt-dlp.yt-dlp -e
```

## Quick Start

```bash
cargo build --release
./target/release/mutui
```

Windows PowerShell:

```powershell
cargo build --release
.\target\release\mutui.exe
```

Notes:
- `mutui` starts `mutuid` automatically if needed.
- `q` closes only the TUI (music keeps playing).
- `Q` shuts down daemon and stops playback.
- Pulse loopback routing is disabled by default to avoid startup audio glitches. Enable it with `MUTUI_ENABLE_AUDIO_ROUTING=1 mutui` if you need the custom sink path.
- Tray integration (`mutui-tray`) is currently Linux-only. The TUI and daemon run on all supported platforms.

## Optional: Desktop Install

Linux only.

```bash
chmod +x scripts/install-desktop-entry.sh
./scripts/install-desktop-entry.sh
```

This installs binaries and a desktop entry in your user environment.

## Basic Usage

- `Tab`: switch tabs (`Search`, `Playlists`, `Library`)
- `?`: show full shortcuts help

Search tab:
- `/`: type query
- `Enter`: play selected result
- `a`: add selected result to queue

Library tab:
- `f`: add folder to local library (use absolute path, ex: `/home/user/Music`)
- `r`: rescan library
- `Enter`: play selected local track
- `a`: add selected local track to queue

Global:
- `Space`: play/pause
- `n` / `p`: next/previous
- `o`: open current track externally
    - YouTube track -> browser
    - Local track -> default system opener (`xdg-open`/`open`/`start`)

## Data Location

Mutui uses the system app-data directory for each platform:
- Linux: `~/.local/share/mutui/`
- macOS: `~/Library/Application Support/org.mutui.mutui/`
- Windows: `%LOCALAPPDATA%\\mutui\\mutui\\data\\`

Files:
- Library config: `<data-dir>/library.json`
- Playlists: `<data-dir>/playlists/*.json`

## License

MIT (`LICENSE`).
