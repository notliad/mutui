# mutui
`mutui` is a lightweight terminal based music player for Linux and macOS.

![mutui](https://github.com/user-attachments/assets/e3ebdd86-3bad-42f4-a6a4-04a876d434ed)

Features:
- Search tracks and playlists on Youtube;
- Local music library folders (supports Artists & Albums);
- Queue and local playlists;
- Background playback;
- Vim like navigation;

## Requirements

- Rust toolchain (`cargo`) to build
- `yt-dlp`
- `mpv` runtime + `libmpv` development files

### Linux dependencies

```bash
# Arch Linux
sudo pacman -S mpv yt-dlp

# Ubuntu/Debian
sudo apt install mpv yt-dlp libmpv-dev pkg-config

# Fedora
sudo dnf install mpv yt-dlp mpv-libs pkgconf-pkg-config
```

### macOS dependencies

```bash
brew install rust mpv yt-dlp pkg-config
```

## Quick Start

Linux desktop install

```bash
chmod +x scripts/install-desktop-entry.sh
./scripts/install-desktop-entry.sh
```

This installs binaries and a desktop entry in your user environment.

macOS build and run:

```bash
cargo build --release
./target/release/mutui
```

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
- Pulse loopback routing (`MUTUI_ENABLE_AUDIO_ROUTING=1`) is Linux-only and disabled by default.
- MPRIS integration is available on Linux desktop sessions.

## License

MIT (`LICENSE`).

This project dynamically links to [libmpv](https://mpv.io/) via the [libmpv2](https://github.com/kohsine/libmpv-rs) crate, which is licensed under the GNU LGPL-2.1. See `THIRD-PARTY-LICENSES.txt` and `LICENSE.LGPL-2.1` for details and obligations regarding LGPL components.
