```
                  _         _ 
                 | |       (_)
  _ __ ___  _   _| |_ _   _ _ 
 | '_ ` _ \| | | | __| | | | |
 | | | | | | |_| | |_| |_| | |
 |_| |_| |_|\__,_|\__|\__,_|_|
                              
```

`mutui` is a lightweight TUI music player for Linux.

![ezgif-5c0563eb14a5c093](https://github.com/user-attachments/assets/6c67ee52-50e2-494e-8697-9a728143b1e1)

Features:
- YouTube search and playback;
- Local music library folders (supports Artists & Albums);
- Queue and playlists;
- Background playback;

## Requirements

- `yt-dlp`
- Rust toolchain (`cargo`) to build

Install deps:

```bash
# Arch Linux
sudo pacman -S yt-dlp

# Ubuntu/Debian
sudo apt install yt-dlp

# Fedora
sudo dnf install yt-dlp
```

## Quick Start

Desktop Install

```bash
chmod +x scripts/install-desktop-entry.sh
./scripts/install-desktop-entry.sh
```

This installs binaries and a desktop entry in your user environment.

or build yourself:

```bash
cargo build --release
./target/release/mutui
```

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

## Tech notes:
- `mutui` starts `mutuid` automatically if needed.
- Pulse loopback routing is disabled by default to avoid startup audio glitches. Enable it with `MUTUI_ENABLE_AUDIO_ROUTING=1 mutui` if you need the custom sink path.

## License

MIT (`LICENSE`).

This project dynamically links to [libmpv](https://mpv.io/) via the [libmpv2](https://github.com/kohsine/libmpv-rs) crate, which is licensed under the GNU LGPL-2.1. See `THIRD-PARTY-LICENSES.txt` and `LICENSE.LGPL-2.1` for details and obligations regarding LGPL components.
