# mutui (MusicUniversalTUI)

`mutui` is an open source YouTube music player for the terminal.

The project exists for one simple reason: a lightweight player that does not consume close to 1GB of RAM, while staying fast and straightforward.

## Why This Project

- Minimal resource usage compared to heavyweight desktop players.
- Keyboard-first UX with no unnecessary UI noise.
- Background playback with a daemon model.
- Easy to hack and extend in Rust.

## Features

- Search tracks via `yt-dlp`.
- Queue management (play selected, remove, reorder).
- Local Playlist save/load/delete.
- Background playback (`mutuid` keeps playing after closing TUI).
- Responsive TUI layout for small and large terminals.

## Architecture

```text
mutui (TUI) <-> Unix Socket <-> mutuid (daemon) <-> mpv (JSON IPC)
                                              \
                                               -> yt-dlp (search + stream URL)
```

Crates:

- `crates/mutui-tui`: Terminal UI client.
- `crates/mutui-daemon`: Background playback service.
- `crates/mutui-common`: Shared IPC types and helpers.

## System Dependencies

```bash
# Arch Linux
sudo pacman -S mpv yt-dlp

# Ubuntu/Debian
sudo apt install mpv yt-dlp

# Fedora
sudo dnf install mpv yt-dlp
```

## Build

```bash
cargo build --release
```

Binary outputs:

- `target/release/mutui`
- `target/release/mutuid`

## Run

```bash
./target/release/mutui
```

Behavior:

- If daemon is not running, TUI starts it automatically.
- `q`: close TUI and keep music playing.
- `Q`: shutdown daemon and stop playback.

## App Drawer (Linux)

To make `mutui` appear in the app drawer, install a desktop entry:

```bash
chmod +x scripts/install-desktop-entry.sh
./scripts/install-desktop-entry.sh
```

What this does:

- Builds `mutui` in release mode.
- Copies binary to `~/.local/bin/mutui`.
- Installs `packaging/mutui.desktop` to `~/.local/share/applications/mutui.desktop`.

If your menu does not refresh immediately, log out/login once.

## Keybindings (Summary)

- Global: `Space`, `n/p`, `Left/Right`, `+/-`, `Tab`, `q`, `Q`, `?`
- Search: `/`, `Enter`, `a`, `j/k`
- Queue: `J/K`, `T`, `D`, `H/L`
- Playlists: `Enter`, `l`, `d`, `s`

## Data Paths

- Playlists: `~/.local/share/mutui/playlists/*.json`

## Project Status

Active personal project, now published as a public open source repository.

Issues and PRs are welcome.

## License

MIT. See `LICENSE`.
