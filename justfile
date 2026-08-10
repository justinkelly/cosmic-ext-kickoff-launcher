# Build
build:
    cargo build --release

# Install for current user
install: build
    install -Dm755 target/release/cosmic-kde-launcher ~/.local/bin/cosmic-kde-launcher
    install -Dm644 resources/com.github.cosmic-kde-launcher.desktop ~/.local/share/applications/com.github.cosmic-kde-launcher.desktop

# Uninstall
uninstall:
    rm -f ~/.local/bin/cosmic-kde-launcher
    rm -f ~/.local/share/applications/com.github.cosmic-kde-launcher.desktop

# Run (for debugging — needs COSMIC panel restart to pick up)
run:
    cargo run

# Check formatting
fmt:
    cargo fmt

# Lint
lint:
    cargo clippy
