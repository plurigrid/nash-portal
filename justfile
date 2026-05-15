default:
    @just --list --unsorted

# run the TUI (debug build, fastest feedback)
nash:
    cargo run -p nash-portal-tui

# release build + run the TUI
nash-release:
    cargo run --release -p nash-portal-tui

# serve the ratzilla web build at http://127.0.0.1:8080
web:
    cd web && trunk serve --open

# cargo check both packages
check:
    cargo check -p nash-portal-tui
    cargo check -p nash-portal-web --target wasm32-unknown-unknown

# release build both
build:
    cargo build --release -p nash-portal-tui
    cd web && trunk build --release
