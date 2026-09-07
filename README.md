# Kickoff Launcher for COSMIC™

A KDE Kickoff-style application menu applet for the [COSMIC™ desktop](https://github.com/pop-os/cosmic-epoch), built with [libcosmic](https://github.com/pop-os/libcosmic).

Based on the [COSMIC Applet Template](https://github.com/pop-os/cosmic-applet-template).

## Features

- Grid, list, and hybrid (favourites/recents grid) layout modes
- Configurable menu size presets and custom dimensions
- Favourites and recent applications
- Pin apps to the COSMIC dock
- Bottom bar with pinned apps and session/power actions
- Project, COSMIC App Library, COSMIC/System76, Xfce, Haiku Deskbar, KDE 2/3,
  KDE Breeze/Oxygen, Debian, mobile, and icon-theme panel button choices with
  monochrome support
- Optional standalone window mode (`--window`)
- Fully translated UI (Fluent/i18n)

## Building

```
just              # build-release
just run          # build and run
just install-user # install for the current user (no root required)
just install      # install system-wide (requires sudo)
just run-window   # build and run the menu in a standalone window
just check        # clippy with pedantic warnings
```

## License

Licensed under the [GPL-3.0-only](LICENSE).

See [Third-party notices](THIRD_PARTY_NOTICES.md) for icon-theme and trademark
information, exact artwork sources, revisions, attributions, and licences.
