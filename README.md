# openit

[![CI](https://github.com/CaddyGlow/openit/workflows/CI/badge.svg)](https://github.com/CaddyGlow/openit/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/CaddyGlow/openit/branch/main/graph/badge.svg)](https://codecov.io/gh/CaddyGlow/openit)
[![Crates.io](https://img.shields.io/crates/v/openit.svg)](https://crates.io/crates/openit)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A modern command-line file opener for Linux that respects XDG MIME associations and provides an interactive fuzzy-finder interface for selecting applications.

## Features

- **XDG Compliance**: Respects system MIME type associations and desktop entries
- **Interactive Selection**: Choose applications using fzf or fuzzel
- **Desktop Actions**: Support for application-specific actions (edit, print, etc.)
- **Caching**: Fast desktop file parsing with intelligent caching
- **JSON Output**: Machine-readable output for integration with other tools
- **Build Information**: Detailed build and version information with git commit tracking
- **Configurable Fuzzy Finders**: Customize fuzzy finder commands and arguments with template support

## Installation

### From Source

```bash
git clone https://github.com/CaddyGlow/openit.git
cd openit
cargo install --path .
```

### From crates.io

```bash
cargo install openit
```

## Usage

### Basic Usage

```bash
# Open a file with interactive application selection
openit document.pdf

# Output available applications as JSON
openit document.pdf --json

# Show desktop actions as separate entries
openit image.png --actions

# Use a specific fuzzy finder
openit file.txt --selector fzf
```

### Options

```
Usage: openit [OPTIONS] [FILE]

Arguments:
  [FILE]  File to open (not required when using --build-info or --clear-cache)

Options:
      --selector <SELECTOR>  Selector profile to use [default: auto] (profile name, e.g. auto, fzf, fuzzel, rofi)
  -j, --json                 Output JSON instead of interactive mode
  -a, --actions              Show desktop actions as separate entries
      --clear-cache          Clear the desktop file cache
  -v, --verbose              Verbose output
      --build-info           Show build information
      --generate-config      Generate default configuration file
      --config <CONFIG>      Path to configuration file
      --terminal-mode <TERMINAL_MODE>
                             Override how terminal applications launch (`current` for in-place, `launcher` for external emulator)
  -h, --help                 Print help
  -V, --version              Print version
```

### Examples

#### Interactive Mode
```bash
openit document.pdf
```
Opens an interactive fuzzy finder showing all applications that can handle PDF files.

#### JSON Output
```bash
openit document.pdf --json | jq .
```
```json
{
  "file": "/path/to/document.pdf",
  "mimetype": "application/pdf",
  "xdg_associations": ["evince.desktop", "firefox.desktop"],
  "applications": [
    {
      "name": "Document Viewer",
      "exec": "evince %U",
      "desktop_file": "/usr/share/applications/evince.desktop",
      "comment": "View multipage documents",
      "icon": "evince",
      "is_xdg": true,
      "xdg_priority": 0,
      "is_default": true,
      "action_id": null
    }
  ]
}
```

#### With Desktop Actions
```bash
openit image.png --actions
```
Shows both the main application entries and their available actions (edit, print, etc.).

#### Generate Configuration
```bash
openit --generate-config
```
Creates a default configuration file at `~/.config/openit/config.toml` with customizable fuzzy finder settings.

#### Generate Shell Completions
```bash
openit completions bash --output ~/.local/share/bash-completion/openit
```
Generates a completion script for the specified shell. Omitting `--output` prints the script to stdout. Dynamic completions are also available via `COMPLETE=<shell> openit` for shells that support clap's auto-completion protocol.

### Manage MIME Associations

`openit` now exposes subcommands to edit the user `mimeapps.list` directly:

```bash
# Set (overwrite) the default handler for a MIME type or extension
openit set text/plain helix.desktop

# Add a secondary handler without replacing the default entry
openit add text/plain code.desktop

# Remove a specific handler
openit remove text/plain code.desktop

# Remove all handlers for a MIME type
openit unset text/plain

# Inspect configured handlers
openit list
openit list --json | jq
```

File extensions are automatically converted to their corresponding MIME types (e.g., `openit set .md helix.desktop`).

## Dependencies

### Runtime Dependencies
- Linux system with XDG desktop environment
- One of the following fuzzy finders:
  - `fzf` (recommended)
  - `fuzzel`

### System Dependencies
The application reads standard XDG directories and files:
- Desktop entries from `/usr/share/applications/`, `~/.local/share/applications/`
- MIME associations from `~/.config/mimeapps.list`, `/etc/xdg/mimeapps.list`
- XDG data directories as specified by environment variables

## Configuration

The application follows XDG Base Directory specifications:

- **Cache**: `~/.cache/openit/desktop_cache.json`
- **Config**: `~/.config/openit/config.toml`
- **Data**: Reads from standard XDG data directories

### Configuration File

Generate a default configuration file:

```bash
openit --generate-config
```

This creates `~/.config/openit/config.toml` with the following structure:

```toml
open_with = true
term_exec_args = "-e"
expand_wildcards = false
terminal_execution = "launcher"
app_launch_prefix = null

[default]
gui = "fuzzel"
tui = "fzf"

[selectors.fzf]
command = "fzf"
args = [
    "--prompt", "{prompt}",
    "--height=40%",
    "--reverse",
    "--header={header}",
    "--cycle"
]
env = {}

[selectors.fuzzel]
command = "fuzzel"
args = [
    "--dmenu",
    "--prompt", "{prompt}",
    "--index",
    "--log-level=info"
]
env = {}

[selectors.rofi]
command = "rofi"
args = [
    "-dmenu",
    "-p", "{prompt}"
]
env = {}
```

The `[default]` table configures which selector profile is preferred in GUI and TUI environments. When `--selector auto` (the default) is used, `openit` chooses the GUI profile when launched from a graphical session and the TUI profile otherwise. Use `--selector <name>` on the CLI to force a specific profile defined under `[selectors.*]`.

`app_launch_prefix` lets you prepend another command before every launch (for example `"flatpak run"` or `"env WAYLAND_DISPLAY=..."`). Set it to an empty string or remove the key to disable the prefix.

### Template Variables

The configuration supports template variables in command arguments:

- `{prompt}`: Replaced with the file selection prompt (e.g., "Open 'file.txt' with: ")
- `{header}`: Replaced with the application type indicators ("★=Default ▶=XDG Associated  =Available")
- `{file}`: Replaced with the filename being opened

You can add modifiers to variables; for example `{file|truncate:20}` shortens the displayed file name to 20 characters and appends `...` when truncation occurs.

### Custom Fuzzy Finders

You can add custom fuzzy finder configurations:

```toml
[selectors.wofi]
command = "wofi"
args = [
    "-dmenu",
    "-p", "{prompt}",
    "-theme", "my-theme"
]
env = { "WOFI_THEME" = "custom.rasi" }
```

### Terminal Applications

If a desktop entry declares `Terminal=true`, `openit` automatically runs it inside a terminal emulator. Resolution happens in two steps:

1. Check for handlers of the virtual MIME type `x-scheme-handler/terminal`.
2. If none are registered, fall back to any desktop entry that advertises the `TerminalEmulator` category.

By default, the terminal command is invoked with `-e` to execute the target application. If your terminal expects different arguments you can adapt the behaviour in `~/.config/openit/config.toml` (or in `~/.config/handlr/handlr.toml` for handlr-compatibility) by updating `term_exec_args`:

```toml
# Terminals that need a different flag (replace `run` as required)
term_exec_args = "run"

# Terminals that already embed the command in the Exec line or do not
# require a launcher flag (e.g. WezTerm, kitty)
term_exec_args = ""
```

When `term_exec_args` is an empty string or the key is omitted, no additional arguments are added before the target command.

Set `terminal_execution = "current"` to run terminal applications inside the invoking shell by replacing the `openit` process via `exec`.
This is equivalent to launching `openit` with `--terminal-mode current`. Keep the value at `"launcher"` (or pass `--terminal-mode launcher`) to continue spawning a separate terminal emulator.

### Environment Variables

- `XDG_DATA_HOME`: User data directory (default: `~/.local/share`)
- `XDG_CONFIG_HOME`: User config directory (default: `~/.config`)
- `XDG_DATA_DIRS`: System data directories (default: `/usr/local/share:/usr/share`)
- `XDG_CONFIG_DIRS`: System config directories (default: `/etc/xdg`)
- `XDG_CURRENT_DESKTOP`: Current desktop environment

## Building

### Prerequisites

- Rust 1.80.0 or later
- System dependencies for development:
  ```bash
  sudo apt-get install desktop-file-utils shared-mime-info xdg-utils
  ```

### Build Commands

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- file.txt --verbose
```

## Architecture

The application consists of several modules:

- **cli**: Command-line argument parsing and build information
- **config**: Configuration file handling with template support
- **desktop_parser**: Desktop entry file parsing
- **mime_associations**: XDG MIME type association handling  
- **xdg**: XDG Base Directory specification utilities

### Caching Strategy

Desktop files are parsed once and cached to `~/.cache/openit/desktop_cache.json` for improved performance. The cache is automatically rebuilt when:

- Cache file doesn't exist
- Cache file is corrupted
- `--clear-cache` flag is used

### Fuzzy Finder Integration

The application supports multiple fuzzy finders with configurable commands and arguments:

- **Auto-detection**: Automatically detects available fuzzy finders (fzf, fuzzel)
- **Template support**: Command arguments support template variables for dynamic content
- **Environment variables**: Set custom environment variables for fuzzy finder execution
- **Icon support**: Fuzzel integration includes icon display for applications

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass: `cargo test`
6. Ensure code is formatted: `cargo fmt`
7. Ensure no clippy warnings: `cargo clippy`
8. Submit a pull request

### Code Style

This project uses standard Rust formatting and linting:

```bash
cargo fmt
cargo clippy
```

## Testing

Run the full test suite:

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test '*'

# Test with all features
cargo test --all-features

# Test documentation
cargo test --doc
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a list of changes in each version.

## Acknowledgments

- Built with [clap](https://github.com/clap-rs/clap) for command-line parsing
- Uses [serde](https://github.com/serde-rs/serde) for JSON serialization
- Follows [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html)
- Respects [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html)
