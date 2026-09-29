name := 'cosmic-ext-kickoff-launcher'
appid := 'com.github.cosmic-kickoff-launcher'
rootdir := ''
prefix := '/usr'

appdata := appid + '.metainfo.xml'
desktop := appid + '.desktop'

# Installation paths
base-dir := absolute_path(clean(rootdir / prefix))
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
appdata-dst := base-dir / 'share' / 'appdata' / appdata
bin-dst := base-dir / 'bin' / name
desktop-dst := base-dir / 'share' / 'applications' / desktop
icon-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg'
symbolic-icon-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '-symbolic.svg'
license-dst := base-dir / 'share' / 'doc' / name / 'LICENSE'
notices-dst := base-dir / 'share' / 'doc' / name / 'THIRD_PARTY_NOTICES.md'
lgpl-license-dst := base-dir / 'share' / 'doc' / name / 'LICENSES' / 'LGPL-3.0-or-later.txt'
kde-breeze-license-dst := base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE-Breeze-Icons-LGPL-3.0-or-later.txt'
kde-oxygen-license-dst := base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE-Oxygen-Icons-LGPL-3.0-or-later.txt'
kde-legacy-license-dst := base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE-Legacy-Icons-LGPL-2.1-or-later.txt'
kde2-kicker-license-dst := base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE2-Kicker-MIT.txt'
xfce-license-dst := base-dir / 'share' / 'doc' / name / 'LICENSES' / 'Xfce-Panel-GPL-2.0-or-later.txt'
haiku-license-dst := base-dir / 'share' / 'doc' / name / 'LICENSES' / 'Haiku-Icons-MIT.txt'
system76-approval-dst := base-dir / 'share' / 'doc' / name / 'LICENSES' / 'System76-COSMIC-APPROVAL-PENDING.md'

# Installation paths for the current user (no root required)
user-base-dir := absolute_path(env('HOME') / '.local')

# Default recipe which runs `just build-release`
default: build-release

# Runs `cargo clean`
clean:
    cargo clean

# Removes vendored dependencies
clean-vendor:
    rm -rf .cargo vendor vendor.tar

# `cargo clean` and removes vendored dependencies
clean-dist: clean clean-vendor

# Compiles with debug profile
build-debug *args:
    cargo build {{args}}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Compiles release profile with vendored dependencies
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Runs a clippy check
check *args:
    cargo clippy --all-features {{args}} -- -W clippy::pedantic

# Runs a clippy check with JSON message format
check-json: (check '--message-format=json')

# Run the application for testing purposes
run *args:
    env RUST_BACKTRACE=full cargo run --release {{args}}

# Run the application in a standalone window for testing
run-window *args:
    env RUST_BACKTRACE=full cargo run --release -- --window {{args}}

# Installs files
install:
    install -Dm0755 {{ cargo-target-dir / 'release' / name }} {{bin-dst}}
    install -Dm0644 {{ 'target' / 'xdgen' / 'app.desktop' }} {{desktop-dst}}
    install -Dm0644 {{ 'target' / 'xdgen' / 'app.metainfo.xml' }} {{appdata-dst}}
    install -Dm0644 resources/icon.svg {{icon-dst}}
    install -Dm0644 resources/icon-symbolic.svg {{symbolic-icon-dst}}
    install -Dm0644 LICENSE {{license-dst}}
    install -Dm0644 THIRD_PARTY_NOTICES.md {{notices-dst}}
    install -Dm0644 LICENSES/LGPL-3.0-or-later.txt {{lgpl-license-dst}}
    install -Dm0644 LICENSES/KDE-Breeze-Icons-LGPL-3.0-or-later.txt {{kde-breeze-license-dst}}
    install -Dm0644 LICENSES/KDE-Oxygen-Icons-LGPL-3.0-or-later.txt {{kde-oxygen-license-dst}}
    install -Dm0644 LICENSES/KDE-Legacy-Icons-LGPL-2.1-or-later.txt {{kde-legacy-license-dst}}
    install -Dm0644 LICENSES/KDE2-Kicker-MIT.txt {{kde2-kicker-license-dst}}
    install -Dm0644 LICENSES/Xfce-Panel-GPL-2.0-or-later.txt {{xfce-license-dst}}
    install -Dm0644 LICENSES/Haiku-Icons-MIT.txt {{haiku-license-dst}}
    install -Dm0644 LICENSES/System76-COSMIC-APPROVAL-PENDING.md {{system76-approval-dst}}

