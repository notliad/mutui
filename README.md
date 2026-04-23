
<img width="1536" height="1024" alt="mutui" src="https://github.com/user-attachments/assets/b6034406-73ff-4606-bca8-e3bb6393b6f0" />


[![GitHub Stars](https://img.shields.io/github/stars/notliad/mutui?style=flat&color=FFD700&logo=starship&logoColor=white)](https://github.com/notliad/mutui/stargazers)
[![GitHub Forks](https://img.shields.io/github/forks/notliad/mutui?style=flat&color=0891b2&logo=github&logoColor=white)](https://github.com/notliad/mutui/network)
[![GitHub License](https://img.shields.io/github/license/notliad/mutui?style=flat&color=22c55e)](https://github.com/notliad/mutui/blob/main/LICENSE)
![Visitors](https://api.visitorbadge.io/api/visitors?path=https%3A%2F%2Fgithub.com%2Fnotliad%2Fmutui&label=visitors&countColor=%230c7ebe&style=flat&labelStyle=none)
![release](https://img.shields.io/github/v/release/notliad/mutui) ![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)

`mutui` is a lightweight terminal based music player for Linux.

Features:

- Search tracks and playlists on Youtube;
- Local music library folders (filter by artists, albums and tracks);
- Local queue and playlists;
- Background playback;
- Vim like navigation;

## Quick Install

```bash
git clone https://github.com/notliad/mutui.git
chmod +x scripts/install.sh
./scripts/install.sh
```

This build and installs binaries and dependencies.

### Arch Linux (AUR) 

You can install **lo** directly from the AUR using an AUR helper like `yay` or `paru`:

```bash
yay -S mutui
```

or

```bash
paru -S mutui
```

If you want tray feature:

```bash
chmod +x scripts/install.sh --with-tray
./scripts/install.sh
```

## Manual build

## Requirements

- `yt-dlp`

```bash
# Arch Linux
sudo pacman -S yt-dlp

# Ubuntu/Debian
sudo apt install yt-dlp

# Fedora
sudo dnf install yt-dlp
```

- Rust toolchain (`cargo`) to build

```bash
curl https://sh.rustup.rs -sSf | sh

source $HOME/.cargo/env
```

- Then build

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

## Tech notes

- `mutui` starts `mutuid` automatically if needed.
- Pulse loopback routing is disabled by default to avoid startup audio glitches. Enable it with `MUTUI_ENABLE_AUDIO_ROUTING=1 mutui` if you need the custom sink path.

## License

MIT (`LICENSE`).

This project dynamically links to [libmpv](https://mpv.io/) via the [libmpv2](https://github.com/kohsine/libmpv-rs) crate, which is licensed under the GNU LGPL-2.1. See `THIRD-PARTY-LICENSES.txt` and `LICENSE.LGPL-2.1` for details and obligations regarding LGPL components.
