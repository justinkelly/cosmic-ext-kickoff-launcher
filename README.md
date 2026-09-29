# Kickoff Launcher for the COSMIC™ desktop

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

## Screenshots

<table>
  <tr>
    <td><img src="screenshots/recent-apps.png" width="380" alt="Kickoff Launcher showing the Recents category in grid layout"><br><strong>Recent apps</strong><br>Recently launched apps appear above the categories.</td>
    <td><img src="screenshots/favourites.png" width="380" alt="Kickoff Launcher showing favourite apps in grid layout"><br><strong>Favourites</strong><br>A quick view of pinned favourites, with the category sidebar still available.</td>
  </tr>
  <tr>
    <td><img src="screenshots/list-view.png" width="380" alt="Kickoff Launcher showing applications in list layout"><br><strong>List view</strong><br>Application names and descriptions are shown alongside their icons.</td>
    <td><img src="screenshots/appearance-settings.png" width="380" alt="Kickoff Launcher appearance settings and panel icon picker"><br><strong>Appearance settings</strong><br>Choose a layout and menu size, and pick the panel icon.</td>
  </tr>
  <tr>
    <td><img src="screenshots/all-apps-grid.png" width="380" alt="Kickoff Launcher showing all applications in a grid"><br><strong>All applications</strong><br>Browse installed apps by category in grid layout.</td>
    <td><img src="screenshots/collapsed-sidebar.png" width="380" alt="Kickoff Launcher with the sidebar collapsed"><br><strong>Collapsed sidebar</strong><br>More room for the app grid when the sidebar is hidden.</td>
  </tr>
  <tr>
    <td><img src="screenshots/search.png" width="380" alt="Kickoff Launcher searching for COSMIC applications"><br><strong>Search</strong><br>Search results update as you type.</td>
    <td><img src="screenshots/sidebar-settings.png" width="380" alt="Kickoff Launcher sidebar and bottom bar settings"><br><strong>Sidebar and bottom bar</strong><br>Choose which sections and actions appear in the menu.</td>
  </tr>
</table>

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