# Uninstalls installed files
uninstall:
    rm {{bin-dst}} {{desktop-dst}} {{appdata-dst}} {{icon-dst}} {{symbolic-icon-dst}} {{license-dst}} {{notices-dst}} {{lgpl-license-dst}} {{kde-breeze-license-dst}} {{kde-oxygen-license-dst}} {{kde-legacy-license-dst}} {{kde2-kicker-license-dst}} {{xfce-license-dst}} {{haiku-license-dst}} {{system76-approval-dst}}

# Installs files for the current user (no root required)
install-user: build-release
    install -Dm0755 {{ cargo-target-dir / 'release' / name }} {{ user-base-dir / 'bin' / name }}
    install -Dm0644 {{ 'target' / 'xdgen' / 'app.desktop' }} {{ user-base-dir / 'share' / 'applications' / desktop }}
    install -Dm0644 {{ 'target' / 'xdgen' / 'app.metainfo.xml' }} {{ user-base-dir / 'share' / 'appdata' / appdata }}
    install -Dm0644 resources/icon.svg {{ user-base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg' }}
    install -Dm0644 resources/icon-symbolic.svg {{ user-base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '-symbolic.svg' }}
    install -Dm0644 LICENSE {{ user-base-dir / 'share' / 'doc' / name / 'LICENSE' }}
    install -Dm0644 THIRD_PARTY_NOTICES.md {{ user-base-dir / 'share' / 'doc' / name / 'THIRD_PARTY_NOTICES.md' }}
    install -Dm0644 LICENSES/LGPL-3.0-or-later.txt {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'LGPL-3.0-or-later.txt' }}
    install -Dm0644 LICENSES/KDE-Breeze-Icons-LGPL-3.0-or-later.txt {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE-Breeze-Icons-LGPL-3.0-or-later.txt' }}
    install -Dm0644 LICENSES/KDE-Oxygen-Icons-LGPL-3.0-or-later.txt {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE-Oxygen-Icons-LGPL-3.0-or-later.txt' }}
    install -Dm0644 LICENSES/KDE-Legacy-Icons-LGPL-2.1-or-later.txt {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE-Legacy-Icons-LGPL-2.1-or-later.txt' }}
    install -Dm0644 LICENSES/KDE2-Kicker-MIT.txt {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE2-Kicker-MIT.txt' }}
    install -Dm0644 LICENSES/Xfce-Panel-GPL-2.0-or-later.txt {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'Xfce-Panel-GPL-2.0-or-later.txt' }}
    install -Dm0644 LICENSES/Haiku-Icons-MIT.txt {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'Haiku-Icons-MIT.txt' }}
    install -Dm0644 LICENSES/System76-COSMIC-APPROVAL-PENDING.md {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'System76-COSMIC-APPROVAL-PENDING.md' }}

# Uninstalls user-installed files
uninstall-user:
    rm {{ user-base-dir / 'bin' / name }} {{ user-base-dir / 'share' / 'applications' / desktop }} {{ user-base-dir / 'share' / 'appdata' / appdata }} {{ user-base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg' }} {{ user-base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '-symbolic.svg' }} {{ user-base-dir / 'share' / 'doc' / name / 'LICENSE' }} {{ user-base-dir / 'share' / 'doc' / name / 'THIRD_PARTY_NOTICES.md' }} {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'LGPL-3.0-or-later.txt' }} {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE-Breeze-Icons-LGPL-3.0-or-later.txt' }} {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE-Oxygen-Icons-LGPL-3.0-or-later.txt' }} {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE-Legacy-Icons-LGPL-2.1-or-later.txt' }} {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'KDE2-Kicker-MIT.txt' }} {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'Xfce-Panel-GPL-2.0-or-later.txt' }} {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'Haiku-Icons-MIT.txt' }} {{ user-base-dir / 'share' / 'doc' / name / 'LICENSES' / 'System76-COSMIC-APPROVAL-PENDING.md' }}

# Vendor dependencies locally
vendor:
    mkdir -p .cargo
    cargo vendor --sync Cargo.toml | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    echo >> .cargo/config.toml
    rm -rf .cargo vendor

# Extracts vendored dependencies
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar

# Bump cargo version, create git commit, and create tag
tag version:
    find -type f -name Cargo.toml -exec sed -i '0,/^version/s/^version.*/version = "{{version}}"/' '{}' \; -exec git add '{}' \;
    cargo check
    cargo clean
    git add Cargo.lock
    git commit -m 'release: {{version}}'
    git tag -a {{version}} -m ''
