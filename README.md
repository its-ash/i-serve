# i-serve

A modern desktop app (Rust + egui) that turns any local folder into a file server with a clean web UI.

Pick a folder, start the server, and share files on your local network.

## Features

- Native desktop app built in Rust (`eframe/egui`)
- Folder picker to choose what to share
- Start/Stop server controls
- Shows local access URLs (LAN IP + port)
- Modern Bootstrap-based web file browser
- Search/filter files in browser
- Folder navigation in browser
- Download button for files
- Smart large-file handling:
  - Files larger than **1 GB** are streamed (memory-efficient)
  - Smaller files are served normally
- Custom branding with `logo.png` as:
  - Desktop app icon
  - Web app logo
  - Web favicon

## Tech Stack

- Rust
- `eframe/egui` (desktop UI)
- `axum` + `tokio` (web server)
- `rfd` (native folder picker)

## Prerequisites

- Rust (stable) + Cargo

Install Rust if needed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then restart terminal and verify:

```bash
rustc --version
cargo --version
```

## Quick Start

```bash
git clone <your-repo-url>
cd i-serve
cargo run
```

## How to Use

1. Open the desktop app.
2. Click **Pick Folder** and choose the directory you want to share.
3. Keep the default port (`8080`) or set your own.
4. Click **Start Server**.
5. Open any shown URL in a browser, for example:
   - `http://127.0.0.1:8080/`
   - `http://<your-local-ip>:8080/`
6. Use the web UI to filter files and download them.

When running, app height expands to show access URLs.

## Build (Release)

```bash
cargo build --release
```

Binary location:

```text
target/release/i-serve
```

## Project Structure

```text
.
├── Cargo.toml
├── src/
│   └── main.rs
└── logo.png
```

## Troubleshooting

### Port already in use

If start fails with a bind/port error, change the port in the app (for example, `8081`) and start again.

### Firewall or network access issues

- Ensure your firewall allows incoming connections to the selected port.
- Make sure both devices are on the same local network.

### Large file downloads

Files over 1 GB are streamed to avoid loading the full file into memory.

## Security Notes

- This tool is designed for trusted local-network sharing.
- Share only folders you trust.
- If exposing beyond your LAN, add authentication/reverse proxy protections first.

## License

Add your preferred license (for example, MIT).
