#!/usr/bin/env bash
set -euo pipefail

install_path="$1"

echo "Uninstalling from $install_path"
echo Removing binary
rm "$install_path/bin/openscq30-gui-qt" || true
echo Removing desktop file
rm "$install_path/share/applications/com.oppzippy.OpenSCQ30.Qt.desktop" || true
