# open-with

A modern command-line file opener for Linux that respects XDG MIME associations and provides an interactive fuzzy-finder interface for selecting applications.

## Features

- **XDG Compliance**: Respects system MIME type associations and desktop entries
- **Interactive Selection**: Choose applications using fzf or fuzzel
- **Desktop Actions**: Support for application-specific actions (edit, print, etc.)
- **Caching**: Fast desktop file parsing with intelligent caching
- **JSON Output**: Machine-readable output for integration with other tools
- **Build Information**: Detailed build and version information with git commit tracking

## Installation

### From Source

```bash
git clone https://github.com/CaddyGlow/open-with.git
cd open-with
cargo install --path .
```

### From crates.io

```bash
cargo install open-with
```

## Usage

### Basic Usage

```bash
# Open a file with interactive application selection
open-with document.pdf

# Output available applications as JSON
open-with document.pdf --json

# Show desktop actions as separate entries
open-with image.png --actions

# Use a specific fuzzy finder
open-with file.txt --fuzzer fzf
```

### Options

```
Usage: open-with [OPTIONS] [FILE]

Arguments:
  [FILE]  File to open (not required when using --build-info)

Options:
      --fuzzer <FUZZER>    Fuzzy finder to use [default: auto] [possible values: fzf, fuzzel, auto]
  -j, --json               Output JSON instead of interactive mode
  -a, --actions            Show desktop actions as separate entries
      --clear-cache        Clear the desktop file cache
  -v, --verbose            Verbose output
      --build-info         Show build information
  -h, --help               Print help
  -V, --version            Print version
```

### Examples

#### Interactive Mode
```bash
open-with document.pdf
```
Opens an interactive fuzzy finder showing all applications that can handle PDF files.

#### JSON Output
```bash
open-with document.pdf --json | jq .
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
open-with image.png --actions
```
Shows both the main application entries and their available actions (edit, print, etc.).

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

- **Cache**: `~/.cache/open-with/desktop_cache.json`
- **Config**: Reads from standard XDG config directories
- **Data**: Reads from standard XDG data directories

### Environment Variables

- `XDG_DATA_HOME`: User data directory (default: `~/.local/share`)
- `XDG_CONFIG_HOME`: User config directory (default: `~/.config`)
- `XDG_DATA_DIRS`: System data directories (default: `/usr/local/share:/usr/share`)
- `XDG_CONFIG_DIRS`: System config directories (default: `/etc/xdg`)
- `XDG_CURRENT_DESKTOP`: Current desktop environment

## Building

### Prerequisites

- Rust 1.70.0 or later
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
- **desktop_parser**: Desktop entry file parsing
- **mime_associations**: XDG MIME type association handling  
- **xdg**: XDG Base Directory specification utilities

### Caching Strategy

Desktop files are parsed once and cached to `~/.cache/open-with/desktop_cache.json` for improved performance. The cache is automatically rebuilt when:

- Cache file doesn't exist
- Cache file is corrupted
- `--clear-cache` flag is used

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
cargo clippy -- -D warnings
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
