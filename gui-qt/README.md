# OpenSCQ30 Qt GUI (`gui-qt`)

Qt6/QML desktop UI for OpenSCQ30. Backend logic is 100% reused from
`openscq30-lib`; this crate only bridges it to QML via
[cxx-qt](https://github.com/KDAB/cxx-qt) and renders the pages in
`qml/main.qml`.

## Prerequisites

- Qt 6 development files (`qmake6` on PATH, or set `QMAKE=/path/to/qmake`)
- C++ compiler, CMake is **not** required (cargo-only cxx-qt build)

```sh
QMAKE=qmake6 cargo build -p openscq30-gui-qt
```

## Recipes

```sh
just gui-qt::run        # QMAKE=qmake6 cargo run
just gui-qt::build release
just gui-qt::test
just gui-qt::install ~/.local
```

Release artifacts: `just build-gui-qt` puts `openscq30-gui-qt` in `build-output/`.

## Coverage vs the COSMIC GUI

- Device selection, add-device wizard (Bluetooth scan), pair/unpair
- All 12 setting kinds incl. equalizer, hue picker, time-of-day, import/export
- Quick presets (create/activate/edit fields/overwrite/delete)
- Legacy v1 equalizer profile migration (`config.toml` in the config dir)
- Device/model/setting strings follow the system language (stored preference
  in the shared `openscq30-gui-config.toml` wins when present); UI chrome
  stays English — Qt Linguist catalogs are future work
- Live updates via `watch_for_changes`, disconnect handling

## KDE integration

- Inside a Plasma session the `org.kde.desktop` style is selected (system
  theme, palette incl. dark mode, Breeze icons on buttons); elsewhere Qt
  defaults apply. `QT_QUICK_CONTROLS_STYLE` always overrides this.
- Page navigation is instant (`StackView.Immediate`) — no slide animations.
- The desktop entry (`com.oppzippy.OpenSCQ30.Qt`) and
  `QGuiApplication::set_desktop_file_name` keep task-manager grouping
  correct on Wayland.

## Packaging

- `just gui-qt::install ~/.local` (needs `just build-gui-qt` first)
- Arch: `packaging/arch/PKGBUILD` (`openscq30-gui-qt-git`, Qt6 + cargo
  build, desktop entry + icon included)

## Trying it out

Pick your model under Add device, Scan, then Pair the found headphones and
Connect. Needs working Bluetooth (BlueZ on Linux). The automated tests use
simulated demo devices so they run headless.
