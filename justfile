# Build
build:
    cargo build --release

# Install for current user
install: build
    install -Dm755 target/release/cosmic-kde-menu ~/.local/bin/cosmic-kde-menu
    install -Dm644 resources/com.github.cosmic-kde-menu.desktop ~/.local/share/applications/com.github.cosmic-kde-menu.desktop

# Uninstall
uninstall:
    rm -f ~/.local/bin/cosmic-kde-menu
    rm -f ~/.local/share/applications/com.github.cosmic-kde-menu.desktop

# Run (for debugging — needs COSMIC panel restart to pick up)
run:
    cargo run

# Check formatting
fmt:
    cargo fmt

# Lint
lint:
    cargo clippy
