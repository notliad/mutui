# mutui
`mutui` is a lightweight terminal based music player for Linux.

![mutui](https://github.com/user-attachments/assets/e3ebdd86-3bad-42f4-a6a4-04a876d434ed)

Features:
- Search tracks and playlists on Youtube;
- Local music library folders (filter by artists, albums and tracks);
- Local queue and playlists;
- Background playback;
- Vim like navigation;

## Requirements

- `yt-dlp`
- Rust toolchain (`cargo`) to build

Install deps:
### Linux dependencies (manual build)

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

This build and installs binaries and add an desktop entry in your user environment.

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
- `Enter`: run one search that returns tracks (top) and playlists (bottom)
- `Ctrl+J` / `Ctrl+K`: jump between track and playlist sections
- `j` / `k`: navigate inside current section or jump to next/previous section
- `Enter`/`->`/`l`: play selected track, or open/close selected playlist as a folder
- `<-`/`h`: close selected opened playlist folder
- `a`: add selected track result to queue, or add all tracks from selected playlist

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
