# Contributing to mutui

Thanks for contributing to `mutui (MusicUniversalTUI)`.

## Development Setup

1. Install Rust stable.
2. Install system dependencies: `mpv` and `yt-dlp`.
3. Build:

```bash
cargo build
```

Platform-specific dependency helpers:

```bash
# Ubuntu/Debian
sudo apt install mpv yt-dlp libmpv-dev pkg-config

# Fedora
sudo dnf install mpv yt-dlp mpv-libs pkgconf-pkg-config

# macOS (Homebrew)
brew install rust mpv yt-dlp pkg-config
```

4. Run TUI:

```bash
cargo run -p mutui-tui
```

## Guidelines

- Keep changes focused and small.
- Prefer keyboard-first UX.
- Keep UI simple and low-noise.
- Preserve lightweight runtime behavior.

## Before Opening a PR

- Run:

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo build
```

- Describe what changed and why.
- Include screenshots or terminal recordings for UI changes when possible.

## Commit Style (recommended)

Use clear, concise messages, for example:

- `feat(tui): add playlist delete confirmation`
- `fix(daemon): stop mpv on shutdown`
- `docs(readme): update keybindings`
