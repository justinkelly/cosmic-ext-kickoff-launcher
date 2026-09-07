# Third-party notices

This file records the origin and licence of artwork embedded in the application.
Theme icons not listed here are looked up from the user's installed icon theme
at runtime and are not copied into this repository.

## Project artwork

`resources/icon.svg`, `resources/icon-symbolic.svg`, and the Mobile Dots, App
Drawer, Bento, and Honeycomb files in `resources/panel-icons/` are original
project artwork licensed under GPL-3.0-only; see `LICENSE`.

## COSMIC App Library

- Files: `resources/panel-icons/cosmic-app-library.svg` and the derived
  `cosmic-app-library-symbolic.svg`
- Source: [`pop-os/cosmic-app-library`](https://github.com/pop-os/cosmic-app-library),
  `data/icons/com.system76.CosmicAppLibrary.svg`
- Source revision: `5b7638f116360c1139efe795c8739cc89c87dca1`
- Copyright: COSMIC App Library contributors
- Licence: GPL-3.0-only; see `LICENSE`

## COSMIC and System76 brand marks (approval pending)

- Files: `resources/icons/cosmic-logo.svg`, `cosmic-logo-mono.svg`,
  `system76-logo.svg`, and `system76-logo-mono.svg`
- Source artwork: [`system76/brand`](https://github.com/system76/brand),
  `COSMIC branding/cosmic logo white + gradient mark.svg` and
  `System76 branding/system76-logo_secondary.svg`
- Source revision checked: `bed1fd3fd39d3f3371c57f15878195888c617777`
- Local changes: square icon canvases and separately prepared monochrome
  variants for panel tinting
- Copyright and trademarks: System76, Inc.

The official brand repository does not include a copyright licence granting
general redistribution of these brand files. Their restoration to this source
tree must not be interpreted as an open-source licence grant. **Before a
release containing these four files is redistributed, obtain and retain
written approval from System76 covering both copyright redistribution and
trademark use.** The COSMIC trademark policy directs permission requests to
`trademark@system76.com`; the COSMIC team can also be reached at
`cosmic@system76.com`. Record the resulting approval and its scope in
`LICENSES/System76-COSMIC-APPROVAL-PENDING.md`.

This approval requirement is separate from the GPL-3.0-only licence on the
COSMIC App Library launcher icon above.

## Xfce Applications Menu

- Files: `resources/panel-icons/xfce-applications-menu.svg` and the derived
  `xfce-applications-menu-symbolic.svg`
- Source: [`xfce/xfce4-panel`](https://gitlab.xfce.org/xfce/xfce4-panel),
  `icons/scalable/org.xfce.panel.applicationsmenu.svg`
- Source revision: `c874435e9e3c5d2bb4649c4a0bec5d163307d65e`
- Copyright: Xfce Panel contributors
- Licence: GPL-2.0-or-later, exercised here under GPL-3.0-only for the derived
  symbolic adaptation; see `LICENSES/Xfce-Panel-GPL-2.0-or-later.txt`

## Haiku Deskbar

- Files: `resources/panel-icons/haiku-deskbar.svg` and the derived
  `haiku-deskbar-symbolic.svg`
- SVG conversion source: [`darealshinji/haiku-icons`](https://github.com/darealshinji/haiku-icons),
  `svg/App_Deskbar.svg`, revision
  `ccf434a0cf31aae47cc8aac91934651619c1b9b2`
- Original Haiku artwork revision:
  [`49f62bc1728ce870478ae4bdb82201f0ee0c204b`](https://github.com/haiku/haiku/commit/49f62bc1728ce870478ae4bdb82201f0ee0c204b)
- Copyright: 2007-2026 Haiku, Inc.; SVG export by darealshinji
- Licence: MIT; see `LICENSES/Haiku-Icons-MIT.txt`

The selected asset is the Deskbar application icon, not Haiku's trademarked
leaf or wordmark. Original BeOS logo/artwork is not included.

## KDE 2 Kicker launcher

- File: `resources/panel-icons/kde2-kicker.png`
- Source: KDE's official
  [`kdebase-2.2.2.tar.bz2`](https://download.kde.org/Attic/2.2.2/src/kdebase-2.2.2.tar.bz2),
  `kicker/data/icons/hi48-app-go.png`
- Release: KDE 2.2.2 (Kicker explicitly loads the `go` icon for its
  application-menu button)
- Copyright: 1996-2001 the Kicker authors, including Matthias Ettrich, Daniel
  M. Duley, Matthias Elter, Preston Brown, Rik Hemsley, Wilco Greven, and John
  Firebaugh
- Licence: MIT; see `LICENSES/KDE2-Kicker-MIT.txt`

The supplied reference screenshot is KDE 1.92 beta and shows the same
K-in-a-gear family. The bundled file is the authentic KDE 2.2.2 Kicker asset,
not an image copied from the screenshot.

## KDE 3 Crystal launcher

- File: `resources/panel-icons/kde3-crystal.svg`, decompressed without artwork
  changes from `pics/crystalsvg/crsc-app-kmenu.svgz`
- Source: KDE's official
  [`kdelibs-3.5.10.tar.bz2`](https://download.kde.org/Attic/3.5.10/src/kdelibs-3.5.10.tar.bz2)
- Release: KDE 3.5.10
- Copyright: Everaldo Coelho and KDE Crystal SVG contributors
- Licence: LGPL-2.1-or-later; see
  `LICENSES/KDE-Legacy-Icons-LGPL-2.1-or-later.txt`

## KDE Classic legacy logo

- File: `resources/icons/kde-classic.svg`; monochrome mode uses the Breeze
  small-size symbolic launcher already attributed below
- Source: KDE's official [clipart archive](https://kde.org/stuff/clipart/),
  `klogo-classic.svg`
- Local status: restored byte-for-byte from this project's history; it is a
  visually equivalent Karbon SVG revision rather than a byte-identical copy
  of the archive's current optimized file
- Copyright: KDE e.V. and KDE artwork contributors
- Licence: LGPL-2.1-or-later; see
  `LICENSES/KDE-Legacy-Icons-LGPL-2.1-or-later.txt`

KDE publishes this logo for archival and historic use and advises against
using legacy marks to create new KDE-related branding. Here it is clearly
labelled as a historical launcher choice.

The restored `resources/icons/kde.svg` and `resources/icons/kde-mono.svg` are
retained as unused compatibility assets and are not selected or embedded by
the current code. They depict KDE's K-gear mark and fall under KDE's LGPL logo
copying terms, but the earlier project commit did not record their exact
upstream revision. Verify and replace them from an authoritative source before
re-enabling them.

## KDE Breeze launcher

- Files: `resources/panel-icons/kde-breeze.svg` and
  `kde-breeze-symbolic.svg`
- Source: [`frameworks/breeze-icons`](https://invent.kde.org/frameworks/breeze-icons),
  `icons/places/64/start-here-kde.svg` and
  `icons/places/22/start-here-kde-symbolic.svg`
- Source revision: `781f2fe1969a3ca2b5037bc72ef9d331dfb8bfb5`
- Copyright: 2014 Uri Herrera and other Breeze Icons contributors
- Licence: LGPL-3.0-or-later, including KDE's artwork-library clarification;
  see `LICENSES/KDE-Breeze-Icons-LGPL-3.0-or-later.txt`

## KDE Oxygen launcher

- File: `resources/icons/kde-oxygen.svg`, decompressed without artwork changes
  from `scalable/places/start-here-kde.svgz`
- Source: [`frameworks/oxygen-icons`](https://invent.kde.org/frameworks/oxygen-icons)
- Source revision: `9894dfad4a5edc3a47962f69266305e66ad677c7`
- Copyright: 2007 Nuno Pinheiro, David Vignoni, David Miller, Johann Ollivier
  Lapeyre, Kenneth Wimer, Riccardo Iaconelli, and other Oxygen Icons contributors
- Licence: LGPL-3.0-or-later, including KDE's artwork-library clarification;
  see `LICENSES/KDE-Oxygen-Icons-LGPL-3.0-or-later.txt`

Oxygen predates symbolic launcher icons. When monochrome mode is enabled, the
application uses the Breeze small-size symbolic KDE launcher glyph rather than
flattening Oxygen's blue tile into an unreadable silhouette.

## Debian Open Use logo

- File: `resources/panel-icons/debian.svg`
- Source: [Debian logos](https://www.debian.org/logos/),
  `openlogo-nd.svg` (the Open Use logo without the wordmark)
- Copyright: 1999 Software in the Public Interest, Inc.
- Creator: Raul Silva
- Licence selected for this distribution: LGPL-3.0-or-later; see
  `LICENSES/LGPL-3.0-or-later.txt`

## Trademarks

COSMIC and System76 are trademarks of System76, Inc. KDE and the KDE logo are
trademarks of KDE e.V. Debian is a registered trademark owned by Software in
the Public Interest, Inc. Xfce and Haiku names and marks belong to their
respective owners. These icons identify selectable desktop/distribution
styles; their inclusion does not imply sponsorship, affiliation, or
endorsement. See the explicit pending-approval warning for the restored
COSMIC/System76 brand files above.

Relevant guidance is published by
[System76](https://github.com/pop-os/cosmic-epoch/blob/master/TRADEMARK.md),
[KDE e.V.](https://kde.org/stuff/clipart/),
[Debian](https://www.debian.org/trademark), and
[Haiku, Inc.](https://www.haiku-inc.org/trademarks/).

## Artwork intentionally not bundled

Ubuntu, Arch Linux, Fedora, FreeBSD, OpenBSD, and original BeOS marks are not
bundled. Their software being open source does not by itself license the
associated artwork.
At the time of this review, the available official terms either restricted
logo use or did not provide a sufficiently clear, file-specific redistribution
licence for this app's commercial and non-commercial downstream packages. See
the current policies from [Canonical](https://canonical.com/legal/intellectual-property-policy),
[Arch Linux](https://terms.archlinux.org/docs/trademark-policy/),
[Fedora](https://fedoraproject.org/wiki/Logo),
[The FreeBSD Foundation](https://freebsdfoundation.org/legal/trademark-usage-terms-and-conditions/),
and [OpenBSD](https://www.openbsd.org/artwork.html) before reconsidering them.
BeOS was proprietary; the MIT-licensed Haiku Deskbar application artwork above
is the open replacement included for that visual lineage.
