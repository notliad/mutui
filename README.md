# mutui

`mutui` is a lightweight terminal music player for Linux.

It supports:
- YouTube search and playback (`yt-dlp` + `mpv`)
- Local music library folders
- Queue and playlists
- Background daemon playback

![mutui](https://github.com/user-attachments/assets/8193afc3-eca8-4e4e-8760-848498b8dec8)

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
```

## Quick Start

```bash
cargo build --release
./target/release/mutui
```

Notes:
- `mutui` starts `mutuid` automatically if needed.
- `q` closes only the TUI (music keeps playing).
- `Q` shuts down daemon and stops playback.

## Optional: Desktop Install

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
    - Local track -> default local player (`xdg-open`)

## Data Location

- Library config: `~/.local/share/mutui/library.json`
- Playlists: `~/.local/share/mutui/playlists/*.json`

## License

MIT (`LICENSE`).
